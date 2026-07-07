use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardItem {
    pub id: i64,
    pub content_type: String,
    pub text_content: Option<String>,
    pub html_content: Option<String>,
    pub blob_content: Option<Vec<u8>>,
    pub file_path: Option<String>,
    pub preview: String,
    pub app_source: Option<String>,
    pub pinned: bool,
    pub created_at: i64,
    pub hash: String,
}
