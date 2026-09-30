use gateway_plugin_sdk::{
    PluginFault,
    call::management::{ManagementRequest, ManagementResponse},
    client::{TypedCall, TypedReply},
};
use serde::{Deserialize, Serialize};

use crate::{
    records::{ModelView, Source},
    settings::{Validation, valid_identifier},
    state::{AppState, Diagnostics, now_ms},
};

use super::{
    directory, load_account,
    response::{error, json_reply},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Query {
    #[serde(default)]
    cursor: Option<String>,
    #[serde(default = "default_limit")]
    limit: u16,
}

const fn default_limit() -> u16 {
    20
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Page {
    accounts: Vec<Account>,
    next_cursor: Option<String>,
    diagnostics: Diagnostics,
    now_ms: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Account {
    account_id: String,
    account_name: String,
    enabled: bool,
    observation_enabled: Option<bool>,
    error: Option<&'static str>,
    models: Vec<ModelSummary>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ModelSummary {
    model: String,
    observations: usize,
    matched: usize,
    mismatched: usize,
    last_observed_at_ms: u64,
    expires_at_ms: u64,
    latest_length: u32,
    latest_fingerprint: String,
    source: Source,
    validation: Validation,
    expired: bool,
}

impl From<ModelView> for ModelSummary {
    fn from(model: ModelView) -> Self {
        Self {
            model: model.model,
            observations: model.observations,
            matched: model.matched,
            mismatched: model.mismatched,
            last_observed_at_ms: model.last_observed_at_ms,
            expires_at_ms: model.expires_at_ms,
            latest_length: model.latest_length,
            latest_fingerprint: model.latest_fingerprint,
            source: model.source,
            validation: model.validation,
            expired: model.expired,
        }
    }
}

pub(super) async fn snapshot(
    state: &AppState,
    call: &TypedCall<ManagementRequest>,
) -> Result<TypedReply<ManagementResponse>, PluginFault> {
    let Ok(query) = serde_json::from_slice::<Query>(&call.payload) else {
        return error(400, "invalid_request", "聚合查询格式无效");
    };
    if !(1..=50).contains(&query.limit)
        || query
            .cursor
            .as_deref()
            .is_some_and(|cursor| !valid_identifier(cursor))
    {
        return error(
            400,
            "invalid_request",
            "分页数量应为 1 到 50，游标必须为有效账号 ID",
        );
    }
    let page = directory::list(&call.host, query.cursor, query.limit).await?;
    let now = now_ms();
    let mut accounts = Vec::with_capacity(page.accounts.len());
    // 每页有界，只读取完整版本的私有记录，不占用业务观测的写锁。
    for account in page.accounts {
        let (observation_enabled, error, models) =
            match load_account(&call.host, &account.account_id, now).await {
                Ok(data) => (
                    Some(data.settings.enabled),
                    None,
                    data.models.into_iter().map(ModelSummary::from).collect(),
                ),
                Err(_) => (None, Some("unavailable"), Vec::new()),
            };
        let account_name = directory::display_label(&account);
        accounts.push(Account {
            account_id: account.account_id,
            account_name,
            enabled: account.enabled,
            observation_enabled,
            error,
            models,
        });
    }
    json_reply(
        200,
        &Page {
            accounts,
            next_cursor: page.next_cursor,
            diagnostics: state.diagnostics(),
            now_ms: now,
        },
    )
}
