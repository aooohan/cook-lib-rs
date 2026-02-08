use crate::core::video::deduplicator::FrameDeduplicator;
use crate::core::video::frame::{Frame, RawFrame};
use crate::core::video::pipeline::{ExtractionConfig, FrameExtractor};
use crate::core::video::text_detector::{
    CookingTextDetector, MockTextDetector, TextDetector,
};
use image::{GrayImage, ImageOutputFormat};
use rayon::prelude::*;
use std::io::Cursor;
use std::sync::{Arc, Mutex};

/// 帧裁剪配置（针对9:16竖屏视频优化）
#[derive(Debug, Clone)]
pub struct FrameCropConfig {
    /// 顶部裁剪比例 (0.0-1.0)，默认 0.15 (15%)
    pub top_crop_ratio: f32,
    /// 底部裁剪比例 (0.0-1.0)，默认 0.20 (20%)
    pub bottom_crop_ratio: f32,
    /// 输出尺寸（正方形），默认 512
    pub output_size: u32,
}

impl Default for FrameCropConfig {
    fn default() -> Self {
        Self {
            top_crop_ratio: 0.15,    // 裁掉顶部15%（标题/水印）
            bottom_crop_ratio: 0.20, // 裁掉底部20%（字幕/水印）
            output_size: 512,        // 输出512x512
        }
    }
}

/// 裁剪后的帧数据（用于输出给多模态模型）
#[derive(Debug, Clone)]
pub struct CroppedFrame {
    /// 裁剪缩放后的 RGB 数据 (512*512*3 = 786432 bytes)
    pub rgb_data: Vec<u8>,
    /// 宽度（固定512）
    pub width: u32,
    /// 高度（固定512）
    pub height: u32,
    /// 原始时间戳
    pub timestamp_ms: u64,
    /// 原始帧编号
    pub frame_number: u64,
}

pub struct FrameExtractorManager {
    extractor: Arc<Mutex<FrameExtractor>>,
    detector: Arc<dyn TextDetector>,
    deduplicator: Arc<Mutex<FrameDeduplicator>>,
    frame_count: Arc<Mutex<u64>>,
    extracted_count: Arc<Mutex<u64>>, // 实际提取的帧数
}

impl FrameExtractorManager {
    #[flutter_rust_bridge::frb(sync)]
    pub fn new() -> Self {
        Self {
            extractor: Arc::new(Mutex::new(FrameExtractor::new())),
            detector: Arc::new(CookingTextDetector::new()),
            deduplicator: Arc::new(Mutex::new(FrameDeduplicator::with_threshold(8))),
            frame_count: Arc::new(Mutex::new(0)),
            extracted_count: Arc::new(Mutex::new(0)),
        }
    }

    #[flutter_rust_bridge::frb(sync)]
    pub fn with_mock_detector(text_frames: Vec<u64>) -> Self {
        Self {
            extractor: Arc::new(Mutex::new(FrameExtractor::new())),
            detector: Arc::new(MockTextDetector::with_fixed_frames(text_frames)),
            deduplicator: Arc::new(Mutex::new(FrameDeduplicator::with_threshold(8))),
            frame_count: Arc::new(Mutex::new(0)),
            extracted_count: Arc::new(Mutex::new(0)),
        }
    }

    #[flutter_rust_bridge::frb(sync)]
    pub fn process_frame(
        &self,
        width: u32,
        height: u32,
        rgba_data: Vec<u8>,
        timestamp_ms: u64,
        frame_number: u64,
    ) -> Option<FrameExtractedInfo> {
        let frame = Frame::new(width, height, rgba_data, timestamp_ms, frame_number);

        let mut extractor = self.extractor.lock().ok()?;
        extractor
            .process_frame(&frame, self.detector.as_ref())
            .map(|result| FrameExtractedInfo {
                timestamp_ms: result.frame_info.timestamp_ms,
                frame_number: result.frame_info.frame_number,
                confidence: result.confidence,
                jpeg_data: vec![], // 单帧接口暂不支持，请用 process_batch
                width: 0,
                height: 0,
            })
    }

