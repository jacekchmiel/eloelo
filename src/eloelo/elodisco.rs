use crate::eloelo::config::Config;
use crate::eloelo::elodisco::async_elodisco::EloDisco;
use crate::eloelo::elodisco::bot_state::BotState;
use crate::eloelo::elodisco::dota_bot::Hero;
use crate::eloelo::message_bus::MessageBus;
use crate::utils::print_err;
use anyhow::{Context as _, Error, Result};
use eloelo_model::player::DiscordUsername;
use futures_util::lock::Mutex;
use futures_util::StreamExt as _;
use itertools::Itertools;
use log::{info, warn};
use poise::serenity_prelude as serenity;
use std::sync::Arc;

pub(crate) mod async_elodisco;
pub(crate) mod bot_state;
pub(crate) mod dota_bot;
pub(crate) mod hero_assignment_strategy;
pub(crate) mod messages;
pub(crate) mod notification_bot;
mod utils;

type SharedEloDisco = Arc<Mutex<EloDisco>>;
type Context<'a> = poise::Context<'a, SharedEloDisco, Error>;

/// Rerolls assigned heroes
#[poise::command(slash_command)]
async fn reroll(ctx: Context<'_>) -> Result<()> {
    let mut elodisco = ctx.data().lock().await;
    let username = DiscordUsername::from(ctx.author().name.as_str());
    let new_pool = elodisco.dota_bot_mut().reroll(&username)?;
    ctx.send(messages::personal_reroll_message(&new_pool))
        .await?;
    //TODO: make reroll visible on main channel
    Ok(())
}

/// Displays configuration status
#[poise::command(slash_command)]
async fn debug(ctx: Context<'_>) -> Result<()> {
    let mut elodisco = ctx.data().lock().await;
    let username = DiscordUsername::from(ctx.author().name.as_str());
    let state = elodisco.dota_bot_mut().get_user_state(&username);
    ctx.send(messages::personal_dota_bot_state_message(&state))
        .await?;
    Ok(())
}

/// Displays debug information
#[poise::command(slash_command)]
async fn status(ctx: Context<'_>) -> Result<()> {
    let mut elodisco = ctx.data().lock().await;
    let username = DiscordUsername::from(ctx.author().name.as_str());
    let state = elodisco.dota_bot_mut().get_user_state(&username);
    ctx.send(messages::personal_dota_bot_state_message(&state))
        .await?;
    Ok(())
}

/// Displays information
#[poise::command(
    slash_command,
    subcommands("allowlist", "banlist", "available", "unavailable", "all"),
    subcommand_required
)]
async fn show(ctx: Context<'_>) -> Result<()> {
    let mut elodisco = ctx.data().lock().await;
    let username = DiscordUsername::from(ctx.author().name.as_str());
    let state = elodisco.dota_bot_mut().get_user_state(&username);
    ctx.send(messages::personal_dota_bot_hero_list_message(
        &state.banned_heroes,
    ))
    .await?;
    Ok(())
}

/// Displays banned heroes
#[poise::command(slash_command)]
async fn banlist(ctx: Context<'_>) -> Result<()> {
    let mut elodisco = ctx.data().lock().await;
    let username = DiscordUsername::from(ctx.author().name.as_str());
    let state = elodisco.dota_bot_mut().get_user_state(&username);
    ctx.send(messages::personal_dota_bot_hero_list_message(
        &state.banned_heroes,
    ))
    .await?;
    Ok(())
}

/// Displays allowed heroes
#[poise::command(slash_command)]
async fn allowlist(ctx: Context<'_>) -> Result<()> {
    let mut elodisco = ctx.data().lock().await;
    let username = DiscordUsername::from(ctx.author().name.as_str());
    let state = elodisco.dota_bot_mut().get_user_state(&username);
    ctx.send(messages::personal_dota_bot_hero_list_message(
        &state.allowed_heroes,
    ))
    .await?;
    Ok(())
}

/// Displays hero pool
#[poise::command(slash_command)]
async fn available(ctx: Context<'_>) -> Result<()> {
    let mut elodisco = ctx.data().lock().await;
    let username = DiscordUsername::from(ctx.author().name.as_str());
    let pool = elodisco.dota_bot_mut().user_hero_pool(&username);
    ctx.send(messages::personal_dota_bot_hero_list_message(&pool))
        .await?;
    Ok(())
}

/// Displays unavailable heroes, regardless of allowlist or banlist being used.
#[poise::command(slash_command)]
async fn unavailable(ctx: Context<'_>) -> Result<()> {
    let mut elodisco = ctx.data().lock().await;
    let username = DiscordUsername::from(ctx.author().name.as_str());
    let unavailable: Vec<Hero> = Hero::all()
        .difference(
            &elodisco
                .dota_bot_mut()
                .user_hero_pool(&username)
                .into_iter()
                .collect(),
        )
        .cloned()
        .collect();
    ctx.send(messages::personal_dota_bot_hero_list_message(&unavailable))
        .await?;
    Ok(())
}

