use crate::clipboard;
use crate::config::Config;
use crate::discovery;
use crate::history::HistoryStore;
use crate::protocol::{ClipboardContent, Message, MessageCodec, PROTOCOL_VERSION, content_hash};
use anyhow::Result;
use futures::SinkExt;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, mpsc};
use tokio_util::codec::Framed;
use uuid::Uuid;

struct ClientHandle {
    tx: mpsc::Sender<Message>,
}

type Clients = Arc<Mutex<HashMap<Uuid, ClientHandle>>>;

pub async fn run(cfg: Config, bind_override: Option<String>, no_mdns: bool) -> Result<()> {
    let bind_addr = bind_override.unwrap_or(cfg.server.bind.clone());
    let listener = TcpListener::bind(&bind_addr).await?;
    tracing::info!("server listening on {bind_addr}");

    let clients: Clients = Arc::new(Mutex::new(HashMap::new()));
    let history = Arc::new(Mutex::new(HistoryStore::new(
        cfg.history.max_entries,
        cfg.history.persist,
    )?));

    let _mdns_guard = if cfg.server.mdns && !no_mdns {
        let port: u16 = bind_addr
            .rsplit(':')
            .next()
            .and_then(|p| p.parse().ok())
            .unwrap_or(9090);
        match discovery::advertise(port) {
            Ok(guard) => {
                tracing::info!("mDNS: advertising _clipchamp._tcp.local on port {port}");
                Some(guard)
            }
            Err(e) => {
                tracing::warn!("mDNS advertisement failed: {e}");
                None
            }
        }
    } else {
        None
    };

    let max_size = cfg.clipboard.max_size_mb * 1024 * 1024;
    let max_frame_size = cfg.protocol.max_frame_size_mb * 1024 * 1024;

    // Channel for client handlers to send clipboard content to the server's clipboard writer
    let (incoming_cb_tx, incoming_cb_rx) = mpsc::channel::<ClipboardContent>(64);

    // Server's own clipboard watcher + writer for incoming updates
    let clipboard_clients = clients.clone();
    let clipboard_history = history.clone();
    let server_id = Uuid::new_v4();
    tokio::spawn(async move {
        if let Err(e) = run_server_clipboard(
            cfg.clipboard.poll_interval_ms,
            clipboard_clients,
            clipboard_history,
            server_id,
            max_size,
            incoming_cb_rx,
        )
        .await
        {
            tracing::error!("server clipboard watcher failed: {e}");
        }
    });

    loop {
        let (stream, addr) = listener.accept().await?;
        tracing::info!("new connection from {addr}");

        let clients = clients.clone();
        let history = history.clone();
        let incoming_cb_tx = incoming_cb_tx.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_client(stream, clients, history, max_size, max_frame_size, incoming_cb_tx).await
            {
                tracing::warn!("client {addr} error: {e}");
            }
        });
    }
}

async fn run_server_clipboard(
    poll_interval_ms: u64,
    clients: Clients,
    history: Arc<Mutex<HistoryStore>>,
    server_id: Uuid,
    max_size: usize,
    mut incoming_rx: mpsc::Receiver<ClipboardContent>,
) -> Result<()> {
    let cb = clipboard::create_clipboard(poll_interval_ms)?;
    let echo_hash = cb.echo_hash.clone();
    let mut changes_rx = cb.changes_rx;

    let mut write_cb = clipboard::create_writer(echo_hash)?;

    loop {
        tokio::select! {
            Some(content) = changes_rx.recv() => {
                if content.data.len() > max_size {
                    tracing::debug!("skipping oversized clipboard content: {} bytes", content.data.len());
                    continue;
                }

                let hash = content_hash(&content.data);
                tracing::info!(
                    "server clipboard: {} ({} bytes)",
                    clipboard::preview_content(&content, 50),
                    content.data.len()
                );

                let msg = Message::ClipboardUpdate {
                    content_type: content.content_type,
                    data: content.data.clone(),
                    hash,
                    origin_client: server_id,
                };

                {
                    let mut hist = history.lock().await;
                    let _ = hist.add(content.content_type, content.data, hash, server_id);
                }

                broadcast(&clients, &msg, Some(server_id)).await;
            }
            Some(content) = incoming_rx.recv() => {
                tracing::info!(
                    "writing remote clipboard to server: {} ({} bytes)",
                    clipboard::preview_content(&content, 50),
                    content.data.len()
                );
                if let Err(e) = write_cb.write(&content) {
                    tracing::warn!("failed to write remote clipboard to server: {e}");
                }
            }
            else => break,
        }
    }

    Ok(())
}

