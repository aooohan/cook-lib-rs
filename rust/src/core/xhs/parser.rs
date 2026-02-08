use scraper::{Html, Selector};
use serde_json::Value;

use crate::core::xhs::ParserError;
use crate::models::xhs::{NoteDetail, NoteType, XhsArticle, XhsVideo};

pub fn extract_initial_state(html: &str) -> Result<Value, ParserError> {
    let document = Html::parse_document(html);
    let script_selector =
        Selector::parse("script").map_err(|_| ParserError::InitialStateMissing)?;

    for element in document.select(&script_selector) {
        let script_content = element.inner_html();
        if let Some(stripped) = script_content.strip_prefix("window.__INITIAL_STATE__=") {
            let cleaned = sanitize_json(stripped);
            let value: Value = serde_json::from_str(&cleaned)?;
            return Ok(value);
        }
    }

    Err(ParserError::InitialStateMissing)
}

pub fn build_article_from_state(state: Value) -> Result<XhsArticle, ParserError> {
    let note_map = state
        .get("note")
        .and_then(|n| n.get("noteDetailMap"))
        .and_then(|m| m.as_object())
        .ok_or(ParserError::InitialStateMissing)?;

    let first_entry = note_map
        .values()
        .next()
        .ok_or(ParserError::InitialStateMissing)?;

    let note_data = first_entry
        .get("note")
        .ok_or(ParserError::InitialStateMissing)?;

    let note: NoteDetail = serde_json::from_value(note_data.clone())
        .map_err(|e| ParserError::ParseNote(format!("反序列化笔记数据失败: {}", e)))?;

    convert_note_to_article(note)
}

fn convert_note_to_article(note: NoteDetail) -> Result<XhsArticle, ParserError> {
    let video = note.video.as_ref().and_then(extract_video_info);

    let images = note
        .image_list
        .into_iter()
        .map(|img| img.url_default)
        .collect::<Vec<_>>();

    let note_type = determine_note_type(&video, &images);

    Ok(XhsArticle {
        title: note.title,
        desc: note.desc,
        author: note.user,
        images,
        video,
        note_type,
    })
}

fn determine_note_type(video: &Option<XhsVideo>, images: &[String]) -> NoteType {
    match (video, images.len()) {
        (Some(_), 0 | 1) => NoteType::Video,
        (Some(_), _) => NoteType::Mixed,
        (None, 0) => NoteType::Text,
        (None, _) => NoteType::Images,
    }
}

fn extract_video_info(video_val: &Value) -> Option<XhsVideo> {
    let duration = video_val.get("capa")?.get("duration")?.as_i64()?;

    let cover = video_val
        .get("image")?
        .get("thumbnailFileid")?
        .as_str()?
        .to_string();

    let play_url = video_val
        .get("media")?
        .get("stream")?
        .get("h264")?
        .get(0)?
        .get("masterUrl")?
        .as_str()?
        .to_string();

    Some(XhsVideo {
        duration,
        cover,
        play_url,
    })
}

fn sanitize_json(raw: &str) -> String {
    raw.replace("undefined", "null")
}