    #[flutter_rust_bridge::frb(sync)]
    pub fn process_yuv_frame(
        &self,
        width: u32,
        height: u32,
        y_plane: Vec<u8>,
        u_plane: Vec<u8>,
        v_plane: Vec<u8>,
        timestamp_ms: u64,
        frame_number: u64,
    ) -> Option<FrameExtractedInfo> {
        let raw_frame = RawFrame {
            width,
            height,
            y_plane,
            u_plane,
            v_plane,
            timestamp_ms,
            frame_number,
        };

        let mut extractor = self.extractor.lock().ok()?;
        extractor
            .process_raw_frame(&raw_frame, self.detector.as_ref())
            .map(|result| FrameExtractedInfo {
                timestamp_ms: result.frame_info.timestamp_ms,
                frame_number: result.frame_info.frame_number,
                confidence: result.confidence,
                jpeg_data: vec![],
                width: 0,
                height: 0,
            })
    }

    #[flutter_rust_bridge::frb(sync)]
    pub fn process_y_frame(
        &self,
        width: u32,
        height: u32,
        y_plane: Vec<u8>,
        timestamp_ms: u64,
        frame_number: u64,
    ) -> Option<FrameExtractedInfo> {
        let mut extractor = self.extractor.lock().ok()?;
        extractor
            .process_y_frame(
                width,
                height,
                &y_plane,
                self.detector.as_ref(),
                timestamp_ms,
                frame_number,
            )
            .map(|result| FrameExtractedInfo {
                timestamp_ms: result.frame_info.timestamp_ms,
                frame_number: result.frame_info.frame_number,
                confidence: result.confidence,
                jpeg_data: vec![],
                width: 0,
                height: 0,
            })
    }

    #[flutter_rust_bridge::frb(sync)]
    pub fn get_stats(&self) -> ExtractionStats {
        let count = self.frame_count.lock().unwrap();
        let extracted = self.extracted_count.lock().unwrap();
        ExtractionStats {
            processed_frames: *count,
            extracted_frames: *extracted,
        }
    }

    #[flutter_rust_bridge::frb(sync)]
    pub fn reset(&self) {
        if let Ok(mut extractor) = self.extractor.lock() {
            extractor.reset();
        }
        if let Ok(mut dedup) = self.deduplicator.lock() {
            dedup.clear();
        }
        if let Ok(mut count) = self.frame_count.lock() {
            *count = 0;
        }
        if let Ok(mut extracted) = self.extracted_count.lock() {
            *extracted = 0;
        }
    }

    /// 异步批量处理 - 智能文字状态去重
    /// 策略：先裁剪缩放，再判断"有没有文字"，再判断"文字变没变"
    #[flutter_rust_bridge::frb]
    pub fn process_batch(&self, frames: Vec<YFrameData>) -> Vec<FrameExtractedInfo> {
        let batch_len = frames.len() as u64;

        // Step 1: 并行裁剪+分析每帧
        // 裁剪：去掉顶部15%、底部20%，保留中间65%
        // 缩放：降到 360 宽度进行分析（节省计算）
        let frame_results: Vec<_> = frames
            .par_iter()
            .map(|f| {
                // 裁剪 Y 平面（顶部11%，底部20%）
                let cropped = Self::crop_y_plane(&f.y_plane, f.width, f.height, 0.11, 0.20);

                // 在裁剪后的区域分析（现在只分析中间内容区，不再区分顶部底部）
                let content_info = Self::analyze_region(&cropped.data, cropped.width, cropped.height, 0, 100);
                (f, content_info, cropped)
            })
            .collect();

        let mut extracted = Vec::new();
        let mut last_content: Option<RegionState> = None;
        let mut last_kept_ms = 0;

        // 强制保留的时间间隔（防止长时间不输出）
        const MAX_INTERVAL_MS: u64 = 5000;

        for (frame_data, curr_content, cropped) in frame_results {
            // 检查内容区域变化
            let content_changed = Self::has_region_changed(&last_content, &curr_content);

            // 时间保底（但必须有文字内容才保底，避免空帧）
            let time_force_keep = frame_data.timestamp_ms.saturating_sub(last_kept_ms) > MAX_INTERVAL_MS
                && curr_content.has_text;

            // 有效变化或时间保底
            if content_changed || time_force_keep {
                // 将灰度数据压缩为 JPEG（质量 75，压缩率约 1/10）
                let jpeg_data = Self::compress_to_jpeg(&cropped.data, cropped.width, cropped.height);

                extracted.push(FrameExtractedInfo {
                    timestamp_ms: frame_data.timestamp_ms,
                    frame_number: frame_data.frame_number,
                    confidence: 1.0,
                    jpeg_data,
                    width: cropped.width,
                    height: cropped.height,
                });

                last_content = Some(curr_content);
                last_kept_ms = frame_data.timestamp_ms;
            }
        }

        if let Ok(mut count) = self.frame_count.lock() {
            *count += batch_len;
        }
        if let Ok(mut extracted_count) = self.extracted_count.lock() {
            *extracted_count += extracted.len() as u64;
        }

        extracted
    }

