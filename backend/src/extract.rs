use gateway_plugin_sdk::call::middleware::{MiddlewareBodyFraming, MiddlewareHeader};
use serde_json::Value;

use crate::records::{Sample, Source};

const MAX_FRAME_BYTES: usize = 256 * 1024;
const MAX_SAMPLES_PER_FRAME: usize = 8;

pub fn headers(headers: &[MiddlewareHeader], now_ms: u64) -> Vec<Sample> {
    headers
        .iter()
        .filter(|header| header.name.eq_ignore_ascii_case("x-codex-turn-state"))
        .take(MAX_SAMPLES_PER_FRAME)
        .filter_map(|header| Sample::capture(&header.value, Source::HttpHeader, now_ms))
        .collect()
}

pub fn frame(payload: &[u8], framing: MiddlewareBodyFraming, now_ms: u64) -> Vec<Sample> {
    if payload.len() > MAX_FRAME_BYTES {
        return Vec::new();
    }
    let (document, source) = match framing {
        MiddlewareBodyFraming::JsonDocument => (
            serde_json::from_slice::<Value>(payload),
            Source::WebsocketMetadata,
        ),
        MiddlewareBodyFraming::SseEvent => {
            let Ok(text) = std::str::from_utf8(payload) else {
                return Vec::new();
            };
            let data = text
                .lines()
                .filter_map(|line| line.strip_prefix("data:"))
                .map(|line| line.strip_prefix(' ').unwrap_or(line))
                .collect::<Vec<_>>()
                .join("\n");
            (serde_json::from_str::<Value>(&data), Source::SseMetadata)
        }
        MiddlewareBodyFraming::RawBytes => return Vec::new(),
    };
    let Ok(document) = document else {
        return Vec::new();
    };
    // 只解释协议头容器，绝不递归搜索模型生成的文本或工具结果。
    [
        document.get("headers"),
        document.pointer("/response/headers"),
    ]
    .into_iter()
    .flatten()
    .filter_map(Value::as_object)
    .flat_map(|headers| headers.iter())
    .filter(|(name, _)| name.eq_ignore_ascii_case("x-codex-turn-state"))
    .flat_map(|(_, value)| match value {
        Value::String(text) => vec![text.as_str()],
        Value::Array(values) => values
            .iter()
            .filter_map(Value::as_str)
            .take(MAX_SAMPLES_PER_FRAME)
            .collect(),
        _ => Vec::new(),
    })
    .take(MAX_SAMPLES_PER_FRAME)
    .filter_map(|value| Sample::capture(value.as_bytes(), source, now_ms))
    .collect()
}