/// Displays all DotA 2 heroes
#[poise::command(slash_command)]
async fn all(ctx: Context<'_>) -> Result<()> {
    ctx.send(messages::personal_dota_bot_hero_list_message(
        &Hero::all_alphabetical(),
    ))
    .await?;
    Ok(())
}

/// Hero Pool operations
#[poise::command(
    slash_command,
    subcommands("ban", "allow", "unban", "unallow", "clear"),
    subcommand_required
)]
async fn pool(ctx: Context<'_>) -> Result<()> {
    let mut elodisco = ctx.data().lock().await;
    let username = DiscordUsername::from(ctx.author().name.as_str());
    let state = elodisco.dota_bot_mut().get_user_state(&username);
    ctx.send(messages::personal_dota_bot_hero_list_message(
        &state.banned_heroes,
    ))
    .await?;
    Ok(())
}

/// Ban hero
#[poise::command(slash_command)]
async fn ban(
    ctx: Context<'_>,
    #[description = "Hero to ban"]
    #[autocomplete = "ban_autocomplete"]
    hero: Vec<String>,
) -> Result<()> {
    let mut elodisco = ctx.data().lock().await;
    let username = DiscordUsername::from(ctx.author().name.as_str());
    for hero in hero {
        let hero = Hero::try_from(hero)?;
        elodisco.dota_bot_mut().ban_hero(&username, &hero);
        elodisco.store_bot_state_if_changed().await;
        ctx.say(format!("{} is now banned", hero)).await?;
    }
    Ok(())
}

/// Unban hero
#[poise::command(slash_command)]
async fn unban(
    ctx: Context<'_>,
    #[description = "Hero to unban"]
    #[autocomplete = "unban_autocomplete"]
    hero: Vec<String>,
) -> Result<()> {
    let mut elodisco = ctx.data().lock().await;
    let username = DiscordUsername::from(ctx.author().name.as_str());
    for hero in hero {
        let hero = Hero::try_from(hero)?;
        elodisco.dota_bot_mut().unban_hero(&username, &hero);
        elodisco.store_bot_state_if_changed().await;
        ctx.say(format!("{} is not banned anymore", hero)).await?;
    }
    Ok(())
}

/// Allow hero
#[poise::command(slash_command)]
async fn allow(
    ctx: Context<'_>,
    #[description = "Hero to allow"]
    #[autocomplete = "allow_autocomplete"]
    hero: Vec<String>,
) -> Result<()> {
    let mut elodisco = ctx.data().lock().await;
    let username = DiscordUsername::from(ctx.author().name.as_str());
    for hero in hero {
        let hero = Hero::try_from(hero)?;
        elodisco.dota_bot_mut().allow_hero(&username, &hero);
        elodisco.store_bot_state_if_changed().await;
        ctx.say(format!("{} is now allowed", hero)).await?;
    }
    Ok(())
}

/// Unallow hero
#[poise::command(slash_command)]
async fn unallow(
    ctx: Context<'_>,
    #[description = "Hero to unallow"]
    #[autocomplete = "unallow_autocomplete"]
    hero: Vec<String>,
) -> Result<()> {
    let mut elodisco = ctx.data().lock().await;
    let username = DiscordUsername::from(ctx.author().name.as_str());
    for hero in hero {
        let hero = Hero::try_from(hero)?;
        elodisco.dota_bot_mut().unallow_hero(&username, &hero);
        elodisco.store_bot_state_if_changed().await;
        ctx.say(format!("{} is not allowed allowed anymore", hero))
            .await?;
    }
    Ok(())
}

/// Clear a list
#[poise::command(
    slash_command,
    subcommands("clear_allowlist", "clear_banlist"),
    subcommand_required
)]
async fn clear(_: Context<'_>) -> Result<()> {
    Ok(())
}

/// Remove all entries from your hero allowlist
#[poise::command(slash_command, rename = "allowlist")]
async fn clear_allowlist(ctx: Context<'_>) -> Result<()> {
    let mut elodisco = ctx.data().lock().await;
    let username = DiscordUsername::from(ctx.author().name.as_str());
    elodisco.dota_bot_mut().clear_allowlist(&username);
    elodisco.store_bot_state_if_changed().await;
    ctx.say(format!("Allowlist cleared")).await?;
    Ok(())
}

