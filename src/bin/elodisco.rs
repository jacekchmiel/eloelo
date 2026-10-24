//! Standalone Discord bot server. A remote eloelo instance connects to it over the bridge.

use anyhow::Context;
use eloelo::message_bus::MessageBus;
use eloelo::utils::{terminate_on_signal, unwrap_or_def_verbose, ResultExt as _};
use eloelo::{bridge, elodisco, logging, store};
use log::info;
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() {
    logging::init();
    let config = unwrap_or_def_verbose(store::load_config());
    let bot_state = unwrap_or_def_verbose(store::load_bot_state());
    let message_bus = MessageBus::new();
    elodisco::run(config.clone(), bot_state, message_bus.clone()).await;

    let shutdown = CancellationToken::new();
    tokio::spawn({
        let shutdown = shutdown.clone();
        async move {
            terminate_on_signal().await.print_err();
            info!("Terminating.");
            shutdown.cancel();
        }
    });

    info!("Running");
    bridge::run_server(
        &config.bridge_listen_addr,
        &config.bridge_secret,
        message_bus,
        shutdown,
    )
    .await
    .context("Bridge server failed")
    .print_err();
}
