use anyhow::Result;
use eloelo_model::player::Player;
use log::debug;
use reqwest::blocking::Client;
use serde::Serialize;

pub fn call_to_lobby<'a>(
    fosiaudio_host: &str,
    players_missing_from_lobby: impl Iterator<Item = &'a Player>,
) -> Result<()> {
    let url = format!("http://{fosiaudio_host}/autogrzybke");
    let player_names: Vec<_> = players_missing_from_lobby
        .map(|p| p.get_fosiaudio_name())
        .collect();
    debug!("Calling {} to lobby", player_names.join(", "));
    let fields = &[("missing", &player_names.join("\n"))];
    let _ = Client::new()
        .post(url)
        .body(serde_urlencoded::to_string(fields)?)
        .send()?;
    debug!("Call to lobby sent successfully");
    Ok(())
}
