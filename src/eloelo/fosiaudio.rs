use std::time::Duration;

use anyhow::Result;
use eloelo_model::player::Player;
use log::debug;
use reqwest::Client;
use serde::Serialize;

#[derive(Serialize)]
struct AutogrzybkeRequest {
    missing: String,
    skip_lobby: bool,
    skip_prefix: bool,
    skip_suffix: bool,
    skip_interlude: bool,
}

pub struct FosiaudioClient {
    host: String,
    timeout: Duration,
    enabled: bool,
}

impl FosiaudioClient {
    pub fn new(host: impl Into<String>) -> Self {
        Self {
            host: host.into(),
            timeout: Duration::from_secs(1),
            enabled: true,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub async fn call_missing_players<'a>(
        &self,
        players_missing_from_lobby: impl IntoIterator<Item = &'a Player>,
    ) -> Result<()> {
        self.call_autogrzybke(players_missing_from_lobby.into_iter(), false)
            .await
    }
    pub async fn call_single_player<'a>(&self, player: &'a Player) -> Result<()> {
        self.call_autogrzybke([player].into_iter(), true).await
    }

    pub async fn announce_winner(&self, winner_team_name: &str) -> Result<()> {
        let winner_team_name = winner_team_name.to_lowercase();
        debug!("winner_team_name={winner_team_name}");
        let winner = if winner_team_name.contains("biedronka") {
            Some("biedronka")
        } else if winner_team_name.contains("lidl") {
            Some("lidl")
        } else {
            None
        };

        let Some(winner) = winner else { return Ok(()) };
        self.send_autogrzybke_request(AutogrzybkeRequest {
            missing: winner.to_string(),
            skip_lobby: true,
            skip_prefix: true,
            skip_suffix: true,
            skip_interlude: true,
        })
        .await?;
        Ok(())
    }

    async fn send_autogrzybke_request<'a>(&self, request: AutogrzybkeRequest) -> Result<()> {
        if !self.enabled {
            debug!("Autogrzybke request skipped");
            return Ok(());
        }
        let url = format!("http://{}/autogrzybke", self.host);
        let _ = Client::new()
            .post(url)
            // .body(serde_urlencoded::to_string(fields)?)
            .body(serde_urlencoded::to_string(request)?)
            .timeout(self.timeout)
            .send()
            .await?;
        debug!("Autogrzybke request sent successfully");
        Ok(())
    }

    async fn call_autogrzybke<'a>(
        &self,
        players_missing_from_lobby: impl Iterator<Item = &'a Player>,
        short: bool,
    ) -> Result<()> {
        let player_names: Vec<_> = players_missing_from_lobby
            .map(|p| p.get_fosiaudio_name())
            .collect();
        debug!("Calling {} to lobby", player_names.join(", "));
        let req = AutogrzybkeRequest {
            missing: player_names.join("\n"),
            skip_lobby: short,
            skip_prefix: false,
            skip_suffix: false,
            skip_interlude: false,
        };
        self.send_autogrzybke_request(req).await?;
        Ok(())
    }
}
