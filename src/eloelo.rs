use std::collections::{HashMap, HashSet};
use std::time::Duration;

use anyhow::{Context, Result};
use call_to_lobby::call_to_lobby;
use chrono::Local;
use config::Config;
use eloelo_model::history::{History, HistoryEntry};
use eloelo_model::player::{Player, PlayerDb};
use eloelo_model::{GameId, GameState, PlayerId, Team, WinScale};
use futures_util::stream::{StreamExt as _, TryStreamExt as _};
use git_mirror::GitMirror;
use log::{debug, error, info, warn};
use message_bus::{
    Event, FinishMatch, MatchStart, MatchStartTeam, Message, MessageBus, RichMatchResult, UiCommand,
};
use spawelo::ml_elo;
use ui_state::{State, UiPlayer, UiState};

use crate::utils::{duration_minutes, print_err, unwrap_or_def_verbose, ResultExt as _};

mod call_to_lobby;
pub(crate) mod config;
pub(crate) mod elodisco;
mod git_mirror;
pub(crate) mod message_bus;
pub(crate) mod silly_responder;
pub(crate) mod store;
pub(crate) mod ui_state;

pub struct EloElo {
    selected_game: GameId,
    players: PlayerDb,
    left_players: Vec<PlayerId>,
    right_players: Vec<PlayerId>,
    lobby: HashSet<PlayerId>,
    game_state: GameState,
    history: History,
    config: Config,
    message_bus: MessageBus,
    git_mirror: GitMirror,
}

impl EloElo {
    pub fn new(state: Option<State>, config: Config, message_bus: MessageBus) -> Self {
        let state = state.unwrap_or_else(|| State::new(config.default_game().clone()));

        let _ = std::fs::create_dir_all(&config.history_git_mirror)
            .inspect_err(|e| error!("Cannot create git mirror directory - {e}"));
        let git_mirror = GitMirror::new(config.history_git_mirror.clone());
        let _ = git_mirror
            .sync(None)
            .context("Initial git mirror sync failed")
            .print_err();
        let history = unwrap_or_def_verbose(store::load_history());

        let mut elo = EloElo {
            selected_game: state.selected_game,
            players: PlayerDb::new(config.players.clone().into_iter().map(Player::from)),
            left_players: state.left_players,
            right_players: state.right_players,
            lobby: state.lobby,
            game_state: state.game_state,
            history,
            config,
            message_bus,
            git_mirror,
        };
        elo.recalculate_elo_from_history();
        elo
    }

    pub async fn dispatch_ui_command(&mut self, ui_command: UiCommand) {
        match ui_command {
            UiCommand::InitializeUi => {}
            UiCommand::AddNewPlayer(player) => self.add_new_player(player),
            UiCommand::RemovePlayer(player_id) => self.remove_player(&player_id),
            UiCommand::MovePlayerToOtherTeam(player_id) => {
                self.move_player_to_other_team(&player_id)
            }
            UiCommand::RemovePlayerFromTeam(player_id) => self.remove_player_from_team(&player_id),
            UiCommand::AddPlayerToTeam(player_id, team) => self.add_player_to_team(player_id, team),
            UiCommand::AddPlayerToLobby(player_id) => self.add_player_to_lobby(player_id),
            UiCommand::RemovePlayerFromLobby(player_id) => {
                self.remove_player_from_lobby(&player_id)
            }
            UiCommand::AddLobbyScreenshotData(player_names) => {
                self.update_lobby_from_screenshot(player_names)
            }
            UiCommand::ChangeGame(game_id) => self.change_game(game_id),
            UiCommand::StartMatch => self.start_match(),
            UiCommand::CallToLobby => self.call_to_lobby().await,
            UiCommand::ShuffleTeams => self.shuffle_teams(),
            UiCommand::RefreshElo => self.recalculate_elo_from_history(),
            UiCommand::FinishMatch(finish_match) => self.finish_match(finish_match),
            UiCommand::CloseApplication => {
                if let Err(e) = self.store_state() {
                    error!("store_state failed: {}", e);
                } else {
                    info!("State stored.");
                }
                if let Err(e) = self.store_config() {
                    error!("store_config failed: {}", e);
                } else {
                    info!("Config stored.");
                };
            }
        }
    }

