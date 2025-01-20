use crate::utils::ResultExt as _;

use super::config::Config;
use super::message_bus::{Event, FinishMatch, Message, MessageBus, UiCommand, UiUpdate};
use anyhow::{Context as _, Result};
use async_elodisco::EloDisco;
use bot_state::BotState;
use log::info;
use serenity::all::GatewayIntents;

mod async_elodisco;
pub(crate) mod bot_state;
pub(crate) mod command_handler;
pub(crate) mod dota_bot;
pub(crate) mod notification_bot;
mod utils;

async fn run_serenity_client(token: String, elodisco: EloDisco) -> Result<()> {
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;
    let mut client = serenity::Client::builder(token, intents)
        .event_handler(elodisco)
        .await?;

    info!("Starting Serenity client");
    client.start().await.context("Serenity client failed")
}

fn start_serenity_in_background(token: String, elodisco: EloDisco) {
    tokio::spawn(async move {
        run_serenity_client(token, elodisco).await.print_err();
    });
}

pub async fn run(config: Config, bot_state: BotState, message_bus: MessageBus) {
    let token = config.discord_bot_token.clone();
    let async_elodisco = EloDisco::new(bot_state, config);
    start_serenity_in_background(token, async_elodisco.clone());

    let mut message_bus_receiver = message_bus.subscribe();
    info!("Elodisco subscribed to message bus.");
    loop {
        match message_bus_receiver.recv().await {
            Some(Message::Event(Event::MatchStart(match_start))) => {
                async_elodisco.send_match_start(match_start).await;
            }
            Some(Message::UiCommand(UiCommand::InitializeUi)) => {
                let discord_players = async_elodisco.fetch_player_info().await;
                message_bus.send(Message::UiUpdate(UiUpdate::DiscordInfo(discord_players)));
            }
            Some(Message::Event(Event::RichMatchResult(rich_match_result))) => {
                async_elodisco.send_match_result(rich_match_result).await;
            }
            Some(Message::UiCommand(UiCommand::FinishMatch(FinishMatch::Cancelled))) => {
                async_elodisco.send_match_cancelled().await;
            }
            Some(_) => {}
            None => {
                info!("EloDisco: message bus closed")
            }
        }
    }
}
