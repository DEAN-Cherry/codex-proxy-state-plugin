use gateway_plugin_sdk::{
    PluginFault,
    call::data::{AccountFacts, AccountFactsPage, AccountFactsQuery},
    client::HostClient,
};

pub(super) fn display_label(account: &AccountFacts) -> String {
    [
        account.email.as_deref().unwrap_or(""),
        account.name.as_str(),
    ]
    .into_iter()
    .map(str::trim)
    .find(|label| !label.is_empty())
    .unwrap_or(&account.account_id)
    .to_owned()
}

pub(super) async fn list(
    host: &HostClient,
    cursor: Option<String>,
    limit: u16,
) -> Result<AccountFactsPage, PluginFault> {
    host.account_facts(AccountFactsQuery {
        provider_id: Some("openai".to_owned()),
        cursor,
        limit,
    })
    .await
}
