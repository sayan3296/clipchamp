use super::{ClipboardHandle, ClipboardWriter};
use crate::protocol::{ClipboardContent, ContentType, content_hash};
use anyhow::Result;
use arboard::Clipboard;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

struct MacosClipboardWriter {
    clipboard: Clipboard,
    last_written_hash: Arc<Mutex<Option<[u8; 32]>>>,
}

impl ClipboardWriter for MacosClipboardWriter {
    fn write(&mut self, content: &ClipboardContent) -> Result<()> {
        let hash = content_hash(&content.data);
        *self.last_written_hash.lock().unwrap() = Some(hash);

        match content.content_type {
            ContentType::Text | ContentType::Url => {
                let text = String::from_utf8_lossy(&content.data);
                self.clipboard.set_text(text.as_ref())?;
            }
            ContentType::Image => {
                let img = image::load_from_memory_with_format(&content.data, image::ImageFormat::Png)?;
                let rgba = img.to_rgba8();
                let (w, h) = rgba.dimensions();
                let image_data = arboard::ImageData {
                    width: w as usize,
                    height: h as usize,
                    bytes: rgba.into_raw().into(),
                };
                self.clipboard.set_image(image_data)?;
            }
        }
        Ok(())
    }
}

pub fn create(poll_interval_ms: u64) -> Result<ClipboardHandle> {
    let (tx, rx) = mpsc::channel(32);
    let last_written_hash: Arc<Mutex<Option<[u8; 32]>>> = Arc::new(Mutex::new(None));
    let echo_hash = last_written_hash.clone();

    std::thread::spawn(move || {
        let mut clipboard = match Clipboard::new() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("failed to open clipboard: {e}");
                return;
            }
        };

        let interval = std::time::Duration::from_millis(poll_interval_ms);
        let mut last_hash: Option<[u8; 32]> = None;

        loop {
            std::thread::sleep(interval);

            let data = if let Ok(text) = clipboard.get_text() {
                if text.is_empty() {
                    continue;
                }
                text.into_bytes()
            } else if let Ok(img) = clipboard.get_image() {
                img.bytes.to_vec()
            } else {
                continue;
            };

            let hash = content_hash(&data);

            if last_hash.as_ref() == Some(&hash) {
                continue;
            }

            {
                let written = echo_hash.lock().unwrap();
                if written.as_ref() == Some(&hash) {
                    continue;
                }
            }

            last_hash = Some(hash);

            let content_type = match String::from_utf8(data.clone()) {
                Ok(ref s) if s.starts_with("http://") || s.starts_with("https://") => ContentType::Url,
                Ok(_) => ContentType::Text,
                Err(_) => ContentType::Image,
            };

            let content = ClipboardContent { content_type, data };

            if tx.blocking_send(content).is_err() {
                break;
            }
        }
    });

    let clipboard = Clipboard::new()?;
    let writer_hash = last_written_hash.clone();
    let writer = MacosClipboardWriter {
        clipboard,
        last_written_hash: writer_hash,
    };

    Ok(ClipboardHandle {
        changes_rx: rx,
        writer: Box::new(writer),
        echo_hash: last_written_hash,
    })
}

pub fn create_writer(last_written_hash: Arc<Mutex<Option<[u8; 32]>>>) -> Result<Box<dyn ClipboardWriter + Send>> {
    let clipboard = Clipboard::new()?;
    Ok(Box::new(MacosClipboardWriter {
        clipboard,
        last_written_hash,
    }))
}
