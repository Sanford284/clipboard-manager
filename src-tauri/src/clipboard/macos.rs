use super::{ClipboardContent, ClipboardImage, ClipboardMonitor};
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
            let mut clipboard = match Clipboard::new() {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("[clipboard] failed to open clipboard: {e}");
                    return;
                }
            };
            let mut last_text = clipboard.get_text().unwrap_or_default();
            let mut last_image: Vec<u8> = Vec::new();

            while running.load(Ordering::SeqCst) {
                if !paused.load(Ordering::SeqCst) {
                    let emitted_image = if let Ok(img) = clipboard.get_image() {
                        let sig = image_signature(&img);
                        if sig != last_image {
                            last_image = sig;
                            callback(ClipboardContent::Image(ClipboardImage {
                                width: img.width,
                                height: img.height,
                                bytes: img.bytes.to_vec(),
                            }));
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    };

                    if !emitted_image {
                        if let Ok(text) = clipboard.get_text() {
                            if !text.is_empty() && text != last_text {
                                last_text = text.clone();
                                callback(ClipboardContent::Text(text));
                            }
                        }
                    }
                }
                thread::sleep(Duration::from_millis(1500));
            }
        });

        Ok(())
    }

    fn paused_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.paused)
    }
}

/// 轻量指纹（宽高 + 前 64 字节），避免逐字节比较大图
fn image_signature(img: &arboard::ImageData) -> Vec<u8> {
    let take = img.bytes.len().min(64);
    let mut sig = Vec::with_capacity(72);
    sig.extend_from_slice(&(img.width as u64).to_le_bytes());
    sig.extend_from_slice(&(img.height as u64).to_le_bytes());
    sig.extend_from_slice(&img.bytes[..take]);
    sig
}
