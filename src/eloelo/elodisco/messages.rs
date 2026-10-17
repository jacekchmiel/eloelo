use std::collections::HashMap;

use crate::eloelo::elodisco::bot_state::DotaBotState;
use crate::eloelo::elodisco::dota_bot::{Hero, RerollResult};
use crate::eloelo::elodisco::hero_assignment_strategy::HeroTag;
use crate::eloelo::message_bus::MatchStart;
use crate::utils;
use eloelo_model::player::DiscordUsername;
use eloelo_model::PlayerId;
use log::info;
use poise::serenity_prelude as serenity;

pub fn personal_match_start_message(
    p: &PlayerId,
    match_start: &MatchStart,
) -> serenity::CreateMessage {
    let team_name = if match_start.left_team.players.contains_key(p) {
        Some(&match_start.left_team.name)
    } else if match_start.right_team.players.contains_key(p) {
        Some(&match_start.right_team.name)
    } else {
        None
    };

    let content = match team_name {
        Some(team) => format!(
            "**{}** match is starting! You're playing in the **{}**!\nGLHF!",
            match_start.game, team
        ),
        None => format!(
            "**{}** match is starting, but you're not playing.\n See you next time!",
            match_start.game
        ),
    };
    serenity::CreateMessage::new().content(content)
}

pub fn personal_hero_assignment_message(
    username: impl Into<DiscordUsername>,
    hero_assignments: &HashMap<DiscordUsername, Vec<Hero>>,
    tags: &HashMap<DiscordUsername, HeroTag>,
) -> serenity::CreateMessage {
    let username = username.into();
    let heroes = hero_assignments.get(&username).cloned().unwrap_or_default();
    let tag_line = tags
        .get(&username)
        .map(|tag| format!("Role: **{tag}**\n"))
        .unwrap_or_default();
    info!(
        "Hero assignment {}: {}",
        username,
        utils::join(&heroes, ", ")
    );
    let heroes_message = format!(
        "{tag_line}Heroes: **{}**\n",
        random_heroes_str_with_links(heroes)
    );
    serenity::CreateMessage::new()
        .content(heroes_message)
        // .flags(serenity::MessageFlags::SUPPRESS_EMBEDS)
}

fn random_heroes_str_with_links(heroes: impl IntoIterator<Item = impl AsRef<Hero>>) -> String {
    let mut heroes: Vec<_> = heroes.into_iter().map(|h| h.as_ref().to_string()).collect();
    heroes.sort();
    if heroes.is_empty() {
        String::from("No heroes.")
    } else {
        let links: Vec<_> = heroes.iter().map(|h| hero_build_link(h)).collect();
        links.join(", ")
    }
}

/// Markdown masked link to the hero's dota2protracker builds page. URL is wrapped in `<>` to
/// suppress the link preview.
fn hero_build_link(hero: &str) -> String {
    let encoded = hero
        .replace('%', "%25")
        .replace(' ', "%20")
        .replace('\'', "%27")
        .replace('(', "%28")
        .replace(')', "%29");
    format!("[{hero}](<https://dota2protracker.com/hero/{encoded}?section=builds>)")
}

fn random_heroes_str(heroes: impl IntoIterator<Item = impl AsRef<Hero>>) -> String {
    let mut heroes: Vec<_> = heroes.into_iter().map(|h| h.as_ref().to_string()).collect();
    heroes.sort();
    if heroes.is_empty() {
        String::from("No heroes.")
    } else {
        heroes.join(",\n")
    }
}

fn random_heroes_str_oneline(heroes: impl IntoIterator<Item = impl AsRef<Hero>>) -> String {
    let mut heroes: Vec<_> = heroes.into_iter().map(|h| h.as_ref().to_string()).collect();
    heroes.sort();
    if heroes.is_empty() {
        String::from("No heroes.")
    } else {
        heroes.join(", ")
    }
}

fn heroes_str(heroes: impl IntoIterator<Item = impl AsRef<Hero>>) -> Option<String> {
    let mut heroes: Vec<String> = heroes.into_iter().map(|h| h.as_ref().to_string()).collect();
    heroes.sort_unstable();
    if heroes.is_empty() {
        return None;
    }
    Some(format!(
        "{}\n\nThat's {} total heroes",
        heroes.join(",\n"),
        heroes.len()
    ))
}

pub fn ephemeral_reroll_reply(reroll: &RerollResult) -> poise::CreateReply {
    match reroll {
        RerollResult::NewPool(new_pool) if new_pool.is_empty() => {
            poise::CreateReply::default().content("Sorry bud. No more heroes in the pool.")
        }
        RerollResult::NewPool(new_pool) => poise::CreateReply::default().content(format!(
            "**Your random heroes for this match are**\n{}",
            random_heroes_str(new_pool)
        )),
        RerollResult::NoRerollUntil(timestamp) => poise::CreateReply::default().content(format!(
            "Sorry bud. Reroll limit exhausted. You can reroll again at {}",
            timestamp.format("%Y-%m-%d %H:%M:%S")
        )),
    }
    .ephemeral(true)
}

pub fn ephemeral_dota_bot_state_reply(state: &DotaBotState) -> poise::CreateReply {
    poise::CreateReply::default()
        .content(format!("{state:#?}"))
        .ephemeral(true)
}

pub fn ephemeral_dota_bot_hero_list_reply<'a>(
    heroes: impl IntoIterator<Item = &'a Hero>,
) -> poise::CreateReply {
    poise::CreateReply::default()
        .content(heroes_str(heroes).unwrap_or_else(|| String::from("No heroes matching criteria.")))
        .ephemeral(true)
}

pub fn ephemeral_reply(content: impl Into<String>) -> poise::CreateReply {
    poise::CreateReply::default()
        .content(content)
        .ephemeral(true)
}

pub fn reroll_broadcast_message(
    username: &DiscordUsername,
    new_pool: &[Hero],
) -> serenity::CreateMessage {
    let heroes = random_heroes_str_oneline(new_pool);
    serenity::CreateMessage::new().content(format!(
        "**{username}** rerolled their hero pool: **{heroes}**"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hero_build_link_encodes_name() {
        assert_eq!(
            hero_build_link("Drow Ranger"),
            "[Drow Ranger](<https://dota2protracker.com/hero/Drow%20Ranger?section=builds>)"
        );
        assert_eq!(
            hero_build_link("Nature's Prophet"),
            "[Nature's Prophet](<https://dota2protracker.com/hero/Nature%27s%20Prophet?section=builds>)"
        );
    }
}
