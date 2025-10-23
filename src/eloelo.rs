use std::collections::{HashMap, HashSet};
use std::time::Duration;

use crate::eloelo::message_bus::MatchInfo;
use crate::eloelo::options::EloEloOptions;
use crate::utils::{duration_minutes, print_err, unwrap_or_def_verbose, ResultExt as _};
use anyhow::{Context, Result};
use chrono::Local;
use config::Config;
use eloelo_model::decimal::Decimal;
use eloelo_model::history::{History, HistoryEntry};
use eloelo_model::player::{Player, PlayerDb, PlayersConfig};
use eloelo_model::{BalancedTeam, GameId, GameState, PlayerId, Team, WinScale};
use futures_util::stream::{StreamExt as _, TryStreamExt as _};
use git_mirror::GitMirror;
use log::{debug, error, info, warn};
use message_bus::{
    Event, FinishMatch, MatchStart, MatchStartTeam, Message, MessageBus, RichMatchResult, UiCommand,
};
use regex::Regex;
use spawelo::ml_elo;
use ui_state::{PityBonus, State, UiPlayer, UiState};

mod config;
pub(crate) mod elodisco;
mod fosiaudio;
mod git_mirror;
pub(crate) mod message_bus;
pub(crate) mod ocr;
pub mod options;
mod silly_responder;
pub(crate) mod store;
mod ui_state;

pub struct EloElo {
    selected_game: GameId,
    players: PlayerDb,
    left_team: BalancedTeam,
    right_team: BalancedTeam,
    lobby: HashSet<PlayerId>,
    game_state: GameState,
    history: History,
    config: Config,
    players_config: PlayersConfig,
    message_bus: MessageBus,
    git_mirror: GitMirror,
    options: EloEloOptions,
    shuffle_temperature: i32,
}

impl EloElo {
    pub fn new(
        state: Option<State>,
        config: Config,
        players_config: PlayersConfig,
        options: EloEloOptions,
        message_bus: MessageBus,
    ) -> Self {
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
            players: PlayerDb::new(players_config.players.iter().cloned().map(Player::from)),
            left_team: state.left_team,
            right_team: state.right_team,
            lobby: state.lobby,
            game_state: state.game_state,
            history,
            config,
            players_config,
            message_bus,
            git_mirror,
            options,
            shuffle_temperature: state.shuffle_temperature,
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
            UiCommand::AddPlayerToLobby(player_id) => self.add_player_to_lobby(player_id).await,
            UiCommand::RemovePlayerFromLobby(player_id) => {
                self.remove_player_from_lobby(&player_id)
            }
            UiCommand::AddLobbyScreenshotData(player_names) => {
                self.update_lobby_from_screenshot(player_names)
            }
            UiCommand::ChangeGame(game_id) => self.change_game(game_id),
            UiCommand::StartMatch => self.start_match(),
            UiCommand::CallToLobby => self.call_to_lobby().await,
            UiCommand::FillLobby => self.fill_lobby().await,
            UiCommand::ClearLobby => self.clear_lobby(),
            UiCommand::CallPlayer(player_id) => self.call_player(&player_id).await,
            UiCommand::ShuffleTeams => self.shuffle_teams(),
            UiCommand::RefreshElo => self.recalculate_elo_from_history(),
            UiCommand::FinishMatch(finish_match) => self.finish_match(finish_match).await,
            UiCommand::UpdateOptions(options) => self.update_options(options),
            UiCommand::SetShuffleTemperature(temperature) => self.shuffle_temperature = temperature,
            UiCommand::CloseApplication => {
                if let Err(e) = self.store_state() {
                    error!("store_state failed: {}", e);
                } else {
                    info!("State stored.");
                }
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
            left_team: self.left_team.clone(),
            right_team: self.right_team.clone(),
            game_state: self.game_state,
            lobby: self.lobby.clone(),
            shuffle_temperature: self.shuffle_temperature,
        };
        store::store_state(&state)?;
        store::store_options(&self.options)?;
        Ok(())
    }

