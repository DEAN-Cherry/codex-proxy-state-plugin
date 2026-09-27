pub mod collector;
pub mod extract;
mod management;
mod observation;
pub mod records;
pub mod settings;
mod state;
mod store;

use gateway_plugin_sdk::{
    Manifest,
    client::{AuthorError, ComposedPlugin, PluginBuilder, methods},
};
use state::AppState;
use std::sync::Arc;

pub const PLUGIN_ID: &str = "dean-cherry.state-observer";

pub fn manifest() -> Result<Manifest, gateway_plugin_sdk::ManifestError> {
    Manifest::from_author_slice(include_bytes!("../../plugin.json"))
}

pub fn plugin() -> Result<ComposedPlugin, AuthorError> {
    let state = Arc::new(AppState::default());
    let middleware_state = Arc::clone(&state);
    let terminal_state = Arc::clone(&state);
    let websocket_state = Arc::clone(&state);
    PluginBuilder::from_json(include_bytes!("../../plugin.json"))?
        .middleware(move |call| {
            let state = Arc::clone(&middleware_state);
            async move { observation::middleware(state, call).await }
        })?
        .on(methods::OBSERVE_REQUEST, move |call| {
            let state = Arc::clone(&terminal_state);
            async move { observation::terminal(&state, call).await }
        })?
        .on(methods::OBSERVE_WEBSOCKET, move |call| {
            let state = Arc::clone(&websocket_state);
            async move { observation::websocket(&state, call).await }
        })?
        .management(management::registration(), move |call| {
            let state = Arc::clone(&state);
            async move { management::handle(&state, call).await }
        })?
        .build()
}
