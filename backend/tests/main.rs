use std::time::Duration;

use codex_proxy_state_plugin::{
    collector::{Attribution, Batch, Collector, event_id},
    extract,
    records::{AccountRecords, Sample, Source},
    settings::{LengthRules, Settings, Validation},
};
use gateway_plugin_sdk::call::middleware::{MiddlewareBodyFraming, MiddlewareHeader};
use serde_json::json;

fn sample(value: &[u8], at: u64, id: &str) -> Sample {
    let mut sample = Sample::capture(value, Source::HttpHeader, at).unwrap();
    sample.event_id = id.to_owned();
    sample
}

#[test]
fn length_rules_cover_boundaries_empty_invalid_duplicates_and_limits() {
    let rules = LengthRules::parse("10,20-30").unwrap();
    assert_eq!(rules.classify(10), Validation::Matched);
    assert_eq!(rules.classify(20), Validation::Matched);
    assert_eq!(rules.classify(30), Validation::Matched);
    assert_eq!(rules.classify(31), Validation::Mismatch);
    assert_eq!(
        LengthRules::parse("").unwrap().classify(1),
        Validation::Unrestricted
    );
    for input in ["1-0", "0", "1,1", "1-3,1-3"] {
        assert!(LengthRules::parse(input).is_err(), "{input}");
    }
    assert!(LengthRules::parse(&("1,".repeat(33) + "2")).is_err());
    assert!(LengthRules::parse(&"1".repeat(1025)).is_err());
}

#[test]
fn samples_use_utf8_bytes_and_stable_fingerprints() {
    let sample = Sample::capture("é".as_bytes(), Source::HttpHeader, 7).unwrap();
    assert_eq!(sample.length, 2);
    assert_eq!(sample.fingerprint, "4a99557e4033");
    assert!(Sample::capture(b"", Source::HttpHeader, 0).is_none());
    assert!(Sample::capture(&vec![b'x'; 16 * 1024 + 1], Source::HttpHeader, 0).is_none());
}

#[test]
fn extraction_is_exact_for_containers_and_sse_is_crlf_safe() {
    let headers = vec![
        MiddlewareHeader {
            name: "x-codex-turn-state".into(),
            value: b"one".to_vec(),
        },
        MiddlewareHeader {
            name: "X-CODEX-TURN-STATE".into(),
            value: b"two".to_vec(),
        },
        MiddlewareHeader {
            name: "x-codex-turn-state-extra".into(),
            value: b"bad".to_vec(),
        },
    ];
    assert_eq!(extract::headers(&headers, 1).len(), 2);
    let document = br#"{"text":"x-codex-turn-state","headers":{"x-codex-turn-state":["a","b"]},"response":{"headers":{"X-Codex-Turn-State":"c"}},"nested":{"headers":{"x-codex-turn-state":"ignored"}}}"#;
    assert_eq!(
        extract::frame(document, MiddlewareBodyFraming::JsonDocument, 1).len(),
        3
    );
    let sse = b"data: {\r\ndata: \"headers\": {\r\ndata: \"x-codex-turn-state\": \"sse\"}\r\ndata: }\r\n\r\n";
    assert_eq!(
        extract::frame(sse, MiddlewareBodyFraming::SseEvent, 1).len(),
        1
    );
    assert!(extract::frame(b"data: {bad}\r\n\r\n", MiddlewareBodyFraming::SseEvent, 1).is_empty());
    assert!(
        extract::frame(
            &vec![b'x'; 256 * 1024 + 1],
            MiddlewareBodyFraming::JsonDocument,
            1
        )
        .is_empty()
    );
}

#[test]
fn collector_joins_order_independently_without_cross_request_attribution() {
    let owner_a = Attribution::new(Some("acct-a"), Some("model-a")).unwrap();
    let owner_b = Attribution::new(Some("acct-b"), Some("model-b")).unwrap();
    let mut collector = Collector::default();
    collector.begin("a", Duration::from_secs(1));
    collector.begin("b", Duration::from_secs(1));
    assert!(
        collector
            .terminal("a", Some(owner_a.clone()), Duration::from_secs(2))
            .is_empty()
    );
    collector.samples(
        "b",
        Batch {
            owner: owner_a,
            samples: vec![sample(b"retry", 1, "retry")],
        },
        Duration::from_secs(2),
    );
    collector.terminal("b", Some(owner_b), Duration::from_secs(2));
    let batches = collector.headers("b", vec![sample(b"header", 2, "")], Duration::from_secs(3));
    assert_eq!(batches.len(), 2);
    assert!(
        !collector
            .headers("a", vec![sample(b"a", 3, "")], Duration::from_secs(3))
            .is_empty()
    );
}

