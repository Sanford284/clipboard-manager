use super::{ClipboardContent, ClipboardMonitor};
use arboard::Clipboard;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

pub struct WindowsClipboardMonitor {
    running: Arc<AtomicBool>,
}

impl WindowsClipboardMonitor {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl ClipboardMonitor for WindowsClipboardMonitor {
    fn start(&mut self, callback: Box<dyn Fn(ClipboardContent) + Send + Sync>) -> Result<(), String> {
        self.running.store(true, Ordering::SeqCst);
        let running = Arc::clone(&self.running);

        thread::spawn(move || {
            let mut clipboard = Clipboard::new().unwrap();
            let mut last_text = String::new();

            while running.load(Ordering::SeqCst) {
                if let Ok(text) = clipboard.get_text() {
                    if text != last_text {
                        last_text = text.clone();
                        callback(ClipboardContent::Text(text));
                    }
                }
                thread::sleep(Duration::from_secs(1));
            }
        });

        Ok(())
    }

    fn stop(&mut self) -> Result<(), String> {
        self.running.store(false, Ordering::SeqCst);
        Ok(())
    }
}
