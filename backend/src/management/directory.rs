use gateway_plugin_sdk::{
    ErrorCode, PluginFault,
    call::host::{AuthListRequest, AuthListResult},
    client::{HostClient, SessionError},
};

pub(super) async fn list(
    host: &HostClient,
    cursor: Option<String>,
    limit: u16,
) -> Result<AuthListResult, PluginFault> {
    // data 目录没有名称；只调用非凭据运行投影，不使用 auth.get 或 auth.save。
    let payload = serde_json::to_vec(&AuthListRequest {
        provider_id: Some("openai".to_owned()),
        cursor,
        limit,
    })
    .map_err(|_| invalid())?;
    let reply = host
        .call("host.auth.list", serde_json::json!({}), payload)
        .await
        .map_err(SessionError::into_plugin_fault)?;
    if reply.result != serde_json::json!({}) {
        return Err(invalid());
    }
    serde_json::from_slice(&reply.payload).map_err(|_| invalid())
}

fn invalid() -> PluginFault {
    PluginFault::new(ErrorCode::Fault, "账号目录响应格式无效")
}
