use std::ffi::OsStr;
use std::fs::{self, File};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::DateTime;
use log::{debug, info};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use super::config::{Config, PlayerConfig};
use super::elodisco::bot_state::BotState;
use super::ui_state::State;
use eloelo_model::history::{History, HistoryEntry, LegacyHistoryEntry};
use eloelo_model::player::PlayerDb;
use eloelo_model::GameId;

fn state_file_path() -> PathBuf {
    data_dir().join("state.yaml")
}

fn bot_state_file_path() -> PathBuf {
    data_dir().join("discord_bot_state.yaml")
}

fn config_file_path() -> PathBuf {
    data_dir().join("config.yaml")
}

pub fn data_dir() -> PathBuf {
    let project_dirs = directories::ProjectDirs::from("com", "eloelo", "eloelo")
        .expect("Cannot retrieve project dirs");
    project_dirs.data_dir().to_owned()
}

pub fn load_state(config: &Config) -> Result<State> {
    info!("Store: State file: {}", state_file_path().to_string_lossy());
    if !state_file_path().exists() {
        store_state(&State::new(config.default_game().clone()))?;
    }
    let state_file = File::open(state_file_path())?;
    Ok(serde_yaml::from_reader(state_file)?)
}

pub fn store_state(state: &State) -> Result<()> {
    ensure_dir_created(&state_file_path())?;
    store_file_with_backup(&state_file_path(), state)?;
    Ok(())
}

pub fn load_bot_state() -> Result<BotState> {
    let path = bot_state_file_path();
    info!("Store: Discord Bot State file: {}", path.to_string_lossy());
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
    info!(
        "Store: Config file: {}",
        config_file_path().to_string_lossy()
    );
    if !config_file_path().exists() {
        store_default_config()?;
    }
    let config_file = File::open(config_file_path())?;
    Ok(serde_yaml::from_reader(config_file)?)
}

pub fn store_default_config() -> Result<()> {
    ensure_dir_created(&config_file_path())?;
    let config_file = File::create(&config_file_path())?;
    Ok(serde_yaml::to_writer(config_file, &Config::default())?)
}

pub fn store_config(players: &PlayerDb) -> Result<()> {
    ensure_dir_created(&config_file_path())?;
    let stored_config = load_config()?;
    let config_to_store = Config {
        players: players
            .all()
            .cloned()
            .map(|p| PlayerConfig { name: p.name })
            .collect(),
        ..stored_config
    };
    store_file_with_backup(&config_file_path(), &config_to_store)?;
    Ok(())
}

#[derive(Serialize, Deserialize, PartialEq)]
struct HistorySerializeWrapper {
    game: GameId,
    entries: Vec<HistoryEntry>,
}

#[derive(Serialize, Deserialize, PartialEq)]
struct LegacyHistorySerializeWrapper {
    game: GameId,
    entries: Vec<LegacyHistoryEntry>,
}

impl From<LegacyHistorySerializeWrapper> for HistorySerializeWrapper {
    fn from(value: LegacyHistorySerializeWrapper) -> Self {
        let mut history = HistorySerializeWrapper {
            game: value.game,
            entries: value.entries.into_iter().map(HistoryEntry::from).collect(),
        };
        for (i, entry) in history.entries.iter_mut().enumerate() {
            entry.timestamp = DateTime::from(DateTime::UNIX_EPOCH)
                + std::time::Duration::from_secs(i as u64 * 3600)
        }
        history
    }
}

pub fn append_history_entry(game: &GameId, entry: &HistoryEntry) -> Result<()> {
    let mut entries = if history_path(game).is_file() {
        load_history_file(&history_path(game))?
    } else {
        vec![]
    };
    entries.push(entry.clone());
    store_file_with_backup(
        &history_path(game),
        &HistorySerializeWrapper {
            game: game.clone(),
            entries,
        },
    )
}

const HISTORY_SUFFIX: &str = ".history.yaml";
const LEGACY_HISTORY_SUFFIX: &str = ".old.history.yaml";

