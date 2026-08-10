mod clipboard;
mod client;
mod config;
mod discovery;
mod history;
mod protocol;
mod server;

use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("clipchamp=info".parse()?))
        .init();

    let cli = Cli::parse();
    let cfg = config::Config::load()?;

    match cli.command {
        Command::Server { bind, no_mdns } => {
            server::run(cfg, bind, no_mdns).await?;
        }
        Command::Client { server: server_addr } => {
            client::run(cfg, server_addr).await?;
        }
        Command::History { action } => {
            history::cli::run(cfg, action).await?;
        }
    }

    Ok(())
}
