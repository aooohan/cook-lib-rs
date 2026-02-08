//! 视频帧提取器 - 用于从小红书做菜视频中提取包含文字的关键帧
//!
//! 核心策略：
//! 1. 帧差预筛 - 使用 pHash + 直方图快速过滤无变化帧
//! 2. 状态机驱动 - 自适应调整采样频率
//! 3. 文字检测 - 仅检测文字区域，无需 OCR
//! 4. 帧去重 - 使用感知哈希避免重复

pub mod deduplicator;
pub mod diff_filter;
pub mod frame;
pub mod pipeline;
pub mod state_machine;
pub mod text_detector;

pub use deduplicator::{DedupDecision, DedupReason, FrameDeduplicator, RegionHashes};
pub use diff_filter::FrameDiffFilter;
pub use frame::{Frame, FrameInfo, RawFrame};
pub use pipeline::{ExtractionConfig, ExtractionResult, FrameExtractor};
pub use state_machine::{ExtractionState, StateMachine};
pub use text_detector::{CookingTextDetector, MockTextDetector, TextDetectionResult, TextDetector};
