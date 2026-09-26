use std::collections::BTreeSet;

use codex_proxy_state_plugin::{PLUGIN_ID, manifest, plugin};
use gateway_plugin_sdk::client::{PluginSession, SessionConfig, SessionError};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let session = PluginSession::accept(
        tokio::io::stdin(),
        tokio::io::stdout(),
        SessionConfig::default(),
    )
    .await?;
    let manifest = manifest()?;
    let granted: BTreeSet<_> = session.handshake().permissions.iter().copied().collect();
    if session.handshake().plugin_id != PLUGIN_ID
        || session.handshake().contributes != manifest.contributes
        || granted != manifest.permissions
        || !session
            .handshake()
            .configuration
            .as_object()
            .is_some_and(serde_json::Map::is_empty)
    {
        return Err(SessionError::Handshake.into());
    }
    session.run(plugin()?).await?;
    Ok(())
}
