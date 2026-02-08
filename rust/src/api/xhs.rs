use crate::models::xhs::XhsArticle;
use regex::Regex;
use serde::{Deserialize, Serialize};

/// 小红书 API 错误类型，FRB 友好的设计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XhsApiError {
    pub error_type: String,
    pub message: String,
}

impl XhsApiError {
    fn url_not_found() -> Self {
        Self {
            error_type: "UrlNotFound".to_string(),
            message: "未找到小红书链接".to_string(),
        }
    }

    fn regex_error(e: String) -> Self {
        Self {
            error_type: "RegexError".to_string(),
            message: format!("正则表达式错误: {}", e),
        }
    }
}

impl std::fmt::Display for XhsApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.error_type, self.message)
    }
}

impl std::error::Error for XhsApiError {}

/// 从混合文本中提取小红书 URL 并解析
///
/// # 示例
/// ```ignore
/// let text = "家庭版馄饨｜早餐自制馄饨 真的太好吃了～好吃到汤都... http://xhslink.com/o/5ZMAfpDOokl 复制后打开【小红书】查看笔记！";
/// let article = parse_xhs_from_text(text)?;
/// println!("标题: {}", article.title);
/// ```
#[flutter_rust_bridge::frb(sync)]
pub fn parse_xhs_from_text(text: String) -> Result<XhsArticle, XhsApiError> {
    let url = extract_xhs_url(&text)?;
    unimplemented!("XHS parsing not yet implemented")
}

/// 直接从 URL 解析小红书笔记
#[flutter_rust_bridge::frb(sync)]
pub fn parse_xhs_from_url(url: String) -> Result<XhsArticle, XhsApiError> {
    unimplemented!("XHS parsing not yet implemented")
}

fn extract_xhs_url(text: &str) -> Result<String, XhsApiError> {
    let regex = Regex::new(r"http[s]?://xhslink\.com/o/[a-zA-Z0-9]+")
        .map_err(|e| XhsApiError::regex_error(e.to_string()))?;

    regex
        .find(text)
        .map(|m| m.as_str().to_string())
        .ok_or_else(XhsApiError::url_not_found)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_xhs_url_from_mixed_text() {
        let text = "家庭版馄饨｜早餐自制馄饨 真的太好吃了～好吃到汤都... http://xhslink.com/o/5ZMAfpDOokl 复制后打开【小红书】查看笔记！";
        let url = extract_xhs_url(text).expect("应该能提取 URL");
        assert_eq!(url, "http://xhslink.com/o/5ZMAfpDOokl");
    }

    #[test]
    fn test_extract_xhs_url_from_https() {
        let text = "检查这个：https://xhslink.com/o/abc123xyz 很棒的笔记";
        let url = extract_xhs_url(text).expect("应该能提取 HTTPS URL");
        assert_eq!(url, "https://xhslink.com/o/abc123xyz");
    }

    #[test]
    fn test_extract_xhs_url_not_found() {
        let text = "这是一个没有链接的文本";
        let result = extract_xhs_url(text);
        assert!(result.is_err());
        match result {
            Err(err) => assert_eq!(err.error_type, "UrlNotFound"),
            _ => panic!("应该返回 UrlNotFound 错误"),
        }
    }
}
