use std::ffi::OsStr;
use std::fs::{self, File};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use eloelo_model::player::PlayersConfig;
use itertools::Itertools;
use log::{debug, info, warn};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use super::config::Config;
use super::elodisco::bot_state::BotState;
use super::elodisco::config::EloDiscoConfig;
use super::git_storage::GitStorage;
use super::ui_state::State;
use eloelo_model::history::{History, HistoryEntry};
use eloelo_model::GameId;

const HISTORY_SUFFIX: &str = ".history.json";
const HISTORY_DIR: &str = "history";
const ELOELO_DIR: &str = "eloelo";
const CONFIG_FILE: &str = "config.yaml";
const PLAYERS_FILE: &str = "players.yaml";
const OPTIONS_FILE: &str = "options.yaml";
const ELODISCO_DIR: &str = "elodisco";
const ELODISCO_CONFIG_FILE: &str = "config.yaml";
const BOT_STATE_FILE: &str = "discord_bot_state.yaml";
pub const DISCORD_BOT_TOKEN_FILE: &str = "discord_bot_token";
pub const BRIDGE_SECRET_FILE: &str = "bridge_secret";

fn state_file_path() -> PathBuf {
    data_dir().join("state.yaml")
}

pub fn data_dir() -> PathBuf {
    let project_dirs = directories::ProjectDirs::from("com", "eloelo", "eloelo")
        .expect("Cannot retrieve project dirs");
    project_dirs.data_dir().to_owned()
}

pub fn load_state() -> Result<Option<State>> {
    info!("State file: {}", state_file_path().to_string_lossy());
    if !state_file_path().exists() {
        return Ok(None);
    }
    let state_file = File::open(state_file_path())?;
    let state = serde_yaml::from_reader(state_file)?;
    Ok(Some(state))
}

pub fn store_state(state: &State) -> Result<()> {
    ensure_dir_created(&state_file_path())?;
    store_file_with_backup(&state_file_path(), state)?;
    Ok(())
}

/// The Discord bot's storage: its config and state.
pub fn elodisco_dir() -> PathBuf {
    data_dir().join(ELODISCO_DIR)
}

pub fn load_elodisco_config(storage: &GitStorage) -> Result<EloDiscoConfig> {
    if let Some(config) = storage.load(ELODISCO_CONFIG_FILE)? {
        return Ok(config);
    }
    info!("Discord bot config file does not exist, creating.");
    let config = EloDiscoConfig::default();
    storage.store(ELODISCO_CONFIG_FILE, &config, "Create Discord bot config")?;
    Ok(config)
}

pub fn load_bot_state(storage: &GitStorage) -> Result<BotState> {
    let mut state: BotState = storage.load(BOT_STATE_FILE)?.unwrap_or_default();
    state.validate();
    Ok(state)
}

pub fn store_bot_state(storage: &GitStorage, state: &BotState) -> Result<()> {
    debug!("Storing bot state");
    storage.store(BOT_STATE_FILE, state, "Update Discord bot state")
}

/// Reads a secret from its own file in the data dir, which is never backed up.
pub fn load_secret(file: &str) -> Result<String> {
    let path = data_dir().join(file);
    let secret = fs::read_to_string(&path)
        .with_context(|| format!("Cannot read secret from {}", path.to_string_lossy()))?;
    let secret = secret.trim();
    if secret.is_empty() {
        bail!("Secret file {} is empty", path.to_string_lossy());
    }
    Ok(secret.to_string())
}

/// eloelo's storage: its config, players and options.
pub fn eloelo_dir() -> PathBuf {
    data_dir().join(ELOELO_DIR)
}

pub fn load_config(storage: &GitStorage) -> Result<Config> {
    if let Some(config) = storage.load(CONFIG_FILE)? {
        return Ok(config);
    }
    info!("Config file does not exist, creating.");
    let config = Config::default();
    storage.store(CONFIG_FILE, &config, "Create config")?;
    Ok(config)
}