/// Remove all entries from your hero banlist
#[poise::command(slash_command, rename = "banlist")]
async fn clear_banlist(ctx: Context<'_>) -> Result<()> {
    let mut elodisco = ctx.data().lock().await;
    let username = DiscordUsername::from(ctx.author().name.as_str());
    elodisco.dota_bot_mut().clear_banlist(&username);
    elodisco.store_bot_state_if_changed().await;
    ctx.say(format!("Banlist cleared")).await?;
    Ok(())
}

fn heroes_to_autocomplete<'a>(
    heroes: impl Iterator<Item = &'a Hero>,
    partial: &'a str,
) -> Vec<serenity::AutocompleteChoice> {
    let mut heroes: Vec<_> = heroes
        .filter(|h| {
            h.as_str()
                .to_lowercase()
                .starts_with(&partial.to_lowercase())
        })
        .collect();
    heroes.sort();
    heroes
        .into_iter()
        .map(|h| serenity::AutocompleteChoice::new(h.as_str(), h.as_str()))
        .collect()
}

async fn ban_autocomplete<'a>(
    ctx: Context<'_>,
    partial: &'a str,
) -> impl Iterator<Item = serenity::AutocompleteChoice> {
    let mut elodisco = ctx.data().lock().await;
    let username = DiscordUsername::from(ctx.author().name.as_str());
    let state = elodisco.dota_bot_mut().get_user_state(&username);
    heroes_to_autocomplete(Hero::all().difference(&state.banned_heroes), partial).into_iter()
}

async fn unban_autocomplete<'a>(
    ctx: Context<'_>,
    partial: &'a str,
) -> impl Iterator<Item = serenity::AutocompleteChoice> {
    let mut elodisco = ctx.data().lock().await;
    let username = DiscordUsername::from(ctx.author().name.as_str());
    let state = elodisco.dota_bot_mut().get_user_state(&username);
    heroes_to_autocomplete(state.banned_heroes.iter(), partial).into_iter()
}

async fn allow_autocomplete<'a>(
    ctx: Context<'_>,
    partial: &'a str,
) -> impl Iterator<Item = serenity::AutocompleteChoice> {
    let mut elodisco = ctx.data().lock().await;
    let username = DiscordUsername::from(ctx.author().name.as_str());
    let state = elodisco.dota_bot_mut().get_user_state(&username);
    heroes_to_autocomplete(Hero::all().difference(&state.allowed_heroes), partial).into_iter()
}

async fn unallow_autocomplete<'a>(
    ctx: Context<'_>,
    partial: &'a str,
) -> impl Iterator<Item = serenity::AutocompleteChoice> {
    let mut elodisco = ctx.data().lock().await;
    let username = DiscordUsername::from(ctx.author().name.as_str());
    let state = elodisco.dota_bot_mut().get_user_state(&username);
    heroes_to_autocomplete(state.allowed_heroes.iter(), partial).into_iter()
}

pub async fn run(config: Config, bot_state: BotState, message_bus: MessageBus) {
    if config.test_mode {
        warn!("Discord running in test mode");
    }
    info!(
        "Announcement channel name: {}",
        config.effective_discord_channel_name()
    );
    if config.test_mode {
        warn!(
            "Players allowed to be notified: {}",
            config.discord_test_mode_players.iter().join(", ")
        );
    }

    tokio::spawn(async move {
        let intents = serenity::GatewayIntents::GUILD_MESSAGES
            | serenity::GatewayIntents::DIRECT_MESSAGES
            | serenity::GatewayIntents::MESSAGE_CONTENT;
        let token = config.discord_bot_token.clone();

        let framework = poise::Framework::builder()
            .options(poise::FrameworkOptions {
                commands: vec![reroll(), debug(), show(), pool()],
                ..Default::default()
            })
            .setup(|ctx, _ready, framework| {
                Box::pin(async move {
                    info!("EloDisco is connected!");
                    poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                    let elodisco =
                        EloDisco::initialize(bot_state, message_bus.clone(), config.clone(), ctx)
                            .await?;
                    let elodisco = Arc::new(Mutex::new(elodisco));
                    spawn_message_handler(Arc::clone(&elodisco), message_bus.clone());
                    Ok(elodisco)
                })
            })
            .build();

        let mut client = serenity::ClientBuilder::new(token, intents)
            .framework(framework)
            .await?;

        info!("Starting Serenity client");
        client.start().await.context("Serenity client failed")
    });
}

fn spawn_message_handler(elodisco: SharedEloDisco, message_bus: MessageBus) {
    tokio::spawn(async move {
        message_bus
            .subscribe()
            .stream()
            .for_each(move |message| {
                let elodisco = elodisco.clone();
                async move {
                    match message {
                        Ok(message) => elodisco.lock().await.on_message(message).await,
                        Err(e) => print_err(&e),
                    }
                }
            })
            .await;
        info!("Elodisco: Message stream finished.");
    });
}