    pub async fn dispatch_ui_commands(mut self, message_bus: MessageBus) {
        let mut ui_command_stream = message_bus.subscribe().ui_command_stream().boxed();
        loop {
            match ui_command_stream.try_next().await {
                Ok(Some(command @ UiCommand::CloseApplication)) => {
                    self.dispatch_ui_command(command).await;
                    break;
                }
                Ok(Some(command)) => {
                    self.dispatch_ui_command(command).await;
                }
                Ok(None) => {
                    break;
                }
                Err(e) => {
                    print_err(&e);
                    break;
                }
            }
            message_bus.send(self.ui_state().into())
        }
    }

    fn store_state(&self) -> Result<()> {
        let state = State {
            selected_game: self.selected_game.clone(),
            left_players: self.left_players.clone(),
            right_players: self.right_players.clone(),
            game_state: self.game_state,
            lobby: self.lobby.clone(),
        };
        store::store_state(&state)
    }

    fn store_config(&self) -> Result<()> {
        store::store_config(&self.players)
    }

    pub fn ui_state(&self) -> UiState {
        let reserve_players: Vec<_> = self.reserve_players().cloned().collect();
        let default_elo = self.default_elo_for_current_game();
        UiState {
            available_games: self.config.games.clone(),
            selected_game: self.selected_game.clone(),
            left_players: self.build_ui_players(&self.left_players, default_elo),
            right_players: self.build_ui_players(&self.right_players, default_elo),
            reserve_players: self.build_ui_players(&reserve_players, default_elo),
            game_state: self.game_state,
            history: self.history.clone(),
        }
    }

    fn players_in_team(&self) -> impl Iterator<Item = &PlayerId> {
        self.players.all().filter_map(|p| {
            if !self.is_in_a_team(&p.id) {
                None
            } else {
                Some(&p.id)
            }
        })
    }

