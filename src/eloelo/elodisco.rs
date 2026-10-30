use crate::eloelo::elodisco::async_elodisco::EloDisco;
use crate::eloelo::elodisco::bot_state::BotState;
use crate::eloelo::elodisco::config::EloDiscoConfig;
use crate::eloelo::elodisco::dota_bot::Hero;
use crate::eloelo::git_storage::GitStorage;
use crate::eloelo::message_bus::MessageBus;
use crate::utils::{print_err, ResultExt as _};
use anyhow::{bail, Context as _, Error, Result};
use eloelo_model::player::DiscordUsername;
use futures_util::lock::Mutex;
use futures_util::StreamExt as _;
use itertools::Itertools;
use log::{debug, error, info};
use poise::serenity_prelude as serenity;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::oneshot;

pub(crate) mod async_elodisco;
pub mod bot_state;
pub mod config;
pub(crate) mod dota_bot;
pub(crate) mod hero_assignment_strategy;
pub(crate) mod messages;
pub(crate) mod notification_bot;
mod utils;

const STORAGE_PUSH_INTERVAL: Duration = Duration::from_secs(60 * 60);

type SharedEloDisco = Arc<Mutex<EloDisco>>;
type Context<'a> = poise::Context<'a, SharedEloDisco, Error>;

/// Rerolls assigned heroes
#[poise::command(slash_command)]
async fn reroll(ctx: Context<'_>) -> Result<()> {
    let mut elodisco = ctx.data().lock().await;
    let username = DiscordUsername::from(ctx.author().name.as_str());
    let reroll = elodisco.handle_user_reroll(&username).await?;
    ctx.send(messages::ephemeral_reroll_reply(&reroll)).await?;
    Ok(())
}

/// Displays configuration status
#[poise::command(slash_command)]
async fn debug(ctx: Context<'_>) -> Result<()> {
    let mut elodisco = ctx.data().lock().await;
    let username = DiscordUsername::from(ctx.author().name.as_str());
    let state = elodisco.dota_bot_mut().get_user_state(&username);
    ctx.send(messages::ephemeral_dota_bot_state_reply(&state))
        .await?;
    Ok(())
}

