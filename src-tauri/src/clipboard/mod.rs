#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "macos")]
pub mod macos;

use serde::{Deserialize, Serialize};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClipboardContent {
    Text(String),
    RichText { plain: String, html: String },
    Image(Vec<u8>),
    FilePath(String),
}

pub trait ClipboardMonitor: Send {
    fn start(&mut self, callback: Box<dyn Fn(ClipboardContent) + Send + Sync>) -> Result<(), String>;
    fn stop(&mut self) -> Result<(), String>;
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
