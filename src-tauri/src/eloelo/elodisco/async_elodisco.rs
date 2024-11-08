use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context as _, Result};
use eloelo_model::player::{DiscordUsername, PlayerDb};
use log::{debug, error, info, trace, warn};
use serenity::all::{
    CacheHttp, Channel, ChannelId, Colour, Context, CreateEmbed, CreateEmbedFooter, CreateMessage,
    EventHandler, GuildChannel, GuildId, GuildInfo, Message, PrivateChannel, Ready, User,
};
use tokio::sync::{Mutex, MutexGuard};

use crate::eloelo::config::Config;
use crate::eloelo::elodisco::command_handler::{parse_command, CommandHandler};
use crate::eloelo::message_bus::{
    AvatarUrl, DiscordPlayerInfo, MatchStart, MatchStartTeam, RichMatchResult,
};
use crate::eloelo::silly_responder::SillyResponder;
use crate::eloelo::{join, print_err};
use eloelo_model::{GameId, PlayerId, WinScale};
use tokio::sync::watch;

use super::bot_state::{BotState, PlayerBotState};
use super::dota_bot::{DotaBot, Hero};
use super::notification_bot::NotificationBot;

enum SerenityInitState {
    Wait,
    Ready(Option<SerenityContext>),
}

impl SerenityInitState {
    fn is_ready(&self) -> bool {
        match self {
            SerenityInitState::Wait => false,
            SerenityInitState::Ready(_) => true,
        }
    }

    fn unwrap(&self) -> Option<SerenityContext> {
        match self {
            SerenityInitState::Wait => panic!("Serenity state not ready!"),
            SerenityInitState::Ready(maybe_context) => maybe_context.clone(),
        }
    }
}

#[derive(Clone)]
struct SerenityContext {
    guild_id: GuildId,
    channel_id: ChannelId,
    members: HashMap<DiscordUsername, User>,
    ctx: Context,
}

struct SerenityContextCell {
    receiver: Mutex<watch::Receiver<SerenityInitState>>,
    sender: watch::Sender<SerenityInitState>,
}

impl SerenityContextCell {
    pub fn new() -> Self {
        let (sender, receiver) = watch::channel(SerenityInitState::Wait);
        Self {
            sender,
            receiver: Mutex::new(receiver),
        }
    }

    pub async fn get(&self) -> Option<SerenityContext> {
        self.receiver
            .lock()
            .await
            .wait_for(|c| c.is_ready())
            .await
            .expect("Serenity Context")
            .unwrap()
    }

    pub fn set_none(&self) {
        self.sender
            .send(SerenityInitState::Ready(None))
            .expect("Context send operation")
    }

    pub fn set(&self, context: SerenityContext) {
        self.sender
            .send(SerenityInitState::Ready(Some(context)))
            .expect("Context send operation")
    }
}

#[derive(Clone)]
pub struct AsyncEloDisco(Arc<AsyncEloDiscoInner>);

struct AsyncEloDiscoInner {
    silly_responder: SillyResponder,
    dota_bot: Mutex<DotaBot>, // TODO: try moving mutex outside
    notification_bot: Mutex<NotificationBot>,
    config: Config,
    serenity_context_cell: SerenityContextCell,
    stored_bot_state: Mutex<BotState>,
}

impl AsyncEloDisco {
    pub fn new(bot_state: BotState, config: Config) -> Self {
        let dota_bot = DotaBot::with_state(
            bot_state
                .players
                .iter()
                .map(|(p, s)| (p.clone(), s.dota.clone()))
                .collect(),
        );
        AsyncEloDisco(Arc::new(AsyncEloDiscoInner {
            silly_responder: SillyResponder::new(),
            dota_bot: Mutex::new(dota_bot),
            notification_bot: Mutex::new(NotificationBot::new(
                bot_state
                    .players
                    .iter()
                    .map(|(p, c)| (p.clone(), c.notifications))
                    .collect(),
            )),
            config,
            stored_bot_state: Mutex::new(bot_state),
            serenity_context_cell: SerenityContextCell::new(),
        }))
    }

