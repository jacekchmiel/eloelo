use std::time::Duration;

use anyhow::Result;
use eloelo_model::player::Player;
use log::debug;
use reqwest::Client;
use serde::Serialize;

pub async fn call_missing_players<'a>(
    fosiaudio_host: &str,
    players_missing_from_lobby: impl Iterator<Item = &'a Player>,
    timeout: Duration,
) -> Result<()> {
    call_autogrzybke(fosiaudio_host, players_missing_from_lobby, timeout, false).await
}

pub async fn call_single_player<'a>(
    fosiaudio_host: &str,
    player: &'a Player,
    timeout: Duration,
) -> Result<()> {
    call_autogrzybke(fosiaudio_host, [player].into_iter(), timeout, true).await
}

pub async fn announce_winner(
    fosiaudio_host: &str,
    winner_team_name: &str,
    timeout: Duration,
) -> Result<()> {
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
    send_autogrzybke_request(
        fosiaudio_host,
        AutogrzybkeRequest {
            missing: winner.to_string(),
            skip_lobby: true,
            skip_prefix: true,
            skip_suffix: true,
            skip_interlude: true,
        },
        timeout,
    )
    .await?;
    Ok(())
}

#[derive(Serialize)]
struct AutogrzybkeRequest {
    missing: String,
    skip_lobby: bool,
    skip_prefix: bool,
    skip_suffix: bool,
    skip_interlude: bool,
}

async fn send_autogrzybke_request<'a>(
    fosiaudio_host: &str,
    request: AutogrzybkeRequest,
    timeout: Duration,
) -> Result<()> {
    let url = format!("http://{fosiaudio_host}/autogrzybke");
    let _ = Client::new()
        .post(url)
        // .body(serde_urlencoded::to_string(fields)?)
        .body(serde_urlencoded::to_string(request)?)
        .timeout(timeout)
        .send()
        .await?;
    Ok(())
}

async fn call_autogrzybke<'a>(
    fosiaudio_host: &str,
    players_missing_from_lobby: impl Iterator<Item = &'a Player>,
    timeout: Duration,
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
    send_autogrzybke_request(fosiaudio_host, req, timeout).await?;
    debug!("Call to lobby sent successfully");
    Ok(())
}
