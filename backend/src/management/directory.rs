use gateway_plugin_sdk::{
    PluginFault,
    call::data::{AccountFactsPage, AccountFactsQuery},
    client::HostClient,
};

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
