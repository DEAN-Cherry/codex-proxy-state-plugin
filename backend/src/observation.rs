use std::sync::Arc;

use gateway_plugin_sdk::{
    PluginFault,
    call::{middleware::MiddlewareMount, policy::ObserveRequest},
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
    let mut response = call.next.run(call.request).await?;
    match mount {
        MiddlewareMount::Request => {
            let batches = state.collector().headers(
                &id,
                extract::headers(&response.headers, now_ms()),
                state.elapsed(),
            );
            state.flush(&call.host, batches).await;
        }
        MiddlewareMount::Attempt => {
            if let (Some(owner), Some(attempt), Some(framing)) =
                (owner, attempt, response.body.framing())
            {
                let mut sequence = 0_u64;
                response.body = response.body.inspect_frames(move |frame| {
                    sequence = sequence.saturating_add(1);
                    let mut samples = extract::frame(&frame.payload, framing, now_ms());
                    for (index, sample) in samples.iter_mut().enumerate() {
                        sample.event_id =
                            event_id(&id, &format!("attempt:{attempt}:frame:{sequence}:{index}"));
                    }
                    if !samples.is_empty() {
                        state.collector().samples(
                            &id,
                            Batch {
                                owner: owner.clone(),
                                samples,
                            },
                            state.elapsed(),
                        );
                    }
                })?;
            }
        }
    }
    Ok(response)
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