    /// 裁剪 Y 平面（去掉顶部和底部）并缩放到目标尺寸
    fn crop_y_plane(y_plane: &[u8], width: u32, height: u32, top_ratio: f32, bottom_ratio: f32) -> CroppedYPlane {
        let w = width as usize;
        let h = height as usize;

        let top_crop = (h as f32 * top_ratio) as usize;
        let bottom_crop = (h as f32 * bottom_ratio) as usize;
        let crop_height = h - top_crop - bottom_crop;

        if crop_height == 0 || w == 0 {
            return CroppedYPlane { data: vec![], width: 0, height: 0 };
        }

        // 目标尺寸：512x512（正方形，省Token）
        const TARGET_SIZE: usize = 512;

        // 计算裁剪区域（居中裁成正方形）
        let crop_size = crop_height.min(w);
        let x_offset = (w - crop_size) / 2;
        let y_offset = top_crop + (crop_height - crop_size) / 2;

        // 缩放比例
        let scale = crop_size as f32 / TARGET_SIZE as f32;

        // 缩放后的数据
        let mut scaled_data = Vec::with_capacity(TARGET_SIZE * TARGET_SIZE);

        for out_y in 0..TARGET_SIZE {
            for out_x in 0..TARGET_SIZE {
                // 映射到原图坐标
                let src_x = x_offset + (out_x as f32 * scale) as usize;
                let src_y = y_offset + (out_y as f32 * scale) as usize;

                // 确保不越界
                let src_x = src_x.min(w - 1);
                let src_y = src_y.min(h - 1);

                let idx = src_y * w + src_x;
                scaled_data.push(y_plane.get(idx).copied().unwrap_or(128));
            }
        }

        CroppedYPlane {
            data: scaled_data,
            width: TARGET_SIZE as u32,
            height: TARGET_SIZE as u32,
        }
    }

    /// 将灰度数据压缩为 JPEG
    fn compress_to_jpeg(gray_data: &[u8], width: u32, height: u32) -> Vec<u8> {
        if gray_data.is_empty() || width == 0 || height == 0 {
            return vec![];
        }

        // 创建灰度图
        let img = match GrayImage::from_raw(width, height, gray_data.to_vec()) {
            Some(img) => img,
            None => return vec![],
        };

        // 压缩为 JPEG（质量 70，平衡大小和清晰度）
        let mut buffer = Cursor::new(Vec::new());
        if img.write_to(&mut buffer, ImageOutputFormat::Jpeg(70)).is_ok() {
            buffer.into_inner()
        } else {
            vec![]
        }
    }

    /// 判断区域状态是否发生"有效变化"
    fn has_region_changed(last: &Option<RegionState>, current: &RegionState) -> bool {
        match last {
            None => current.has_text, // 第一帧，如果有字就保留，没字就不保留(视为没变)
            Some(prev) => {
                if prev.has_text != current.has_text {
                    return true; // 字幕 出现 或 消失
                }
                if prev.has_text && current.has_text {
                    // 都有字，比较内容哈希 (汉明距离 > 4 视为不同)
                    let dist = (prev.hash ^ current.hash).count_ones();
                    return dist > 4;
                }
                // 都没字 -> 视为没变（忽略背景变化）
                false
            }
        }
    }

