mod support;

use gateway_plugin_sdk::{Message, Stage};
use serde_json::json;
use support::Peer;

#[tokio::test]
async fn overview_reads_do_not_wait_for_the_observation_write_lock() {
    // Given: 账号配置写入占有与业务观测相同的锁，宿主尚未确认写入。
    let mut peer = Peer::start().await;
    let settings = json!({"enabled":false,"models":[],"lengthRules":"","retentionHours":24});
    let write_id = peer
        .begin(
            "management.handle",
            Stage::Management,
            json!({
                "method":"POST","path":"api/settings","query":"","content_type":"application/json",
            }),
            serde_json::to_vec(&json!({
                "accountId":"acct-a","expectedVersion":null,"settings":settings,
            }))
            .unwrap(),
        )
        .await;
    let Message::Callback {
        id: put_id,
        parent_id,
        method,
        ..
    } = peer.recv().await.message
    else {
        panic!("expected state write")
    };
    assert_eq!(parent_id, write_id);
    assert_eq!(method, "host.state.put");

    // When: 不先完成写入，独立聚合读取仍必须能完整返回。
    let (status, overview) = peer.api("POST", "api/overview", Some(json!({})), |method, _, _| match method {
        "host.auth.list" => Ok((json!({}), serde_json::to_vec(&json!({
            "next_cursor":null,"accounts":[{
                "account_id":"acct-a","provider_id":"openai","enabled":true,"name":"Account A",
                "credential_revision":1,"authentication_kind":"oauth","credential_state":"ready","has_refresh_token":true,
            }],
        })).unwrap())),
        "host.state.get" => Ok((json!({"record":null}), Vec::new())),
        _ => panic!("unexpected callback {method}"),
    }).await;

    // Then: 先收到聚合结果，再允许原写入完成，不依赖调度延迟。
    assert_eq!(status, 200);
    assert_eq!(overview["accounts"][0]["observationEnabled"], true);
    peer.callback(put_id, Ok((json!({"version":1}), Vec::new())))
        .await;
    let saved = peer
        .finish(write_id, |method, params, _| {
            assert_eq!(method, "host.state.get");
            let record = if params["namespace"] == "settings" {
                json!({"schema_version":1,"version":1,"value":settings})
            } else {
                serde_json::Value::Null
            };
            Ok((json!({"record":record}), Vec::new()))
        })
        .await;
    assert!(matches!(saved.message, Message::Result { .. }));
}