pub fn load_players(storage: &GitStorage) -> Result<PlayersConfig> {
    let config = match storage.load(PLAYERS_FILE)? {
        Some(config) => config,
        None => {
            info!("Players file does not exist, creating.");
            let config = PlayersConfig::example();
            storage.store(PLAYERS_FILE, &config, "Create players")?;
            config
        }
    };
    let player_ids: String = config.players.iter().map(|p| &p.id).join(", ");
    let n = config.players.len();

    if n == 0 {
        warn!("Loaded {n} players");
    } else {
        info!("Loaded {n} players: {player_ids}");
    }
    Ok(config)
}

pub fn store_players(storage: &GitStorage, players: &PlayersConfig) -> Result<()> {
    storage.store(PLAYERS_FILE, players, "Update players")
}

pub fn load_options<T: DeserializeOwned + Default>(storage: &GitStorage) -> Result<T> {
    Ok(storage.load(OPTIONS_FILE)?.unwrap_or_default())
}

pub fn store_options<T: Serialize>(storage: &GitStorage, v: &T) -> Result<()> {
    storage.store(OPTIONS_FILE, v, "Update options")
}

#[derive(Serialize, Deserialize, PartialEq)]
struct HistorySerializeWrapper {
    game: GameId,
    entries: Vec<HistoryEntry>,
}

/// Appends `entry` to the history of `game` and commits it with `change` as the message.
pub fn append_history_entry(
    storage: &GitStorage,
    game: &GameId,
    entry: &HistoryEntry,
    change: &str,
) -> Result<()> {
    let file = history_file_name(game);
    let mut entries = storage
        .load::<HistorySerializeWrapper>(&file)?
        .map(|history| history.entries)
        .unwrap_or_default();
    entries.push(entry.clone());
    storage.store(
        &file,
        &HistorySerializeWrapper {
            game: game.clone(),
            entries,
        },
        change,
    )
}

pub fn history_dir() -> PathBuf {
    data_dir().join(HISTORY_DIR)
}

pub fn load_history() -> Result<History> {
    let mut out = History::default();
    info!("History Dir: {}", history_dir().to_string_lossy());
    for dir_entry in fs::read_dir(history_dir())? {
        let dir_entry = dir_entry?;
        if is_regular_history_file(&dir_entry.path()) {
            info!("History File: {}", dir_entry.path().to_string_lossy());
            let history = load_history_file(&dir_entry.path())?;
            out.entries
                .entry(history.game)
                .or_default()
                .extend(history.entries);
        }
    }
    Ok(out)
}

fn load_history_file(path: &Path) -> Result<HistorySerializeWrapper> {
    let history_file = File::open(path)?;
    let history: HistorySerializeWrapper = serde_json::from_reader(history_file)?;
    Ok(history)
}

fn is_regular_history_file(path: &Path) -> bool {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .ends_with(HISTORY_SUFFIX)
}

fn history_file_name(game: &GameId) -> String {
    let safe_game_id = game.as_str().replace(" ", "_").replace(":", "_");
    format!("{}{}", safe_game_id, HISTORY_SUFFIX)
}

fn store_file_with_backup<T>(path: &Path, data: &T) -> Result<()>
where
    T: Serialize + DeserializeOwned + PartialEq,
{
    let orig = if path.is_file() {
        let orig_file = File::open(path)?;
        let orig_content: T = serde_yaml::from_reader(orig_file)?;
        Some(orig_content)
    } else {
        None
    };
    if orig.as_ref() == Some(data) {
        // No need to change anything
        return Ok(());
    }
    // We are about to overwrite this file: create backup
    if path.is_file() {
        let orig_filename = path
            .file_name()
            .map(OsStr::to_string_lossy)
            .unwrap_or_default();
        let backup_path = path.with_file_name(format!("{}{}", orig_filename, ".bak"));
        std::fs::rename(path, backup_path)?;
    }
    let out_file = File::create(path)?;
    serde_yaml::to_writer(out_file, data)?;
    Ok(())
}

fn ensure_dir_created(path: &Path) -> Result<()> {
    let dir = path.parent().expect("Parent directory");
    if !dir.exists() {
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("Cannot create {}", &dir.to_string_lossy()))?;
    }
    Ok(())
}
