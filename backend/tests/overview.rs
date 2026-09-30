mod support;

use gateway_plugin_sdk::{ErrorCode, PluginFault};
use serde_json::{Value, json};
use support::Peer;

fn account(id: &str, enabled: bool) -> Value {
    json!({"account_id":id,"provider_id":"openai","name":"Shared account",
        "email":"private@example.com","group_ids":[],"updated_at_ms":1,"enabled":enabled})
}

#[tokio::test]
async fn management_account_labels_use_email_then_name_then_id_without_merging_accounts() {
    let cases = [
        (
            "acct-a",
            "openai OAuth",
            Some("  oauth@example.com \t"),
            "oauth@example.com",
        ),
        (
            "acct-b",
            "Custom account name",
            Some("custom@example.com"),
            "custom@example.com",
        ),
        ("acct-c", "  Missing email  ", None, "Missing email"),
        ("acct-d", "  Empty email\t", Some(""), "Empty email"),
        ("acct-e", "\tBlank email  ", Some(" \t\n "), "Blank email"),
        ("acct-f", " \t ", None, "acct-f"),
        ("acct-g", "", Some(""), "acct-g"),
        ("acct-h", " \n ", Some("\t "), "acct-h"),
        (
            "acct-i",
            "Another account",
            Some("oauth@example.com"),
            "oauth@example.com",
        ),
    ];
    let accounts: Vec<_> = cases
        .iter()
        .map(|(id, name, email, _)| {
            json!({"account_id":id,"provider_id":"openai","name":name,"email":email,
                "enabled":*id != "acct-b","group_ids":["private-group"],"updated_at_ms":1})
        })
        .collect();
    let mut peer = Peer::start().await;
    for (method, path, body, limit) in [
        ("GET", "api/accounts", None, 200),
        ("POST", "api/overview", Some(json!({})), 20),
    ] {
        let (status, result) = peer
            .api(method, path, body, |method, params, payload| match method {
                "host.data.accounts.list" => {
                    assert_eq!(params, &json!({}));
                    assert_eq!(
                        serde_json::from_slice::<Value>(payload).unwrap(),
                        json!({"provider_id":"openai","cursor":null,"limit":limit})
                    );
                    Ok((
                        json!({}),
                        serde_json::to_vec(&json!({
                            "schema_version":1,"accounts":accounts,"next_cursor":null,
                        }))
                        .unwrap(),
                    ))
                }
                "host.state.get" => {
                    assert_eq!(path, "api/overview");
                    assert!(payload.is_empty());
                    assert!(cases.iter().any(|(id, ..)| params["key"] == *id));
                    Ok((json!({"record":null}), Vec::new()))
                }
                _ => panic!("unexpected callback {method}"),
            })
            .await;
        assert_eq!(status, 200, "{path}");
        assert!(result["nextCursor"].is_null());
        let expected: Vec<_> = cases
            .iter()
            .map(|(id, _, _, label)| {
                let mut account = json!({
                    "accountId":id,"accountName":label,"enabled":*id != "acct-b",
                });
                if path == "api/overview" {
                    account["observationEnabled"] = json!(true);
                    account["error"] = Value::Null;
                    account["models"] = json!([]);
                }
                account
            })
            .collect();
        assert_eq!(result["accounts"], json!(expected), "{path}");
    }
}