    fn reserve_players(&self) -> impl Iterator<Item = &PlayerId> + '_ {
        self.players.all().filter_map(|p| {
            if self.is_in_a_team(&p.id) {
                None
            } else {
                Some(&p.id)
            }
        })
    }

    fn is_in_a_team(&self, p: &PlayerId) -> bool {
        self.left_players.contains(p) || self.right_players.contains(p)
    }

    fn default_elo_for_current_game(&self) -> i32 {
        let players_ranks: Vec<i32> = self
            .left_players
            .iter()
            .chain(self.right_players.iter())
            .flat_map(|p| self.players.get_rank(p, &self.selected_game))
            .collect();
        let elo_sum: i32 = players_ranks.iter().sum();
        if players_ranks.is_empty() {
            1000
        } else {
            elo_sum / players_ranks.len() as i32
        }
    }

    fn build_ui_players(&self, player_ids: &[PlayerId], default_elo: i32) -> Vec<UiPlayer> {
        player_ids
            .iter()
            .cloned()
            .map(|player_id| {
                let elo = self
                    .players
                    .get_rank(&player_id, &self.selected_game)
                    .unwrap_or(default_elo);
                let name = self
                    .players
                    .get(&player_id)
                    .map(|p| p.get_display_name().to_string())
                    .unwrap_or_else(|| player_id.to_string());
                let discord_username = self
                    .players
                    .get(&player_id)
                    .and_then(|p| p.discord_username().map(|n| n.to_string()));
                let present_in_lobby = self.lobby.contains(&player_id);
                UiPlayer {
                    id: player_id,
                    name,
                    discord_username,
                    elo,
                    present_in_lobby,
                }
            })
            // .map(UiPlayer::build_for(&self.selected_game, self.players.all()))
            .collect()
    }

    fn add_new_player(&mut self, player: Player) {
        self.players.insert(player);
    }

    fn remove_player(&mut self, player_id: &PlayerId) {
        self.players.remove(player_id);
    }

    fn move_player_to_other_team(&mut self, player_id: &PlayerId) {
        if let Some(player) = remove_player_id(&mut self.left_players, player_id) {
            self.right_players.push(player);
            return;
        }
        if let Some(player) = remove_player_id(&mut self.right_players, player_id) {
            self.left_players.push(player);
        }
    }

    fn remove_player_from_team(&mut self, player_id: &PlayerId) {
        remove_player_id(&mut self.left_players, player_id)
            .or_else(|| remove_player_id(&mut self.right_players, player_id));
        self.remove_player_from_lobby(player_id)
    }

    fn add_player_to_team(&mut self, player_id: PlayerId, team: Team) {
        match team {
            Team::Left => self.left_players.push(player_id),
            Team::Right => self.right_players.push(player_id),
        }
    }

    fn change_game(&mut self, game: GameId) {
        self.selected_game = game;
        self.recalculate_elo_from_history();
    }

    fn start_match(&mut self) {
        let default_elo = self.default_elo_for_current_game();
        self.game_state = GameState::MatchInProgress;
        self.message_bus
            .send(Message::Event(Event::MatchStart(MatchStart {
                game: self.selected_game.clone(),
                player_db: self.players.clone(),
                left_team: MatchStartTeam {
                    name: self
                        .config
                        .games
                        .iter()
                        .find(|g| g.name == self.selected_game)
                        .map_or("Left Team".to_string(), |g| g.left_team.clone()),
                    players: self.players.get_ranked_owned(
                        &self.left_players,
                        &self.selected_game,
                        default_elo,
                    ),
                },
                right_team: MatchStartTeam {
                    name: self
                        .config
                        .games
                        .iter()
                        .find(|g| g.name == self.selected_game)
                        .map_or("Right Team".to_string(), |g| g.right_team.clone()),
                    players: self.players.get_ranked_owned(
                        &self.right_players,
                        &self.selected_game,
                        default_elo,
                    ),
                },
            })));
    }

    fn finish_match(&mut self, finish_match: FinishMatch) {
        if let FinishMatch::Finished {
            winner,
            scale,
            duration,
            fake,
        } = finish_match
        {
            let commit_message = self.mk_finish_match_commit_message(winner, scale, duration, fake);
            let winner_team_name = self.get_team_name(winner);
            let (winner, loser) = match winner {
                Team::Left => (self.left_players.clone(), self.right_players.clone()),
                Team::Right => (self.right_players.clone(), self.left_players.clone()),
            };
            let history_entry = HistoryEntry {
                timestamp: Local::now(),
                winner,
                loser,
                scale,
                duration,
                fake,
            };
            let _ = store::append_history_entry(&self.selected_game, &history_entry)
                .context("Failed to append history entry")
                .print_err(); // TODO: proper error propagation
            let _ = self
                .git_mirror
                .sync(Some(&commit_message))
                .context("Failed to sync history git mirror")
                .print_err(); // TODO: proper error propagation

            // Send rich event
            if !fake {
                self.message_bus
                    .send(Message::Event(Event::RichMatchResult(RichMatchResult {
                        winner_team_name,
                        duration,
                        scale,
                    })));
            }

            // Failsafe history message in log
            let history_log_msg = serde_json::to_string(&history_entry)
                .unwrap_or_else(|e| format!("Failed to serialize history: {e}"));
            info!(target: "history", "FinishMatch: {history_log_msg}");

            self.history_for_current_game_mut().push(history_entry);
            self.update_elo();
            self.lobby = HashSet::new();
        }

        self.game_state = GameState::AssemblingTeams;
        debug!("finish_match handled");
    }

    fn mk_finish_match_commit_message(
        &self,
        winner: Team,
        scale: WinScale,
        duration: Duration,
        fake: bool,
    ) -> String {
        let winner_team = self.get_team_name(winner);
        let match_type = if fake { "Fake Match" } else { "Match" };
        [
            format!(
                "{} {} - {} {}",
                match_type, self.selected_game, winner_team, scale
            ),
            String::from(""),
            format!("Duration: {}", duration_minutes(duration)),
        ]
        .join("\n")
    }

    fn get_team_name(&self, t: Team) -> String {
        self.config
            .games
            .iter()
            .find_map(|g| {
                if g.name == self.selected_game {
                    return Some(g.team_name(t).to_string());
                }
                None
            })
            .unwrap_or_else(|| t.to_string())
    }

    fn update_elo(&mut self) {
        let updates = ml_elo(self.history_for_elo_calc());

        for (player, new_elo) in updates.iter() {
            self.players
                .set_rank(player, &self.selected_game, *new_elo as i32);
        }
    }

    fn history_for_current_game_mut(&mut self) -> &mut Vec<HistoryEntry> {
        self.history
            .entries
            .entry(self.selected_game.clone())
            .or_default()
    }

    fn history_for_elo_calc(&self) -> &[HistoryEntry] {
        let n = self.config.max_elo_history;
        debug!("Selected game: {}, Max history: {}", self.selected_game, n);
        match self.history.entries.get(&self.selected_game) {
            Some(history) => {
                debug!("Entries count: {}", history.len());
                if n > 0 && history.len() > n {
                    &history[history.len() - n..]
                } else {
                    &history
                }
            }
            None => {
                warn!("No history entries found");
                &[]
            }
        }
    }

    fn shuffle_teams(&mut self) {
        let default_elo = self.default_elo_for_current_game();
        let left =
            self.players
                .get_ranked_owned(&self.left_players, &self.selected_game, default_elo);
        let right =
            self.players
                .get_ranked_owned(&self.right_players, &self.selected_game, default_elo);

        let (_, left, right) = spawelo::shuffle_teams(left.into_iter().chain(right));

        self.left_players = left.into_iter().map(|p| p.0).collect();
        self.right_players = right.into_iter().map(|p| p.0).collect();
    }

    fn recalculate_elo_from_history(&mut self) {
        info!("Reloading history");
        self.history = unwrap_or_def_verbose(store::load_history());

        info!("Recalculating {} elo from history", &self.selected_game);

        let history = self.history_for_elo_calc();
        if history.is_empty() {
            let all_players: Vec<_> = self.players.all().map(|p| p.id.clone()).collect();
            for player in all_players {
                self.players.remove_rank(&player, &self.selected_game);
            }
        } else {
            let elo = ml_elo(self.history_for_elo_calc());
            for (player, new_elo) in elo.iter() {
                self.players
                    .set_rank(player, &self.selected_game, *new_elo as i32);
            }
        }
    }

    fn add_player_to_lobby(&mut self, player_id: PlayerId) {
        self.lobby.insert(player_id);
    }

    fn remove_player_from_lobby(&mut self, player_id: &PlayerId) {
        self.lobby.remove(player_id);
    }

    async fn call_to_lobby(&self) {
        let _ = call_to_lobby(
            &self.config.fosiaudio_host,
            self.players_missing_from_lobby(),
        )
        .await
        .context("Call to lobby failed")
        .print_err();
    }

    fn players_missing_from_lobby(&self) -> impl Iterator<Item = &Player> {
        self.left_players
            .iter()
            .chain(&self.right_players)
            .filter(|p| !self.lobby.contains(p))
            .flat_map(|p| self.players.get(p))
    }

    fn update_lobby_from_screenshot(&mut self, player_names: Vec<String>) {
        let mut player_ids = HashMap::new();
        for player_id in self.players_in_team() {
            let p = self.config.get_player(player_id).unwrap();
            let mut possible_names = Vec::new();
            possible_names.push(player_id.as_str().to_lowercase().to_string());
            possible_names.extend(
                p.discord_username
                    .as_ref()
                    .map(|n| n.as_str().to_lowercase()),
            );
            possible_names.extend(p.display_name.as_ref().map(|n| n.to_lowercase()));
            possible_names.extend(p.ocr_names.iter().by_ref().map(|n| n.to_lowercase()));
            possible_names.extend(p.fosiaudio_name.as_ref().map(|n| n.to_lowercase()));
            debug!(
                "{player_id} aliases: {}",
                possible_names
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            player_ids.extend(possible_names.into_iter().map(|n| (n, &p.id)));
        }
        let player_ids = player_ids;

        for name in player_names {
            match player_ids.get(&name.to_lowercase()) {
                Some(&player_id) => {
                    info!("Found {player_id} in screenshot data (as {name})");
                    self.lobby.insert(player_id.clone());
                }
                None => {
                    debug!("Player name `{name}` not found among known player aliases");
                }
            }
        }
    }
}

fn remove_player_id(players: &mut Vec<PlayerId>, player_id: &PlayerId) -> Option<PlayerId> {
    players
        .iter()
        .enumerate()
        .find_map(|(i, p)| if p == player_id { Some(i) } else { None })
        .map(|idx| players.remove(idx))
}
