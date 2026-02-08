//! 帧提取管理器

use image::{GrayImage, ImageOutputFormat};
use rayon::prelude::*;
use std::io::Cursor;
use std::sync::{Arc, Mutex};

/// 帧提取信息
#[derive(Debug, Clone)]
pub struct FrameExtractedInfo {
    pub timestamp_ms: u64,
    pub frame_number: u64,
    pub confidence: f32,
    pub jpeg_data: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// Y 平面帧数据
#[derive(Debug, Clone)]
pub struct YFrameData {
    pub width: u32,
    pub height: u32,
    pub y_plane: Vec<u8>,
    pub timestamp_ms: u64,
    pub frame_number: u64,
}

/// 提取统计
#[derive(Debug, Clone)]
pub struct ExtractionStats {
    pub processed_frames: u64,
    pub extracted_frames: u64,
}

/// 帧提取管理器
pub struct FrameExtractorManager {
    frame_count: Arc<Mutex<u64>>,
    extracted_count: Arc<Mutex<u64>>,
}

impl FrameExtractorManager {
    pub fn new() -> Self {
        Self {
            frame_count: Arc::new(Mutex::new(0)),
            extracted_count: Arc::new(Mutex::new(0)),
        }
    }

    pub fn get_stats(&self) -> ExtractionStats {
        let count = self.frame_count.lock().unwrap();
        let extracted = self.extracted_count.lock().unwrap();
        ExtractionStats {
            processed_frames: *count,
            extracted_frames: *extracted,
        }
    }

    pub fn reset(&self) {
        if let Ok(mut count) = self.frame_count.lock() {
            *count = 0;
        }
        if let Ok(mut extracted) = self.extracted_count.lock() {
            *extracted = 0;
        }
    }

    /// 批量处理 - 智能文字状态去重
    pub fn process_batch(&self, frames: Vec<YFrameData>) -> Vec<FrameExtractedInfo> {
        let batch_len = frames.len() as u64;

        let frame_results: Vec<_> = frames
            .par_iter()
            .map(|f| {
                let cropped = Self::crop_y_plane(&f.y_plane, f.width, f.height, 0.11, 0.20);
                let content_info = Self::analyze_region(&cropped.data, cropped.width, cropped.height, 0, 100);
                (f, content_info, cropped)
            })
            .collect();

        let mut extracted = Vec::new();
        let mut last_content: Option<RegionState> = None;
        let mut last_kept_ms = 0;

        const MAX_INTERVAL_MS: u64 = 5000;

        for (frame_data, curr_content, cropped) in frame_results {
            let content_changed = Self::has_region_changed(&last_content, &curr_content);
            let time_force_keep = frame_data.timestamp_ms.saturating_sub(last_kept_ms) > MAX_INTERVAL_MS
                && curr_content.has_text;

            if content_changed || time_force_keep {
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

    fn crop_y_plane(y_plane: &[u8], width: u32, height: u32, top_ratio: f32, bottom_ratio: f32) -> CroppedYPlane {
        let w = width as usize;
        let h = height as usize;

        let top_crop = (h as f32 * top_ratio) as usize;
        let bottom_crop = (h as f32 * bottom_ratio) as usize;
        let crop_height = h - top_crop - bottom_crop;

        if crop_height == 0 || w == 0 {
            return CroppedYPlane { data: vec![], width: 0, height: 0 };
        }

        const TARGET_SIZE: usize = 512;

        let crop_size = crop_height.min(w);
        let x_offset = (w - crop_size) / 2;
        let y_offset = top_crop + (crop_height - crop_size) / 2;

        let scale = crop_size as f32 / TARGET_SIZE as f32;

        let mut scaled_data = Vec::with_capacity(TARGET_SIZE * TARGET_SIZE);

        for out_y in 0..TARGET_SIZE {
            for out_x in 0..TARGET_SIZE {
                let src_x = x_offset + (out_x as f32 * scale) as usize;
                let src_y = y_offset + (out_y as f32 * scale) as usize;

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

    fn compress_to_jpeg(gray_data: &[u8], width: u32, height: u32) -> Vec<u8> {
        if gray_data.is_empty() || width == 0 || height == 0 {
            return vec![];
        }

        let img = match GrayImage::from_raw(width, height, gray_data.to_vec()) {
            Some(img) => img,
            None => return vec![],
        };

        let mut buffer = Cursor::new(Vec::new());
        if img.write_to(&mut buffer, ImageOutputFormat::Jpeg(70)).is_ok() {
            buffer.into_inner()
        } else {
            vec![]
        }
    }

    fn has_region_changed(last: &Option<RegionState>, current: &RegionState) -> bool {
        match last {
            None => current.has_text,
            Some(prev) => {
                if prev.has_text != current.has_text {
                    return true;
                }
                if prev.has_text && current.has_text {
                    let dist = (prev.hash ^ current.hash).count_ones();
                    return dist > 4;
                }
                false
            }
        }
    }

    fn analyze_region(y_plane: &[u8], width: u32, height: u32, start_pct: u32, end_pct: u32) -> RegionState {
        let w = width as usize;
        let h = height as usize;
        let y_start = h * start_pct as usize / 100;
        let y_end = h * end_pct as usize / 100;

        if y_end <= y_start || w == 0 {
            return RegionState { has_text: false, hash: 0 };
        }

        let region_h = y_end - y_start;
        let mut row_features = vec![0u32; region_h];
        let mut row_jumps = vec![0u32; region_h];
        let mut feature_pixels = Vec::new();

        for y in y_start..y_end {
            let row_offset = y * w;
            let local_y = y - y_start;

            for x in 1..w-1 {
                let idx = row_offset + x;
                let val = y_plane[idx];

                if val > 140 {
                    let left = y_plane[idx-1] as i16;
                    let right = y_plane[idx+1] as i16;
                    let diff = (right - left).abs();

                    if diff > 25 {
                        row_features[local_y] += 1;
                        feature_pixels.push((x, local_y));

                        if x > 1 {
                            let prev_diff = (y_plane[idx-1] as i16 - y_plane[idx-2] as i16).abs();
                            if prev_diff < 10 {
                                row_jumps[local_y] += 1;
                            }
                        }
                    }
                }
            }
        }

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

impl Default for FrameExtractorManager {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy)]
struct RegionState {
    has_text: bool,
    hash: u64,
}

struct CroppedYPlane {
    data: Vec<u8>,
    width: u32,
    height: u32,
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
