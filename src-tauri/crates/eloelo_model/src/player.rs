use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{GameId, PlayerId};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Player {
    pub id: PlayerId,
    pub display_name: Option<String>,
    pub discord_username: Option<DiscordUsername>,
    #[serde(default)]
    pub elo: HashMap<GameId, i32>,
}

impl Player {
    // fn with_player_id(value: String) -> Self {
    //     Player {
    //         id: PlayerId::from(value.clone()),
    //         display_name: None,
    //         discord_username: None,
    //         elo: Default::default(),
    //     }
    // }

    pub fn default_elo() -> i32 {
        1000
    }

    pub fn get_elo(&self, game: &GameId) -> i32 {
        self.elo.get(game).copied().unwrap_or(Player::default_elo())
    }

    pub fn get_elo_mut(&mut self, game: &GameId) -> &mut i32 {
        self.elo
            .entry(game.clone())
            .or_insert(Player::default_elo())
    }
}

#[derive(Debug, Clone)]
pub struct PlayerDb {
    players: HashMap<PlayerId, Player>,
}

impl PlayerDb {
    pub fn new(players: impl IntoIterator<Item = Player>) -> Self {
        Self {
            players: players.into_iter().map(|p| (p.id.clone(), p)).collect(),
        }
    }

    pub fn get(&self, id: &PlayerId) -> Option<&Player> {
        self.players.get(id)
    }

    pub fn all(&self) -> impl Iterator<Item = &Player> {
        self.players.values()
    }

    pub fn get_ranked<'a>(
        &'a self,
        players: &'a [PlayerId],
        game: &'a GameId,
    ) -> impl IntoIterator<Item = (&'a PlayerId, i32)> + 'a {
        self.players
            .iter()
            .filter(|(k, _)| players.contains(k))
            .map(|(k, v)| (k, v.get_elo(game)))
    }

    pub fn get_ranked_owned(&self, players: &[PlayerId], game: &GameId) -> HashMap<PlayerId, i32> {
        self.players
            .iter()
            .filter(|(k, _)| players.contains(k))
            .map(|(k, v)| (k.clone(), v.get_elo(game)))
            .collect()
    }

    pub fn get_rank(&self, player_id: &PlayerId, game: &GameId) -> i32 {
        self.players
            .get(player_id)
            .map(|p| p.get_elo(game))
            .unwrap_or(Player::default_elo())
    }

    pub fn insert(&mut self, player: Player) {
        self.players.insert(player.id.clone(), player);
    }

    pub fn remove(&mut self, player_id: &PlayerId) -> Option<Player> {
        self.players.remove(player_id)
    }

    pub fn set_rank(&mut self, player_id: &PlayerId, selected_game: &GameId, new_elo: i32) {
        if let Some(player) = self.players.get_mut(player_id) {
            *player.get_elo_mut(selected_game) = new_elo;
        }
    }

    pub fn all_mut(&mut self) -> impl Iterator<Item = &mut Player> {
        self.players.values_mut()
    }
}

pub type PlayerWithElo = (PlayerId, i32);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Hash)]
pub struct DiscordUsername(String);

impl From<String> for DiscordUsername {
    fn from(value: String) -> Self {
        DiscordUsername(value)
    }
}

impl std::fmt::Display for DiscordUsername {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
