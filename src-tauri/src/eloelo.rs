use std::borrow::Borrow;
use std::fmt::Display;

use anyhow::Result;
use chrono::Local;
use config::Config;
use eloelo_model::history::{History, HistoryEntry};
use eloelo_model::player::{Player, PlayerDb};
use eloelo_model::{GameId, GameState, PlayerId, Team};
use log::{error, info};
use message_bus::{Event, FinishMatch, MatchStart, MatchStartTeam, Message, MessageBus, UiCommand};
use spawelo::ml_elo;
use store::append_history_entry;
use ui_state::{State, UiPlayer, UiState};

pub(crate) mod config;
pub(crate) mod elodisco;
pub(crate) mod message_bus;
pub(crate) mod silly_responder;
pub(crate) mod store;
pub(crate) mod ui_state;

pub struct EloElo {
    selected_game: GameId,
    players: PlayerDb,
    left_players: Vec<PlayerId>,
    right_players: Vec<PlayerId>,
    game_state: GameState,
    history: History,
    config: Config,
    message_bus: MessageBus,
}

impl EloElo {
    pub fn new(state: State, history: History, config: Config, message_bus: MessageBus) -> Self {
        let mut elo = EloElo {
            selected_game: state.selected_game,
            players: PlayerDb::new(config.players.clone().into_iter().map(Player::from)),
            left_players: state.left_players,
            right_players: state.right_players,
            game_state: state.game_state,
            history,
            config,
            message_bus,
        };
        elo.recalculate_elo_from_history();
        elo
    }

    pub fn dispatch_ui_command(&mut self, ui_command: UiCommand) {
        match ui_command {
            UiCommand::InitializeUi => {}
            UiCommand::AddNewPlayer(player) => self.add_new_player(player),
            UiCommand::RemovePlayer(player_id) => self.remove_player(&player_id),
            UiCommand::MovePlayerToOtherTeam(name) => self.move_player_to_other_team(name),
            UiCommand::RemovePlayerFromTeam(name) => self.remove_player_from_team(name),
            UiCommand::AddPlayerToTeam(name, team) => self.add_player_to_team(name, team),
            UiCommand::ChangeGame(game_id) => self.change_game(game_id),
            UiCommand::StartMatch => self.start_match(),
            UiCommand::ShuffleTeams => self.shuffle_teams(),
            UiCommand::RefreshElo => self.recalculate_elo_from_history(),
            UiCommand::FinishMatch(finish_match) => self.finish_match(finish_match),
            UiCommand::CloseApplication => {
                if let Err(e) = self.store_state() {
                    error!("store_state failed: {}", e);
                } else {
                    info!("State stored.");
                };
                if let Err(e) = self.store_config() {
                    error!("store_config failed: {}", e);
                } else {
                    info!("Config stored.");
                };
            }
        }
    }

    fn store_state(&self) -> Result<()> {
        let state = State {
            selected_game: self.selected_game.clone(),
            left_players: self.left_players.clone(),
            right_players: self.right_players.clone(),
            game_state: self.game_state,
        };
        store::store_state(&state)
    }

    fn store_config(&self) -> Result<()> {
        store::store_config(&self.players)
    }

    pub fn ui_state(&self) -> UiState {
        let reserve_players: Vec<_> = self.reserve_players().cloned().collect();
        UiState {
            available_games: self.config.games.clone(),
            selected_game: self.selected_game.clone(),
            left_players: self.build_ui_players(&self.left_players),
            right_players: self.build_ui_players(&self.right_players),
            reserve_players: self.build_ui_players(&reserve_players),
            game_state: self.game_state,
            history: self.history.clone(),
        }
    }