    async fn collect_bot_state(
        &self,
        notification_bot: &NotificationBot,
        dota_bot: &DotaBot,
    ) -> BotState {
        let mut dota_state = dota_bot.get_state().clone();
        let mut notification_state = notification_bot.get_state().clone();
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

    async fn store_bot_state_if_changed(
        &self,
        notification_bot: &NotificationBot,
        dota_bot: &DotaBot,
    ) {
        let current_state = self.collect_bot_state(notification_bot, dota_bot).await;
        if current_state != *self.0.stored_bot_state.lock().await {
            self.store_bot_state(current_state).await;
        }
    }

    async fn store_bot_state(&self, state: BotState) {
        debug!("Storing bot state");
        if let Err(e) =
            tokio::task::spawn_blocking(move || crate::store::store_bot_state(&state)).await
        {
            error!("Failed to store bot state: {}", e);
        }
    }

    async fn context(&self) -> Option<SerenityContext> {
        self.0.serenity_context_cell.get().await
    }

    pub async fn send_match_start(&self, match_start: MatchStart) {
        let Some(SerenityContext {
            ctx,
            channel_id,
            members,
            ..
        }) = self.context().await
        else {
            warn!("Match start not sent: Discord integration not available");
            return;
        };
        // TODO: make dota a hardcoded game
        if match_start.game == GameId::from("DotA 2") {
            let dota_bot = self.0.dota_bot.lock().await;
            let hero_assignments = dota_bot.match_start(&match_start, &ctx, &members).await;
            send_start_match_message(&ctx, &channel_id, match_start.clone(), &hero_assignments)
                .await;
        } else {
            let heroes: HashMap<_, _> = Default::default();
            send_start_match_message(&ctx, &channel_id, match_start.clone(), &heroes).await;
        }
        self.0
            .notification_bot
            .lock()
            .await
            .match_start(&match_start, &ctx, &members)
            .await;
    }

    pub async fn send_match_result(&self, match_result: RichMatchResult) {
        if let Some(SerenityContext {
            ctx, channel_id, ..
        }) = self.context().await
        {
            send_match_result_message(&ctx, &channel_id, match_result).await;
        }
    }

    pub async fn send_match_cancelled(&self) {
        if let Some(SerenityContext {
            ctx, channel_id, ..
        }) = self.context().await
        {
            send_match_cancelled_message(&ctx, &channel_id).await;
        }
    }

    pub async fn fetch_player_info(&self) -> Vec<DiscordPlayerInfo> {
        let Some(serenity) = self.context().await else {
            return Default::default();
        };

        info!("Discord: Initializing Guild data");
        gather_guild_data(serenity.ctx, serenity.guild_id)
            .await
            .context("fetch_avatars")
            .inspect_err(print_err)
            .unwrap_or_default()
    }

    async fn get_notification_bot(&self) -> MutexGuard<'_, NotificationBot> {
        self.0.notification_bot.lock().await
    }

    async fn dispatch_command(&self, username: &DiscordUsername, command: &str) -> String {
        let (command, args) = parse_command(command);
        debug!("Received command: {}, args: {:?}", command, args);
        if command == "help" {
            return self.dispatch_help().await;
        }

        let mut notification_bot = self.get_notification_bot().await;
        let mut dota_bot = self.0.dota_bot.lock().await;

        let result = None
            .or_else(|| notification_bot.dispatch_command(username, command, &args))
            .or_else(|| dota_bot.dispatch_command(username, command, &args));

        match result {
            Some(Ok(response)) => {
                self.store_bot_state_if_changed(&notification_bot, &dota_bot)
                    .await;
                response
            }
            Some(Err(error)) => error.to_string(),
            None => format!("Unknown command {}", command),
        }
    }

    async fn dispatch_help(&self) -> String {
        let mut commands = Vec::new();
        commands.extend(self.0.dota_bot.lock().await.supported_commands());
        commands.extend(self.0.notification_bot.lock().await.supported_commands());
        commands
            .into_iter()
            .map(|c| format!(" - `/{}` {}", c.keyword, c.description))
            .collect::<Vec<_>>()
            .join("\n")
    }

