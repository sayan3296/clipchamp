use crate::protocol::{ClipboardContent, ContentType};
use anyhow::Result;
use tokio::sync::mpsc;

#[cfg(target_os = "linux")]
mod wayland;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "windows")]
mod windows;

pub struct ClipboardHandle {
    pub changes_rx: mpsc::Receiver<ClipboardContent>,
    writer: Box<dyn ClipboardWriter + Send>,
    pub echo_hash: std::sync::Arc<std::sync::Mutex<Option<[u8; 32]>>>,
}

pub trait ClipboardWriter: Send {
    fn write(&mut self, content: &ClipboardContent) -> Result<()>;
    fn read(&mut self) -> Result<Option<ClipboardContent>>;
}

impl ClipboardHandle {
    pub fn write(&mut self, content: &ClipboardContent) -> Result<()> {
        self.writer.write(content)
    }

    pub fn read(&mut self) -> Result<Option<ClipboardContent>> {
        self.writer.read()
    }
}

pub fn create_clipboard(poll_interval_ms: u64) -> Result<ClipboardHandle> {
    #[cfg(target_os = "linux")]
    {
        wayland::create(poll_interval_ms)
    }
    #[cfg(target_os = "macos")]
    {
        macos::create(poll_interval_ms)
    }
    #[cfg(target_os = "windows")]
    {
        windows::create(poll_interval_ms)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        let _ = poll_interval_ms;
        anyhow::bail!("unsupported platform");
    }
}

pub fn create_writer(last_written_hash: std::sync::Arc<std::sync::Mutex<Option<[u8; 32]>>>) -> Result<Box<dyn ClipboardWriter + Send>> {
    #[cfg(target_os = "linux")]
    {
        wayland::create_writer(last_written_hash)
    }
    #[cfg(target_os = "macos")]
    {
        macos::create_writer(last_written_hash)
    }
    #[cfg(target_os = "windows")]
    {
        windows::create_writer(last_written_hash)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        let _ = last_written_hash;
        anyhow::bail!("unsupported platform");
    }
}

pub fn text_content(s: &str) -> ClipboardContent {
    ClipboardContent {
        content_type: ContentType::Text,
        data: s.as_bytes().to_vec(),
    }
}

pub fn url_content(s: &str) -> ClipboardContent {
    ClipboardContent {
        content_type: ContentType::Url,
        data: s.as_bytes().to_vec(),
    }
}

pub fn image_content(png_data: Vec<u8>) -> ClipboardContent {
    ClipboardContent {
        content_type: ContentType::Image,
        data: png_data,
    }
}

pub fn preview_content(content: &ClipboardContent, max_len: usize) -> String {
    match content.content_type {
        ContentType::Text | ContentType::Url => {
            let s = String::from_utf8_lossy(&content.data);
            if s.len() > max_len {
                format!("{}...", &s[..max_len])
            } else {
                s.to_string()
            }
        }
        ContentType::Image => {
            format!("[image {} bytes]", content.data.len())
        }
    }
}