    /// 分析区域特征 - 升级版：水平投影锁定
    fn analyze_region(y_plane: &[u8], width: u32, height: u32, start_pct: u32, end_pct: u32) -> RegionState {
        let w = width as usize;
        let h = height as usize;
        let y_start = h * start_pct as usize / 100;
        let y_end = h * end_pct as usize / 100;

        if y_end <= y_start || w == 0 {
            return RegionState { has_text: false, hash: 0 };
        }

        // 1. 水平投影：统计每一行的特征像素数
        let region_h = y_end - y_start;
        let mut row_features = vec![0u32; region_h];
        let mut row_jumps = vec![0u32; region_h]; // 新增：记录跳变次数
        // 记录特征像素的位置 (x, y)，用于后续哈希
        let mut feature_pixels = Vec::new();

        // 步长为1，为了更准确的水平投影
        for y in y_start..y_end {
            let row_offset = y * w;
            // 内部行索引
            let local_y = y - y_start;

            for x in 1..w-1 {
                let idx = row_offset + x;
                let val = y_plane[idx];

                // 宽松的高亮阈值
                if val > 140 {
                    // 严格的边缘检测 (左右梯度)
                    let left = y_plane[idx-1] as i16;
                    let right = y_plane[idx+1] as i16;
                    let diff = (right - left).abs();

                    // 只有高对比度边缘才算
                    if diff > 25 {
                        row_features[local_y] += 1;
                        feature_pixels.push((x, local_y));

                        // 统计跳变次数 (Jump Count)
                        // 用来区分"文字行"和"光滑物体边缘"
                        // 文字行会有密集的明暗交替，物体边缘通常是连续的
                        if x > 1 {
                            let prev_diff = (y_plane[idx-1] as i16 - y_plane[idx-2] as i16).abs();
                            // 如果前一个像素是平坦的，当前像素是边缘 -> 这是一个跳变起始点
                            if prev_diff < 10 {
                                row_jumps[local_y] += 1;
                            }
                        }
                    }
                }
            }
        }

        // 2. 锁定字幕行 (更严格的条件)
        // 条件A: 特征像素超过宽度 5% (已有)
        // 条件B: 跳变次数超过 5 次 (新增，排除光滑的长直线)
        let line_threshold = (w as f32 * 0.05) as u32;
        let jump_threshold = 5;
        let mut valid_lines = vec![false; region_h];
        let mut has_text_lines = false;

        for (y, &count) in row_features.iter().enumerate() {
            if count > line_threshold && row_jumps[y] > jump_threshold {
                valid_lines[y] = true;
                has_text_lines = true;
            }
        }

        if !has_text_lines {
            return RegionState { has_text: false, hash: 0 };
        }

        // 3. 只对"有效字幕行"计算哈希 (降级为 4x4 网格，提高容错率)
        // 忽略其他行的噪音 (如手部动作)
        let block_w = w / 4;
        let block_h = region_h / 4;
        let mut grid_features = [0u64; 16];

        for (x, y) in feature_pixels {
            if valid_lines[y] {
                let bx = (x / block_w.max(1)).min(3);
                let by = (y / block_h.max(1)).min(3);
                grid_features[by * 4 + bx] += 1;
            }
        }

        // 生成哈希 (16 bits)
        let mean = grid_features.iter().sum::<u64>() / 16;
        let mut hash = 0u64;
        for (i, &val) in grid_features.iter().enumerate() {
            if val > mean {
                hash |= 1 << i;
            }
        }

        RegionState { has_text: true, hash }
    }
}

#[derive(Debug, Clone, Copy)]
struct RegionState {
    has_text: bool,
    hash: u64,
}

/// 裁剪后的 Y 平面（内部使用）
struct CroppedYPlane {
    data: Vec<u8>,
    width: u32,
    height: u32,
}

#[derive(Debug, Clone)]
pub struct FrameExtractedInfo {
    pub timestamp_ms: u64,
    pub frame_number: u64,
    pub confidence: f32,
    /// JPEG 压缩后的图片数据
    pub jpeg_data: Vec<u8>,
    /// 图片宽度
    pub width: u32,
    /// 图片高度
    pub height: u32,
}

