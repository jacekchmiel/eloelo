use anyhow::Context;
use eloelo::config::DiscordBotMode;
use eloelo::git_storage::GitStorage;
use eloelo::message_bus::{DiscordBotStatus, Event, Message, MessageBus, UiCommand};
use eloelo::utils::{terminate_on_signal, unwrap_or_def_verbose, unwrap_or_exit, ResultExt as _};
use eloelo::{api, bridge, elodisco, logging, ocr, store, EloElo};
use log::{debug, error, info, warn};
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() {
    logging::init();
    let config = unwrap_or_def_verbose(store::load_config());
    if config.test_mode {
        warn!("Running in test mode.");
    } else {
        info!("Running in normal mode (test mode off).");
    }
    let players_config = unwrap_or_def_verbose(store::load_players());
    let state = unwrap_or_def_verbose(store::load_state());
    let spawelo_options = unwrap_or_def_verbose(store::load_options());
    // Test mode never touches the shared history repo.
    let history_storage =
        (!config.test_mode).then(|| unwrap_or_exit(GitStorage::open(store::history_dir(), true)));
    let message_bus = MessageBus::new();
    // Subscribe before anything else runs, so EloElo doesn't miss messages sent during startup
    // (e.g. Discord bot status).
    let eloelo_messages = message_bus.subscribe();
    let shutdown = CancellationToken::new();
    let (bridge_task, elodisco_storage) = match &config.discord_bot {
        DiscordBotMode::Embedded => {
            let storage = unwrap_or_exit(GitStorage::open(store::elodisco_dir(), false));
            let mut bot_config = unwrap_or_exit(store::load_elodisco_config(&storage));
            bot_config.test_mode = config.test_mode;
            let token = unwrap_or_exit(store::load_secret(store::DISCORD_BOT_TOKEN_FILE));
            let bot_state = unwrap_or_exit(store::load_bot_state(&storage));
            // eloelo doesn't wait for the bot, its messages sent meanwhile are lost. The status
            // sent once the bot is ready lets eloelo send what the bot needs from the start.
            let bot_ready = elodisco::run(
                bot_config,
                token,
                bot_state,
                storage.clone(),
                message_bus.clone(),
            );
            tokio::spawn({
                let message_bus = message_bus.clone();
                async move {
                    match bot_ready.await {
                        Ok(()) => message_bus.send(Message::Event(Event::DiscordBotStatus(
                            DiscordBotStatus::Embedded,
                        ))),
                        Err(e) => error!("{e:#}"),
                    }
                }
            });
            (None, Some(storage))
        }
        DiscordBotMode::Remote { url } => {
            let bridge_secret = unwrap_or_exit(store::load_secret(store::BRIDGE_SECRET_FILE));
            let bridge_task = tokio::spawn(bridge::run_client(
                url.clone(),
                bridge_secret,
                config.test_mode,
                message_bus.clone(),
                shutdown.clone(),
            ));
            (Some(bridge_task), None)
        }
        DiscordBotMode::Disabled => {
            info!("Discord bot disabled.");
            (None, None)
        }
    };
    let eloelo = EloElo::new(
        state,
        config.clone(),
        players_config.clone(),
        spawelo_options,
        history_storage,
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
    debug!("All workers stopped.");
    if let Some(storage) = elodisco_storage {
        elodisco::push_storage(&storage).await;
    }
}
