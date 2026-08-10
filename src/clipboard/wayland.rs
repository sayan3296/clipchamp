use super::{ClipboardHandle, ClipboardWriter};
use crate::protocol::{ClipboardContent, ContentType, content_hash};
use anyhow::Result;
use arboard::Clipboard;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

struct WaylandClipboardWriter {
    clipboard: Clipboard,
    last_written_hash: Arc<Mutex<Option<[u8; 32]>>>,
}

impl ClipboardWriter for WaylandClipboardWriter {
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

    fn read(&mut self) -> Result<Option<ClipboardContent>> {
        if let Ok(text) = self.clipboard.get_text() {
            if !text.is_empty() {
                let content_type = if text.starts_with("http://") || text.starts_with("https://") {
                    ContentType::Url
                } else {
                    ContentType::Text
                };
                return Ok(Some(ClipboardContent {
                    content_type,
                    data: text.into_bytes(),
                }));
            }
        }
        if let Ok(img) = self.clipboard.get_image() {
            let rgba_image = image::RgbaImage::from_raw(
                img.width as u32,
                img.height as u32,
                img.bytes.to_vec(),
            );
            if let Some(rgba) = rgba_image {
                let mut png_buf = Vec::new();
                let encoder = image::codecs::png::PngEncoder::new(&mut png_buf);
                image::ImageEncoder::write_image(
                    encoder,
                    &rgba,
                    rgba.width(),
                    rgba.height(),
                    image::ExtendedColorType::Rgba8,
                )?;
                return Ok(Some(ClipboardContent {
                    content_type: ContentType::Image,
                    data: png_buf,
                }));
            }
        }
        Ok(None)
    }
}

pub fn create(poll_interval_ms: u64) -> Result<ClipboardHandle> {
    let (tx, rx) = mpsc::channel(32);
    let last_written_hash: Arc<Mutex<Option<[u8; 32]>>> = Arc::new(Mutex::new(None));
    let echo_hash = last_written_hash.clone();

    std::thread::spawn(move || {
        run_watcher(tx, echo_hash, poll_interval_ms);
    });

    let clipboard = Clipboard::new()?;
    let writer_hash = last_written_hash.clone();
    let writer = WaylandClipboardWriter {
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
    Ok(Box::new(WaylandClipboardWriter {
        clipboard,
        last_written_hash,
    }))
}

fn run_watcher(
    tx: mpsc::Sender<ClipboardContent>,
    last_written_hash: Arc<Mutex<Option<[u8; 32]>>>,
    poll_interval_ms: u64,
) {
    use wayland_clipboard_listener::{
        ClipBoardListenContext, WlClipboardListenerStream, WlListenType,
    };

    let stream = match WlClipboardListenerStream::init(WlListenType::ListenOnCopy) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("wayland clipboard listener failed, falling back to polling: {e}");
            run_polling_watcher(tx, last_written_hash, poll_interval_ms);
            return;
        }
    };

    let mut last_hash: Option<[u8; 32]> = None;

    for event in stream {
        let msg = match event {
            Ok(Some(m)) => m,
            Ok(None) => continue,
            Err(e) => {
                tracing::debug!("clipboard event error: {e}");
                continue;
            }
        };

        let (data, content_type) = match msg.context {
            ClipBoardListenContext::Text(s) => {
                let ct = if s.starts_with("http://") || s.starts_with("https://") {
                    ContentType::Url
                } else {
                    ContentType::Text
                };
                (s.into_bytes(), ct)
            }
            ClipBoardListenContext::File(bytes) => (bytes, ContentType::Image),
        };

        let hash = content_hash(&data);

        if last_hash.as_ref() == Some(&hash) {
            continue;
        }

        {
            let written = last_written_hash.lock().unwrap();
            if written.as_ref() == Some(&hash) {
                continue;
            }
        }

        last_hash = Some(hash);

        let content = ClipboardContent { content_type, data };

        if tx.blocking_send(content).is_err() {
            break;
        }
    }
}

fn run_polling_watcher(
    tx: mpsc::Sender<ClipboardContent>,
    last_written_hash: Arc<Mutex<Option<[u8; 32]>>>,
    poll_interval_ms: u64,
) {
    let mut clipboard = match Clipboard::new() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("failed to open clipboard for polling: {e}");
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
            let written = last_written_hash.lock().unwrap();
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
}
