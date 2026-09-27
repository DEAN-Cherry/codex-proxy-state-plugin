use std::sync::Arc;

use gateway_plugin_sdk::{
    PluginFault,
    call::{
        middleware::MiddlewareMount, observation::ObserveWebSocketResponse, policy::ObserveRequest,
    },
    client::{Empty, MiddlewareCall, MiddlewareResponse, TypedCall, TypedReply},
};

use crate::{
    collector::{Attribution, Batch, event_id},
    extract,
    state::{AppState, now_ms},
};

pub async fn middleware(
    state: Arc<AppState>,
    call: MiddlewareCall,
) -> Result<MiddlewareResponse, PluginFault> {
    let id = call.request.head.request_id.clone();
    let mount = call.request.head.mount;
    let owner = (call.request.head.provider.as_deref() == Some("openai"))
        .then(|| {
            Attribution::new(
                call.request.head.account_id.as_deref(),
                call.request.head.model.as_deref(),
            )
        })
        .flatten();
    let attempt = call.request.head.attempt_index;
    if mount == MiddlewareMount::Request {
        state.collector().begin(&id, state.elapsed());
    }
    if mount == MiddlewareMount::Attempt
        && let (Some(owner), Some(attempt)) = (owner, attempt)
    {
        // 先冻结真实选号与映射模型，旁路事件即使早于 next 返回也能正确归属。
        state
            .collector()
            .attempt(&id, attempt, owner, state.elapsed());
    }
    let response = call.next.run(call.request).await?;
    if mount == MiddlewareMount::Request {
        let batches = state.collector().headers(
            &id,
            extract::headers(&response.headers, now_ms()),
            state.elapsed(),
        );
        state.flush(&call.host, batches).await;
    }
    // 3.15.2 的内部 RawBytes 控制帧不能经过 SSE/JSON 正文回调校验。
    // 原样归还未读取的句柄，让宿主保留流、取消、提交和终态信封的所有权。
    Ok(response)
}

pub async fn websocket(
    state: &AppState,
    call: TypedCall<ObserveWebSocketResponse>,
) -> Result<TypedReply<Empty>, PluginFault> {
    if call.request.provider != "openai" || !call.request.payload_included {
        return Ok(TypedReply::new(Empty {}));
    }
    let mut samples = extract::frame(
        &call.payload,
        gateway_plugin_sdk::call::middleware::MiddlewareBodyFraming::JsonDocument,
        now_ms(),
    );
    if samples.is_empty() {
        return Ok(TypedReply::new(Empty {}));
    }
    let owner = state.collector().attempt_owner(
        &call.request.request_id,
        call.request.attempt_index,
        state.elapsed(),
    );
    let Some(owner) =
        owner.filter(|owner| call.request.account_id.as_deref() == Some(owner.account.as_str()))
    else {
        let mut collector = state.collector();
        collector.unattributed = collector
            .unattributed
            .saturating_add(u64::try_from(samples.len()).unwrap_or(u64::MAX));
        return Ok(TypedReply::new(Empty {}));
    };
    for (index, sample) in samples.iter_mut().enumerate() {
        sample.event_id = event_id(
            &call.request.event_id,
            &format!("websocket:{}:{index}", call.request.sequence),
        );
    }
    state
        .flush(&call.host, vec![Batch { owner, samples }])
        .await;
    Ok(TypedReply::new(Empty {}))
}

pub async fn terminal(
    state: &AppState,
    call: TypedCall<ObserveRequest>,
) -> Result<TypedReply<Empty>, PluginFault> {
    let owner = (call.request.provider.as_deref() == Some("openai"))
        .then(|| {
            Attribution::new(
                call.request.account_id.as_deref(),
                call.request.upstream_model.as_deref(),
            )
        })
        .flatten();
    let batches = state
        .collector()
        .terminal(&call.request.request_id, owner, state.elapsed());
    state.flush(&call.host, batches).await;
    Ok(TypedReply::new(Empty {}))
}
