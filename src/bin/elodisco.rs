//! Standalone Discord bot server. A remote eloelo instance connects to it over the bridge.

use anyhow::Context;
use eloelo::message_bus::MessageBus;
use eloelo::utils::{terminate_on_signal, unwrap_or_def_verbose, ResultExt as _};
use eloelo::{bridge, elodisco, logging, store};
use log::{error, info};
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() {
    logging::init();
    let config = unwrap_or_def_verbose(store::load_config());
    let bot_state = unwrap_or_def_verbose(store::load_bot_state());
    let message_bus = MessageBus::new();
    let bot_ready = elodisco::run(config.clone(), bot_state, message_bus.clone());

    let shutdown = CancellationToken::new();
    tokio::spawn({
        let shutdown = shutdown.clone();
        async move {
            terminate_on_signal().await.print_err();
            info!("Terminating.");
            shutdown.cancel();
        }
    });

    // eloelo is let in only once the bot handles its messages, otherwise the first ones (e.g. its
    // test mode) would be lost.
    info!("Waiting for the Discord bot to start...");
    tokio::select! {
        ready = bot_ready => if let Err(e) = ready {
            error!("{e:#}");
            std::process::exit(1);
        },
        _ = shutdown.cancelled() => return,
    }

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
