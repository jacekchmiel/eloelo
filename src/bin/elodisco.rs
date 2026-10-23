//! Standalone Discord bot server. A remote eloelo instance connects to it over the bridge.

use anyhow::Context;
use eloelo::message_bus::MessageBus;
use eloelo::utils::{terminate_on_signal, unwrap_or_def_verbose, ResultExt as _};
use eloelo::{bridge, elodisco, logging, store};
use log::info;

#[tokio::main]
async fn main() {
    logging::init();
    let config = unwrap_or_def_verbose(store::load_config());
    let bot_state = unwrap_or_def_verbose(store::load_bot_state());
    let message_bus = MessageBus::new();
    elodisco::run(config.clone(), bot_state, message_bus.clone()).await;

    info!("Running");
    tokio::select! {
        result = bridge::run_server(&config.bridge_listen_addr, &config.bridge_secret, message_bus) => {
            result.context("Bridge server failed").print_err()
        }
        result = terminate_on_signal() => result.print_err(),
    }
    info!("Terminating.");
}