    pub fn ui_state(&self) -> UiState {
        let default_elo = self.default_elo_for_current_game();
        let reserve_players = &self.make_reserve_players();
        UiState {
            available_games: self.config.games.clone(),
            selected_game: self.selected_game.clone(),
            left_players: self.build_ui_players(&self.left_team.players, default_elo),
            right_players: self.build_ui_players(&self.right_team.players, default_elo),
            reserve_players: self.build_ui_players(reserve_players, default_elo),
            pity_bonus: self.make_pity_bonus_data(&self.left_team, &self.right_team),
            game_state: self.game_state,
            history: self.history.clone(),
            options: self.options.to_described_options_group_vec(),
            win_prediction: Decimal::with_precision(
                spawelo::calculate_win_prediction(
                    self.left_team.real_elo,
                    self.right_team.real_elo,
                ),
                3,
            ),
            shuffle_temperature: self.shuffle_temperature,
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

    fn make_reserve_players(&self) -> Vec<PlayerId> {
        self.players
            .all()
            .filter_map(|p| {
                if self.is_in_a_team(&p.id) {
                    None
                } else {
                    Some(p.id.clone())
                }
            })
            .collect()
    }

    fn is_in_a_team(&self, p: &PlayerId) -> bool {
        self.left_team.players.iter().find(|x| *x == p).is_some()
            || self.right_team.players.iter().find(|x| *x == p).is_some()
    }

    fn default_elo_for_current_game(&self) -> i32 {
        let players_ranks: Vec<i32> = self
            .left_team
            .players
            .iter()
            .chain(self.right_team.players.iter())
            .flat_map(|p| self.players.get_rank(p, &self.selected_game))
            .collect();
        let elo_sum: i32 = players_ranks.iter().sum();
        if players_ranks.is_empty() {
            1000
        } else {
            elo_sum / players_ranks.len() as i32
        }
    }

    fn build_ui_players(&self, players: &[PlayerId], default_elo: i32) -> Vec<UiPlayer> {
        let lose_streaks = self.lose_streaks_for_current_lobby();
        players
            .iter()
            .cloned()
            .map(|player| {
                let elo = self
                    .players
                    .get_rank(&player, &self.selected_game)
                    .unwrap_or(default_elo);
                let name = self
                    .players
                    .get(&player)
                    .map(|p| p.get_display_name().to_string())
                    .unwrap_or_else(|| player.to_string());
                let discord_username = self
                    .players
                    .get(&player)
                    .and_then(|p| p.discord_username().map(|n| n.to_string()));
                let present_in_lobby = self.lobby.contains(&player);
                let lose_streak = lose_streaks.get(&player).copied();
                UiPlayer {
                    id: player,
                    name,
                    discord_username,
                    elo,
                    present_in_lobby,
                    lose_streak,
                }
            })
            .collect()
    }

    fn add_new_player(&mut self, player: Player) {
        self.players.insert(player);
        store::store_players(self.players.to_players_config()).print_err();
    }

    fn remove_player(&mut self, player_id: &PlayerId) {
        self.players.remove(player_id);
        store::store_players(self.players.to_players_config()).print_err();
    }

    fn transfer_player_id(&mut self, player_id: &PlayerId) {
        if let Some(player) = remove_player_id(&mut self.left_team.players, player_id) {
            self.right_team.players.push(player);
            return;
        }
        if let Some(player) = remove_player_id(&mut self.right_team.players, player_id) {
            self.left_team.players.push(player);
        }
    }

    fn move_player_to_other_team(&mut self, player_id: &PlayerId) {
        self.transfer_player_id(player_id);
        self.update_teams_elo();
    }

    fn update_teams_elo(&mut self) {
        let default_elo = self.default_elo_for_current_game();
        let left = self
            .players
            .get_ranked_owned(&self.left_team.players, &self.selected_game, default_elo)
            .into_iter()
            .collect();
        let right = self
            .players
            .get_ranked_owned(&self.right_team.players, &self.selected_game, default_elo)
            .into_iter()
            .collect();
        (self.left_team, self.right_team) = spawelo::calculate_teams_elo(
            left,
            right,
            &&self.lose_streaks_for_current_lobby(),
            &self.options.spawelo,
        );
    }

    fn remove_player_from_team(&mut self, player_id: &PlayerId) {
        remove_player_id(&mut self.left_team.players, player_id)
            .or_else(|| remove_player_id(&mut self.right_team.players, player_id));
        self.remove_player_from_lobby(player_id);
        self.update_teams_elo();
    }

    fn add_player_to_team(&mut self, player_id: PlayerId, team: Team) {
        match team {
            Team::Left => self.left_team.players.push(player_id),
            Team::Right => self.right_team.players.push(player_id),
        };
        self.update_teams_elo();
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
                    players: self
                        .players
                        .get_ranked_owned(&self.left_team.players, &self.selected_game, default_elo)
                        .map(|p| (p.id, p.elo))
                        .collect(),
                },
                right_team: MatchStartTeam {
                    name: self
                        .config
                        .games
                        .iter()
                        .find(|g| g.name == self.selected_game)
                        .map_or("Right Team".to_string(), |g| g.right_team.clone()),
                    players: self
                        .players
                        .get_ranked_owned(
                            &self.right_team.players,
                            &self.selected_game,
                            default_elo,
                        )
                        .map(|p| (p.id, p.elo))
                        .collect(),
                },
            })));
    }

    async fn finish_match(&mut self, finish_match: FinishMatch) {
        if let FinishMatch::Finished(info) = finish_match {
            let history_entry = self.make_history_entry(info);
            self.store_updated_history(&history_entry, info.winner);
            self.play_winner_theme(info.winner).await;
            self.send_match_result(info);

            // Failsafe history message in log
            let history_log_msg = serde_json::to_string(&history_entry)
                .unwrap_or_else(|e| format!("Failed to serialize history: {e}"));
            info!(target: "history", "FinishMatch: {history_log_msg}");

            // Update local state
            self.history_for_current_game_mut().push(history_entry);
            self.update_elo();
            self.lobby = HashSet::new();
        }

        self.game_state = GameState::AssemblingTeams;
        debug!("finish_match handled");
    }

    fn update_options(&mut self, options: EloEloOptions) {
        info!("Update options: {:?}", options);
        self.options = options;
        store::store_options(&self.options).print_err();
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
        let left = self.players.get_ranked_owned(
            &self.left_team.players,
            &self.selected_game,
            default_elo,
        );
        let right = self.players.get_ranked_owned(
            &self.right_team.players,
            &self.selected_game,
            default_elo,
        );

        let (left, right) = spawelo::shuffle_teams(
            left.into_iter().chain(right),
            &self.lose_streaks_for_current_lobby(),
            self.shuffle_temperature,
            &self.options.spawelo,
        );

        self.left_team = left;
        self.right_team = right;
    }

    fn lose_streaks_for_current_lobby(&self) -> HashMap<PlayerId, i32> {
        let max_days = if self.options.spawelo.lose_streak_max_days > 0 {
            Some(self.options.spawelo.lose_streak_max_days as u64)
        } else {
            None
        };
        self.history
            .calculate_lose_streaks(&self.selected_game, self.players_in_team(), max_days)
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

    async fn add_player_to_lobby(&mut self, player_id: PlayerId) {
        self.lobby.insert(player_id);
        if self.everybody_in_lobby() {
            // Empty call to lobby will trigger match starting audio track
            self.call_to_lobby().await
        }
    }

    fn remove_player_from_lobby(&mut self, player_id: &PlayerId) {
        self.lobby.remove(player_id);
    }

    async fn call_to_lobby(&self) {
        let _ = fosiaudio::call_missing_players(
            &self.config.fosiaudio_host,
            self.players_missing_from_lobby(),
            Duration::from_millis(self.config.fosiaudio_timeout_ms),
        )
        .await
        .context("Call to lobby failed")
        .print_err();
    }

    fn players_missing_from_lobby(&self) -> impl Iterator<Item = &Player> {
        self.left_team
            .players
            .iter()
            .chain(&self.right_team.players)
            .filter(|p| !self.lobby.contains(p))
            .flat_map(|p| self.players.get(p))
    }

    fn update_lobby_from_screenshot(&mut self, player_names: Vec<String>) {
        let mut player_ids = HashMap::new();
        for player_id in self.players_in_team() {
            let p = self.players_config.get_player(player_id).unwrap();
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

        let player_names: Vec<String> = player_names
            .into_iter()
            .flat_map(|p| with_alternative_matches(&p))
            .collect();
        debug!(
            "Matching players against extended list: {}",
            player_names.join(", ")
        );

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

    async fn fill_lobby(&mut self) {
        let full_lobby: HashSet<_> = self.players_in_team().cloned().collect();
        self.lobby = full_lobby;
        // Empty call to lobby will trigger match starting audio track
        self.call_to_lobby().await
    }

    fn clear_lobby(&mut self) {
        self.lobby.clear();
    }

    async fn call_player(&self, player_id: &PlayerId) {
        let Some(player) = self.players.get(player_id) else {
            return;
        };
        //TODO: move to background
        let _ = fosiaudio::call_single_player(
            &self.config.fosiaudio_host,
            player,
            Duration::from_millis(self.config.fosiaudio_timeout_ms),
        )
        .await
        .context("Call to lobby failed")
        .print_err();
    }

    fn everybody_in_lobby(&self) -> bool {
        let expected: HashSet<_> = self.players_in_team().cloned().collect();
        expected == self.lobby
    }

    async fn play_winner_theme(&self, winner: Team) {
        let winner_team_name = self.get_team_name(winner);
        fosiaudio::announce_winner(
            &self.config.fosiaudio_host,
            &winner_team_name,
            Duration::from_millis(self.config.fosiaudio_timeout_ms),
        )
        .await
        .print_err(); // TODO: proper error propagation
    }

    fn send_match_result(&self, info: MatchInfo) {
        if info.fake {
            return;
        }
        self.message_bus
            .send(Message::Event(Event::RichMatchResult(RichMatchResult {
                winner_team_name: self.get_team_name(info.winner),
                duration: info.duration,
                scale: info.scale,
            })));
    }

    fn store_updated_history(&self, history_entry: &HistoryEntry, winner: Team) {
        let _ = store::append_history_entry(&self.selected_game, &history_entry)
            .context("Failed to append history entry")
            .print_err(); // TODO: proper error propagation
        if !self.config.test_mode {
            let commit_message = self.mk_finish_match_commit_message(
                winner,
                history_entry.scale,
                history_entry.duration,
                history_entry.fake,
            );
            let _ = self
                .git_mirror
                .sync(Some(&commit_message))
                .context("Failed to sync history git mirror")
                .print_err(); // TODO: proper error propagation
        }
    }

    fn make_history_entry(&self, info: MatchInfo) -> HistoryEntry {
        let (winner, loser) = match info.winner {
            Team::Left => (
                self.left_team.players.clone(),
                self.right_team.players.clone(),
            ),
            Team::Right => (
                self.right_team.players.clone(),
                self.left_team.players.clone(),
            ),
        };
        HistoryEntry {
            timestamp: Local::now(),
            winner,
            loser,
            scale: info.scale,
            duration: info.duration,
            fake: info.fake,
        }
    }

    fn make_pity_bonus_data(
        &self,
        left_team: &BalancedTeam,
        right_team: &BalancedTeam,
    ) -> PityBonus {
        PityBonus {
            left: left_team.into(),
            right: right_team.into(),
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

fn with_alternative_matches(p: &str) -> Vec<String> {
    let mut out = vec![String::from(p)];
    let re = Regex::new(r"(?<alt>\w+?)(.?kuce.?)$").unwrap();
    if let Some(captures) = re.captures(p) {
        out.extend(captures.name("alt").map(|m| String::from(m.as_str())));
    }
    out
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_with_alternative_matches() {
        assert_eq!(
            with_alternative_matches("spawektkuce]"),
            vec![String::from("spawektkuce]"), String::from("spawek")]
        );
        assert_eq!(
            with_alternative_matches("jikuce"),
            vec![String::from("jikuce"), String::from("j")]
        );
        assert_eq!(
            with_alternative_matches("jkuce}"),
            vec![String::from("jkuce}"), String::from("j")]
        );
        assert_eq!(
            with_alternative_matches("jkuce"),
            vec![String::from("jkuce"), String::from("j")]
        );
        assert_eq!(
            with_alternative_matches("jkucet"),
            vec![String::from("jkucet"), String::from("j")]
        );
    }
}
