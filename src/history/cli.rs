use crate::config::Config;
use crate::protocol::{Message, MessageCodec, PROTOCOL_VERSION};
use anyhow::Result;
use futures::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio_util::codec::Framed;
use uuid::Uuid;

pub async fn run(cfg: Config, action: Option<crate::HistoryAction>) -> Result<()> {
    let server_addr = if cfg.client.server == "auto" {
        let (host, port) = crate::discovery::discover().await?;
        format!("{host}:{port}")
    } else {
        cfg.client.server.clone()
    };

    let max_frame_size = cfg.protocol.max_frame_size_mb * 1024 * 1024;
    let stream = TcpStream::connect(&server_addr).await?;
    let mut framed = Framed::new(stream, MessageCodec::new(max_frame_size));

    let client_id = Uuid::new_v4();
    framed
        .send(Message::Hello {
            client_id,
            protocol_version: PROTOCOL_VERSION,
        })
        .await?;

    match framed.next().await {
        Some(Ok(Message::Welcome { .. })) => {}
        Some(Ok(msg)) => anyhow::bail!("expected Welcome, got {msg:?}"),
        Some(Err(e)) => return Err(e),
        None => anyhow::bail!("connection closed during handshake"),
    }

    match action {
        None => {
            framed
                .send(Message::HistoryRequest { count: 20 })
                .await?;

            match framed.next().await {
                Some(Ok(Message::HistoryResponse { entries })) => {
                    if entries.is_empty() {
                        println!("No clipboard history.");
                    } else {
                        for entry in &entries {
                            println!(
                                "{:>3}. [{:?}] {} ({} bytes)",
                                entry.index, entry.content_type, entry.preview, entry.size_bytes,
                            );
                        }
                    }
                }
                Some(Ok(msg)) => anyhow::bail!("unexpected response: {msg:?}"),
                Some(Err(e)) => return Err(e),
                None => anyhow::bail!("connection closed"),
            }
        }
        Some(crate::HistoryAction::Get { index }) => {
            framed
                .send(Message::HistoryRequest { count: index })
                .await?;

            match framed.next().await {
                Some(Ok(Message::HistoryResponse { entries })) => {
                    let entry = entries
                        .iter()
                        .find(|e| e.index == index)
                        .ok_or_else(|| anyhow::anyhow!("entry {index} not found"))?;

                    framed.send(Message::HistoryFetch { hash: entry.hash }).await?;

                    match framed.next().await {
                        Some(Ok(Message::HistoryContent { content_type, data })) => {
                            let content = crate::protocol::ClipboardContent { content_type, data };
                            let mut cb = crate::clipboard::create_clipboard(500)?;
                            cb.write(&content)?;
                            println!("Entry {index} copied to clipboard.");
                        }
                        Some(Ok(msg)) => anyhow::bail!("unexpected response: {msg:?}"),
                        Some(Err(e)) => return Err(e),
                        None => anyhow::bail!("connection closed"),
                    }
                }
                Some(Ok(msg)) => anyhow::bail!("unexpected response: {msg:?}"),
                Some(Err(e)) => return Err(e),
                None => anyhow::bail!("connection closed"),
            }
        }
        Some(crate::HistoryAction::Clear) => {
            framed.send(Message::HistoryClear).await?;
            match framed.next().await {
                Some(Ok(Message::HistoryClearAck)) => {
                    println!("History cleared.");
                }
                Some(Ok(msg)) => anyhow::bail!("unexpected response: {msg:?}"),
                Some(Err(e)) => return Err(e),
                None => anyhow::bail!("connection closed"),
            }
        }
    }

    Ok(())
}
