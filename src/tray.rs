use crate::config::Config;
use crate::TrayMode;
use anyhow::Result;
use muda::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{Icon, TrayIconBuilder};
use tokio::runtime::Runtime;

const ICON_BYTES: &[u8] = include_bytes!("../assets/icon.png");

pub fn run(
    rt: Runtime,
    cfg: Config,
    mode: TrayMode,
    bind: Option<String>,
    no_mdns: bool,
    server: Option<String>,
) -> Result<()> {
    gtk::init()?;

    let bind_addr = bind.clone().unwrap_or(cfg.server.bind.clone());
    let mdns_enabled = !no_mdns && cfg.server.mdns;
    let server_addr = server.clone().unwrap_or(cfg.client.server.clone());
    let log_level = cfg.logging.level.clone();
    let log_file = cfg.logging.file.clone();
    let max_size = cfg.clipboard.max_size_mb;
    let max_frame = cfg.protocol.max_frame_size_mb;
    let config_path = Config::config_path();

    let status_item = MenuItem::new("Status: Starting...", false, None);
    let mode_label = match mode {
        TrayMode::Server => "Mode: Server",
        TrayMode::Client => "Mode: Client",
    };
    let mode_item = MenuItem::new(mode_label, false, None);

    let config_submenu = Submenu::new("Configuration", true);
    match mode {
        TrayMode::Server => {
            config_submenu.append(&MenuItem::new(format!("Bind: {bind_addr}"), false, None))?;
        }
        TrayMode::Client => {
            config_submenu.append(&MenuItem::new(format!("Server: {server_addr}"), false, None))?;
        }
    }
    let mdns_label = if mdns_enabled { "mDNS: enabled" } else { "mDNS: disabled" };
    config_submenu.append(&MenuItem::new(mdns_label, false, None))?;
    config_submenu.append(&MenuItem::new(format!("Log Level: {log_level}"), false, None))?;
    config_submenu.append(&MenuItem::new(format!("Max Clipboard: {max_size} MB"), false, None))?;
    config_submenu.append(&MenuItem::new(format!("Max Frame: {max_frame} MB"), false, None))?;

    let edit_config_item = MenuItem::new("Edit Config", true, None);
    let has_log_file = log_file.is_some();
    let open_log_item = MenuItem::new("Monitor Log", has_log_file, None);
    let quit_item = MenuItem::new("Quit", true, None);

    let menu = Menu::new();
    menu.append(&status_item)?;
    menu.append(&mode_item)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&config_submenu)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&edit_config_item)?;
    menu.append(&open_log_item)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&quit_item)?;

    let icon = load_icon()?;

    let _tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("clipchamp")
        .with_icon(icon)
        .build()?;

    let handle = match mode {
        TrayMode::Server => rt.spawn(async move {
            if let Err(e) = crate::server::run(cfg, bind, no_mdns).await {
                tracing::error!("server failed: {e}");
            }
        }),
        TrayMode::Client => rt.spawn(async move {
            if let Err(e) = crate::client::run(cfg, server).await {
                tracing::error!("client failed: {e}");
            }
        }),
    };

    status_item.set_text("Status: Running");

    let menu_channel = MenuEvent::receiver();
    let quit_id = quit_item.id().clone();
    let edit_config_id = edit_config_item.id().clone();
    let open_log_id = open_log_item.id().clone();

    loop {
        while gtk::events_pending() {
            gtk::main_iteration_do(false);
        }

        if let Ok(event) = menu_channel.try_recv() {
            if event.id() == &quit_id {
                tracing::info!("quit requested from tray");
                break;
            } else if event.id() == &edit_config_id {
                if let Err(e) = open::that(&config_path) {
                    tracing::error!("failed to open config: {e}");
                }
            } else if event.id() == &open_log_id
                && let Some(ref path) = log_file
                && let Err(e) = open_log_in_terminal(path)
            {
                tracing::error!("failed to open log monitor: {e}");
            }
        }

        if handle.is_finished() {
            tracing::warn!("background task exited unexpectedly");
            status_item.set_text("Status: Stopped");
            break;
        }

        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    handle.abort();
    rt.shutdown_timeout(std::time::Duration::from_secs(5));

    Ok(())
}

fn open_log_in_terminal(path: &std::path::Path) -> Result<()> {
    let path_str = path.to_string_lossy();
    let tail_cmd = format!("tail -f {path_str}");

    #[cfg(target_os = "linux")]
    {
        let terminals = ["gnome-terminal", "konsole", "xfce4-terminal", "xterm"];
        for term in &terminals {
            let result = match *term {
                "gnome-terminal" => std::process::Command::new(term)
                    .args(["--", "sh", "-c", &tail_cmd])
                    .spawn(),
                "konsole" | "xfce4-terminal" => std::process::Command::new(term)
                    .args(["-e", &tail_cmd])
                    .spawn(),
                _ => std::process::Command::new(term)
                    .args(["-e", &tail_cmd])
                    .spawn(),
            };
            if result.is_ok() {
                return Ok(());
            }
        }
        anyhow::bail!("no supported terminal emulator found");
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args(["-a", "Terminal.app", "--args", "-e", &format!("sh -c '{tail_cmd}'")])
            .spawn()?;
        Ok(())
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/c", "start", "cmd", "/k", &format!("powershell Get-Content -Path '{}' -Wait", path_str)])
            .spawn()?;
        Ok(())
    }
}

fn load_icon() -> Result<Icon> {
    let img = image::load_from_memory(ICON_BYTES)?;
    let rgba = img.to_rgba8();
    let (w, h) = (rgba.width(), rgba.height());
    let icon = Icon::from_rgba(rgba.into_raw(), w, h)?;
    Ok(icon)
}
