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
    #[serde(default = "default_message_buffer")]
    pub message_buffer_capacity: usize,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_yaml::Value>,
}

fn default_message_buffer() -> usize {
    1024
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
    message_buffer_capacity: Option<usize>,
    #[serde(flatten)]
    extra: HashMap<String, serde_yaml::Value>,
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
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
            message_buffer_capacity: default_message_buffer(),
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
        if let Ok(buf_cap) = env::var("OVOS_BUS_MSG_BUFFER_CAPACITY") {
            if let Ok(cap) = buf_cap.parse() {
                config.message_buffer_capacity = cap;
            }
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
            config.message_buffer_capacity = websocket_config
                .message_buffer_capacity
                .unwrap_or(config.message_buffer_capacity);
            config.extra = websocket_config.extra;
        }
        config
    }
}

#[cfg(test)]
mod tests {
    use crate::Config;
    use std::env;
    use std::fs;
    use std::path::PathBuf;

    use serial_test::serial;

    fn setup_default_config_environment() {
        env::remove_var("OVOS_BUS_CONFIG_FILE");
        env::remove_var("OVOS_BUS_PORT");
        env::remove_var("OVOS_BUS_HOST");
        env::remove_var("OVOS_BUS_ROUTE");
        env::remove_var("OVOS_BUS_USE_SSL");
        env::remove_var("OVOS_BUS_MAX_MSG_SIZE");
        env::remove_var("OVOS_BUS_MSG_BUFFER_CAPACITY");
    }

    fn write_temp_config(name: &str, contents: &str) -> PathBuf {
        let mut path = env::temp_dir();
        path.push(format!("ovos_messagebus_test_{name}"));
        fs::write(&path, contents).expect("failed to write temp config");
        path
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
        assert_eq!(test_conf.message_buffer_capacity, 1024);
        assert!(!test_conf.ssl);
        assert!(test_conf.extra.is_empty());
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
        env::set_var("OVOS_BUS_MSG_BUFFER_CAPACITY", "4096");

        let test_conf = Config::new();
        assert_eq!(test_conf.port, 1337);
        assert_eq!(test_conf.host, "battle.net".to_string());
        assert_eq!(test_conf.max_msg_size, 42);
        assert_eq!(test_conf.route, "/modermodemet");
        assert_eq!(test_conf.message_buffer_capacity, 4096);
        assert!(test_conf.ssl);
    }

    #[serial]
    #[test]
    fn test_invalid_env_values_are_ignored() {
        setup_default_config_environment();
        env::set_var("OVOS_BUS_PORT", "not-a-port");
        env::set_var("OVOS_BUS_MAX_MSG_SIZE", "huge");
        env::set_var("OVOS_BUS_MSG_BUFFER_CAPACITY", "-1");

        let test_conf = Config::new();
        assert_eq!(test_conf.port, 8181);
        assert_eq!(test_conf.max_msg_size, 25);
        assert_eq!(test_conf.message_buffer_capacity, 1024);
    }

    fn setup_test_config() {
        let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        d.push("resources/test/test_config.json");
        env::set_var("OVOS_BUS_CONFIG_FILE", d);
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

    #[serial]
    #[test]
    fn test_env_overrides_beat_config_file() {
        setup_default_config_environment();
        setup_test_config();
        env::set_var("OVOS_BUS_PORT", "9999");

        let test_conf = Config::new();

        assert_eq!(test_conf.port, 9999);
        assert_eq!(test_conf.host, "openvoiceos.org".to_string());
    }

    #[serial]
    #[test]
    fn test_missing_config_file_uses_defaults() {
        setup_default_config_environment();
        env::set_var("OVOS_BUS_CONFIG_FILE", "/definitely/not/a/real/file.json");

        let test_conf = Config::new();

        assert_eq!(test_conf.host, "127.0.0.1");
        assert_eq!(test_conf.port, 8181);
    }

    #[serial]
    #[test]
    fn test_malformed_config_file_uses_defaults() {
        setup_default_config_environment();
        let path = write_temp_config("malformed.json", "{{{ this is not valid");
        env::set_var("OVOS_BUS_CONFIG_FILE", &path);

        let test_conf = Config::new();

        assert_eq!(test_conf.host, "127.0.0.1");
        assert_eq!(test_conf.port, 8181);
    }

    #[serial]
    #[test]
    fn test_yaml_config_file() {
        setup_default_config_environment();
        let path = write_temp_config(
            "config.yaml",
            "websocket:\n  host: yaml.example.org\n  port: 4242\n  message_buffer_capacity: 2048\n",
        );
        env::set_var("OVOS_BUS_CONFIG_FILE", &path);

        let test_conf = Config::new();

        assert_eq!(test_conf.host, "yaml.example.org");
        assert_eq!(test_conf.port, 4242);
        assert_eq!(test_conf.message_buffer_capacity, 2048);
    }

    #[serial]
    #[test]
    fn test_partial_config_file_keeps_defaults() {
        setup_default_config_environment();
        let path = write_temp_config("partial.json", r#"{"websocket": {"port": 7777}}"#);
        env::set_var("OVOS_BUS_CONFIG_FILE", &path);

        let test_conf = Config::new();

        assert_eq!(test_conf.port, 7777);
        assert_eq!(test_conf.host, "127.0.0.1");
        assert_eq!(test_conf.route, "/core");
        assert_eq!(test_conf.max_msg_size, 25);
        assert_eq!(test_conf.message_buffer_capacity, 1024);
    }

    #[serial]
    #[test]
    fn test_config_file_without_websocket_section_uses_defaults() {
        setup_default_config_environment();
        let path = write_temp_config("no_websocket.json", r#"{"other_section": {"a": 1}}"#);
        env::set_var("OVOS_BUS_CONFIG_FILE", &path);

        let test_conf = Config::new();

        assert_eq!(test_conf.host, "127.0.0.1");
        assert_eq!(test_conf.port, 8181);
    }

    #[serial]
    #[test]
    fn test_config_file_extra_keys_are_captured() {
        setup_default_config_environment();
        let path = write_temp_config(
            "extra.json",
            r#"{"websocket": {"port": 1234, "shared_connection": true}}"#,
        );
        env::set_var("OVOS_BUS_CONFIG_FILE", &path);

        let test_conf = Config::new();

        assert_eq!(test_conf.port, 1234);
        assert_eq!(
            test_conf.extra.get("shared_connection"),
            Some(&serde_yaml::Value::Bool(true))
        );
    }

    #[serial]
    #[test]
    fn test_config_file_with_comments() {
        setup_default_config_environment();
        let path = write_temp_config(
            "commented.json",
            "// mycroft.conf style comments\n{\"websocket\": {\"port\": 5555}}",
        );
        env::set_var("OVOS_BUS_CONFIG_FILE", &path);

        let test_conf = Config::new();

        assert_eq!(test_conf.port, 5555);
    }
}