    async fn get_guild(&self, ctx: &Context) -> Option<GuildInfo> {
        ctx.http
            .get_guilds(None, None)
            .await
            .context("get_guilds")
            .inspect_err(print_err)
            .inspect(|g| g.iter().for_each(|g| debug!("Fetched guild: {}", g.name)))
            .ok()
            .into_iter()
            .flatten()
            .find(|g| g.name == self.0.config.discord_server_name)
    }

    async fn get_guild_members(
        &self,
        ctx: &Context,
        guild: GuildId,
    ) -> HashMap<DiscordUsername, User> {
        guild
            .members(ctx.http(), None, None)
            .await
            .context("get_guild_members")
            .inspect_err(print_err)
            .inspect(|members| {
                members
                    .iter()
                    .for_each(|m| debug!("Fetched member: {}", m.display_name()));
            })
            .ok()
            .into_iter()
            .flatten()
            .map(|m| (DiscordUsername::from(m.user.name.clone()), m.user))
            .collect()
    }

    async fn get_channel(&self, ctx: &Context, guild: GuildId) -> Option<GuildChannel> {
        ctx.http
            .get_channels(guild)
            .await
            .context("get_channels")
            .inspect(|c| c.iter().for_each(|c| trace!("Fetched channel: {}", c.name)))
            .inspect_err(print_err)
            .ok()
            .into_iter()
            .flatten()
            .find(|c| c.name == self.0.config.discord_channel_name)
    }

    async fn respond(&self, ctx: &Context, channel_id: ChannelId, response: &str) {
        let _ = channel_id
            .say(&ctx.http, response)
            .await
            .inspect_err(print_err);
    }
}

#[serenity::async_trait]
impl EventHandler for AsyncEloDisco {
    async fn message(&self, context: Context, msg: Message) {
        // Don't answer own messages
        if msg.author.bot {
            return;
        }
        // Collapse to private channel
        let Some(private_channel): Option<PrivateChannel> = msg
            .channel(&context)
            .await
            .inspect_err(print_err)
            .ok()
            .and_then(Channel::private)
        else {
            // Ignore non-private messages
            return;
        };
        if msg.content.starts_with("/") {
            let username = &DiscordUsername::from(private_channel.recipient.name);
            self.respond(
                &context,
                msg.channel_id,
                &self.dispatch_command(&username, &msg.content).await,
            )
            .await;
        } else {
            // If it's not a command then we can be rude
            self.respond(&context, msg.channel_id, self.0.silly_responder.respond())
                .await;
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);

        let Some(guild) = self.get_guild(&ctx).await else {
            info!("Discord: Message relay disabled due to errors.");
            self.0.serenity_context_cell.set_none();
            return;
        };
        let members = self.get_guild_members(&ctx, guild.id).await;
        if let Some(channel) = self.get_channel(&ctx, guild.id).await {
            self.0.serenity_context_cell.set(SerenityContext {
                ctx: ctx.clone(),
                channel_id: channel.id,
                guild_id: guild.id,
                members,
            });
        } else {
            info!("Discord: Message relay disabled due to errors.");
            self.0.serenity_context_cell.set_none();
            return;
        };

        info!("Discord client ready");
    }
}

async fn send_start_match_message(
    ctx: &Context,
    channel: &ChannelId,
    msg: MatchStart,
    hero_assignments: &HashMap<DiscordUsername, Vec<&Hero>>,
) {
    let msg = CreateMessage::new()
        .content(format!("# {} Match Starting", msg.game))
        .add_embeds(vec![
            make_team_embed(
                TeamEmbedData::new(&msg.player_db, &msg.left_team, hero_assignments),
                Colour::DARK_GREEN,
            ),
            make_team_embed(
                TeamEmbedData::new(&msg.player_db, &msg.right_team, hero_assignments),
                Colour::DARK_RED,
            ),
        ]);
    send_message(channel, ctx, msg).await;
}

