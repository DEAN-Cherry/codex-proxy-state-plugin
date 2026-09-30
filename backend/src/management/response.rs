use gateway_plugin_sdk::{
    ErrorCode, PluginFault, call::management::ManagementResponse, client::TypedReply,
};
use serde::Serialize;
use serde_json::json;

pub fn json_reply(
    status: u16,
    value: &impl Serialize,
) -> Result<TypedReply<ManagementResponse>, PluginFault> {
    let payload = serde_json::to_vec(value)
        .map_err(|_| PluginFault::new(ErrorCode::Fault, "管理响应编码失败"))?;
    Ok(TypedReply::new(ManagementResponse {
        status,
        content_type: "application/json".to_owned(),
        headers: Vec::new(),
    })
    .with_payload(payload))
}

pub fn error(
    status: u16,
    code: &str,
    message: &str,
) -> Result<TypedReply<ManagementResponse>, PluginFault> {
    json_reply(status, &json!({"error":{"code":code,"message":message}}))
}
