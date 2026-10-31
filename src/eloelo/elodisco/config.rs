use std::collections::HashSet;

use eloelo_model::PlayerId;
use serde::{Deserialize, Serialize};

use super::dota_bot::Hero;

/// The Discord bot's config.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EloDiscoConfig {
    #[serde(default)]
    pub discord_server_name: String,

    #[serde(default)]
    pub discord_channel_name: String,

    /// Test mode until an eloelo connects, then the bot follows eloelo's. An embedded bot always
    /// uses eloelo's.
    #[serde(default)]
    pub test_mode: bool,

    #[serde(default)]
    pub discord_test_channel_name: String,

    /// List of usernames that will receive notifications even in test mode.
    #[serde(default)]
    pub discord_test_mode_players: HashSet<PlayerId>,

    #[serde(default)]
    pub global_banned_heroes: HashSet<Hero>,

    /// Players not affected by `global_banned_heroes`.
    #[serde(default)]
    pub unrestricted_players: HashSet<PlayerId>,

    /// Address the Discord bot server (elodisco binary) listens on for eloelo connections.
    #[serde(default = "default_bridge_listen_addr")]
    pub bridge_listen_addr: String,
}

impl Default for EloDiscoConfig {
    fn default() -> Self {
        Self {
            discord_server_name: Default::default(),
            discord_channel_name: Default::default(),
            test_mode: true,
            discord_test_channel_name: Default::default(),
            discord_test_mode_players: Default::default(),
            global_banned_heroes: Default::default(),
            unrestricted_players: Default::default(),
            bridge_listen_addr: default_bridge_listen_addr(),
        }
    }
}

fn default_bridge_listen_addr() -> String {
    "0.0.0.0:3001".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_fields_of_other_configs_and_secrets() {
        let config: EloDiscoConfig = serde_yaml::from_str("discordServerName: server").unwrap();
        assert_eq!(config.discord_server_name, "server");
        for yaml in [
            "discordBotToken: token",
            "bridgeSecret: secret",
            "fosiaudioHost: host",
            "heroAssignmentStrategy: tags",
        ] {
            assert!(
                serde_yaml::from_str::<EloDiscoConfig>(yaml).is_err(),
                "{yaml}"
            );
        }
    }
}