async fn handle_client(
    stream: TcpStream,
    clients: Clients,
    history: Arc<Mutex<HistoryStore>>,
    max_size: usize,
    max_frame_size: usize,
    incoming_cb_tx: mpsc::Sender<ClipboardContent>,
) -> Result<()> {
    use futures::StreamExt;

    let mut framed = Framed::new(stream, MessageCodec::new(max_frame_size));

    // Handshake
    let client_id = match framed.next().await {
        Some(Ok(Message::Hello {
            client_id,
            protocol_version,
        })) => {
            if protocol_version != PROTOCOL_VERSION {
                tracing::warn!(
                    "client {client_id} protocol version mismatch: {protocol_version} != {PROTOCOL_VERSION}"
                );
                anyhow::bail!("protocol version mismatch");
            }
            client_id
        }
        Some(Ok(msg)) => anyhow::bail!("expected Hello, got {msg:?}"),
        Some(Err(e)) => return Err(e),
        None => anyhow::bail!("connection closed before handshake"),
    };

    framed.send(Message::Welcome { client_id }).await?;
    tracing::info!("client {client_id} connected");

    let (msg_tx, mut msg_rx) = mpsc::channel::<Message>(64);

    {
        let mut clients = clients.lock().await;
        clients.insert(client_id, ClientHandle { tx: msg_tx });
    }

    let (mut sink, mut stream) = framed.split();

    // Writer task: forwards messages from the channel to the TCP stream
    let write_handle = tokio::spawn(async move {
        while let Some(msg) = msg_rx.recv().await {
            if sink.send(msg).await.is_err() {
                break;
            }
        }
    });

    // Keepalive
    let clients_ping = clients.clone();
    let ping_id = client_id;
    let keepalive = tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
        loop {
            interval.tick().await;
            let clients = clients_ping.lock().await;
            if let Some(handle) = clients.get(&ping_id) {
                if handle.tx.send(Message::Ping).await.is_err() {
                    break;
                }
            } else {
                break;
            }
        }
    });

    // Reader: process incoming messages from this client
    while let Some(result) = stream.next().await {
        let msg = match result {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!("read error from {client_id}: {e}");
                break;
            }
        };

        match msg {
            Message::ClipboardUpdate {
                content_type,
                data,
                hash,
                origin_client,
            } => {
                if data.len() > max_size {
                    tracing::debug!("dropping oversized update from {client_id}");
                    continue;
                }

                tracing::info!(
                    "clipboard from {}: {:?} ({} bytes)",
                    origin_client,
                    content_type,
                    data.len()
                );

                let broadcast_msg = Message::ClipboardUpdate {
                    content_type,
                    data: data.clone(),
                    hash,
                    origin_client,
                };

                // Write to server's own clipboard
                let content = ClipboardContent {
                    content_type,
                    data: data.clone(),
                };
                let _ = incoming_cb_tx.send(content).await;

                {
                    let mut hist = history.lock().await;
                    hist.add(content_type, data, hash, origin_client)?;
                }

                broadcast(&clients, &broadcast_msg, Some(client_id)).await;
            }
            Message::Pong => {}
            Message::HistoryRequest { count } => {
                let hist = history.lock().await;
                let entries = hist.list(count);
                let clients = clients.lock().await;
                if let Some(handle) = clients.get(&client_id) {
                    let _ = handle
                        .tx
                        .send(Message::HistoryResponse { entries })
                        .await;
                }
            }
            Message::HistoryFetch { hash } => {
                let hist = history.lock().await;
                let clients = clients.lock().await;
                if let Some(handle) = clients.get(&client_id)
                    && let Some(entry) = hist.get_by_hash(&hash)
                {
                    let _ = handle
                        .tx
                        .send(Message::HistoryContent {
                            content_type: entry.content_type,
                            data: entry.data.clone(),
                        })
                        .await;
                }
            }
            Message::HistoryClear => {
                let mut hist = history.lock().await;
                hist.clear()?;
                let clients = clients.lock().await;
                if let Some(handle) = clients.get(&client_id) {
                    let _ = handle.tx.send(Message::HistoryClearAck).await;
                }
            }
            _ => {
                tracing::debug!("unexpected message from {client_id}: {msg:?}");
            }
        }
    }

    // Cleanup
    {
        let mut clients = clients.lock().await;
        clients.remove(&client_id);
    }
    keepalive.abort();
    write_handle.abort();
    tracing::info!("client {client_id} disconnected");

    Ok(())
}

async fn broadcast(clients: &Clients, msg: &Message, exclude: Option<Uuid>) {
    let clients = clients.lock().await;
    for (id, handle) in clients.iter() {
        if Some(*id) == exclude {
            continue;
        }
        if handle.tx.send(msg.clone()).await.is_err() {
            tracing::debug!("failed to send to client {id}");
        }
    }
}
