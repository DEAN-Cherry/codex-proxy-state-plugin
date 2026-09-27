mod directory;
mod overview;
mod registration;
mod response;

use gateway_plugin_sdk::{
    ErrorCode, PluginFault,
    call::management::{ManagementRequest, ManagementResponse},
    client::{TypedCall, TypedReply},
};
use serde::Deserialize;
use serde_json::json;

use crate::{
    records::{AccountRecords, ModelView},
    settings::{Settings, valid_identifier},
    state::{AppState, now_ms},
    store,
};
pub use registration::registration;
use response::{error, json_reply};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AccountQuery {
    account_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SettingsUpdate {
    account_id: String,
    expected_version: Option<u64>,
    settings: Settings,
}

pub async fn handle(
    state: &AppState,
    call: TypedCall<ManagementRequest>,
) -> Result<TypedReply<ManagementResponse>, PluginFault> {
    if call.payload.len() > 8192 {
        return error(400, "invalid_request", "请求正文过大");
    }
    let result = match (call.request.method.as_str(), call.request.path.as_str()) {
        ("GET", "api/accounts") => accounts(&call).await,
        ("POST", "api/overview") if call.request.query.is_empty() => {
            overview::snapshot(state, &call).await
        }
        ("POST", "api/account") if call.request.query.is_empty() => {
            let Ok(query) = serde_json::from_slice::<AccountQuery>(&call.payload) else {
                return error(400, "invalid_request", "账号查询格式无效");
            };
            if !valid_identifier(&query.account_id) {
                return error(400, "invalid_request", "账号标识无效");
            }
            let _guard = state.storage.lock().await;
            snapshot(state, &call.host, &query.account_id).await
        }
        ("POST", "api/settings") if call.request.query.is_empty() => {
            let Ok(update) = serde_json::from_slice::<SettingsUpdate>(&call.payload) else {
                return error(400, "invalid_request", "配置格式无效");
            };
            if !valid_identifier(&update.account_id) {
                return error(400, "invalid_request", "账号标识无效");
            }
            if let Err(message) = update.settings.validate() {
                return error(400, "invalid_request", message);
            }
            let _guard = state.storage.lock().await;
            match store::put(
                &call.host,
                ("settings", &update.account_id, update.expected_version),
                &update.settings,
            )
            .await
            {
                Ok(_) => snapshot(state, &call.host, &update.account_id).await,
                Err(fault) => Err(fault),
            }
        }
        _ => error(404, "not_found", "未找到插件接口"),
    };
    match result {
        Err(fault) if fault.code == ErrorCode::Conflict => {
            error(409, "conflict", "配置已被其他页面更新，请重新加载后再保存")
        }
        Err(_) => error(502, "storage_unavailable", "宿主数据读取或保存失败，请重试"),
        success => success,
    }
}

async fn accounts(
    call: &TypedCall<ManagementRequest>,
) -> Result<TypedReply<ManagementResponse>, PluginFault> {
    if !call.payload.is_empty() {
        return error(400, "invalid_request", "账号列表不接受正文");
    }
    let mut cursor = None;
    for (name, value) in url::form_urlencoded::parse(call.request.query.as_bytes()) {
        if name != "cursor" || cursor.is_some() || !valid_identifier(&value) {
            return error(400, "invalid_request", "账号分页参数无效");
        }
        cursor = Some(value.into_owned());
    }
    let page = directory::list(&call.host, cursor, 200).await?;
    let accounts: Vec<_> = page
        .accounts
        .into_iter()
        .map(|account| json!({"accountId":account.account_id, "accountName":account.name, "enabled":account.enabled}))
        .collect();
    json_reply(
        200,
        &json!({"accounts":accounts,"nextCursor":page.next_cursor}),
    )
}

async fn snapshot(
    state: &AppState,
    host: &gateway_plugin_sdk::client::HostClient,
    account: &str,
) -> Result<TypedReply<ManagementResponse>, PluginFault> {
    let now = now_ms();
    let data = load_account(host, account, now).await?;
    json_reply(
        200,
        &json!({
            "accountId":account, "version":data.version, "settings":data.settings,
            "models":data.models, "diagnostics":state.diagnostics(), "nowMs":now
        }),
    )
}

struct AccountData {
    version: Option<u64>,
    settings: Settings,
    models: Vec<ModelView>,
}

async fn load_account(
    host: &gateway_plugin_sdk::client::HostClient,
    account: &str,
    now: u64,
) -> Result<AccountData, PluginFault> {
    let (settings, version) = store::get::<Settings>(host, "settings", account).await?;
    let rules = settings
        .validate()
        .map_err(|message| PluginFault::new(ErrorCode::Fault, message))?;
    let (mut records, _) = store::get::<AccountRecords>(host, "observations", account).await?;
    // 展示始终按当前配置解释保留的长度，不把旧规则的匹配结果冒充当前结果。
    records.reclassify(&rules);
    Ok(AccountData {
        models: records.views(&settings, now),
        version,
        settings,
    })
}
