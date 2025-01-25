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
    skip_separator: bool,
}

pub async fn call_to_lobby<'a>(
    fosiaudio_host: &str,
    players_missing_from_lobby: impl Iterator<Item = &'a Player>,
    timeout: Duration,
    short: bool,
) -> Result<()> {
    let url = format!("http://{fosiaudio_host}/autogrzybke");
    let player_names: Vec<_> = players_missing_from_lobby
        .map(|p| p.get_fosiaudio_name())
        .collect();
    debug!("Calling {} to lobby", player_names.join(", "));
    // let fields = &[("missing", &player_names.join("\n"))];
    let _ = Client::new()
        .post(url)
        // .body(serde_urlencoded::to_string(fields)?)
        .body(serde_urlencoded::to_string(AutogrzybkeRequest {
            missing: player_names.join("\n"),
            skip_lobby: short,
            skip_prefix: false,
            skip_suffix: false,
            skip_separator: false,
        })?)
        .timeout(timeout)
        .send()
        .await?;
    debug!("Call to lobby sent successfully");
    Ok(())
}
