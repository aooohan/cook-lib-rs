use serde::{Deserialize, Serialize};

/// 笔记类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum NoteType {
    /// 纯视频笔记
    Video,
    /// 纯图片笔记
    Images,
    /// 视频 + 图片混合
    Mixed,
    /// 纯文本笔记
    #[default]
    Text,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XhsArticle {
    pub title: String,
    pub desc: String,
    pub author: XhsAuthor,
    pub images: Vec<String>,
    pub video: Option<XhsVideo>,
    /// 笔记类型，自动推断
    #[serde(skip)]
    pub note_type: NoteType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XhsAuthor {
    pub nickname: String,
    #[serde(rename = "userId")]
    pub user_id: String,
    pub avatar: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XhsVideo {
    pub duration: i64,
    pub cover: String,
    pub play_url: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NoteDetail {
    pub title: String,
    pub desc: String,
    pub user: XhsAuthor,
    #[serde(rename = "imageList", default)]
    pub image_list: Vec<ImageItem>,
    pub video: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ImageItem {
    #[serde(rename = "urlDefault")]
    pub url_default: String,
}
