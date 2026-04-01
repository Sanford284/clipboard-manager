use super::{ClipboardContent, ClipboardMonitor};
use arboard::Clipboard;
use cocoa::appkit::NSPasteboard;
use cocoa::base::{id, nil};
use cocoa::foundation::NSAutoreleasePool;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

pub struct MacOSClipboardMonitor {
    last_change_count: Arc<AtomicI64>,
    running: Arc<AtomicBool>,
}

impl MacOSClipboardMonitor {
    pub fn new() -> Self {
        Self {
            last_change_count: Arc::new(AtomicI64::new(-1)),
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    fn get_change_count() -> i64 {
        unsafe {
            let _pool = NSAutoreleasePool::new(nil);
            let pasteboard: id = NSPasteboard::generalPasteboard(nil);
            cocoa::appkit::NSPasteboard::changeCount(pasteboard)
        }
    }
}

impl ClipboardMonitor for MacOSClipboardMonitor {
    fn start(&mut self, callback: Box<dyn Fn(ClipboardContent) + Send + Sync>) -> Result<(), String> {
        self.running.store(true, Ordering::SeqCst);
        let running = Arc::clone(&self.running);
        let last_change_count = Arc::clone(&self.last_change_count);

        thread::spawn(move || {
            let mut clipboard = Clipboard::new().unwrap();
            while running.load(Ordering::SeqCst) {
                let current_count = Self::get_change_count();
                let last_count = last_change_count.load(Ordering::SeqCst);

                if current_count != last_count {
                    last_change_count.store(current_count, Ordering::SeqCst);

                    if let Ok(text) = clipboard.get_text() {
                        callback(ClipboardContent::Text(text));
                    } else if let Ok(image) = clipboard.get_image() {
                        let rgba = image.bytes.to_vec();
                        callback(ClipboardContent::Image(rgba));
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
