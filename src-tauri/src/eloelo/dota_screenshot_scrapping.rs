use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use anyhow::Result;
use chrono::Utc;
use eloelo_model::history::dota;

pub struct RawDotaMatchDetails {
    players: HashMap<String, dota::PlayerDetails>,
}

pub fn scrap_dota_screenshot_data(path: &Path) -> Result<dota::MatchDetails> {
    Ok(dota::MatchDetails {
        players: HashMap::new(),
        duration: Duration::from_secs(0),
        time: Utc::now(),
    })
}
