use anyhow::Result;
use bytes::Bytes;
use chrono::{DateTime, Local};
use eloelo_model::player::{DiscordUsername, Player, PlayerDb};
use eloelo_model::{GameId, PlayerId, Team, WinScale};
use futures_util::{Stream, StreamExt};
use log::error;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::broadcast::{Receiver, Sender};
use tokio_stream::wrappers::BroadcastStream;

use crate::eloelo::elodisco::dota_bot::Hero;
use crate::eloelo::elodisco::hero_assignment_strategy::HeroTag;
use crate::eloelo::options::{EloEloOptions, HeroAssignmentOptions};

use super::ui_state::UiState;

#[derive(Clone)]
pub struct MessageBus(Sender<Message>);

impl MessageBus {
    pub fn new() -> Self {
        let (sender, _) = tokio::sync::broadcast::channel(100);
        Self(sender)
    }

    pub fn send(&self, message: Message) {
        if let Err(message) = self.0.send(message) {
            error!("Message not sent {:?}", message);
        }
    }

    pub fn subscribe(&self) -> MessageBusSubscription {
        MessageBusSubscription(self.0.subscribe())
    }
}

pub struct MessageBusSubscription(Receiver<Message>);

impl MessageBusSubscription {
    pub fn stream(self) -> impl Stream<Item = Result<Message>> {
        BroadcastStream::new(self.0).map(|r| r.map_err(anyhow::Error::from))
    }

    pub fn ui_update_stream(self) -> impl Stream<Item = Result<UiUpdate>> {
        self.stream().filter_map(|r| async move {
            match r {
                Ok(Message::UiUpdate(ui_update)) => Some(Ok(ui_update)),
                Err(e) => Some(Err(e)),
                _ => None,
            }
        })
    }

    pub fn event_stream(self) -> impl Stream<Item = Result<Event>> {
        self.stream().filter_map(|r| async move {
            match r {
                Ok(Message::Event(event)) => Some(Ok(event)),
                Err(e) => Some(Err(e)),
                _ => None,
            }
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Message {
    UiUpdate(UiUpdate),
    UiCommand(UiCommand),
    Event(Event),
}

impl From<UiState> for Message {
    fn from(value: UiState) -> Self {
        Message::UiUpdate(UiUpdate::State(value))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UiUpdate {
    State(UiState),
    DiscordInfo(Vec<DiscordPlayerInfo>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchStart {
    pub game: GameId,
    // Sending full player db seems wasteful though any optimization here seems more wasteful
    pub player_db: PlayerDb,
    pub left_team: MatchStartTeam,
    pub right_team: MatchStartTeam,
    /// Missing when sent by an eloelo older than the bot.
    #[serde(default)]
    pub hero_assignment: HeroAssignmentOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchStartTeam {
    // TODO(j): since we have full playerdb in MatchStart, maybe we shouldn't send elo here?
    pub players: HashMap<PlayerId, i32>,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RichMatchResult {
    pub winner_team_name: String,
    pub duration: Duration,
    pub scale: WinScale,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeroesAssigned {
    pub match_start: MatchStart,
    pub assignments: HashMap<DiscordUsername, Vec<Hero>>,
    /// Tags assigned to players. Empty if hero assignment strategy doesn't use tags.
    pub tags: HashMap<DiscordUsername, HeroTag>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvatarUrl(String);

impl From<String> for AvatarUrl {
    fn from(value: String) -> Self {
        AvatarUrl(value)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscordPlayerInfo {
    pub id: PlayerId,
    pub display_name: String,
    pub username: DiscordUsername,
    pub avatar_url: AvatarUrl,
}

/// Discord bot status as seen by eloelo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "status")]
pub enum DiscordBotStatus {
    Embedded,
    Disabled,
    /// Remote bot, bridge connected.
    Connected,
    /// Remote bot, bridge not connected.
    #[serde(rename_all = "camelCase")]
    Disconnected {
        since: DateTime<Local>,
        error: String,
    },
}

#[derive(Debug, Clone, Copy)]
pub enum ImageFormat {
    Jpg,
    Bmp,
    Png,
}

impl ImageFormat {
    pub fn from_mime_type(mime: &str) -> Option<Self> {
        match mime {
            "image/jpg" => Some(ImageFormat::Jpg),
            "image/png" => Some(ImageFormat::Png),
            "image/bmp" => Some(ImageFormat::Bmp),
            _ => None,
        }
    }

    pub fn to_str(&self) -> &str {
        match self {
            ImageFormat::Jpg => "jpg",
            ImageFormat::Bmp => "bmp",
            ImageFormat::Png => "png",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    MatchStart(MatchStart),
    HeroesAssigned(HeroesAssigned),
    RichMatchResult(RichMatchResult),
    #[serde(skip)]
    DotaScreenshotReceived(Bytes, Option<ImageFormat>),
    DiscordBotStatus(DiscordBotStatus),
    /// Test mode of the eloelo connected to the bot server. Published on the bot server's bus by
    /// the bridge when eloelo connects.
    TestMode(bool),
    /// eloelo's players, sent whenever the Discord bot becomes reachable and when players change.
    /// The bot keeps the latest copy, so it can map Discord usernames to players between matches.
    PlayerDbUpdated(PlayerDb),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum UiCommand {
    InitializeUi,
    AddNewPlayer(Player),
    RemovePlayer(PlayerId),
    MovePlayerToOtherTeam(PlayerId),
    RemovePlayerFromTeam(PlayerId),
    AddPlayerToTeam(PlayerId, Team),
    AddPlayerToLobby(PlayerId),
    RemovePlayerFromLobby(PlayerId),
    ChangeGame(GameId),
    FillLobby,
    ClearLobby,
    CallToLobby,
    CallPlayer(PlayerId),
    StartMatch,
    ShuffleTeams,
    RefreshElo,
    FinishMatch(FinishMatch),
    AddLobbyScreenshotData(Vec<String>),
    UpdateOptions(EloEloOptions),
    SetShuffleTemperature(i32),
    CloseApplication,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum FinishMatch {
    Cancelled,
    Finished(MatchInfo),
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct MatchInfo {
    pub winner: Team,
    pub scale: WinScale,
    pub duration: Duration,
    pub fake: bool,
}
