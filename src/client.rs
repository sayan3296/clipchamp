use crate::clipboard;
use crate::config::Config;
use crate::discovery;
use crate::protocol::{Message, MessageCodec, PROTOCOL_VERSION, content_hash};
use anyhow::Result;
use futures::{SinkExt, StreamExt};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio_util::codec::Framed;
use uuid::Uuid;

pub async fn run(cfg: Config, server_override: Option<String>) -> Result<()> {
    let server_addr_cfg = server_override.unwrap_or(cfg.client.server.clone());
    let client_id = Uuid::new_v4();
    let max_size = cfg.clipboard.max_size_mb * 1024 * 1024;
    let max_frame_size = cfg.protocol.max_frame_size_mb * 1024 * 1024;
    let poll_interval_ms = cfg.clipboard.poll_interval_ms;

    tracing::info!("client id: {client_id}");

    let mut backoff = Duration::from_secs(1);
    let max_backoff = Duration::from_secs(30);

    loop {
        let server_addr = resolve_server(&server_addr_cfg).await?;
        tracing::info!("connecting to {server_addr}...");

        match TcpStream::connect(&server_addr).await {
            Ok(stream) => {
                backoff = Duration::from_secs(1);
                tracing::info!("connected to {server_addr}");

                if let Err(e) = run_session(
                    stream,
                    client_id,
                    max_size,
                    max_frame_size,
                    poll_interval_ms,
                )
                .await
                {
                    tracing::warn!("session ended: {e}");
                }
            }
            Err(e) => {
                tracing::warn!("connection failed: {e}");
            }
        }

        tracing::info!("reconnecting in {}s...", backoff.as_secs());
        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(max_backoff);
    }
}

async fn resolve_server(addr: &str) -> Result<String> {
    if addr == "auto" {
        tracing::info!("discovering server via mDNS...");
        let (host, port) = discovery::discover().await?;
        let resolved = format!("{host}:{port}");
        tracing::info!("discovered server at {resolved}");
        Ok(resolved)
    } else {
        Ok(addr.to_string())
    }
}

async fn run_session(
    stream: TcpStream,
    client_id: Uuid,
    max_size: usize,
    max_frame_size: usize,
    poll_interval_ms: u64,
) -> Result<()> {
    let mut framed = Framed::new(stream, MessageCodec::new(max_frame_size));

    // Handshake
    framed
        .send(Message::Hello {
            client_id,
            protocol_version: PROTOCOL_VERSION,
        })
        .await?;

    match framed.next().await {
        Some(Ok(Message::Welcome { client_id: id })) => {
            tracing::info!("handshake complete, server confirmed id: {id}");
        }
        Some(Ok(msg)) => anyhow::bail!("expected Welcome, got {msg:?}"),
        Some(Err(e)) => return Err(e),
        None => anyhow::bail!("connection closed during handshake"),
    }

    let (mut sink, mut stream) = framed.split();

    let cb = clipboard::create_clipboard(poll_interval_ms)?;
    let echo_hash = cb.echo_hash.clone();
    let mut changes_rx = cb.changes_rx;
    let running = Arc::new(AtomicBool::new(true));

    // Clipboard watcher → send updates to server
    let running_w = running.clone();
    let (outgoing_tx, mut outgoing_rx) = tokio::sync::mpsc::channel::<Message>(64);

    let clipboard_tx = outgoing_tx.clone();
    tokio::spawn(async move {
        while running_w.load(Ordering::Relaxed) {
            match changes_rx.recv().await {
                Some(content) => {
                    if content.data.len() > max_size {
                        tracing::debug!("skipping oversized clipboard: {} bytes", content.data.len());
                        continue;
                    }
                    let hash = content_hash(&content.data);
                    tracing::info!(
                        "local clipboard: {} ({} bytes)",
                        clipboard::preview_content(&content, 50),
                        content.data.len()
                    );
                    let msg = Message::ClipboardUpdate {
                        content_type: content.content_type,
                        data: content.data,
                        hash,
                        origin_client: client_id,
                    };
                    if clipboard_tx.send(msg).await.is_err() {
                        break;
                    }
                }
                None => break,
            }
        }
    });

    // Writer: drain outgoing channel to TCP
    let running_wr = running.clone();
    let write_handle = tokio::spawn(async move {
        while running_wr.load(Ordering::Relaxed) {
            match outgoing_rx.recv().await {
                Some(msg) => {
                    if sink.send(msg).await.is_err() {
                        break;
                    }
                }
                None => break,
            }
        }
    });

    // Write-only clipboard handle sharing the same echo_hash as the watcher
    let mut write_cb = clipboard::create_writer(echo_hash)?;

    // Reader: process incoming messages from server
    while let Some(result) = stream.next().await {
        let msg = match result {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!("read error: {e}");
                break;
            }
        };

        match msg {
            Message::ClipboardUpdate {
                content_type,
                data,
                origin_client,
                ..
            } => {
                if origin_client == client_id {
                    continue;
                }
                tracing::info!(
                    "remote clipboard from {origin_client}: {:?} ({} bytes)",
                    content_type,
                    data.len()
                );
                let content = crate::protocol::ClipboardContent { content_type, data };
                if let Err(e) = write_cb.write(&content) {

                    tracing::warn!("failed to write clipboard: {e}");
                }
            }
            Message::Ping => {
                if outgoing_tx.send(Message::Pong).await.is_err() {
                    break;
                }
            }
            Message::HistoryResponse { entries } => {
                for entry in &entries {
                    println!(
                        "{:>3}. [{:?}] {} ({} bytes) from {}",
                        entry.index,
                        entry.content_type,
                        entry.preview,
                        entry.size_bytes,
                        entry.origin_client,
                    );
                }
            }
            Message::HistoryContent { content_type, data } => {
                let content = crate::protocol::ClipboardContent { content_type, data };
                if let Err(e) = write_cb.write(&content) {

                    tracing::warn!("failed to write history entry to clipboard: {e}");
                } else {
                    tracing::info!("history entry written to clipboard");
                }
            }
            Message::HistoryClearAck => {
                tracing::info!("history cleared");
            }
            _ => {
                tracing::debug!("unexpected message: {msg:?}");
            }
        }
    }

    running.store(false, Ordering::Relaxed);
    write_handle.abort();

    Ok(())
}
