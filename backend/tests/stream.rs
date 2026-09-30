mod support;

use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use gateway_plugin_sdk::{ErrorCode, Message, PluginFault, Stage};
use serde_json::{Value, json};
use support::{Peer, Reply};

#[derive(Default)]
struct Store {
    values: BTreeMap<String, Value>,
    fail: bool,
}

impl Store {
    fn call(&mut self, method: &str, params: &Value, payload: &[u8]) -> Reply {
        assert!(payload.is_empty());
        if self.fail {
            return Err(PluginFault::new(ErrorCode::Fault, "synthetic failure"));
        }
        let key = format!("{}:{}", params["namespace"], params["key"]);
        match method {
            "host.state.get" => Ok((json!({"record":self.values.get(&key)}), Vec::new())),
            "host.state.put" => {
                let old = self
                    .values
                    .get(&key)
                    .map(|value| value["version"].clone())
                    .unwrap_or(Value::Null);
                assert_eq!(old, params["expected_version"]);
                let version = old.as_u64().unwrap_or_default() + 1;
                self.values.insert(
                    key,
                    json!({"version":version,"schema_version":1,"value":params["value"]}),
                );
                Ok((json!({"version":version}), Vec::new()))
            }
            _ => panic!("unexpected callback {method}"),
        }
    }
}

async fn transfer(peer: &mut Peer, account: &str, framing: &str) {
    transfer_attempt(peer, account, framing, 1).await;
}

async fn transfer_attempt(peer: &mut Peer, account: &str, framing: &str, attempt: u32) {
    let mut next_calls = 0;
    let response = peer
        .call(
            "middleware.handle",
            Stage::Attempt,
            json!({
                "request_id":"request","mount":"attempt","attempt_index":attempt,"operation":"generate",
                "protocol":"openai","endpoint":"/v1/responses",
                "transport":if framing == "sse_event" {"http_sse"} else {"web_socket"},
                "provider":"openai","model":"mapped-model","account_id":account,
                "headers":[],"settings_sources":{},"client_key_id":"key-a","account_group_ids":[],
            }),
            b"{\"model\":\"public-alias\"}".to_vec(),
            |method, params, payload| {
                // 任何 body_read/body_close 都会再次进入官方的内部 RawBytes 帧拒绝路径。
                assert_eq!(method, "host.middleware.next");
                next_calls += 1;
                assert_eq!(params["body"], "preserve");
                assert_eq!(params["header_mutations"], json!([]));
                assert!(payload.is_empty());
                Ok((
                    json!({"response":"original-response","protocol":"openai","status":200,
            "headers":[],"body":{"handle":"opaque-original-body","framing":framing}}),
                    Vec::new(),
                ))
            },
        )
        .await;
    assert_eq!(next_calls, 1);
    let Message::Result { result, .. } = response.message else {
        panic!("expected response")
    };
    assert_eq!(result["response"], "original-response");
    assert!(result["status"].is_null());
    assert_eq!(result["header_mutations"], json!([]));
    assert_eq!(
        result["body"],
        json!({
            "kind":"pass_through","body":{"handle":"opaque-original-body","framing":framing},
        })
    );
}

fn event(account: &str) -> Value {
    json!({
        "event_id":"ws-event","request_id":"request","config_revision":1,
        "operation":"generate","protocol":"openai","provider":"openai",
        "attempt_index":1,"sequence":1,"payload_included":true,
        "requested_model":"public-alias","account_id":account,"event_type":"codex.response.metadata",
    })
}

async fn observe(peer: &mut Peer, store: &mut Store, params: Value) {
    let response = peer.call("observer.observe", Stage::Observation, json!({"event":"websocket_response","data":params}),
        br#"{"type":"codex.response.metadata","headers":{"x-codex-turn-state":"synthetic-state"},"response":{"model":"reported-model"}}"#.to_vec(),
        |method, params, payload| store.call(method, params, payload)).await;
    assert!(matches!(response.message, Message::Result { .. }));
}

async fn snapshot(peer: &mut Peer, store: &mut Store, account: &str) -> Value {
    let (status, value) = peer
        .api(
            "POST",
            "api/account",
            Some(json!({"accountId":account})),
            |method, params, payload| store.call(method, params, payload),
        )
        .await;
    assert_eq!(status, 200);
    value
}

#[tokio::test]
async fn attempt_returns_original_handle_without_reading_sse_or_json_bodies() {
    // Given / When / Then: 不只比较业务文本，断言整个未读宿主句柄被原样交还。
    for framing in ["sse_event", "json_document"] {
        let mut peer = Peer::start().await;
        transfer(&mut peer, "acct-a", framing).await;
    }
}

#[tokio::test]
async fn passive_websocket_observation_keeps_actual_model_after_terminal_and_deduplicates() {
    // Given
    let mut peer = Peer::start().await;
    let mut store = Store::default();
    transfer(&mut peer, "acct-a", "json_document").await;
    peer.call("observer.observe", Stage::Observation, json!({"event":"request_completed","data":{
            "event_id":"terminal","request_id":"request","config_revision":1,"operation":"generate",
            "account_id":"acct-a","upstream_model":"mapped-model","provider":"openai","completed_at_ms":1,
            "terminal":{"outcome":"succeeded","send_state":"sent","attempt_count":1},"usage":{}
        }}), Vec::new(), |method, params, payload| store.call(method, params, payload)).await;
    // When: 旁路队列晚于请求终态投递，同一事件重复投递不会重复累计。
    observe(&mut peer, &mut store, event("acct-a")).await;
    observe(&mut peer, &mut store, event("acct-a")).await;
    // Then
    let result = snapshot(&mut peer, &mut store, "acct-a").await;
    assert_eq!(result["models"][0]["model"], "mapped-model");
    assert_eq!(result["models"][0]["source"], "websocket_metadata");
    assert_eq!(result["models"][0]["latestLength"], 15);
    assert_eq!(result["models"][0]["observations"], 1);
}

