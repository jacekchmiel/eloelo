use crate::UiCommand;

use super::config::Config;
use super::message_bus::{AvatarUpdate, Event, Message, MessageBus, UiUpdate};
use anyhow::Result;
use async_elodisco::AsyncEloDisco;
use bot_state::BotState;
use log::{error, info};
use serenity::all::GatewayIntents;
use tokio::runtime::Runtime;

mod async_elodisco;
pub(crate) mod bot_state;
pub(crate) mod command_handler;
pub(crate) mod dota_bot;
pub(crate) mod notification_bot;

pub struct EloDisco {
    _runtime: Runtime,
}

impl EloDisco {
    pub fn new(config: Config, bot_state: BotState, message_bus: MessageBus) -> Self {
        let _runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .max_blocking_threads(8)
            .build()
            .unwrap();

        let token = config.discord_bot_token.clone();
        let async_elodisco = AsyncEloDisco::new(bot_state, config);

        _runtime.spawn({
            let async_elodisco = async_elodisco.clone();
            async move {
                if let Err(e) = start_serenity(token, async_elodisco).await {
                    error!("Serenity failed: {}", e);
                }
            }
        });

        let mut message_bus_receiver = message_bus.subscribe();
        _runtime.spawn(async move {
            loop {
                match message_bus_receiver.recv().await {
                    Some(Message::Event(Event::MatchStart(match_start))) => {
                        async_elodisco.send_match_start(match_start).await;
                    }
                    Some(Message::UiCommand(UiCommand::InitializeUi)) => {
                        let avatars = async_elodisco.fetch_avatars().await;
                        let avatar_updates = avatars
                            .into_iter()
                            .map(|(player, avatar_url)| AvatarUpdate { player, avatar_url })
                            .collect();

                        message_bus.send(Message::UiUpdate(UiUpdate::Avatars(avatar_updates)));
                    }
                    // Some(Message::UiCommand(UiCOmmand::InitializeUi)))
                    Some(_) => {}
                    None => {
                        info!("EloDisco: message bus closed")
                    }
                }
            }
        });

        Self { _runtime }
    }
}

async fn start_serenity(token: String, elodisco: AsyncEloDisco) -> Result<()> {
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;
    let mut client = serenity::Client::builder(token, intents)
        .event_handler(elodisco)
        .await?;

    info!("Discord: Starting Discord client");
    Ok(client.start().await?)
}