/// Displays debug information
#[poise::command(slash_command)]
async fn status(ctx: Context<'_>) -> Result<()> {
    let mut elodisco = ctx.data().lock().await;
    let username = DiscordUsername::from(ctx.author().name.as_str());
    let state = elodisco.dota_bot_mut().get_user_state(&username);
    ctx.send(messages::ephemeral_dota_bot_state_reply(&state))
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
    ctx.send(messages::ephemeral_dota_bot_hero_list_reply(
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
    ctx.send(messages::ephemeral_dota_bot_hero_list_reply(
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
    ctx.send(messages::ephemeral_dota_bot_hero_list_reply(
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
    ctx.send(messages::ephemeral_dota_bot_hero_list_reply(&pool))
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
    ctx.send(messages::ephemeral_dota_bot_hero_list_reply(&unavailable))
        .await?;
    Ok(())
}

/// Displays all DotA 2 heroes
#[poise::command(slash_command)]
async fn all(ctx: Context<'_>) -> Result<()> {
    ctx.send(messages::ephemeral_dota_bot_hero_list_reply(
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
    ctx.send(messages::ephemeral_dota_bot_hero_list_reply(
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

        ctx.send(messages::ephemeral_reply(format!("{} is now banned", hero)))
            .await?;
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

        ctx.send(messages::ephemeral_reply(format!(
            "{} is not banned anymore",
            hero
        )))
        .await?;
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

        ctx.send(messages::ephemeral_reply(format!(
            "{} is now allowed",
            hero
        )))
        .await?;
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
        ctx.send(messages::ephemeral_reply(format!(
            "{} is not allowed anymore",
            hero
        )))
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
        .inspect(|h| debug!("Autocomplete: {}", h.as_str()))
        .map(|h| serenity::AutocompleteChoice::new(h.as_str(), h.as_str()))
        .take(100)
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

/// Show this menu
#[poise::command(prefix_command, track_edits, slash_command)]
pub async fn help(
    ctx: Context<'_>,
    #[description = "Specific command to show help about"] command: Option<String>,
) -> Result<(), Error> {
    //     let config = poise::builtins::HelpConfiguration {
    //         extra_text_at_bottom: "\
    // Type ?help command for more info on a command.
    // You can edit your message to the bot and the bot will edit its response.",
    //         ..Default::default()
    //     };
    poise::builtins::help(ctx, command.as_deref(), Default::default()).await?;
    Ok(())
}

/// Starts the Discord bot in the background. The returned future completes once the bot handles
/// messages from `message_bus`, or fails if the bot doesn't start. Messages sent before that are
/// lost. The bot runs whether the future is awaited or not. Its data in `storage` is pushed every
/// `STORAGE_PUSH_INTERVAL`; the caller pushes it on exit with `push_storage`.
pub fn run(
    config: EloDiscoConfig,
    token: String,
    bot_state: BotState,
    storage: GitStorage,
    message_bus: MessageBus,
) -> impl Future<Output = Result<()>> {
    info!(
        "Discord bot starting with test mode: {}. A remote bot follows the connected eloelo.",
        config.test_mode
    );
    info!(
        "Announcement channel name: {}, in test mode: {}",
        config.discord_channel_name, config.discord_test_channel_name
    );
    info!(
        "Players allowed to be notified in test mode: {}",
        config.discord_test_mode_players.iter().join(", ")
    );

    tokio::spawn(push_storage_periodically(storage.clone()));

    let (bot_ready, bot_ready_receiver) = oneshot::channel();
    let bot = tokio::spawn(async move {
        let intents = serenity::GatewayIntents::GUILD_MESSAGES
            | serenity::GatewayIntents::DIRECT_MESSAGES
            | serenity::GatewayIntents::MESSAGE_CONTENT;

        let framework = poise::Framework::builder()
            .options(poise::FrameworkOptions {
                commands: vec![reroll(), debug(), show(), pool(), help()],
                ..Default::default()
            })
            .setup(|ctx, _ready, framework| {
                Box::pin(async move {
                    info!("EloDisco is connected!");
                    poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                    let elodisco =
                        EloDisco::initialize(bot_state, storage, message_bus.clone(), config, ctx)
                            .await?;
                    let elodisco = Arc::new(Mutex::new(elodisco));
                    spawn_message_handler(Arc::clone(&elodisco), message_bus.clone());
                    let _ = bot_ready.send(());
                    Ok(elodisco)
                })
            })
            .build();

        let client = serenity::ClientBuilder::new(token, intents).framework(framework);
        let started = async {
            info!("Starting Serenity client");
            client.await?.start().await
        };
        started.await.context("Serenity client failed").print_err();
    });
    async move {
        // The sender is dropped when setup fails, but not when the client fails before setup.
        tokio::select! {
            ready = bot_ready_receiver => ready.context("Discord bot setup failed"),
            _ = bot => bail!("Discord bot stopped"),
        }
    }
}

async fn push_storage_periodically(storage: GitStorage) {
    let mut interval = tokio::time::interval(STORAGE_PUSH_INTERVAL);
    // The first tick is immediate, and opening the storage has just pushed.
    interval.tick().await;
    loop {
        interval.tick().await;
        push_storage(&storage).await;
    }
}

/// Commits and pushes the bot's config and state.
pub async fn push_storage(storage: &GitStorage) {
    let storage = storage.clone();
    let pushed = tokio::task::spawn_blocking(move || storage.push("Update Discord bot data")).await;
    if let Err(e) = pushed.map_err(Error::from).and_then(|r| r) {
        error!("Failed to push Discord bot data: {e:#}");
    }
}

fn spawn_message_handler(elodisco: SharedEloDisco, message_bus: MessageBus) {
    // Subscribed before returning, so messages sent once the bot is reported ready aren't missed.
    let messages = message_bus.subscribe();
    tokio::spawn(async move {
        messages
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
