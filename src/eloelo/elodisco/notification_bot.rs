use std::collections::HashMap;

use anyhow::{format_err, Result};
use eloelo_model::player::DiscordUsername;
use log::{error, info};
use serenity::all::{Context, CreateMessage, User};

use crate::eloelo::message_bus::MatchStart;
use eloelo_model::PlayerId;

use super::command_handler::{CommandDescription, CommandHandler};
use super::utils::send_direct_message;

pub struct NotificationBot {
    notifications: HashMap<DiscordUsername, bool>,
}

impl NotificationBot {
    pub fn new(notifications: HashMap<DiscordUsername, bool>) -> Self {
        Self { notifications }
    }

    pub fn get_state(&self) -> &HashMap<DiscordUsername, bool> {
        &self.notifications
    }

    pub async fn match_start(
        &self,
        message: &MatchStart,
        ctx: &Context,
        members: &HashMap<DiscordUsername, User>,
    ) {
        let players = message
            .left_team
            .players
            .keys()
            .chain(message.right_team.players.keys());
        let discord_users = players
            .flat_map(|p| message.player_db.get(p))
            .flat_map(|p| p.discord_username().map(|d| (&p.id, d)));
        for (player_id, username) in discord_users {
            let notifications_enabled = self.notifications.get(username).copied().unwrap_or(false);
            if notifications_enabled {
                match members.get(username) {
                    Some(user) => {
                        let message = CreateMessage::new()
                            .content(create_personal_match_start_message(player_id, &message));
                        tokio::spawn(send_direct_message(
                            ctx.clone(),
                            user.clone(),
                            message,
                            "match_start",
                        ));
                    }
                    None => error!("{} not found in guild members", username),
                }
            }
        }
    }
}

impl CommandHandler for NotificationBot {
    fn supported_commands(&self) -> Vec<CommandDescription> {
        vec![
            CommandDescription {
                keyword: "notifications".into(),
                description: "Control Start Match notifications. Use with enable/disable argument (e.g. `/notifications enable`)".into(),
            },
        ]
    }

    fn dispatch_command(
        &mut self,
        username: &DiscordUsername,
        command: &str,
        args: &[&str],
    ) -> Option<Result<String>> {
        if command == "notifications" {
            let first_arg_is = |pred: &str| args.first().map(|v| *v == pred).unwrap_or(false);
            if first_arg_is("enable") {
                *self.notifications.entry(username.clone()).or_default() = true;
                info!("{} enabled notifications", username);
                return Some(Ok(String::from("Notifications enabled")));
            }
            if first_arg_is("disable") {
                *self.notifications.entry(username.clone()).or_default() = false;
                info!("{} disabled notifications", username);
                return Some(Ok(String::from("Notifications disabled")));
            }
            return Some(Err(format_err!("Invalid argument")));
        }
        None
    }
}

fn create_personal_match_start_message(p: &PlayerId, match_start: &MatchStart) -> String {
    let team_name = if match_start.left_team.players.contains_key(p) {
        Some(&match_start.left_team.name)
    } else if match_start.right_team.players.contains_key(p) {
        Some(&match_start.right_team.name)
    } else {
        None
    };

    match team_name {
        Some(team) => format!(
            "**{}** match is starting! You're playing in the **{}**!\nGLHF!",
            match_start.game, team
        ),
        None => format!(
            "**{}** match is starting, but you're not playing.\n See you next time!",
            match_start.game
        ),
    }
}