fn make_hero_assignments_message(
    hero_assignments: &HashMap<DiscordUsername, Vec<&Hero>>,
) -> String {
    hero_assignments
        .iter()
        .map(|(user, heroes)| format!("**{user}**: {}", join(heroes, ", ")))
        .collect::<Vec<_>>()
        .join("\n")
}

async fn send_match_cancelled_message(ctx: &Context, channel: &ChannelId) {
    let msg = CreateMessage::new().content("## Match cancelled 😩");
    send_message(channel, ctx, msg).await;
}

async fn send_match_result_message(
    ctx: &Context,
    channel: &ChannelId,
    match_result: RichMatchResult,
) {
    let msg = CreateMessage::new().content(
        [
            format!("## 🏆 {} have won!", match_result.winner_team_name),
            make_win_scale_comment(match_result.scale),
            String::new(),
            make_duration_comment(match_result.duration),
        ]
        .join("\n"),
    );
    send_message(channel, ctx, msg).await;
}

fn make_win_scale_comment(scale: WinScale) -> String {
    let text = match scale {
        WinScale::Even => "⚖️ The match was even. Good work EloElo!",
        WinScale::Advantage => "💪 The advantage was significant.",
        WinScale::Pwnage => "🔥🔥🔥 It was a serious **PWNAGE!!!**",
    };

    String::from(text)
}

fn make_duration_comment(duration: Duration) -> String {
    let minutes = duration.as_secs() / 60;
    format!("⏱️ It took {minutes} minutes to beat the losers.")
}

async fn send_message(channel: &ChannelId, ctx: &Context, msg: CreateMessage) {
    let _ = channel.send_message(ctx, msg).await.inspect_err(print_err);
}

// fn make_player_entry(
//     player: &Player,
//     elo: i32,
//     hero_assignments: &HashMap<DiscordUsername, Vec<&Hero>>,
// ) {
// }

#[derive(Clone, Debug)]
struct PlayerEmbedData {
    name: String,
    rank: i32,
    recommendations: String,
}

#[derive(Clone, Debug)]
struct TeamEmbedData {
    name: String,
    players: Vec<PlayerEmbedData>,
}

impl TeamEmbedData {
    pub fn new(
        playerdb: &PlayerDb,
        team: &MatchStartTeam,
        hero_assignments: &HashMap<DiscordUsername, Vec<&Hero>>,
    ) -> Self {
        let mut players: Vec<PlayerEmbedData> = Vec::new();
        for (player_id, elo) in team.players.iter() {
            let discord_username = playerdb
                .get(player_id)
                .and_then(|p| p.discord_username.clone())
                .unwrap_or("INVALID".into());
            players.push(PlayerEmbedData {
                name: playerdb
                    .get(player_id)
                    .map(|p| p.get_display_name())
                    .unwrap_or("INVALID")
                    .to_string(),
                rank: *elo,
                recommendations: hero_assignments
                    .get(&discord_username)
                    .map(|ha| join(ha, ", "))
                    .unwrap_or_else(|| "No hero recommendations".into()),
            });
        }
        TeamEmbedData {
            name: team.name.clone(),
            players,
        }
    }
}

fn make_team_embed(team: TeamEmbedData, colour: Colour) -> CreateEmbed {
    let total_elo: i32 = team.players.iter().map(|p| p.rank).sum();
    let mut players = team.players.clone();
    players.sort_by_key(|p| p.rank);
    players.reverse();
    CreateEmbed::new()
        .title(team.name)
        .fields(players.into_iter().map(|p| {
            (
                p.name,
                format!("{} - {}", p.rank.to_string(), p.recommendations),
                false,
            )
        }))
        .colour(colour)
        .footer(CreateEmbedFooter::new(format!("Total rank: {}", total_elo)))
}

async fn gather_guild_data(ctx: impl CacheHttp, guild: GuildId) -> Result<Vec<DiscordPlayerInfo>> {
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
