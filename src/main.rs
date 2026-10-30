use anyhow::Context;
use eloelo::config::DiscordBotMode;
use eloelo::message_bus::{Message, MessageBus, UiCommand};
use eloelo::utils::{terminate_on_signal, unwrap_or_def_verbose, ResultExt as _};
use eloelo::{api, bridge, elodisco, logging, ocr, store, EloElo};
use log::{debug, error, info, warn};
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() {
    logging::init();
    let config = unwrap_or_def_verbose(store::load_config());
    if config.test_mode {
        warn!("Running in test mode.");
    }
    let players_config = unwrap_or_def_verbose(store::load_players());
    let state = unwrap_or_def_verbose(store::load_state());
    let spawelo_options = unwrap_or_def_verbose(store::load_options());
    let message_bus = MessageBus::new();
    // Subscribe before anything else runs, so EloElo doesn't miss messages sent during startup
    // (e.g. Discord bot status).
    let eloelo_messages = message_bus.subscribe();
    let shutdown = CancellationToken::new();
    let bridge_task = match &config.discord_bot {
        DiscordBotMode::Embedded => {
            let bot_state = unwrap_or_def_verbose(store::load_bot_state());
            tokio::spawn(elodisco::run(
                config.clone(),
                bot_state,
                message_bus.clone(),
            ));
            None
        }
        DiscordBotMode::Remote { url } => Some(tokio::spawn(bridge::run_client(
            url.clone(),
            config.bridge_secret.clone(),
            config.test_mode,
            message_bus.clone(),
            shutdown.clone(),
        ))),
        DiscordBotMode::Disabled => {
            info!("Discord bot disabled.");
            None
        }
    };
    let eloelo = EloElo::new(
        state,
        config.clone(),
        players_config.clone(),
        spawelo_options,
        message_bus.clone(),
    );
    let eloelo_task = tokio::spawn(eloelo.dispatch_messages(eloelo_messages, message_bus.clone()));
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
    shutdown.cancel();
    message_bus.send(Message::UiCommand(UiCommand::CloseApplication));

    debug!("Waiting for workers to stop...");
    eloelo_task.await.context("Eloelo task failed").print_err();
    if let Some(bridge_task) = bridge_task {
        bridge_task.await.context("Bridge task failed").print_err();
    }
    debug!("All workers stopped.")
}
