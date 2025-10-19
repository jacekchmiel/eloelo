use anyhow::{Context, Result};
use eloelo::message_bus::{Message, MessageBus, UiCommand};
use eloelo::{elodisco, ocr, store, EloElo};
use log::{debug, error, info, warn};
use serenity::futures;
use std::future::Future;
use std::pin::Pin;
use tokio::signal;
use utils::{unwrap_or_def_verbose, ResultExt as _};

mod api;
mod eloelo;
mod logging;
pub(crate) mod utils;

#[cfg(unix)]
async fn terminate_on_signal() -> Result<()> {
    let interrupt_signal: Pin<Box<dyn Future<Output = _>>> = Box::pin(async {
        signal::unix::signal(signal::unix::SignalKind::interrupt())
            .context("Failed to register terminate signal handlers!")?
            .recv()
            .await;
        Ok(())
    });
    let terminate_signal = Box::pin(async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .context("Failed to register terminate signal handlers!")?
            .recv()
            .await;
        Ok(())
    });
    futures::future::select_all([interrupt_signal, terminate_signal])
        .await
        .0
}

#[cfg(windows)]
async fn terminate_on_signal() -> Result<()> {
    signal::ctrl_c()
        .await
        .context("Failed to register ctrl_c signal.")
}

#[tokio::main]
async fn main() {
    logging::init();
    let config = unwrap_or_def_verbose(store::load_config());
    if config.test_mode {
        warn!("Running in test mode.");
    }
    let players_config = unwrap_or_def_verbose(store::load_players());
    let state = unwrap_or_def_verbose(store::load_state());
    let bot_state = unwrap_or_def_verbose(store::load_bot_state());
    let spawelo_options = unwrap_or_def_verbose(store::load_options());
    let message_bus = MessageBus::new();
    tokio::spawn(elodisco::run(
        config.clone(),
        bot_state,
        message_bus.clone(),
    ));
    let eloelo = EloElo::new(
        state,
        config.clone(),
        players_config.clone(),
        spawelo_options,
        message_bus.clone(),
    );
    let eloelo_task = tokio::spawn(eloelo.dispatch_ui_commands(message_bus.clone()));
    let _ = ocr::spawn_dota_screenshot_parser(config.clone(), message_bus.clone())
        .context("spawn_dota_screenshot_parser failed")
        .inspect_err(|e| error!("{e:#}"));
    tokio::spawn(api::serve(
        message_bus.clone(),
        config.static_serving_dir.clone(),
        config.serving_addr.clone(),
    ));

    info!("Running");
    terminate_on_signal().await.print_err();
    info!("Terminating.");
    message_bus.send(Message::UiCommand(UiCommand::CloseApplication));

    debug!("Waiting for workers to stop...");
    eloelo_task.await.context("Eloelo task failed").print_err();
    debug!("All workers stopped.")
}