#[derive(Debug, Clone)]
pub struct YFrameData {
    pub width: u32,
    pub height: u32,
    pub y_plane: Vec<u8>,
    pub timestamp_ms: u64,
    pub frame_number: u64,
}

#[derive(Debug, Clone)]
pub struct ExtractionStats {
    pub processed_frames: u64,
    pub extracted_frames: u64,
}

#[flutter_rust_bridge::frb(sync)]
pub fn create_high_motion_config() -> ExtractionConfig {
    ExtractionConfig::for_high_motion()
}

#[flutter_rust_bridge::frb(sync)]
pub fn create_low_motion_config() -> ExtractionConfig {
    ExtractionConfig::for_low_motion()
}

/// 裁剪并缩放 YUV 帧（用于多模态模型输入）
///
/// 针对9:16竖屏视频：
/// - 裁掉顶部15%（标题/水印区）
/// - 裁掉底部20%（字幕/水印区）
/// - 保留中间65%（菜谱核心内容）
/// - 缩放到512x512（省Token）
#[flutter_rust_bridge::frb(sync)]
pub fn crop_and_resize_frame(
    y_plane: Vec<u8>,
    u_plane: Vec<u8>,
    v_plane: Vec<u8>,
    width: u32,
    height: u32,
    timestamp_ms: u64,
    frame_number: u64,
) -> CroppedFrame {
    crop_and_resize_frame_with_config(
        y_plane, u_plane, v_plane,
        width, height,
        timestamp_ms, frame_number,
        FrameCropConfig::default(),
    )
}

/// 带自定义配置的裁剪缩放
#[flutter_rust_bridge::frb(sync)]
pub fn crop_and_resize_frame_with_config(
    y_plane: Vec<u8>,
    u_plane: Vec<u8>,
    v_plane: Vec<u8>,
    width: u32,
    height: u32,
    timestamp_ms: u64,
    frame_number: u64,
    config: FrameCropConfig,
) -> CroppedFrame {
    let w = width as usize;
    let h = height as usize;

    // 1. 计算裁剪区域
    let top_crop = (h as f32 * config.top_crop_ratio) as usize;
    let bottom_crop = (h as f32 * config.bottom_crop_ratio) as usize;
    let crop_y_start = top_crop;
    let crop_y_end = h - bottom_crop;
    let crop_height = crop_y_end - crop_y_start;

    // 保持宽高比，计算水平裁剪（居中）
    // 9:16视频裁剪后变成 9:(16*0.65) ≈ 9:10.4
    // 为了输出正方形，需要裁掉左右
    let target_width = crop_height; // 正方形
    let crop_x_start = if w > target_width { (w - target_width) / 2 } else { 0 };
    let crop_x_end = if w > target_width { crop_x_start + target_width } else { w };
    let crop_width = crop_x_end - crop_x_start;

    let output_size = config.output_size as usize;

    // 2. 裁剪 + 缩放 + YUV->RGB 转换（一次遍历完成）
    let mut rgb_data = vec![0u8; output_size * output_size * 3];

    let x_ratio = crop_width as f32 / output_size as f32;
    let y_ratio = crop_height as f32 / output_size as f32;

    // UV平面尺寸（YUV420: UV是Y的1/4）
    let uv_w = w / 2;

    for out_y in 0..output_size {
        for out_x in 0..output_size {
            // 映射到裁剪区域的坐标
            let src_x = crop_x_start + (out_x as f32 * x_ratio) as usize;
            let src_y = crop_y_start + (out_y as f32 * y_ratio) as usize;

            // 确保不越界
            let src_x = src_x.min(w - 1);
            let src_y = src_y.min(h - 1);

            // 获取 Y 值
            let y_idx = src_y * w + src_x;
            let y_val = y_plane.get(y_idx).copied().unwrap_or(128) as i32;

            // 获取 UV 值（YUV420: 每2x2像素共享一个UV）
            let uv_x = src_x / 2;
            let uv_y = src_y / 2;
            let uv_idx = uv_y * uv_w + uv_x;
            let u_val = u_plane.get(uv_idx).copied().unwrap_or(128) as i32 - 128;
            let v_val = v_plane.get(uv_idx).copied().unwrap_or(128) as i32 - 128;

            // YUV -> RGB (BT.601)
            let r = (y_val + ((359 * v_val) >> 8)).clamp(0, 255) as u8;
            let g = (y_val - ((88 * u_val + 183 * v_val) >> 8)).clamp(0, 255) as u8;
            let b = (y_val + ((454 * u_val) >> 8)).clamp(0, 255) as u8;

            // 写入 RGB
            let out_idx = (out_y * output_size + out_x) * 3;
            rgb_data[out_idx] = r;
            rgb_data[out_idx + 1] = g;
            rgb_data[out_idx + 2] = b;
        }
    }

    CroppedFrame {
        rgb_data,
        width: config.output_size,
        height: config.output_size,
        timestamp_ms,
        frame_number,
    }
}