#[test]
fn collector_expiry_unknown_owner_and_event_deduplication_are_bounded() {
    let mut collector = Collector::default();
    collector.begin("unknown", Duration::ZERO);
    assert!(
        collector
            .terminal("unknown", None, Duration::from_secs(1))
            .is_empty()
    );
    collector.begin("expired", Duration::ZERO);
    collector.begin("new", Duration::from_secs(901));
    assert!(collector.dropped >= 1);
    assert_eq!(event_id("request", "event"), event_id("request", "event"));
    assert_ne!(event_id("request", "event"), event_id("request", "other"));
    let mut records = AccountRecords::default();
    records.record(
        "model",
        vec![sample(b"x", 1, "same"), sample(b"x", 1, "same")],
        &Settings::default(),
    );
    assert_eq!(records.models[0].history.len(), 1);
}

#[test]
fn records_bound_models_history_and_current_rule_reclassification() {
    let settings = Settings {
        length_rules: "2".into(),
        ..Settings::default()
    };
    let mut records = AccountRecords::default();
    for index in 0..17 {
        records.record(
            &format!("model-{index}"),
            vec![sample(b"x", index, &index.to_string())],
            &settings,
        );
    }
    assert_eq!(records.models.len(), 16);
    for index in 1..=13 {
        records.record(
            "kept",
            vec![sample(b"xx", index, &format!("history-{index}"))],
            &settings,
        );
    }
    let kept = records
        .models
        .iter()
        .find(|model| model.model == "kept")
        .unwrap();
    assert_eq!(kept.history.len(), 12);
    assert_eq!(kept.history[0].observed_at_ms, 2);
    records.reclassify(&LengthRules::parse("3").unwrap());
    assert!(
        records
            .models
            .iter()
            .find(|model| model.model == "kept")
            .unwrap()
            .history
            .iter()
            .all(|sample| sample.validation == Validation::Mismatch)
    );
}

#[test]
fn attribution_never_guesses_missing_account_or_model() {
    assert!(Attribution::new(None, Some("model")).is_none());
    assert!(Attribution::new(Some("account"), None).is_none());
}

#[test]
fn manifest_accepts_supported_hosts_without_an_upper_cap() {
    let manifest = codex_proxy_state_plugin::manifest().unwrap();
    manifest.validate().unwrap();
    let hosts = &manifest.engines.codex_proxy_rs;
    for version in ["3.18.0", "3.18.1", "3.18.2", "3.19.0"] {
        assert!(hosts.matches(&version.parse().unwrap()), "{version}");
    }
    for version in ["3.17.9", "3.16.0", "3.15.2"] {
        assert!(!hosts.matches(&version.parse().unwrap()), "{version}");
    }
}

#[test]
fn source_manifest_requires_the_new_contract_and_rejects_old_shapes() {
    assert_eq!(gateway_plugin_sdk::PROTOCOL_VERSION, 2);
    let source: serde_json::Value =
        serde_json::from_str(include_str!("../../plugin.json")).unwrap();
    assert_eq!(source["manifestVersion"], 2);
    assert_eq!(source["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(source["contributes"]["middleware"]["version"], 3);
    assert_eq!(
        source["contributes"]["middleware"]["stages"],
        json!(["request", "attempt"])
    );
    assert!(source["contributes"]["observer"].is_object());
    for removed in ["permissions", "request_lifecycle", "web_socket_observer"] {
        assert!(source.get(removed).is_none());
        assert!(source["contributes"].get(removed).is_none());
    }
    for field in ["manifestVersion", "contributes", "permissions"] {
        let mut old = source.clone();
        match field {
            "manifestVersion" => old[field] = json!(1),
            "contributes" => old[field] = json!({"request_lifecycle":{}}),
            _ => old[field] = json!(["accounts"]),
        }
        assert!(
            gateway_plugin_sdk::Manifest::from_author_slice(&serde_json::to_vec(&old).unwrap())
                .is_err(),
            "{field}"
        );
    }
}

#[tokio::test(flavor = "current_thread")]
async fn plugin_session_reads_display_labels_without_requesting_credentials() {
    let mut peer = support::Peer::start().await;
    let (status, body) = peer.api("GET", "api/accounts", None, |method, params, payload| {
        assert_eq!(method, "host.data.accounts.list");
        assert_eq!(params, &serde_json::json!({}));
        let query: serde_json::Value = serde_json::from_slice(payload).unwrap();
        assert_eq!(query["provider_id"], "openai");
        Ok((serde_json::json!({}), serde_json::to_vec(&serde_json::json!({
            "schema_version":1,"accounts": [{"account_id":"acct-a","provider_id":"openai","name":"Primary account","enabled":true,
                "email":"private@example.com","group_ids":[],"updated_at_ms":1}],"next_cursor": null
        })).unwrap()))
    }).await;
    assert_eq!(status, 200);
    assert_eq!(body["accounts"][0]["accountId"], "acct-a");
    assert_eq!(body["accounts"][0]["accountName"], "private@example.com");
    assert_eq!(
        body["accounts"][0],
        serde_json::json!({"accountId":"acct-a","accountName":"private@example.com","enabled":true})
    );
}

mod support;
