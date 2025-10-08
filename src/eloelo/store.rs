use std::ffi::OsStr;
use std::fs::{self, File};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use eloelo_model::player::PlayersConfig;
use itertools::Itertools;
use log::{debug, info, warn};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use super::config::Config;
use super::elodisco::bot_state::BotState;
use super::ui_state::State;
use eloelo_model::history::{History, HistoryEntry};
use eloelo_model::GameId;

const HISTORY_SUFFIX: &str = ".history.json";
const HISTORY_GIT_DIR: &str = "history_git";

fn state_file_path() -> PathBuf {
    data_dir().join("state.yaml")
}

fn bot_state_file_path() -> PathBuf {
    data_dir().join("discord_bot_state.yaml")
}

fn config_file_path() -> PathBuf {
    data_dir().join("config.yaml")
}

fn players_file_path() -> PathBuf {
    data_dir().join("players.yaml")
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

pub fn load_bot_state() -> Result<BotState> {
    let path = bot_state_file_path();
    info!("Discord Bot State file: {}", path.to_string_lossy());
    if !path.exists() {
        store_bot_state(&Default::default())?;
    }
    let state_file = File::open(path)?;
    Ok(serde_yaml::from_reader(state_file)?)
}

pub fn store_bot_state(state: &BotState) -> Result<()> {
    debug!("Storing bot state {:?}", state);
    ensure_dir_created(&bot_state_file_path())?;
    store_file_with_backup(&bot_state_file_path(), state)?;
    Ok(())
}

pub fn load_config() -> Result<Config> {
    info!("Config file: {}", config_file_path().to_string_lossy());
    if !config_file_path().exists() {
        info!("Config file does not exist, creating.");
        store_default_config()?;
    }
    let config_file = File::open(config_file_path())?;
    Ok(serde_yaml::from_reader(config_file)?)
}

pub fn load_players() -> Result<PlayersConfig> {
    info!("Players file: {}", players_file_path().to_string_lossy());
    if !players_file_path().exists() {
        info!("Players file does not exist, creating.");
        store_default_players_config()?;
    }
    let config_file = File::open(players_file_path())?;
    let config: PlayersConfig = serde_yaml::from_reader(config_file)?;
    let player_ids: String = config.players.iter().map(|p| &p.id).join(", ");
    let n = config.players.len();

    if n == 0 {
        warn!("Loaded {n} players");
    } else {
        info!("Loaded {n} players: {player_ids}");
    }
    Ok(config)
}

pub fn store_default_config() -> Result<()> {
    ensure_dir_created(&config_file_path())?;
    let config_file = File::create(&config_file_path())?;
    Ok(serde_yaml::to_writer(config_file, &Config::default())?)
}

pub fn store_default_players_config() -> Result<()> {
    ensure_dir_created(&players_file_path())?;
    let config_file = File::create(&players_file_path())?;
    Ok(serde_yaml::to_writer(
        config_file,
        &PlayersConfig::example(),
    )?)
}

pub fn store_players(players: PlayersConfig) -> Result<()> {
    ensure_dir_created(&players_file_path())?;
    let config_file = File::create(&players_file_path())?;
    Ok(serde_yaml::to_writer(config_file, &players)?)
}

#[derive(Serialize, Deserialize, PartialEq)]
struct HistorySerializeWrapper {
    game: GameId,
    entries: Vec<HistoryEntry>,
}

pub fn append_history_entry(game: &GameId, entry: &HistoryEntry) -> Result<()> {
    let mut entries = if history_path(game).is_file() {
        load_history_file(&history_path(game))?.entries
    } else {
        vec![]
    };
    entries.push(entry.clone());

    let out_file = File::create(&history_path(game))?;
    serde_json::to_writer_pretty(
        out_file,
        &HistorySerializeWrapper {
            game: game.clone(),
            entries,
        },
    )?;
    Ok(())
}

pub fn history_dir() -> PathBuf {
    data_dir().join(HISTORY_GIT_DIR)
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

pub fn history_path(game: &GameId) -> PathBuf {
    let safe_game_id = game.as_str().replace(" ", "_").replace(":", "_");
    let filename = format!("{}{}", safe_game_id, HISTORY_SUFFIX);
    history_dir().join(filename)
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
