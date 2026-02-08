use reqwest::blocking::Client;
use thiserror::Error;

use crate::models::xhs::XhsArticle;

mod parser;

#[derive(Error, Debug)]
pub enum ParserError {
    #[error("HTTP 请求失败: {0}")]
    Http(#[from] reqwest::Error),
    #[error("从页面中提取初始数据失败")]
    InitialStateMissing,
    #[error("JSON 数据解析失败: {0}")]
    Json(#[from] serde_json::Error),
    #[error("解析笔记数据失败: {0}")]
    ParseNote(String),
}

pub struct XhsParser {
    client: Client,
}

impl XhsParser {
    pub fn new() -> Self {
        let client = Client::builder()
            .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36")
            .build()
            .unwrap();
        Self { client }
    }

    /// 从小红书链接获取文章详情
    pub fn parse_by_url(&self, url: &str) -> Result<XhsArticle, ParserError> {
        let html = self.fetch_html(url)?;
        self.parse_from_html(&html)
    }

    /// 从 HTML 内容直接解析
    pub fn parse_from_html(&self, html: &str) -> Result<XhsArticle, ParserError> {
        let state = parser::extract_initial_state(html)?;
        parser::build_article_from_state(state)
    }

    fn fetch_html(&self, url: &str) -> Result<String, ParserError> {
        self.fetch_html_internal(url)
    }

    fn fetch_html_internal(&self, url: &str) -> Result<String, ParserError> {
        let resp = self.client.get(url).send()?;
        Ok(resp.text()?)
    }
}

impl Default for XhsParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use std::sync::OnceLock;

    static CACHED_HTML: OnceLock<String> = OnceLock::new();

    fn get_test_html() -> String {
        CACHED_HTML
            .get_or_init(|| {
                let temp_path = "/tmp/xhs_test_cache.html";
                if Path::new(temp_path).exists() {
                    println!("从缓存读取小红书内容: {}", temp_path);
                    fs::read_to_string(temp_path).expect("无法读取缓存 HTML")
                } else {
                    println!("从小红书实时获取内容...");
                    let parser = XhsParser::new();
                    let url = "http://xhslink.com/o/9vGyN9oI440";
                    let content = parser
                        .fetch_html_internal(url)
                        .expect("无法从小红书获取内容");
                    fs::write(temp_path, &content).expect("无法写入缓存");
                    println!("缓存已保存到: {}", temp_path);
                    content
                }
            })
            .clone()
    }

    #[test]
    fn test_parse_video_note_from_sample_html() {
        let html = get_test_html();

        let parser = XhsParser::new();
        let article = parser.parse_from_html(&html).expect("解析失败");

        assert_eq!(article.title, "爱死蹄花汤了，有种喝肉的体验！");
        assert!(!article.desc.is_empty());
        assert_eq!(article.author.nickname, "开饭啦小志");
        assert!(!article.author.avatar.is_empty());
        assert!(!article.author.user_id.is_empty());
        assert_eq!(article.images.len(), 1);
        assert!(article.video.is_some());

        let video = article.video.unwrap();
        assert_eq!(video.duration, 65);
        assert!(!video.cover.is_empty());
        assert!(video.play_url.contains("sns-video") && video.play_url.contains("xhscdn.com"));
    }

    #[test]
    fn test_extract_author_info() {
        let html = get_test_html();

        let parser = XhsParser::new();
        let article = parser.parse_from_html(&html).expect("解析失败");

        assert_eq!(article.author.nickname, "开饭啦小志");
        assert_eq!(article.author.user_id, "5c1fa2d500000000070373a3");
        assert!(article.author.avatar.contains("xhscdn.com"));
    }

    #[test]
    fn test_extract_title_and_desc() {
        let html = get_test_html();

        let parser = XhsParser::new();
        let article = parser.parse_from_html(&html).expect("解析失败");

        assert_eq!(article.title, "爱死蹄花汤了，有种喝肉的体验！");
        assert!(article.desc.contains("慢炖"));
        assert!(article.desc.contains("白芸豆"));
    }

    #[test]
    fn test_extract_images() {
        let html = get_test_html();

        let parser = XhsParser::new();
        let article = parser.parse_from_html(&html).expect("解析失败");

        assert_eq!(article.images.len(), 1);
        assert!(article.images[0].contains("sns-webpic-qc.xhscdn.com"));
        assert!(article.images[0].contains("spectrum"));
    }

    #[test]
    fn test_extract_video_info() {
        let html = get_test_html();

        let parser = XhsParser::new();
        let article = parser.parse_from_html(&html).expect("解析失败");

        assert!(article.video.is_some());
        let video = article.video.unwrap();
        assert_eq!(video.duration, 65);
        assert!(video.cover.contains("webp"));
        assert!(video.play_url.contains("mp4"));
        assert!(video.play_url.contains("sns-video") && video.play_url.contains("xhscdn.com"));
    }

    fn get_pure_images_html() -> String {
        let temp_path = "/tmp/xhs_pure_images.html";
        if Path::new(temp_path).exists() {
            println!("从缓存读取纯图片笔记: {}", temp_path);
            fs::read_to_string(temp_path).expect("无法读取缓存 HTML")
        } else {
            println!("从小红书实时获取纯图片笔记...");
            let parser = XhsParser::new();
            let url = "http://xhslink.com/o/5ZMAfpDOokl";
            let content = parser
                .fetch_html_internal(url)
                .expect("无法从小红书获取内容");
            fs::write(temp_path, &content).expect("无法写入缓存");
            println!("纯图片笔记缓存已保存到: {}", temp_path);
            content
        }
    }

    #[test]
    fn test_parse_pure_images_note() {
        let html = get_pure_images_html();

        let parser = XhsParser::new();
        let article = parser.parse_from_html(&html).expect("解析失败");

        assert_eq!(article.title, "家庭版馄饨｜早餐自制馄饨");
        assert!(!article.desc.is_empty());
        assert!(!article.author.nickname.is_empty());
        assert!(!article.author.avatar.is_empty());
        assert!(!article.author.user_id.is_empty());

        assert_eq!(article.images.len(), 13);
        assert!(article.video.is_none());

        assert_eq!(article.note_type, crate::models::xhs::NoteType::Images);
    }

    #[test]
    fn test_note_type_detection_video() {
        let html = get_test_html();
        let parser = XhsParser::new();
        let article = parser.parse_from_html(&html).expect("解析失败");

        assert_eq!(article.note_type, crate::models::xhs::NoteType::Video);
    }

    #[test]
    fn test_note_type_detection_pure_images() {
        let html = get_pure_images_html();
        let parser = XhsParser::new();
        let article = parser.parse_from_html(&html).expect("解析失败");

        assert_eq!(article.note_type, crate::models::xhs::NoteType::Images);
    }
}
