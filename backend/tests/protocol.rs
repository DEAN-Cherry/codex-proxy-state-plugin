mod support;

use std::collections::BTreeMap;

use gateway_plugin_sdk::{ErrorCode, Message, PluginFault, Stage};
use serde_json::{Value, json};
use support::{Peer, Reply};

#[derive(Default)]
struct Store {
    records: BTreeMap<String, Value>,
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
            "host.state.get" => Ok((json!({"record":self.records.get(&key)}), Vec::new())),
            "host.state.put" => {
                let old = self
                    .records
                    .get(&key)
                    .map(|record| record["version"].clone())
                    .unwrap_or(Value::Null);
                if old != params["expected_version"] {
                    return Err(PluginFault::new(ErrorCode::Conflict, "CAS conflict"));
                }
                let version = old.as_u64().unwrap_or(0) + 1;
                self.records.insert(
                    key,
                    json!({
                        "value":params["value"], "version":version, "schema_version":1,
                    }),
                );
                Ok((json!({"version":version}), Vec::new()))
            }
            _ => panic!("unexpected callback {method}"),
        }
    }
}

fn head() -> Value {
    json!({"request_id":"request","mount":"request","operation":"generate","protocol":"openai",
        "endpoint":"/v1/responses","transport":"http_sse","model":"public-alias","headers":[],
        "body_visible":true})
}

fn response() -> Reply {
    Ok((
        json!({"response":"response-1","protocol":"openai","status":201,
        "headers":[{"name":"x-codex-turn-state","value":[97,98,99]}]}),
        Vec::new(),
    ))
}

async fn terminal(peer: &mut Peer, store: &mut Store) {
    peer.call("policy.observe_request", Stage::Observation, json!({}),
        serde_json::to_vec(&json!({
            "event_id":"terminal","request_id":"request","config_revision":1,"operation":"generate",
            "account_id":"acct-a","upstream_model":"actual-model","response_model":"reported-other-model",
            "requested_model":"public-alias","provider":"openai","completed_at_ms":1,
        })).unwrap(),
        |method, params, payload| store.call(method, params, payload)).await;
}

#[tokio::test]
async fn header_first_roundtrip_persists_actual_model_and_survives_restart() {
    // Given
    let mut peer = Peer::start().await;
    let mut store = Store::default();
    let mut calls = 0;
    let frame = peer
        .call(
            "middleware.handle",
            Stage::Request,
            head(),
            b"{\"model\":\"public-alias\"}".to_vec(),
            |method, params, payload| {
                assert_eq!(method, "host.middleware.next");
                assert_eq!(params["body"], "preserve");
                assert_eq!(params["header_mutations"], json!([]));
                assert!(payload.is_empty());
                calls += 1;
                response()
            },
        )
        .await;
    assert_eq!(calls, 1);
    let Message::Result { result, .. } = frame.message else {
        panic!("missing result")
    };
    assert!(result["status"].is_null());
    assert_eq!(result["header_mutations"], json!([]));
    // When
    terminal(&mut peer, &mut store).await;
    drop(peer);
    let mut peer = Peer::start().await;
    let (status, snapshot) = peer
        .api(
            "POST",
            "api/account",
            Some(json!({"accountId":"acct-a"})),
            |method, params, payload| store.call(method, params, payload),
        )
        .await;
    // Then
    assert_eq!(status, 200);
    assert_eq!(snapshot["models"][0]["model"], "actual-model");
    assert_eq!(snapshot["models"][0]["latestLength"], 3);
    assert_eq!(snapshot["models"][0]["observations"], 1);
    assert!(
        !serde_json::to_string(&store.records)
            .unwrap()
            .contains("\"abc\"")
    );
}

#[tokio::test]
async fn terminal_first_joins_from_the_live_middleware_callback() {
    // Given
    let mut peer = Peer::start().await;
    let mut store = Store::default();
    let id = peer
        .begin("middleware.handle", Stage::Request, head(), b"{}".to_vec())
        .await;
    let callback = peer.recv().await;
    let Message::Callback {
        id: next_id,
        method,
        ..
    } = callback.message
    else {
        panic!("missing next")
    };
    assert_eq!(method, "host.middleware.next");
    terminal(&mut peer, &mut store).await;
    // When
    peer.callback(next_id, response()).await;
    peer.finish(id, |method, params, payload| {
        store.call(method, params, payload)
    })
    .await;
    // Then
    let (_, snapshot) = peer
        .api(
            "POST",
            "api/account",
            Some(json!({"accountId":"acct-a"})),
            |method, params, payload| store.call(method, params, payload),
        )
        .await;
    assert_eq!(snapshot["models"][0]["observations"], 1);
    assert_eq!(snapshot["diagnostics"]["storageFailures"], 0);
}

#[tokio::test]
async fn storage_failure_does_not_replace_the_business_response() {
    // Given
    let mut peer = Peer::start().await;
    let mut store = Store {
        fail: true,
        ..Store::default()
    };
    let id = peer
        .begin("middleware.handle", Stage::Request, head(), b"{}".to_vec())
        .await;
    let Message::Callback { id: next_id, .. } = peer.recv().await.message else {
        panic!("missing next")
    };
    terminal(&mut peer, &mut store).await;
    // When
    peer.callback(next_id, response()).await;
    let result = peer
        .finish(id, |method, params, payload| {
            store.call(method, params, payload)
        })
        .await;
    // Then
    assert!(matches!(result.message, Message::Result { .. }));
    store.fail = false;
    let (_, snapshot) = peer
        .api(
            "POST",
            "api/account",
            Some(json!({"accountId":"acct-a"})),
            |method, params, payload| store.call(method, params, payload),
        )
        .await;
    assert_eq!(snapshot["diagnostics"]["storageFailures"], 1);
    assert_eq!(snapshot["models"], json!([]));
}

#[tokio::test]
async fn settings_cas_rejects_stale_version_without_overwriting() {
    // Given
    let mut peer = Peer::start().await;
    let mut store = Store::default();
    let update = json!({"accountId":"acct-a","expectedVersion":null,
        "settings":{"enabled":true,"models":["actual-model"],"lengthRules":"3-4","retentionHours":24}});
    let (status, first) = peer
        .api(
            "POST",
            "api/settings",
            Some(update.clone()),
            |method, params, payload| store.call(method, params, payload),
        )
        .await;
    assert_eq!(status, 200);
    // When
    let (status, _) = peer
        .api(
            "POST",
            "api/settings",
            Some(update),
            |method, params, payload| store.call(method, params, payload),
        )
        .await;
    // Then
    assert_eq!(status, 409);
    let (_, actual) = peer
        .api(
            "POST",
            "api/account",
            Some(json!({"accountId":"acct-a"})),
            |method, params, payload| store.call(method, params, payload),
        )
        .await;
    assert_eq!(actual["settings"], first["settings"]);
    assert_eq!(actual["version"], first["version"]);
}

#[tokio::test]
async fn invalid_settings_are_rejected_without_storage_calls() {
    // Given
    let mut peer = Peer::start().await;
    // When
    let (status, _) = peer
        .api(
            "POST",
            "api/settings",
            Some(json!({
                "accountId":"acct-a","expectedVersion":null,
                "settings":{"enabled":true,"models":[],"lengthRules":"5-3","retentionHours":24},
            })),
            |method, _, _| panic!("unexpected callback {method}"),
        )
        .await;
    // Then
    assert_eq!(status, 400);
}