#[tokio::test]
async fn overview_keeps_empty_disabled_and_unavailable_accounts_distinct() {
    // Given: 当前规则与持久化的旧判断不同，聚合必须复用当前规则。
    let mut peer = Peer::start().await;
    let accounts = vec![
        account("acct-a", true),
        account("acct-b", false),
        account("acct-c", true),
    ];
    // When
    let (status, result) = peer
        .api(
            "POST",
            "api/overview",
            Some(json!({})),
            |method, params, payload| match method {
                "host.data.accounts.list" => {
                    assert_eq!(params, &json!({}));
                    let query: Value = serde_json::from_slice(payload).unwrap();
                    assert_eq!(
                        query,
                        json!({"provider_id":"openai","cursor":null,"limit":20})
                    );
                    Ok((
                        json!({}),
                        serde_json::to_vec(&json!({
                            "schema_version":1,"accounts":accounts,"next_cursor":null,
                        }))
                        .unwrap(),
                    ))
                }
                "host.state.get" => {
                    assert!(payload.is_empty());
                    if params["key"] == "acct-c" {
                        return Err(PluginFault::new(
                            ErrorCode::Fault,
                            "synthetic storage failure",
                        ));
                    }
                    let record = if params["key"] == "acct-a" && params["namespace"] == "settings" {
                        json!({"schema_version":1,"version":7,"value":{
                            "enabled":false,"models":[],"lengthRules":"3-5","retentionHours":24,
                        }})
                    } else if params["key"] == "acct-a" && params["namespace"] == "observations" {
                        json!({"schema_version":1,"version":2,"value":{"models":[{
                            "model":"mapped-model","history":[{
                                "observedAtMs":10,"length":3,"fingerprint":"abc123def456",
                                "source":"http_header","validation":"mismatch","eventId":"event-1",
                            }],
                        }]}})
                    } else {
                        Value::Null
                    };
                    Ok((json!({"record":record}), Vec::new()))
                }
                _ => panic!("unexpected callback {method}"),
            },
        )
        .await;
    // Then
    assert_eq!(status, 200);
    assert_eq!(result["accounts"].as_array().unwrap().len(), 3);
    let observed = &result["accounts"][0];
    assert_eq!(observed["accountId"], "acct-a");
    assert_eq!(observed["accountName"], "private@example.com");
    assert_eq!(result["accounts"][1]["accountName"], "private@example.com");
    assert_ne!(observed["accountId"], result["accounts"][1]["accountId"]);
    assert!(observed.get("credentialRevision").is_none());
    assert!(observed.get("email").is_none());
    assert_eq!(observed["observationEnabled"], false);
    assert!(observed["error"].is_null());
    assert_eq!(observed["models"][0]["validation"], "matched");
    assert_eq!(observed["models"][0]["model"], "mapped-model");
    assert!(observed["models"][0].get("history").is_none());
    assert!(observed["models"][0].get("lengths").is_none());
    assert_eq!(result["accounts"][1]["enabled"], false);
    assert_eq!(result["accounts"][1]["models"], json!([]));
    assert!(result["accounts"][1]["error"].is_null());
    assert_eq!(result["accounts"][2]["error"], "unavailable");
    assert!(result["accounts"][2]["observationEnabled"].is_null());
}

#[tokio::test]
async fn overview_forwards_account_cursor_and_preserves_next_page() {
    // Given
    let mut peer = Peer::start().await;
    // When
    let (status, result) = peer
        .api(
            "POST",
            "api/overview",
            Some(json!({"cursor":"acct-b","limit":1})),
            |method, _, payload| {
                if method == "host.data.accounts.list" {
                    assert_eq!(
                        serde_json::from_slice::<Value>(payload).unwrap(),
                        json!({"provider_id":"openai","cursor":"acct-b","limit":1})
                    );
                    Ok((
                        json!({}),
                        serde_json::to_vec(&json!({
                            "schema_version":1,"accounts":[account("acct-c",true)],"next_cursor":"acct-c",
                        }))
                        .unwrap(),
                    ))
                } else {
                    assert_eq!(method, "host.state.get");
                    Ok((json!({"record":null}), Vec::new()))
                }
            },
        )
        .await;
    // Then
    assert_eq!(status, 200);
    assert_eq!(result["accounts"][0]["accountId"], "acct-c");
    assert_eq!(result["nextCursor"], "acct-c");
}

#[tokio::test]
async fn overview_rejects_unbounded_or_unknown_queries_before_host_access() {
    // Given
    let mut peer = Peer::start().await;
    for query in [
        json!({"limit":0}),
        json!({"limit":51}),
        json!({"cursor":" "}),
        json!({"unexpected":true}),
    ] {
        // When
        let (status, _) = peer
            .api("POST", "api/overview", Some(query), |method, _, _| {
                panic!("unexpected callback {method}")
            })
            .await;
        // Then
        assert_eq!(status, 400);
    }
}

#[tokio::test]
async fn overview_directory_failure_is_not_an_empty_success() {
    // Given
    let mut peer = Peer::start().await;
    // When
    let (status, _) = peer
        .api("POST", "api/overview", Some(json!({})), |method, _, _| {
            assert_eq!(method, "host.data.accounts.list");
            Err(PluginFault::new(
                ErrorCode::Fault,
                "synthetic directory failure",
            ))
        })
        .await;
    // Then
    assert_eq!(status, 502);
}
