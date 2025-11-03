use std::collections::HashMap;
use std::time::Duration;

use eloelo_model::player::{DiscordUsername, PlayerDb};
use itertools::join;
use log::{error, info};

use crate::eloelo::config::Config;
use crate::eloelo::elodisco::dota_bot::Hero;
use crate::eloelo::elodisco::messages;
use crate::eloelo::elodisco::utils::DirectMessenger;
use crate::eloelo::message_bus::{
    Event, FinishMatch, MatchStart, MatchStartTeam, Message, RichMatchResult, UiCommand,
};
use crate::utils::print_err;
use eloelo_model::{GameId, PlayerId, WinScale};
use poise::serenity_prelude as serenity;

pub struct NotificationBot {
    notifications: HashMap<DiscordUsername, bool>,
    config: Config,
    ctx: serenity::Context,
    channel: serenity::GuildChannel,
    guild_members: HashMap<DiscordUsername, serenity::User>,
}

impl NotificationBot {
    pub fn new(
        notifications: HashMap<DiscordUsername, bool>,
        config: Config,
        ctx: serenity::Context,
        channel: serenity::GuildChannel,
        guild_members: HashMap<DiscordUsername, serenity::User>,
    ) -> Self {
        Self {
            notifications,
            config,
            ctx,
            channel,
            guild_members,
        }
    }

    pub async fn on_message(&mut self, message: &Message) {
        match message {
            Message::Event(Event::HeroesAssigned(event)) => {
                self.send_match_start(&event.match_start, &event.assignments)
                    .await;
            }
            Message::Event(Event::RichMatchResult(rich_match_result)) => {
                self.send_match_result(rich_match_result).await;
            }
            Message::UiCommand(UiCommand::FinishMatch(FinishMatch::Cancelled)) => {
                self.send_match_cancelled().await;
            }
            _ => {}
        }
    }

    pub fn get_state(&self) -> &HashMap<DiscordUsername, bool> {
        &self.notifications
    }

    pub async fn match_start(
        &self,
        match_start: &MatchStart,
        hero_assignments: &HashMap<DiscordUsername, Vec<Hero>>,
    ) {
        let players = match_start
            .left_team
            .players
            .keys()
            .chain(match_start.right_team.players.keys());

        let discord_users = players
            .flat_map(|p| match_start.player_db.get(p))
            .flat_map(|p| p.discord_username().map(|d| (&p.id, d)))
            .filter(|(p, u)| self.notifications_allowed(&p) && self.notifications_enabled(&u))
            .filter_map(|(p, u)| {
                let user = self.guild_members.get(u);
                if user.is_none() {
                    error!("{} not found in guild members. This should not happen.", u);
                }
                user.cloned()
                    .map(|u| (p, DirectMessenger::new(&self.ctx, u)))
            });

        for (player_id, dm) in discord_users {
            dm.send_dm(
                messages::personal_match_start_message(player_id, &match_start),
                "match_start",
            )
            .await;
            dm.send_dm(
                messages::personal_hero_assignment_message(dm.username(), hero_assignments),
                "hero_assignment",
            )
            .await;
        }
    }

    fn notifications_enabled(&self, username: &DiscordUsername) -> bool {
        self.notifications.get(username).copied().unwrap_or(false)
    }

    fn notifications_allowed(&self, player_id: &PlayerId) -> bool {
        let player_on_allowlist = self.config.discord_test_mode_players.contains(player_id);

        !self.config.test_mode || player_on_allowlist
    }

    async fn send_match_start(
        &mut self,
        match_start: &MatchStart,
        hero_assignments: &HashMap<DiscordUsername, Vec<Hero>>,
    ) {
        self.match_start(&match_start, &hero_assignments).await;

        info!("Sending match start message to common channel");
        if match_start.game == GameId::from("DotA 2") {
            send_start_match_message(
                &self.ctx,
                &self.channel.id,
                match_start.clone(),
                &hero_assignments,
            )
            .await;
        } else {
            let heroes: HashMap<_, _> = Default::default();
            send_start_match_message(&self.ctx, &self.channel.id, match_start.clone(), &heroes)
                .await;
        }
    }

    pub async fn send_match_result(&self, match_result: &RichMatchResult) {
        let msg = serenity::CreateMessage::new().content(
            [
                format!("## 🏆 {} have won!", match_result.winner_team_name),
                make_win_scale_comment(match_result.scale),
                String::new(),
                make_duration_comment(match_result.duration),
            ]
            .join("\n"),
        );
        send_message(&self.ctx, &self.channel.id, msg).await;
    }

    pub async fn send_match_cancelled(&self) {
        let msg = serenity::CreateMessage::new().content("## Match cancelled 😩");
        send_message(&self.ctx, &self.channel.id, msg).await;
    }

    pub async fn send_user_reroll(&self, username: &DiscordUsername, new_pool: &[Hero]) {
        //TODO: use player name instead of username here
        send_message(
            &self.ctx,
            &self.channel.id,
            messages::reroll_broadcast_message(username, new_pool),
        )
        .await;
    }
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

async fn send_start_match_message(
    ctx: &serenity::Context,
    channel: &serenity::ChannelId,
    msg: MatchStart,
    hero_assignments: &HashMap<DiscordUsername, Vec<Hero>>,
) {
    let msg = serenity::CreateMessage::new()
        .content(format!("# {} Match Starting", msg.game))
        .add_embeds(vec![
            make_team_embed(
                TeamEmbedData::new(&msg.player_db, &msg.left_team, hero_assignments),
                serenity::Colour::DARK_GREEN,
            ),
            make_team_embed(
                TeamEmbedData::new(&msg.player_db, &msg.right_team, hero_assignments),
                serenity::Colour::DARK_RED,
            ),
        ]);
    send_message(&ctx, &channel, msg).await;
}

async fn send_message(
    ctx: &serenity::Context,
    channel: &serenity::ChannelId,
    msg: serenity::CreateMessage,
) {
    let _ = channel.send_message(ctx, msg).await.inspect_err(print_err);
}

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
        hero_assignments: &HashMap<DiscordUsername, Vec<Hero>>,
    ) -> Self {
        let mut players: Vec<PlayerEmbedData> = Vec::new();
        for (player_id, elo) in team.players.iter() {
            let discord_username = playerdb
                .get(player_id)
                .and_then(|p| p.discord_username().cloned())
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

fn make_team_embed(team: TeamEmbedData, colour: serenity::Colour) -> serenity::CreateEmbed {
    let total_elo: i32 = team.players.iter().map(|p| p.rank).sum();
    let mut players = team.players.clone();
    players.sort_by_key(|p| p.rank);
    players.reverse();
    serenity::CreateEmbed::new()
        .title(team.name)
        .fields(players.into_iter().map(|p| {
            (
                format!("{}   [{}]", p.name, p.rank),
                format!("{}", p.recommendations),
                false,
            )
        }))
        .colour(colour)
        .footer(serenity::CreateEmbedFooter::new(format!(
            "Total rank: {}",
            total_elo
        )))
}