/// 批量裁剪缩放（并行处理）
#[flutter_rust_bridge::frb]
pub fn crop_and_resize_batch(
    frames: Vec<YuvFrameData>,
    config: FrameCropConfig,
) -> Vec<CroppedFrame> {
    frames
        .into_par_iter()
        .map(|f| {
            crop_and_resize_frame_with_config(
                f.y_plane, f.u_plane, f.v_plane,
                f.width, f.height,
                f.timestamp_ms, f.frame_number,
                config.clone(),
            )
        })
        .collect()
}

/// 完整的 YUV 帧数据（包含 U/V 平面）
#[derive(Debug, Clone)]
pub struct YuvFrameData {
    pub width: u32,
    pub height: u32,
    pub y_plane: Vec<u8>,
    pub u_plane: Vec<u8>,
    pub v_plane: Vec<u8>,
    pub timestamp_ms: u64,
    pub frame_number: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manager_creation() {
        let manager = FrameExtractorManager::new();
        let stats = manager.get_stats();
        assert_eq!(stats.processed_frames, 0);
        assert_eq!(stats.extracted_frames, 0);
    }

    fn create_frame_with_edges(width: u32, height: u32, frame_number: u64) -> YFrameData {
        let mut y_plane = vec![128u8; (width * height) as usize];
        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) as usize;
                if x % 4 == 0 || y % 4 == 0 {
                    y_plane[idx] = 255;
                } else {
                    y_plane[idx] = 0;
                }
            }
        }
        YFrameData {
            width,
            height,
            y_plane,
            timestamp_ms: frame_number * 33,
            frame_number,
        }
    }

    fn create_uniform_frame(width: u32, height: u32, value: u8, frame_number: u64) -> YFrameData {
        YFrameData {
            width,
            height,
            y_plane: vec![value; (width * height) as usize],
            timestamp_ms: frame_number * 33,
            frame_number,
        }
    }

    #[test]
    fn test_manager_batch_extracts_text_frames() {
        let manager = FrameExtractorManager::new();

        let frames = vec![
            create_uniform_frame(100, 100, 128, 1),
            create_frame_with_edges(100, 100, 2),
            create_uniform_frame(100, 100, 128, 3),
            create_frame_with_edges(100, 100, 4),
        ];

        let results = manager.process_batch(frames);
        assert!(results.len() >= 1);

        let stats = manager.get_stats();
        assert_eq!(stats.processed_frames, 4);
    }

    #[test]
    fn test_manager_batch_deduplicates() {
        let manager = FrameExtractorManager::new();

        let frames = vec![
            create_frame_with_edges(100, 100, 1),
            create_frame_with_edges(100, 100, 2),
            create_frame_with_edges(100, 100, 3),
        ];

        let results = manager.process_batch(frames);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_manager_reset() {
        let manager = FrameExtractorManager::new();

        let frames = vec![create_frame_with_edges(100, 100, 1)];

        manager.process_batch(frames);

        let stats_before = manager.get_stats();
        assert!(stats_before.extracted_frames > 0 || stats_before.processed_frames > 0);

        manager.reset();

        let stats_after = manager.get_stats();
        assert_eq!(stats_after.processed_frames, 0);
        assert_eq!(stats_after.extracted_frames, 0);
    }
}