    fn reserve_players(&self) -> impl Iterator<Item = &PlayerId> + '_ {
        self.players.all().filter_map(|p| {
            if self.is_in_a_team(&p.name) {
                None
            } else {
                Some(&p.name)
            }
        })
    }

    fn is_in_a_team(&self, p: &PlayerId) -> bool {
        self.left_players.contains(p) || self.right_players.contains(p)
    }

    fn build_ui_players(&self, player_ids: &[PlayerId]) -> Vec<UiPlayer> {
        player_ids
            .iter()
            .cloned()
            .map(|player_id| {
                let rank = self.players.get_rank(&player_id, &self.selected_game);
                UiPlayer {
                    name: player_id,
                    elo: rank,
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

    fn move_player_to_other_team(&mut self, name: String) {
        if let Some(player) = remove_player_id(&mut self.left_players, &name) {
            self.right_players.push(player);
            return;
        }
        if let Some(player) = remove_player_id(&mut self.right_players, &name) {
            self.left_players.push(player);
        }
    }

    fn remove_player_from_team(&mut self, name: String) {
        remove_player_id(&mut self.left_players, &name)
            .or_else(|| remove_player_id(&mut self.right_players, &name));
    }

    fn add_player_to_team(&mut self, name: String, team: Team) {
        match team {
            Team::Left => self.left_players.push(PlayerId::from(name)),
            Team::Right => self.right_players.push(PlayerId::from(name)),
        }
    }

    fn change_game(&mut self, game: GameId) {
        self.selected_game = game;
        self.recalculate_elo_from_history();
    }

    fn start_match(&mut self) {
        self.game_state = GameState::MatchInProgress;
        self.message_bus
            .send(Message::Event(Event::MatchStart(MatchStart {
                game: self.selected_game.clone(),
                left_team: MatchStartTeam {
                    name: self
                        .config
                        .games
                        .iter()
                        .find(|g| g.name == self.selected_game)
                        .map_or("Left Team".to_string(), |g| g.left_team.clone()),
                    players: self
                        .players
                        .get_ranked_owned(&self.left_players, &self.selected_game),
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
                        .get_ranked_owned(&self.right_players, &self.selected_game),
                },
            })));
    }

    fn finish_match(&mut self, finish_match: FinishMatch) {
        if let Some(winner) = finish_match.winner {
            let (winner, loser) = match winner {
                Team::Left => (self.left_players.clone(), self.right_players.clone()),
                Team::Right => (self.right_players.clone(), self.left_players.clone()),
            };
            let history_entry = HistoryEntry {
                timestamp: Local::now(),
                winner,
                loser,
                scale: finish_match.scale,
            };
            let _ =
                append_history_entry(&self.selected_game, &history_entry).inspect_err(print_err); // TODO: proper error propagation
            self.history_for_current_game_mut().push(history_entry);
            self.update_elo();
        }

        self.game_state = GameState::AssemblingTeams;
    }

    fn update_elo(&mut self) {
        let players: Vec<_> = self.players.all().map(|p| p.name.clone()).collect();
        let players = self.players.get_ranked_owned(&players, &self.selected_game);
        let updates = ml_elo(self.history_for_elo_calc(), &players);

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
        match self.history.entries.get(&self.selected_game) {
            Some(history) if n > 0 => &history[history.len() - n..],
            Some(history) => &history,
            None => &[],
        }
    }

    fn shuffle_teams(&mut self) {
        // TODO: single shuffle?
        self.shuffle();
        let mut best = (
            self.elo_diff(),
            self.left_players.clone(),
            self.right_players.clone(),
        );
        for _ in 1..100 {
            self.shuffle();
            let diff = self.elo_diff();
            if diff < best.0 {
                best.0 = diff;
                best.1 = self.left_players.clone();
                best.2 = self.right_players.clone();
            }
        }
        self.left_players = best.1;
        self.right_players = best.2;
    }

    fn shuffle(&mut self) -> i32 {
        let left = self
            .players
            .get_ranked_owned(&self.left_players, &self.selected_game);
        let right = self
            .players
            .get_ranked_owned(&self.right_players, &self.selected_game);

        let (diff, left, right) = spawelo::shuffle_teams(left.into_iter().chain(right));

        self.left_players = left.into_iter().map(|p| p.0).collect();
        self.right_players = right.into_iter().map(|p| p.0).collect();
        diff
    }

    fn elo_diff(&self) -> i32 {
        let ranked_left: Vec<_> = self
            .players
            .get_ranked(&self.left_players, &self.selected_game)
            .into_iter()
            .collect();
        let ranked_right: Vec<_> = self
            .players
            .get_ranked(&self.right_players, &self.selected_game)
            .into_iter()
            .collect();
        elo_diff(&ranked_left, &ranked_right)
    }

    fn recalculate_elo_from_history(&mut self) {
        info!("Recalculating {} elo from history", &self.selected_game);
        self.reset_elo();

        let elo = ml_elo(self.history_for_elo_calc(), &Default::default());
        for (player, new_elo) in elo.iter() {
            self.players
                .set_rank(player, &self.selected_game, *new_elo as i32);
        }
    }

    fn reset_elo(&mut self) {
        for p in self.players.all_mut() {
            *p.get_elo_mut(&self.selected_game) = Player::default_elo();
        }
    }
}

fn elo_diff(left: &[(impl Borrow<PlayerId>, i32)], right: &[(impl Borrow<PlayerId>, i32)]) -> i32 {
    let left_avg = if !left.is_empty() {
        left.iter().map(|p| p.1).sum::<i32>() / left.len() as i32
    } else {
        0
    };
    let right_avg = if !right.is_empty() {
        right.iter().map(|p| p.1).sum::<i32>() / right.len() as i32
    } else {
        0
    };
    (left_avg - right_avg).abs()
}

fn remove_player_id(players: &mut Vec<PlayerId>, name: &str) -> Option<PlayerId> {
    players
        .iter()
        .enumerate()
        .find_map(|(i, p)| if p.as_str() == name { Some(i) } else { None })
        .map(|idx| players.remove(idx))
}

pub(crate) fn print_err<E: Display>(e: &E) {
    error!("{}", e);
}
