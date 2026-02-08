# src/core/xhs - 核心解析逻辑

**职责**: 实现小红书笔记解析的核心业务逻辑：获取 HTML、提取初始数据、反序列化成模型。

**关键文件**:
- `mod.rs` - XhsParser、ParserError、单元测试
- `parser.rs` - HTML 解析实现、JSON 处理

## 模块结构

```
core/xhs/
├── mod.rs           # XhsParser, ParserError, 8 个测试
└── parser.rs
    ├── extract_initial_state()      ← 从 HTML 中找 window.__INITIAL_STATE__
    ├── build_article_from_state()   ← Redux state → XhsArticle
    ├── convert_note_to_article()    ← NoteDetail → XhsArticle
    ├── determine_note_type()        ← 识别笔记类型
    ├── extract_video_info()         ← 提取视频数据
    └── sanitize_json()              ← 清理 JavaScript 对象
```

## 职责边界

### XhsParser（mod.rs）

✅ **做什么**:
- 管理 `reqwest::blocking::Client`（单例）
- 提供 pub fn `parse_by_url()` - 获取 HTML 后调用 parse_from_html
- 提供 pub fn `parse_from_html()` - 从 HTML 字符串解析
- 内部 fn `fetch_html()` - HTTP 网络请求

❌ **不做什么**:
- HTML DOM 遍历（由 scraper 和 parser.rs 完成）
- JSON 反序列化细节（由 serde_json 完成）

### parser.rs（解析实现）

✅ **做什么**:
- 用 `scraper` 库找 `<script>window.__INITIAL_STATE__=</script>`
- 清理 JSON（`undefined` → `null`）
- serde_json 反序列化为 `NoteDetail`（pub(crate)）
- 转换 NoteDetail → XhsArticle，同时自动识别笔记类型
- 提取视频信息（duration、cover、play_url）

❌ **不做什么**:
- 网络请求（由 XhsParser 管理）
- 错误处理细节（返回给 XhsParser）

## 依赖关系

```
core::xhs
  ├─→ models::xhs (XhsArticle, NoteDetail, NoteType, XhsVideo)
  ├─→ reqwest::blocking::Client   （网络请求）
  ├─→ scraper::Html/Selector      （HTML 解析）
  ├─→ serde_json                  （JSON 反序列化）
  ├─→ regex                        （不直接用，但 dependencies 中有）
  └─→ thiserror                    （ParserError derive）
```

## 错误处理

```rust
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
```

所有变体都有中文错误消息。

## 核心算法

### 1. 笔记类型识别（determine_note_type）

```rust
match (video, images.len()) {
    (Some(_), 0 | 1) => NoteType::Video,      // 有视频 + 0-1 图
    (Some(_), _)     => NoteType::Mixed,      // 有视频 + 2+ 图
    (None, 0)        => NoteType::Text,       // 无视频无图
    (None, _)        => NoteType::Images,     // 无视频 + 2+ 图
}
```

### 2. Redux State 数据路径

小红书在 `window.__INITIAL_STATE__` 存储 Redux state：

```
state.note.noteDetailMap[0].note
    └─ title
    └─ desc
    └─ user (XhsAuthor)
    └─ imageList (Vec<ImageItem>)
    └─ video (Option<JSON>)
```

### 3. JSON 清理规则

小红书 HTML 包含不严格的 JavaScript 对象，需要清理：
- `undefined` → `null`
- 尾部逗号（scraper 自动处理）

## 测试覆盖

| 测试 | 位置 | 用途 |
|------|------|------|
| `test_parse_video_note_from_sample_html` | mod.rs | 完整视频笔记解析流程 |
| `test_extract_author_info` | mod.rs | 作者信息准确性 |
| `test_extract_title_and_desc` | mod.rs | 标题和描述 |
| `test_extract_images` | mod.rs | 图片 URL 列表 |
| `test_extract_video_info` | mod.rs | 视频时长、封面、播放 URL |
| `test_note_type_detection_video` | mod.rs | 视频笔记类型识别 |
| `test_parse_pure_images_note` | mod.rs | 纯图片笔记解析 |
| `test_note_type_detection_pure_images` | mod.rs | 纯图片笔记类型识别 |

**缓存**: 
- `/tmp/xhs_test_cache.html` - 视频笔记（384KB）
- `/tmp/xhs_pure_images.html` - 纯图片笔记（391KB）
- 使用 `std::sync::OnceLock` 进程内缓存，避免重复网络请求

## 关键函数签名

```rust
// 公开 API
pub fn parse_by_url(url: &str) -> Result<XhsArticle, ParserError>
pub fn parse_from_html(html: &str) -> Result<XhsArticle, ParserError>

// 内部
fn fetch_html(&self, url: &str) -> Result<String, ParserError>
fn extract_initial_state(html: &str) -> Result<Value, ParserError>
fn build_article_from_state(state: Value) -> Result<XhsArticle, ParserError>
fn determine_note_type(video: &Option<XhsVideo>, images: &[String]) -> NoteType
fn extract_video_info(video_val: &Value) -> Option<XhsVideo>
```

## 反模式

❌ **禁止**:
- 在库代码中使用 `unwrap()` / `expect()`（仅测试允许）
- 返回 `Result<T, Box<dyn Error>>`（FFI 无法处理）
- 解析错误时直接打印（使用 `Error` trait）

## 改进建议（优先级）

### 高优先级

1. **抽象网络层**
   - 当前: `XhsParser` 包含 `reqwest::blocking::Client`，直接做网络请求
   - 建议: 定义 `trait HttpClient { fn get_text(&self, url: &str) -> Result<String>; }`
   - 好处: 便于测试（mock HTTP），支持未来的异步迁移

2. **单元测试去除网络依赖**
   - 当前: 测试进行真实 HTTP 请求（虽然有缓存）
   - 建议: 使用静态 HTML fixture 或 mock HttpClient
   - 好处: 测试更快、更稳定、无网络依赖

### 中优先级

3. **移动解析中间类型**
   - 当前: `NoteDetail`, `ImageItem` 在 models 模块（pub(crate)）
   - 建议: 移到 core::xhs 或 core::xhs::parser（serde 反序列化是解析的内部细节）
   - 好处: models 专注于公开 API 结构

4. **正则编译缓存**
   - 当前: api::xhs 每次调用 `extract_xhs_url` 都编译正则
   - 建议: 用 `OnceLock` 或 `lazy_static` 预编译
   - 好处: 避免重复编译

### 低优先级

5. **异步支持**
   - 当前: `reqwest::blocking` 同步 API
   - 建议: 未来考虑双实现或异步版本
   - 时机: 当 CookFollow 应用需要异步时

## 数据流

```
HTML 文本
    ↓
extract_initial_state() 
    ├─ scraper 找 <script> 标签
    ├─ 提取 window.__INITIAL_STATE__= 后的 JSON
    ├─ sanitize_json() 清理 undefined
    └─ 返回 serde_json::Value
    ↓
build_article_from_state()
    ├─ 导航 state.note.noteDetailMap[0].note
    ├─ serde_json::from_value() → NoteDetail
    └─ convert_note_to_article()
        ├─ determine_note_type()
        ├─ extract_video_info()
        └─ 返回 XhsArticle
```

---

**相关**: [../AGENTS.md](../AGENTS.md) 获取全局架构
