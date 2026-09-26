use gateway_plugin_sdk::{
    ErrorCode, PluginFault,
    call::host::{StateGetRequest, StateGetResult, StatePutRequest, StatePutResult},
    client::{HostClient, SessionError},
};
use serde::{Serialize, de::DeserializeOwned};

pub async fn get<T: DeserializeOwned + Default>(
    host: &HostClient,
    namespace: &str,
    key: &str,
) -> Result<(T, Option<u64>), PluginFault> {
    let reply = host
        .call(
            "host.state.get",
            serde_json::to_value(StateGetRequest {
                namespace: namespace.to_owned(),
                key: key.to_owned(),
            })
            .map_err(|_| invalid())?,
            Vec::new(),
        )
        .await
        .map_err(SessionError::into_plugin_fault)?;
    let response: StateGetResult = serde_json::from_value(reply.result).map_err(|_| invalid())?;
    if !reply.payload.is_empty() {
        return Err(invalid());
    }
    match response.record {
        Some(record) if record.schema_version == 1 => Ok((
            serde_json::from_value(record.value).map_err(|_| invalid())?,
            Some(record.version),
        )),
        Some(_) => Err(invalid()),
        None => Ok((T::default(), None)),
    }
}

pub async fn put<T: Serialize>(
    host: &HostClient,
    request: (&str, &str, Option<u64>),
    value: &T,
) -> Result<u64, PluginFault> {
    let (namespace, key, expected_version) = request;
    let params = serde_json::to_value(StatePutRequest {
        namespace: namespace.to_owned(),
        key: key.to_owned(),
        value: serde_json::to_value(value).map_err(|_| invalid())?,
        expected_version,
    })
    .map_err(|_| invalid())?;
    // RPC 控制信封有 64 KiB 上限，留出方法名和上下文空间。
    if serde_json::to_vec(&params).map_err(|_| invalid())?.len() > 56 * 1024 {
        return Err(PluginFault::new(
            ErrorCode::InvalidInput,
            "观测记录超过存储上限",
        ));
    }
    let reply = host
        .call("host.state.put", params, Vec::new())
        .await
        .map_err(SessionError::into_plugin_fault)?;
    let response: StatePutResult = serde_json::from_value(reply.result).map_err(|_| invalid())?;
    if !reply.payload.is_empty() {
        return Err(invalid());
    }
    Ok(response.version)
}

fn invalid() -> PluginFault {
    PluginFault::new(ErrorCode::Fault, "插件私有状态格式无效")
}
