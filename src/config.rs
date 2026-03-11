use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use tracing::{error, warn};

use crate::utils::remove_comments;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub route: String,
    pub ssl: bool,
    pub max_msg_size: u32,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_yaml::Value>,
}

#[derive(Deserialize)]
struct RootConfig {
    websocket: Option<WebSocketConfig>,
}

#[derive(Deserialize)]
struct WebSocketConfig {
    host: Option<String>,
    port: Option<u16>,
    route: Option<String>,
    ssl: Option<bool>,
    max_msg_size: Option<u32>,
    #[serde(flatten)]
    extra: HashMap<String, serde_yaml::Value>,
}

impl Config {
    pub fn new() -> Self {
        // Default configuration
        let mut config = Config {
            host: "127.0.0.1".to_string(),
            port: 8181,
            route: "/core".to_string(),
            ssl: false,
            max_msg_size: 25,
            extra: HashMap::new(),
        };

        // Load configuration from file if OVOS_BUS_CONFIG_FILE is set
        if let Ok(config_file) = env::var("OVOS_BUS_CONFIG_FILE") {
            if let Ok(contents) = fs::read_to_string(config_file) {
                config = Self::parse_config(&contents, config);
            } else {
                warn!("Failed to read config file. Using defaults.");
            }
        }

        // Override with environment variables if set
        if let Ok(host) = env::var("OVOS_BUS_HOST") {
            config.host = host;
        }
        if let Ok(port) = env::var("OVOS_BUS_PORT") {
            if let Ok(port) = port.parse() {
                config.port = port;
            }
        }
        if let Ok(max_msg_size) = env::var("OVOS_BUS_MAX_MSG_SIZE") {
            if let Ok(size) = max_msg_size.parse() {
                config.max_msg_size = size;
            }
        }
        if let Ok(route) = env::var("OVOS_BUS_ROUTE") {
            config.route = route;
        }
        if env::var("OVOS_BUS_USE_SSL").is_ok() {
            config.ssl = true;
        }

        config
    }

    fn parse_config(contents: &str, config: Config) -> Config {
        match serde_yaml::from_str::<RootConfig>(contents) {
            Ok(root_config) => Self::apply_config(root_config, config),
            Err(_) => {
                // If parsing fails, try removing comments and parse again
                let cleaned_contents = remove_comments(contents);
                match serde_yaml::from_str::<RootConfig>(&cleaned_contents) {
                    Ok(root_config) => Self::apply_config(root_config, config),
                    Err(e) => {
                        error!("Failed to parse config file even after removing comments: {}. Using defaults.", e);
                        config
                    }
                }
            }
        }
    }

    fn apply_config(root_config: RootConfig, mut config: Config) -> Config {
        if let Some(websocket_config) = root_config.websocket {
            config.host = websocket_config.host.unwrap_or(config.host);
            config.port = websocket_config.port.unwrap_or(config.port);
            config.route = websocket_config.route.unwrap_or(config.route);
            config.ssl = websocket_config.ssl.unwrap_or(config.ssl);
            config.max_msg_size = websocket_config.max_msg_size.unwrap_or(config.max_msg_size);
            config.extra = websocket_config.extra;
        }
        config
    }
}

#[cfg(test)]
mod tests {
    use crate::config::env;
    use crate::Config;
    use std::path::PathBuf;

    use serial_test::serial;

    fn setup_default_config_environment() {
        env::remove_var("OVOS_BUS_CONFIG_FILE");
        env::remove_var("OVOS_BUS_PORT");
        env::remove_var("OVOS_BUS_HOST");
        env::remove_var("OVOS_BUS_ROUTE");
        env::remove_var("OVOS_BUS_USE_SSL");
        env::remove_var("OVOS_BUS_MAX_MSG_SIZE");
    }

    #[serial]
    #[test]
    fn test_default_config() {
        setup_default_config_environment();
        let test_conf = Config::new();
        assert_eq!(test_conf.host, "127.0.0.1".to_string());
        assert_eq!(test_conf.port, 8181);
        assert_eq!(test_conf.route, "/core".to_string());
        assert_eq!(test_conf.max_msg_size, 25);
        assert!(!test_conf.ssl);
    }

    #[serial]
    #[test]
    fn test_env_overrides() {
        setup_default_config_environment();
        env::set_var("OVOS_BUS_PORT", "1337");
        env::set_var("OVOS_BUS_HOST", "battle.net");
        env::set_var("OVOS_BUS_MAX_MSG_SIZE", "42");
        env::set_var("OVOS_BUS_ROUTE", "/modermodemet");
        env::set_var("OVOS_BUS_USE_SSL", "true");

        let test_conf = Config::new();
        assert_eq!(test_conf.port, 1337);
        assert_eq!(test_conf.host, "battle.net".to_string());
        assert_eq!(test_conf.max_msg_size, 42);
        assert_eq!(test_conf.route, "/modermodemet");
        assert!(test_conf.ssl);
    }

    fn setup_test_config() {
        let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        d.push("resources/test/test_config.json");
        env::set_var("OVOS_BUS_CONFIG_FILE", d);
        println!("config is {}", env::var("OVOS_BUS_CONFIG_FILE").unwrap());
    }

    #[serial]
    #[test]
    fn test_config_file() {
        setup_default_config_environment();
        setup_test_config();

        let test_conf = Config::new();

        assert_eq!(test_conf.port, 847);
        assert_eq!(test_conf.host, "openvoiceos.org".to_string());
        assert_eq!(test_conf.max_msg_size, 64);
    }
}
