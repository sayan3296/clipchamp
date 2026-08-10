pub mod cli;

use crate::clipboard;
use crate::config::Config;
use crate::protocol::{ContentType, HistoryEntry};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
struct StoredEntry {
    content_type: ContentType,
    hash: [u8; 32],
    origin_client: Uuid,
    timestamp_secs: u64,
    /// Inline data for text, blob filename for images
    data: EntryData,
}

#[derive(Debug, Serialize, Deserialize)]
enum EntryData {
    Inline(Vec<u8>),
    Blob(String),
}

pub struct FullEntry {
    pub content_type: ContentType,
    pub data: Vec<u8>,
}

pub struct HistoryStore {
    entries: VecDeque<StoredEntry>,
    max_entries: usize,
    persist: bool,
    data_dir: PathBuf,
    blob_dir: PathBuf,
}

impl HistoryStore {
    pub fn new(max_entries: usize, persist: bool) -> Result<Self> {
        let data_dir = Config::data_dir();
        let blob_dir = data_dir.join("blobs");

        if persist {
            std::fs::create_dir_all(&blob_dir)?;
        }

        let entries = if persist {
            Self::load_from_disk(&data_dir)?
        } else {
            VecDeque::new()
        };

        Ok(Self {
            entries,
            max_entries,
            persist,
            data_dir,
            blob_dir,
        })
    }

    pub fn add(
        &mut self,
        content_type: ContentType,
        data: Vec<u8>,
        hash: [u8; 32],
        origin_client: Uuid,
    ) -> Result<()> {
        // Dedup: skip if last entry has same hash
        if let Some(last) = self.entries.back() {
            if last.hash == hash {
                return Ok(());
            }
        }

        let timestamp_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let entry_data = match content_type {
            ContentType::Image => {
                let hash_hex = hex_hash(&hash);
                if self.persist {
                    let blob_path = self.blob_dir.join(&hash_hex);
                    if !blob_path.exists() {
                        std::fs::write(&blob_path, &data)?;
                    }
                }
                EntryData::Blob(hash_hex)
            }
            _ => EntryData::Inline(data),
        };

        self.entries.push_back(StoredEntry {
            content_type,
            hash,
            origin_client,
            timestamp_secs,
            data: entry_data,
        });

        while self.entries.len() > self.max_entries {
            self.entries.pop_front();
        }

        if self.persist {
            self.save_to_disk()?;
        }

        Ok(())
    }

    pub fn list(&self, count: usize) -> Vec<HistoryEntry> {
        self.entries
            .iter()
            .rev()
            .take(count)
            .enumerate()
            .map(|(i, entry)| {
                let preview = match &entry.data {
                    EntryData::Inline(data) => {
                        let content = crate::protocol::ClipboardContent {
                            content_type: entry.content_type,
                            data: data.clone(),
                        };
                        clipboard::preview_content(&content, 60)
                    }
                    EntryData::Blob(name) => format!("[image blob: {name}]"),
                };
                let size_bytes = match &entry.data {
                    EntryData::Inline(d) => d.len(),
                    EntryData::Blob(name) => self
                        .blob_dir
                        .join(name)
                        .metadata()
                        .map(|m| m.len() as usize)
                        .unwrap_or(0),
                };

                HistoryEntry {
                    index: i + 1,
                    content_type: entry.content_type,
                    hash: entry.hash,
                    origin_client: entry.origin_client,
                    preview,
                    size_bytes,
                    timestamp_secs: entry.timestamp_secs,
                }
            })
            .collect()
    }

    pub fn get_by_hash(&self, hash: &[u8; 32]) -> Option<FullEntry> {
        self.entries.iter().find(|e| &e.hash == hash).and_then(|e| {
            let data = match &e.data {
                EntryData::Inline(d) => d.clone(),
                EntryData::Blob(name) => std::fs::read(self.blob_dir.join(name)).ok()?,
            };
            Some(FullEntry {
                content_type: e.content_type,
                data,
            })
        })
    }

    pub fn clear(&mut self) -> Result<()> {
        self.entries.clear();
        if self.persist {
            self.save_to_disk()?;
            if self.blob_dir.exists() {
                for entry in std::fs::read_dir(&self.blob_dir)? {
                    let entry = entry?;
                    std::fs::remove_file(entry.path())?;
                }
            }
        }
        Ok(())
    }

    fn history_path(data_dir: &std::path::Path) -> PathBuf {
        data_dir.join("history.json")
    }

    fn load_from_disk(data_dir: &std::path::Path) -> Result<VecDeque<StoredEntry>> {
        let path = Self::history_path(data_dir);
        if !path.exists() {
            return Ok(VecDeque::new());
        }
        let content = std::fs::read_to_string(&path)?;
        let entries: VecDeque<StoredEntry> = serde_json::from_str(&content)?;
        Ok(entries)
    }

    fn save_to_disk(&self) -> Result<()> {
        let path = Self::history_path(&self.data_dir);
        let content = serde_json::to_string(&self.entries)?;
        std::fs::write(&path, content)?;
        Ok(())
    }
}

fn hex_hash(hash: &[u8; 32]) -> String {
    hash.iter().map(|b| format!("{b:02x}")).collect()
}
