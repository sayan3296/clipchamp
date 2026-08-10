use bytes::{Buf, BufMut, BytesMut};
use serde::{Deserialize, Serialize};
use tokio_util::codec::{Decoder, Encoder};
use uuid::Uuid;

pub const PROTOCOL_VERSION: u32 = 1;
pub const DEFAULT_MAX_FRAME_SIZE: usize = 16 * 1024 * 1024; // 16 MB

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContentType {
    Text,
    Image,
    Url,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardContent {
    pub content_type: ContentType,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    Hello {
        client_id: Uuid,
        protocol_version: u32,
    },
    Welcome {
        client_id: Uuid,
    },
    ClipboardUpdate {
        content_type: ContentType,
        data: Vec<u8>,
        hash: [u8; 32],
        origin_client: Uuid,
    },
    Ping,
    Pong,
    HistoryRequest {
        count: usize,
    },
    HistoryResponse {
        entries: Vec<HistoryEntry>,
    },
    HistoryFetch {
        hash: [u8; 32],
    },
    HistoryContent {
        content_type: ContentType,
        data: Vec<u8>,
    },
    HistoryClear,
    HistoryClearAck,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub index: usize,
    pub content_type: ContentType,
    pub hash: [u8; 32],
    pub origin_client: Uuid,
    pub preview: String,
    pub size_bytes: usize,
    pub timestamp_secs: u64,
}

pub fn content_hash(data: &[u8]) -> [u8; 32] {
    *blake3::hash(data).as_bytes()
}

pub struct MessageCodec {
    max_frame_size: usize,
}

impl MessageCodec {
    pub fn new(max_frame_size: usize) -> Self {
        Self { max_frame_size }
    }
}

impl Default for MessageCodec {
    fn default() -> Self {
        Self {
            max_frame_size: DEFAULT_MAX_FRAME_SIZE,
        }
    }
}

impl Decoder for MessageCodec {
    type Item = Message;
    type Error = anyhow::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() < 4 {
            return Ok(None);
        }
        let length = u32::from_be_bytes([src[0], src[1], src[2], src[3]]) as usize;
        if length > self.max_frame_size {
            anyhow::bail!("frame too large: {length} bytes");
        }
        if src.len() < 4 + length {
            src.reserve(4 + length - src.len());
            return Ok(None);
        }
        src.advance(4);
        let payload = src.split_to(length);
        let msg: Message = rmp_serde::from_slice(&payload)?;
        Ok(Some(msg))
    }
}

impl Encoder<Message> for MessageCodec {
    type Error = anyhow::Error;

    fn encode(&mut self, item: Message, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let payload = rmp_serde::to_vec(&item)?;
        if payload.len() > self.max_frame_size {
            anyhow::bail!("message too large: {} bytes", payload.len());
        }
        dst.reserve(4 + payload.len());
        dst.put_u32(payload.len() as u32);
        dst.extend_from_slice(&payload);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(msg: Message) {
        let mut codec = MessageCodec::default();
        let mut buf = BytesMut::new();
        codec.encode(msg.clone(), &mut buf).unwrap();
        let decoded = codec.decode(&mut buf).unwrap().unwrap();
        let re_encoded = rmp_serde::to_vec(&decoded).unwrap();
        let orig_encoded = rmp_serde::to_vec(&msg).unwrap();
        assert_eq!(orig_encoded, re_encoded);
    }

    #[test]
    fn test_hello_roundtrip() {
        roundtrip(Message::Hello {
            client_id: Uuid::new_v4(),
            protocol_version: PROTOCOL_VERSION,
        });
    }

    #[test]
    fn test_clipboard_update_roundtrip() {
        let data = b"hello world".to_vec();
        let hash = content_hash(&data);
        roundtrip(Message::ClipboardUpdate {
            content_type: ContentType::Text,
            data,
            hash,
            origin_client: Uuid::new_v4(),
        });
    }

    #[test]
    fn test_ping_pong_roundtrip() {
        roundtrip(Message::Ping);
        roundtrip(Message::Pong);
    }

    #[test]
    fn test_history_roundtrip() {
        roundtrip(Message::HistoryRequest { count: 10 });
        roundtrip(Message::HistoryResponse {
            entries: vec![HistoryEntry {
                index: 1,
                content_type: ContentType::Text,
                hash: [0u8; 32],
                origin_client: Uuid::new_v4(),
                preview: "hello...".to_string(),
                size_bytes: 11,
                timestamp_secs: 1234567890,
            }],
        });
    }

    #[test]
    fn test_partial_frame() {
        let mut codec = MessageCodec::default();
        let msg = Message::Ping;
        let mut full = BytesMut::new();
        codec.encode(msg, &mut full).unwrap();

        let mut partial = full.split_to(2);
        assert!(codec.decode(&mut partial).unwrap().is_none());

        partial.extend_from_slice(&full);
        let decoded = codec.decode(&mut partial).unwrap();
        assert!(decoded.is_some());
    }
}
