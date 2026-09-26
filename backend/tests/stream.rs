mod support;

use std::collections::BTreeMap;

use gateway_plugin_sdk::{Message, Stage, call::middleware::MiddlewareBodyFrame};
use serde_json::{Value, json};
use support::{Peer, Reply};

#[tokio::test]
async fn attempt_observation_preserves_stream_bytes_order_and_exact_model() {
    // Given
    let mut peer = Peer::start().await;
    let frames = [
        b"event: codex.response.metadata\ndata: {\"type\":\"codex.response.metadata\",\"headers\":{\"x-codex-turn-state\":\"synthetic-state\"}}\n\n".to_vec(),
        b"event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"model\":\"reported-model\"}}\n\n".to_vec(),
    ];
    let response = peer
        .call(
            "middleware.handle",
            Stage::Attempt,
            json!({
                "request_id":"request","mount":"attempt","attempt_index":1,"operation":"generate",
                "protocol":"openai","endpoint":"/v1/responses","transport":"http_sse",
                "provider":"openai","model":"mapped-model","account_id":"acct-stream",
                "headers":[],"body_visible":true,
            }),
            b"{\"model\":\"public-alias\"}".to_vec(),
            |method, params, payload| {
                assert_eq!(method, "host.middleware.next");
                assert_eq!(params["body"], "preserve");
                assert!(payload.is_empty());
                Ok((
                    json!({"response":"attempt-response","protocol":"openai","status":200,
            "headers":[],"body":{"handle":"body","framing":"sse_event"}}),
                    Vec::new(),
                ))
            },
        )
        .await;
    let Message::Result { id, result } = response.message else {
        panic!("missing response")
    };
    assert_eq!(result["body"]["kind"], "stream");
    let mut next = 0;
    let mut delivered = Vec::new();
    // When
    loop {
        let frame = peer.recv().await;
        match frame.message {
            Message::Callback {
                id: callback_id,
                parent_id,
                method,
                ..
            } => {
                assert_eq!(parent_id, id);
                let reply = match method.as_str() {
                    "host.middleware.body_read" if next < frames.len() => {
                        let payload = frames[next].clone();
                        next += 1;
                        Ok((
                            json!({"framing":"sse_event","source_id":next,
                            "eof":false,"terminal":next == frames.len()}),
                            payload,
                        ))
                    }
                    "host.middleware.body_read" => Ok((
                        json!({
                            "framing":"sse_event","source_id":0,"eof":true,"terminal":false,
                        }),
                        Vec::new(),
                    )),
                    "host.middleware.body_close" => Ok((json!({}), Vec::new())),
                    _ => panic!("unexpected callback {method}"),
                };
                peer.callback(callback_id, reply).await;
            }
            Message::Stream { id: stream_id, .. } => {
                assert_eq!(stream_id, id);
                delivered.push(MiddlewareBodyFrame::decode(&frame.payload).unwrap());
            }
            Message::End {
                id: stream_id,
                error,
            } => {
                assert_eq!(stream_id, id);
                assert!(error.is_none());
                break;
            }
            other => panic!("unexpected frame {other:?}"),
        }
    }
    let mut store = BTreeMap::<String, Value>::new();
    let mut callback = |method: &str, params: &Value, payload: &[u8]| -> Reply {
        assert!(payload.is_empty());
        let key = format!("{}:{}", params["namespace"], params["key"]);
        match method {
            "host.state.get" => Ok((json!({"record":store.get(&key)}), Vec::new())),
            "host.state.put" => {
                assert!(params["expected_version"].is_null());
                store.insert(
                    key,
                    json!({"version":1,"schema_version":1,"value":params["value"]}),
                );
                Ok((json!({"version":1}), Vec::new()))
            }
            _ => panic!("unexpected callback {method}"),
        }
    };
    peer.call("policy.observe_request",Stage::Observation,json!({}),serde_json::to_vec(&json!({
        "event_id":"terminal","request_id":"request","config_revision":1,"operation":"generate",
        "account_id":"acct-stream","upstream_model":"mapped-model","provider":"openai","completed_at_ms":1,
    })).unwrap(), &mut callback).await;
    let (status, snapshot) = peer
        .api(
            "POST",
            "api/account",
            Some(json!({"accountId":"acct-stream"})),
            &mut callback,
        )
        .await;
    // Then
    assert_eq!(
        delivered
            .iter()
            .map(|frame| &frame.payload)
            .collect::<Vec<_>>(),
        frames.iter().collect::<Vec<_>>()
    );
    assert_eq!(
        delivered
            .iter()
            .map(MiddlewareBodyFrame::source_id)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(status, 200);
    assert_eq!(snapshot["models"][0]["model"], "mapped-model");
    assert_eq!(snapshot["models"][0]["latestLength"], 15);
    assert_eq!(snapshot["models"][0]["source"], "sse_metadata");
}
