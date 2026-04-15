use super::{ClipboardContent, ClipboardMonitor};
use arboard::Clipboard;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

pub struct MacOSClipboardMonitor {
    running: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
}

impl MacOSClipboardMonitor {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            paused: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl ClipboardMonitor for MacOSClipboardMonitor {
    fn start(&mut self, callback: Box<dyn Fn(ClipboardContent) + Send + Sync>) -> Result<(), String> {
        self.running.store(true, Ordering::SeqCst);
        let running = Arc::clone(&self.running);
        let paused = Arc::clone(&self.paused);

        thread::spawn(move || {
            let mut clipboard = Clipboard::new().unwrap();
            let mut last_text = clipboard.get_text().unwrap_or_default();

            while running.load(Ordering::SeqCst) {
                if !paused.load(Ordering::SeqCst) {
                    if let Ok(text) = clipboard.get_text() {
                        if !text.is_empty() && text != last_text {
                            last_text = text.clone();
                            callback(ClipboardContent::Text(text));
                        }
                    }
                }
                thread::sleep(Duration::from_millis(1500));
            }
        });

        Ok(())
    }

    fn stop(&mut self) -> Result<(), String> {
        self.running.store(false, Ordering::SeqCst);
        Ok(())
    }

    fn paused_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.paused)
    }
}