pub fn load_history() -> Result<History> {
    let mut out = History::default();
    info!("Store: Data Dir: {}", data_dir().to_string_lossy());
    for dir_entry in fs::read_dir(data_dir())? {
        let dir_entry = dir_entry?;
        if is_regular_history_file(&dir_entry.path()) {
            if is_legacy_history_file(&dir_entry.path()) {
                info!(
                    "Store: Legacy history File: {}",
                    dir_entry.path().to_string_lossy()
                );
                let history = load_legacy_history_file(&dir_entry.path())?;
                prepend_game_history(&mut out, history);
            } else {
                info!(
                    "Store: History File: {}",
                    dir_entry.path().to_string_lossy()
                );
                let history_file = File::open(dir_entry.path())?;
                let history: HistorySerializeWrapper = serde_yaml::from_reader(history_file)?;
                out.entries
                    .entry(history.game)
                    .or_default()
                    .extend(history.entries);
            }
        }
    }
    Ok(out)
}

fn load_history_file(path: &Path) -> Result<Vec<HistoryEntry>> {
    let history_file = File::open(path)?;
    let history: HistorySerializeWrapper = serde_yaml::from_reader(history_file)?;
    Ok(history.entries)
}

fn is_regular_history_file(path: &Path) -> bool {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .ends_with(HISTORY_SUFFIX)
}

fn is_legacy_history_file(entry: &Path) -> bool {
    entry
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .ends_with(LEGACY_HISTORY_SUFFIX)
}

fn load_legacy_history_file(path: &Path) -> Result<HistorySerializeWrapper> {
    let history_file = File::open(path)?;
    let history: LegacyHistorySerializeWrapper = serde_yaml::from_reader(history_file)?;
    Ok(history.into())
}

fn history_path(game: &GameId) -> PathBuf {
    let safe_game_id = game.as_str().replace(" ", "_").replace(":", "_");
    let filename = format!("{}{}", safe_game_id, HISTORY_SUFFIX);
    data_dir().join(filename)
}

fn prepend_game_history(out: &mut History, mut history: HistorySerializeWrapper) {
    out.entries
        .entry(history.game)
        .and_modify(|e| {
            // swap to make legacy history appear at the beginning
            std::mem::swap(e, &mut history.entries);
            // append whatever was originally in the entry
            // e.extend(history.entries);
        })
        .or_default()
        .extend(history.entries);
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

#[cfg(test)]
mod test {
    use chrono::Local;

    use eloelo_model::PlayerId;

    use super::*;

    #[test]
    fn test_prepend_history_empty() {
        let mut history = History::default();
        let timestamp = Local::now();
        prepend_game_history(
            &mut history,
            HistorySerializeWrapper {
                game: GameId::from("Dota 2"),
                entries: vec![HistoryEntry {
                    timestamp: timestamp.clone(),
                    winner: vec![PlayerId::from("Winner")],
                    loser: vec![PlayerId::from("Loser")],
                    win_probability: 0.56,
                }],
            },
        );
        assert_eq!(
            history,
            History {
                entries: [(
                    GameId::from("Dota 2"),
                    vec![HistoryEntry {
                        timestamp: timestamp.clone(),
                        winner: vec![PlayerId::from("Winner")],
                        loser: vec![PlayerId::from("Loser")],
                        win_probability: 0.56,
                    }]
                )]
                .into_iter()
                .collect()
            }
        )
    }

    #[test]
    fn test_prepend_history_non_empty() {
        let mut history = History::default();
        let other_timestamp = Local::now();
        history.entries.insert(
            GameId::from("Dota 2"),
            vec![HistoryEntry {
                timestamp: other_timestamp.clone(),
                winner: vec![PlayerId::from("Other Winner")],
                loser: vec![PlayerId::from("Other Loser")],
                win_probability: 0.56,
            }],
        );

        let timestamp = Local::now();
        prepend_game_history(
            &mut history,
            HistorySerializeWrapper {
                game: GameId::from("Dota 2"),
                entries: vec![HistoryEntry {
                    timestamp: timestamp.clone(),
                    winner: vec![PlayerId::from("Winner")],
                    loser: vec![PlayerId::from("Loser")],
                    win_probability: 0.62,
                }],
            },
        );
        assert_eq!(
            history,
            History {
                entries: [(
                    GameId::from("Dota 2"),
                    vec![
                        HistoryEntry {
                            timestamp: timestamp.clone(),
                            winner: vec![PlayerId::from("Winner")],
                            loser: vec![PlayerId::from("Loser")],
                            win_probability: 0.62,
                        },
                        HistoryEntry {
                            timestamp: other_timestamp.clone(),
                            winner: vec![PlayerId::from("Other Winner")],
                            loser: vec![PlayerId::from("Other Loser")],
                            win_probability: 0.56,
                        }
                    ]
                )]
                .into_iter()
                .collect()
            }
        )
    }
}