#[tokio::test]
async fn persisted_013_websocket_observation_survives_redelivery_after_restart() {
    let peer = Peer::start().await;
    let mut store = Store::default();
    let observed_at_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let persisted_event_id = "7694996dc3c2a5e0ad0df7a9d8fb0d15adabc292e158adf836bda6090a2344fc";
    store.values.insert(
        format!("{}:{}", json!("observations"), json!("acct-a")),
        json!({"version":1,"schema_version":1,"value":{"models":[{
            "model":"mapped-model","history":[{
                "observedAtMs":observed_at_ms,"length":15,"fingerprint":"b88066f8d3d5",
                "source":"websocket_metadata","validation":"unrestricted","eventId":persisted_event_id,
            }],
        }]}}),
    );
    drop(peer);

    let mut peer = Peer::start().await;
    transfer(&mut peer, "acct-a", "json_document").await;
    observe(&mut peer, &mut store, event("acct-a")).await;

    let result = snapshot(&mut peer, &mut store, "acct-a").await;
    assert_eq!(result["models"][0]["model"], "mapped-model");
    assert_eq!(result["models"][0]["observations"], 1);
    assert_eq!(result["models"][0]["history"].as_array().unwrap().len(), 1);
    assert_eq!(
        result["models"][0]["history"][0]["eventId"],
        persisted_event_id
    );
    let persisted = &store.values[&format!("{}:{}", json!("observations"), json!("acct-a"))];
    assert_eq!(
        persisted["value"]["models"][0]["history"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        persisted["value"]["models"][0]["history"][0]["eventId"],
        persisted_event_id
    );
}

#[tokio::test]
async fn request_wide_websocket_sequence_and_attempt_owner_keep_distinct_accounts() {
    let mut peer = Peer::start().await;
    let mut store = Store::default();
    transfer_attempt(&mut peer, "acct-a", "json_document", 1).await;
    transfer_attempt(&mut peer, "acct-b", "json_document", 2).await;
    let first = event("acct-a");
    observe(&mut peer, &mut store, first.clone()).await;
    let mut second = event("acct-b");
    second["attempt_index"] = json!(2);
    second["sequence"] = json!(2);
    second["event_id"] = json!("second-event");
    observe(&mut peer, &mut store, second.clone()).await;
    observe(&mut peer, &mut store, second).await;
    let mut conflicting = first;
    conflicting["account_id"] = json!("acct-b");
    observe(&mut peer, &mut store, conflicting).await;
    assert_eq!(
        snapshot(&mut peer, &mut store, "acct-a").await["models"][0]["observations"],
        1
    );
    assert_eq!(
        snapshot(&mut peer, &mut store, "acct-b").await["models"][0]["observations"],
        1
    );
}

#[tokio::test]
async fn passive_observation_never_guesses_a_missing_or_conflicting_account() {
    // Given
    let mut peer = Peer::start().await;
    let mut store = Store::default();
    observe(&mut peer, &mut store, event("acct-a")).await;
    transfer(&mut peer, "acct-a", "sse_event").await;
    // When
    observe(&mut peer, &mut store, event("acct-b")).await;
    let mut hidden = event("acct-a");
    hidden["payload_included"] = json!(false);
    observe(&mut peer, &mut store, hidden).await;
    // Then
    let result = snapshot(&mut peer, &mut store, "acct-a").await;
    assert_eq!(result["models"], json!([]));
    assert_eq!(result["diagnostics"]["unattributed"], 2);
}

#[tokio::test]
async fn passive_storage_failure_is_reported_without_failing_the_observer_call() {
    // Given
    let mut peer = Peer::start().await;
    let mut store = Store {
        fail: true,
        ..Store::default()
    };
    transfer(&mut peer, "acct-a", "sse_event").await;
    // When
    observe(&mut peer, &mut store, event("acct-a")).await;
    // Then
    store.fail = false;
    let result = snapshot(&mut peer, &mut store, "acct-a").await;
    assert_eq!(result["models"], json!([]));
    assert_eq!(result["diagnostics"]["storageFailures"], 1);
}

#[tokio::test]
async fn websocket_sequence_increments_across_attempts_and_redelivery_is_idempotent() {
    let mut peer = Peer::start().await;
    let mut store = Store::default();
    transfer(&mut peer, "acct-a", "json_document").await;
    let mut second = json!({
        "request_id":"request","mount":"attempt","attempt_index":2,"operation":"generate",
        "protocol":"openai","endpoint":"/v1/responses","transport":"web_socket",
        "provider":"openai","model":"mapped-model","account_id":"acct-a",
        "headers":[],"settings_sources":{},"client_key_id":"key-a","account_group_ids":[],
    });
    peer.call(
        "middleware.handle",
        Stage::Attempt,
        second.take(),
        Vec::new(),
        |method, _, _| {
            assert_eq!(method, "host.middleware.next");
            Ok((
                json!({"response":"response-2","protocol":"openai","status":200,"headers":[]}),
                Vec::new(),
            ))
        },
    )
    .await;
    let first = event("acct-a");
    let mut retry = first.clone();
    retry["attempt_index"] = json!(2);
    retry["sequence"] = json!(2);
    retry["event_id"] = json!("second-event");
    observe(&mut peer, &mut store, first.clone()).await;
    observe(&mut peer, &mut store, retry).await;
    observe(&mut peer, &mut store, first).await;
    let result = snapshot(&mut peer, &mut store, "acct-a").await;
    assert_eq!(result["models"][0]["observations"], 2);
}
