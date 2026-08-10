use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "ServerConfig::default")]
    pub server: ServerConfig,
    #[serde(default = "ClientConfig::default")]
    pub client: ClientConfig,
    #[serde(default)]
    pub tls: TlsConfig,
    #[serde(default = "HistoryConfig::default")]
    pub history: HistoryConfig,
    #[serde(default = "ClipboardConfig::default")]
    pub clipboard: ClipboardConfig,
    #[serde(default = "ProtocolConfig::default")]
    pub protocol: ProtocolConfig,
    #[serde(default = "LoggingConfig::default")]
    pub logging: LoggingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_bind")]
    pub bind: String,
    #[serde(default = "default_true")]
    pub mdns: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    #[serde(default = "default_server")]
    pub server: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TlsConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub cert: String,
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub ca: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryConfig {
    #[serde(default = "default_max_entries")]
    pub max_entries: usize,
    #[serde(default = "default_true")]
    pub persist: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardConfig {
    #[serde(default = "default_max_size_mb")]
    pub max_size_mb: usize,
    #[serde(default = "default_poll_interval_ms")]
    pub poll_interval_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolConfig {
    #[serde(default = "default_max_frame_size_mb")]
    pub max_frame_size_mb: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    #[serde(default = "default_log_file")]
    pub file: Option<PathBuf>,
    #[serde(default = "default_log_level")]
    pub level: String,
}

fn default_bind() -> String {
    "0.0.0.0:9090".to_string()
}
fn default_server() -> String {
    "auto".to_string()
}
fn default_true() -> bool {
    true
}
fn default_max_entries() -> usize {
    100
}
fn default_max_size_mb() -> usize {
    10
}
fn default_poll_interval_ms() -> u64 {
    500
}
fn default_max_frame_size_mb() -> usize {
    16
}
fn default_log_file() -> Option<PathBuf> {
    Some(PathBuf::from("/var/log/clipchamp/clipchamp.log"))
}
fn default_log_level() -> String {
    "info".to_string()
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: default_bind(),
            mdns: true,
        }
    }
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            server: default_server(),
        }
    }
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            max_entries: default_max_entries(),
            persist: true,
        }
    }
}

impl Default for ClipboardConfig {
    fn default() -> Self {
        Self {
            max_size_mb: default_max_size_mb(),
            poll_interval_ms: default_poll_interval_ms(),
        }
    }
}

impl Default for ProtocolConfig {
    fn default() -> Self {
        Self {
            max_frame_size_mb: default_max_frame_size_mb(),
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            file: default_log_file(),
            level: default_log_level(),
        }
    }
}

impl Config {
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("clipchamp")
            .join("config.toml")
    }

    pub fn data_dir() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("clipchamp")
    }

    pub fn validate(&self) {
        if self.clipboard.max_size_mb > self.protocol.max_frame_size_mb {
            tracing::warn!(
                "clipboard.max_size_mb ({}) exceeds protocol.max_frame_size_mb ({}); \
                 large clipboard entries will be rejected at the protocol layer",
                self.clipboard.max_size_mb,
                self.protocol.max_frame_size_mb,
            );
        }
        let valid_levels = ["trace", "debug", "info", "warn", "error"];
        if !valid_levels.contains(&self.logging.level.to_lowercase().as_str()) {
            tracing::warn!(
                "logging.level '{}' is not valid (expected one of: {}); falling back to 'info'",
                self.logging.level,
                valid_levels.join(", "),
            );
        }
    }

    pub fn load() -> anyhow::Result<Self> {
        let path = Self::config_path();
        let config = if path.exists() {
            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("reading config from {}", path.display()))?;
            toml::from_str(&content)
                .with_context(|| format!("parsing config from {}", path.display()))?
        } else {
            let config = Config::default();
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let content = toml::to_string_pretty(&config)?;
            std::fs::write(&path, &content)
                .with_context(|| format!("writing default config to {}", path.display()))?;
            tracing::info!("wrote default config to {}", path.display());
            config
        };
        Ok(config)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            client: ClientConfig::default(),
            tls: TlsConfig::default(),
            history: HistoryConfig::default(),
            clipboard: ClipboardConfig::default(),
            protocol: ProtocolConfig::default(),
            logging: LoggingConfig::default(),
        }
    }
}
