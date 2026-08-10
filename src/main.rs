mod clipboard;
mod client;
mod config;
mod discovery;
mod history;
mod protocol;
mod server;
#[cfg(feature = "gui")]
mod tray;

use clap::{Parser, Subcommand};
use std::path::Path;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[derive(Parser)]
#[command(name = "clipchamp", about = "Network clipboard sync")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run the clipboard sync server (hub)
    Server {
        /// Bind address (overrides config)
        #[arg(long)]
        bind: Option<String>,
        /// Disable mDNS advertisement
        #[arg(long)]
        no_mdns: bool,
    },
    /// Run the clipboard sync client
    Client {
        /// Server address (overrides config, use "auto" for mDNS discovery)
        #[arg(long)]
        server: Option<String>,
    },
    /// View or manage clipboard history
    History {
        #[command(subcommand)]
        action: Option<HistoryAction>,
    },
    /// Run with system tray icon
    #[cfg(feature = "gui")]
    Tray {
        /// Run as server or client
        #[arg(long)]
        mode: TrayMode,
        /// Bind address (server mode, overrides config)
        #[arg(long)]
        bind: Option<String>,
        /// Disable mDNS advertisement (server mode)
        #[arg(long)]
        no_mdns: bool,
        /// Server address (client mode, overrides config)
        #[arg(long)]
        server: Option<String>,
    },
}

#[cfg(feature = "gui")]
#[derive(Clone, clap::ValueEnum)]
pub enum TrayMode {
    Server,
    Client,
}

#[derive(Subcommand)]
enum HistoryAction {
    /// Copy a history entry to the local clipboard
    Get {
        /// Entry index (1-based, most recent first)
        index: usize,
    },
    /// Clear all history
    Clear,
}

fn init_tracing(logging: &config::LoggingConfig) -> anyhow::Result<()> {
    let valid_levels = ["trace", "debug", "info", "warn", "error"];
    let level = if valid_levels.contains(&logging.level.to_lowercase().as_str()) {
        logging.level.to_lowercase()
    } else {
        "info".to_string()
    };

    let level_directive = format!("clipchamp={level}");
    let env_filter = EnvFilter::from_default_env().add_directive(level_directive.parse()?);

    let stdout_layer = fmt::layer();

    let file_layer = logging.file.as_ref().and_then(|log_path| {
        if let Some(parent) = log_path.parent() {
            if !parent.as_os_str().is_empty() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    eprintln!(
                        "Warning: could not create log directory {}: {}\n  \
                         To fix: sudo mkdir -p {} && sudo chmod 1777 {}\n  \
                         File logging disabled.",
                        parent.display(),
                        e,
                        parent.display(),
                        parent.display()
                    );
                    return None;
                }
            }
        }

        let dir = log_path.parent().unwrap_or(Path::new("."));
        let filename = log_path
            .file_name()
            .unwrap_or(std::ffi::OsStr::new("clipchamp.log"));

        let file_appender = tracing_appender::rolling::never(dir, filename);
        Some(fmt::layer().with_writer(file_appender).with_ansi(false))
    });

    tracing_subscriber::registry()
        .with(env_filter)
        .with(stdout_layer)
        .with(file_layer)
        .init();

    Ok(())
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let cfg = config::Config::load()?;
    init_tracing(&cfg.logging)?;
    cfg.validate();

    let rt = tokio::runtime::Runtime::new()?;

    match cli.command {
        Command::Server { bind, no_mdns } => {
            rt.block_on(server::run(cfg, bind, no_mdns))?;
        }
        Command::Client { server: server_addr } => {
            rt.block_on(client::run(cfg, server_addr))?;
        }
        Command::History { action } => {
            rt.block_on(history::cli::run(cfg, action))?;
        }
        #[cfg(feature = "gui")]
        Command::Tray {
            mode,
            bind,
            no_mdns,
            server,
        } => {
            tray::run(rt, cfg, mode, bind, no_mdns, server)?;
        }
    }

    Ok(())
}
