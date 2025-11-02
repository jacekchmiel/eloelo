use super::bot_state::{BotState, PlayerBotState};
use super::dota_bot::DotaBot;
use super::notification_bot::NotificationBot;
use crate::eloelo::config::Config;
use crate::eloelo::elodisco::dota_bot::Hero;
use crate::eloelo::message_bus::{
    AvatarUrl, DiscordPlayerInfo, Message, MessageBus, UiCommand, UiUpdate,
};
use crate::eloelo::print_err;
use anyhow::{bail, Context as _, Result};
use eloelo_model::player::DiscordUsername;
use eloelo_model::PlayerId;
use log::{debug, error, info, trace};
use poise::serenity_prelude as serenity;
use poise::serenity_prelude::CacheHttp as _;
use std::collections::{HashMap, HashSet};

pub struct EloDisco {
    dota_bot: DotaBot,
    notification_bot: NotificationBot,
    #[allow(unused)]
    config: Config,
    stored_bot_state: BotState,
    message_bus: MessageBus,
    guild_id: serenity::GuildId,
    ctx: serenity::Context,
}

impl EloDisco {
    pub async fn initialize(
        bot_state: BotState,
        message_bus: MessageBus,
        config: Config,
        ctx: &serenity::Context,
    ) -> Result<Self> {
        let Some(guild) = get_guild(&ctx, &config.discord_server_name).await else {
            bail!("Discord: Failed to read guild info.");
        };
        let members = get_guild_members(&ctx, &guild.id).await;
        let dota_bot_state = bot_state
            .players
            .iter()
            .map(|(p, s)| (p.clone(), s.dota.clone()))
            .collect();
        let dota_bot = DotaBot::new(dota_bot_state, message_bus.clone(), &config);

        let channel = get_channel(&ctx, guild.id, &config.effective_discord_channel_name()).await?;
        let notifications_state = bot_state
            .players
            .iter()
            .map(|(p, c)| (p.clone(), c.notifications))
            .collect();
        let notification_bot = NotificationBot::new(
            notifications_state,
            config.clone(),
            ctx.clone(),
            channel,
            members,
        );

        let elodisco = EloDisco {
            notification_bot,
            dota_bot,
            config,
            stored_bot_state: bot_state,
            message_bus,
            guild_id: guild.id,
            ctx: ctx.clone(),
        };

        Ok(elodisco)
    }

    pub async fn on_message(&mut self, message: Message) {
        self.dota_bot.on_message(&message).await;
        self.notification_bot.on_message(&message).await;
        match message {
            Message::UiCommand(UiCommand::InitializeUi) => {
                let discord_players = self.fetch_player_info().await;
                self.message_bus
                    .send(Message::UiUpdate(UiUpdate::DiscordInfo(discord_players)));
            }
            _ => {}
        }
    }

    async fn collect_bot_state(&self) -> BotState {
        let mut dota_state = self.dota_bot.get_state().await;
        let mut notification_state = self.notification_bot.get_state().clone();
        let players: HashSet<_> = dota_state
            .keys()
            .chain(notification_state.keys())
            .cloned()
            .collect();
        let players = players
            .into_iter()
            .map(|p| {
                let dota = dota_state.remove(&p).unwrap_or_default();
                let notifications = notification_state.remove(&p).unwrap_or_default();
                (
                    p.clone(),
                    PlayerBotState {
                        notifications,
                        dota,
                    },
                )
            })
            .collect();
        BotState { players }
    }

    pub async fn store_bot_state_if_changed(&self) {
        let current_state = self.collect_bot_state().await;
        if current_state != self.stored_bot_state {
            self.store_bot_state(current_state).await;
        }
    }

    async fn store_bot_state(&self, state: BotState) {
        if let Err(e) =
            tokio::task::spawn_blocking(move || crate::store::store_bot_state(&state)).await
        {
            error!("Failed to store bot state: {}", e);
        }
    }

    async fn fetch_player_info(&self) -> Vec<DiscordPlayerInfo> {
        info!("Discord: Initializing Guild data");
        gather_guild_data(&self.ctx, self.guild_id)
            .await
            .context("gather_guild_data")
            .inspect_err(print_err)
            .unwrap_or_default()
    }

    pub fn dota_bot_mut(&mut self) -> &mut DotaBot {
        &mut self.dota_bot
    }

    pub async fn handle_user_reroll(&mut self, username: &DiscordUsername) -> Result<Vec<Hero>> {
        let new_pool = self.dota_bot.reroll(&username)?;
        self.store_bot_state_if_changed().await;
        self.notification_bot
            .send_user_reroll(username, &new_pool)
            .await;
        Ok(new_pool)
    }
}

async fn get_guild(ctx: &serenity::Context, guild_name: &str) -> Option<serenity::GuildInfo> {
    ctx.http
        .get_guilds(None, None)
        .await
        .context("get_guilds")
        .inspect_err(print_err)
        .inspect(|g| g.iter().for_each(|g| debug!("Fetched guild: {}", g.name)))
        .ok()
        .into_iter()
        .flatten()
        .find(|g| g.name == guild_name)
}

async fn get_guild_members(
    ctx: &serenity::Context,
    guild: &serenity::GuildId,
) -> HashMap<DiscordUsername, serenity::User> {
    guild
        .members(ctx.http(), None, None)
        .await
        .context("get_guild_members")
        .inspect_err(print_err)
        .inspect(|members| {
            let members = members
                .iter()
                .map(serenity::Member::display_name)
                .collect::<Vec<_>>()
                .join(", ");
            debug!("Fetched members: {members}");
        })
        .ok()
        .into_iter()
        .flatten()
        .map(|m| (DiscordUsername::from(m.user.name.clone()), m.user))
        .collect()
}

async fn get_channel(
    ctx: &serenity::Context,
    guild_id: serenity::GuildId,
    channel_name: &str,
) -> Result<serenity::GuildChannel> {
    ctx.http
        .get_channels(guild_id)
        .await
        .context("get_channels")
        .inspect(|c| c.iter().for_each(|c| trace!("Fetched channel: {}", c.name)))
        .inspect_err(print_err)
        .ok()
        .into_iter()
        .flatten()
        .find(|c| &c.name == channel_name)
        .with_context(|| format!("Discord: Failed to read channel {}.", &channel_name))
}

async fn gather_guild_data(
    ctx: impl serenity::CacheHttp,
    guild: serenity::GuildId,
) -> Result<Vec<DiscordPlayerInfo>> {
    let members = ctx.http().get_guild_members(guild, None, None).await?;
    let members: HashMap<_, _> = members
        .into_iter()
        .map(|m| (PlayerId::from(m.display_name()), m))
        .collect();
    Ok(members
        .into_iter()
        .filter(|(_, m)| !m.user.bot)
        .map(|(id, m)| {
            let avatar_url = AvatarUrl::from(m.user.face());
            DiscordPlayerInfo {
                id,
                display_name: m.display_name().to_string(),
                username: DiscordUsername::from(m.user.name),
                avatar_url,
            }
        })
        .collect())
}
