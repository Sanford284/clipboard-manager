#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "macos")]
pub mod macos;

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

/// Owned RGBA image (decoupled from arboard's borrowed ImageData lifetime).
#[derive(Debug, Clone)]
pub struct ClipboardImage {
    pub width: usize,
    pub height: usize,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
pub enum ClipboardContent {
    Text(String),
    RichText { plain: String, html: String },
    Image(ClipboardImage),
    FilePath(String),
}

pub trait ClipboardMonitor: Send {
    fn start(&mut self, callback: Box<dyn Fn(ClipboardContent) + Send + Sync>) -> Result<(), String>;
    fn paused_flag(&self) -> Arc<AtomicBool>;
}

#[cfg(target_os = "macos")]
pub fn create_monitor() -> Box<dyn ClipboardMonitor> {
    Box::new(macos::MacOSClipboardMonitor::new())
}

#[cfg(target_os = "windows")]
pub fn create_monitor() -> Box<dyn ClipboardMonitor> {
    Box::new(windows::WindowsClipboardMonitor::new())
}
