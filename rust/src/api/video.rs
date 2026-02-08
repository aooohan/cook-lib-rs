//! 视频帧提取器

use crate::core::video::{ExtractionStats, FrameExtractedInfo, FrameExtractorManager, YFrameData};
use flutter_rust_bridge::frb;
use log::info;

/// 视频帧提取器 - 智能去重 + JPEG 压缩
///
/// ```dart
/// final extractor = VideoFrameExtractor.create();
/// final frames = extractor.processBatch(yuvFrames);
/// final stats = extractor.stats;
/// ```
#[frb(opaque)]
pub struct VideoFrameExtractor {
    manager: FrameExtractorManager,
}

impl VideoFrameExtractor {
    /// 创建视频帧提取器（无需模型）
    #[frb(sync)]
    pub fn create() -> Self {
        info!("🎬 VideoFrameExtractor: created");
        Self {
            manager: FrameExtractorManager::new(),
        }
    }

    /// 批量处理帧（智能去重）
    #[frb]
    pub fn process_batch(&self, frames: Vec<YFrameData>) -> Vec<FrameExtractedInfo> {
        self.manager.process_batch(frames)
    }

    /// 获取提取统计
    #[frb(sync, getter)]
    pub fn stats(&self) -> ExtractionStats {
        self.manager.get_stats()
    }

    /// 重置状态
    #[frb(sync)]
    pub fn reset(&self) {
        self.manager.reset()
    }
}

impl Drop for VideoFrameExtractor {
    fn drop(&mut self) {
        info!("🗑️ VideoFrameExtractor: released");
    }
}
