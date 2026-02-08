use crate::frame_extractor::frame::Frame;

#[derive(Debug, Clone)]
pub struct TextDetectionResult {
    pub has_text: bool,
    pub confidence: f32,
    pub text_region_count: u32,
}

pub trait TextDetector: Send + Sync {
    fn detect(&self, frame: &Frame) -> TextDetectionResult;

    /// Detect text directly from raw YUV data without RGBA conversion
    fn detect_yuv(&self, width: u32, height: u32, y_plane: &[u8]) -> TextDetectionResult {
        // Default: convert to Frame and use existing detect
        // Subclasses should override for efficiency
        let rgba: Vec<u8> = y_plane.iter().flat_map(|&y| [y, y, y, 255]).collect();
        let frame = Frame::new(width, height, rgba, 0, 0);
        self.detect(&frame)
    }
}

pub struct MockTextDetector {
    // 模拟在特定帧编号有文字
    text_frame_pattern: Option<Box<dyn Fn(u64) -> bool + Send + Sync>>,
}

impl MockTextDetector {
    pub fn new() -> Self {
        Self {
            text_frame_pattern: None,
        }
    }

    pub fn with_pattern<F>(pattern: F) -> Self
    where
        F: Fn(u64) -> bool + Send + Sync + 'static,
    {
        Self {
            text_frame_pattern: Some(Box::new(pattern)),
        }
    }

    pub fn with_fixed_frames(frames: Vec<u64>) -> Self {
        Self {
            text_frame_pattern: Some(Box::new(move |frame_num| frames.contains(&frame_num))),
        }
    }
}

impl Default for MockTextDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl TextDetector for MockTextDetector {
    fn detect(&self, frame: &Frame) -> TextDetectionResult {
        let has_text = self
            .text_frame_pattern
            .as_ref()
            .map(|p| p(frame.frame_number))
            .unwrap_or(false);

        TextDetectionResult {
            has_text,
            confidence: if has_text { 0.85 } else { 0.0 },
            text_region_count: if has_text { 2 } else { 0 },
        }
    }
}

/// 基于简单特征的轻量文字检测器（用于测试和降级场景）
pub struct SimpleFeatureDetector {
    edge_threshold: f32,
    texture_threshold: f32,
}

impl SimpleFeatureDetector {
    pub fn new() -> Self {
        Self {
            edge_threshold: 0.08,
            texture_threshold: 0.08,
        }
    }

    /// Fast edge detection using integer math and pixel skipping (every 2 pixels)
    /// This reduces computation by ~75% compared to checking every pixel
    fn detect_edges_fast(&self, gray_bytes: &[u8], width: u32, height: u32) -> f32 {
        let w = width as usize;
        let h = height as usize;
        let skip = 3; // Check every 3rd pixel
        let threshold_i32 = (self.edge_threshold * 255.0) as i32;
        let mut edge_count = 0;
        let mut total = 0;

        for y in (1..(h - 1)).step_by(skip) {
            for x in (1..(w - 1)).step_by(skip) {
                let idx = y * w + x;
                // Use integer math to avoid float conversion
                let gx = gray_bytes[idx + 1] as i32 - gray_bytes[idx - 1] as i32;
                let gy = gray_bytes[idx + w] as i32 - gray_bytes[idx - w] as i32;
                // Avoid sqrt: compare squared gradients
                let gradient_squared = gx * gx + gy * gy;
                let threshold_squared = threshold_i32 * threshold_i32;

                if gradient_squared > threshold_squared {
                    edge_count += 1;
                }
                total += 1;
            }
        }

        if total == 0 {
            0.0
        } else {
            edge_count as f32 / total as f32
        }
    }

    fn detect_edges(&self, gray: &[f32], width: u32, height: u32) -> f32 {
        let w = width as usize;
        let h = height as usize;
        let mut edge_count = 0;
        let mut total = 0;

        for y in 1..(h - 1) {
            for x in 1..(w - 1) {
                let idx = y * w + x;
                let gx = gray[idx + 1] - gray[idx - 1];
                let gy = gray[idx + w] - gray[idx - w];
                let gradient = (gx * gx + gy * gy).sqrt();

                if gradient > self.edge_threshold {
                    edge_count += 1;
                }
                total += 1;
            }
        }

        edge_count as f32 / total as f32
    }

    fn detect_texture(&self, gray: &[f32]) -> f32 {
        if gray.is_empty() {
            return 0.0;
        }

        let mean = gray.iter().sum::<f32>() / gray.len() as f32;
        let variance = gray.iter().map(|&v| (v - mean).powi(2)).sum::<f32>() / gray.len() as f32;

        variance.sqrt()
    }
}

impl Default for SimpleFeatureDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl TextDetector for SimpleFeatureDetector {
    fn detect(&self, frame: &Frame) -> TextDetectionResult {
        let gray: Vec<f32> = frame
            .data
            .chunks_exact(4)
            .map(|rgba| {
                let r = rgba[0] as f32 / 255.0;
                let g = rgba[1] as f32 / 255.0;
                let b = rgba[2] as f32 / 255.0;
                r * 0.299 + g * 0.587 + b * 0.114
            })
            .collect();

        let edge_density = self.detect_edges(&gray, frame.width, frame.height);
        let texture_score = self.detect_texture(&gray);

        let has_text = edge_density > 0.05 && texture_score > self.texture_threshold;

        TextDetectionResult {
            has_text,
            confidence: (edge_density + texture_score).min(1.0),
            text_region_count: if has_text { 1 } else { 0 },
        }
    }

    /// Fast path: use Y plane directly as grayscale, avoiding RGBA conversion
    fn detect_yuv(&self, width: u32, height: u32, y_plane: &[u8]) -> TextDetectionResult {
        // Y plane is already grayscale (0-255), use integer fast path
        let edge_density = self.detect_edges_fast(y_plane, width, height);

        // Compute texture from raw bytes to avoid allocation
        if y_plane.is_empty() {
            return TextDetectionResult {
                has_text: false,
                confidence: 0.0,
                text_region_count: 0,
            };
        }

        let mean = y_plane.iter().map(|&y| y as u32).sum::<u32>() as f32 / y_plane.len() as f32;
        let variance = y_plane
            .iter()
            .map(|&y| {
                let diff = y as f32 - mean;
                diff * diff
            })
            .sum::<f32>()
            / y_plane.len() as f32;
        let texture_score = (variance.sqrt()) / 255.0;

        // Lowered thresholds to account for downsampled frames and faster edge detection
        let has_text = edge_density > 0.015 && texture_score > self.texture_threshold;

        TextDetectionResult {
            has_text,
            confidence: (edge_density + texture_score).min(1.0),
            text_region_count: if has_text { 1 } else { 0 },
        }
    }
}

/// 做菜视频专用文字检测器
/// 针对做菜视频字幕特点优化：
/// 1. 字幕通常在底部 1/3 区域
/// 2. 字幕颜色通常是白色/黄色/黑色，对比度高
/// 3. 文字有水平笔画特征
pub struct CookingTextDetector {
    /// 亮度阈值 (0-255)，用于检测白字/黄字
    brightness_threshold: u8,
    /// 对比度阈值
    contrast_threshold: f32,
    /// 最小文字区域占比
    min_text_area_ratio: f32,
}

impl CookingTextDetector {
    pub fn new() -> Self {
        Self {
            brightness_threshold: 180, // 检测较亮的字幕
            contrast_threshold: 40.0,
            min_text_area_ratio: 0.005, // 0.5% 区域有文字特征
        }
    }

    /// 检测白色字幕条带特征
    /// 返回 (是否有字幕条带, 条带位置y, 条带高度)
    fn detect_subtitle_bands(&self, gray: &[u8], width: u32, height: u32) -> (bool, usize, usize) {
        let w = width as usize;
        let h = height as usize;

        // 只检测底部 40% 区域
        let start_y = h * 6 / 10;
        let check_height = h - start_y;

        // 每行的高亮度像素比例
        let mut row_brightness_ratio = Vec::with_capacity(check_height);

        for y in start_y..h {
            let mut bright_count = 0usize;
            let mut total = 0usize;

            for x in 0..w {
                let idx = y * w + x;
                if gray[idx] > self.brightness_threshold {
                    bright_count += 1;
                }
                total += 1;
            }

            let ratio = bright_count as f32 / total as f32;
            row_brightness_ratio.push(ratio);
        }

        // 找连续的高亮度行（字幕条带）
        let mut max_band_height = 0usize;
        let mut max_band_y = 0usize;
        let mut current_height = 0usize;
        let mut current_y = 0usize;

        for (i, &ratio) in row_brightness_ratio.iter().enumerate() {
            // 高亮度像素占比超过 15% 认为是字幕行
            if ratio > 0.15 {
                if current_height == 0 {
                    current_y = i;
                }
                current_height += 1;
            } else {
                if current_height > max_band_height {
                    max_band_height = current_height;
                    max_band_y = current_y;
                }
                current_height = 0;
            }
        }

        // 检查最后一个条带
        if current_height > max_band_height {
            max_band_height = current_height;
            max_band_y = current_y;
        }

        // 字幕条带高度通常在 20-80 像素之间（相对 640px 高度约 3-12%）
        let min_band_height = (h as f32 * 0.03) as usize;
        let max_band_height_limit = (h as f32 * 0.15) as usize;

        let has_subtitle = max_band_height >= min_band_height
            && max_band_height <= max_band_height_limit
            && max_band_height > 0;

        (has_subtitle, start_y + max_band_y, max_band_height)
    }

    /// 计算字幕条带区域的特征哈希（用于去重）
    pub fn subtitle_band_hash(&self, gray: &[u8], width: u32, height: u32) -> Option<(u64, usize, usize)> {
        let (has_subtitle, band_y, band_height) = self.detect_subtitle_bands(gray, width, height);

        if !has_subtitle {
            return None;
        }

        let w = width as usize;
        let band_hash = self.compute_band_hash(gray, w, band_y, band_height);

        Some((band_hash, band_y, band_height))
    }

    /// 计算条带区域的哈希
    fn compute_band_hash(&self, gray: &[u8], width: usize, band_y: usize, band_height: usize) -> u64 {
        let mut samples = [0u32; 64];
        let mut sum = 0u32;

        let block_w = width.max(1) / 8;
        let block_h = band_height.max(1) / 8;

        for by in 0..8 {
            for bx in 0..8 {
                let mut block_sum = 0u32;
                let mut count = 0u32;

                let y_start = (band_y + by * block_h).min(band_y + band_height);
                let y_end = (band_y + (by + 1) * block_h).min(band_y + band_height);
                let x_start = bx * block_w;
                let x_end = ((bx + 1) * block_w).min(width);

                for y in y_start..y_end {
                    let row_start = y * width;
                    for x in x_start..x_end {
                        let idx = row_start + x;
                        if idx < gray.len() {
                            block_sum += gray[idx] as u32;
                            count += 1;
                        }
                    }
                }

                let avg = if count > 0 { block_sum / count } else { 0 };
                samples[by * 8 + bx] = avg;
                sum += avg;
            }
        }

        let mean = sum / 64;
        let mut hash: u64 = 0;
        for (i, &val) in samples.iter().enumerate().take(48) {
            if val > mean {
                hash |= 1 << i;
            }
        }
        hash
    }

    /// 检测帧底部区域的文字
    /// 做菜视频字幕通常在底部 1/4 到 1/3 区域
    fn detect_bottom_region(&self, gray: &[u8], width: u32, height: u32) -> TextDetectionResult {
        let w = width as usize;
        let h = height as usize;

        // 只检测底部 30% 区域（字幕常见位置）
        let subtitle_height = (h as f32 * 0.3) as usize;
        let start_y = h - subtitle_height;

        // 计算底部区域的亮度和对比度
        let mut bright_pixels = 0u32;
        let mut total_pixels = 0u32;
        let mut sum_brightness = 0u64;
        let mut sum_squared_diff = 0u64;

        // 第一遍：计算均值
        for y in start_y..h {
            for x in 0..w {
                let idx = y * w + x;
                let pixel = gray[idx];
                sum_brightness += pixel as u64;
                total_pixels += 1;
            }
        }

        if total_pixels == 0 {
            return TextDetectionResult {
                has_text: false,
                confidence: 0.0,
                text_region_count: 0,
            };
        }

        let mean = (sum_brightness / total_pixels as u64) as u8;

        // 第二遍：计算方差和高亮像素
        for y in start_y..h {
            for x in 0..w {
                let idx = y * w + x;
                let pixel = gray[idx];

                // 检测高亮像素（可能是白字/黄字）
                if pixel > self.brightness_threshold {
                    bright_pixels += 1;
                }

                let diff = pixel as i32 - mean as i32;
                sum_squared_diff += (diff * diff) as u64;
            }
        }

        let variance = (sum_squared_diff / total_pixels as u64) as f32;
        let std_dev = variance.sqrt();

        // 检测水平边缘（文字有水平笔画）
        let horizontal_edge_ratio = self.detect_horizontal_edges(gray, w, h, start_y);

        // 高亮区域占比
        let bright_ratio = bright_pixels as f32 / total_pixels as f32;

        // 综合判断：
        // 1. 有足够的高亮像素（白字）
        // 2. 对比度足够高（文字 vs 背景）
        // 3. 有水平方向的边缘（文字笔画）
        let has_bright_text = bright_ratio > self.min_text_area_ratio;
        let has_high_contrast = std_dev > self.contrast_threshold;
        let has_horizontal_edges = horizontal_edge_ratio > 0.02;

        let has_text = has_bright_text && has_high_contrast && has_horizontal_edges;

        // 置信度计算
        let confidence = if has_text {
            let bright_score = (bright_ratio * 10.0).min(0.4);
            let contrast_score = (std_dev / 100.0).min(0.3);
            let edge_score = (horizontal_edge_ratio * 5.0).min(0.3);
            (bright_score + contrast_score + edge_score).min(1.0)
        } else {
            0.0
        };

        TextDetectionResult {
            has_text,
            confidence,
            text_region_count: if has_text { 1 } else { 0 },
        }
    }

    /// 检测水平方向的边缘（文字笔画特征）
    fn detect_horizontal_edges(&self, gray: &[u8], w: usize, h: usize, start_y: usize) -> f32 {
        let mut edge_count = 0u32;
        let mut total = 0u32;

        // 检测水平梯度（左右像素差）
        for y in start_y..h {
            for x in 1..(w - 1) {
                let idx = y * w + x;
                let left = gray[idx - 1] as i32;
                let right = gray[idx + 1] as i32;
                let diff = (right - left).abs();

                // 水平边缘阈值
                if diff > 30 {
                    edge_count += 1;
                }
                total += 1;
            }
        }

        if total == 0 {
            0.0
        } else {
            edge_count as f32 / total as f32
        }
    }
}

impl Default for CookingTextDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl TextDetector for CookingTextDetector {
    fn detect(&self, frame: &Frame) -> TextDetectionResult {
        // 转换为灰度图
        let gray: Vec<u8> = frame
            .data
            .chunks_exact(4)
            .map(|rgba| {
                // RGB to grayscale
                let r = rgba[0] as u32;
                let g = rgba[1] as u32;
                let b = rgba[2] as u32;
                ((r * 299 + g * 587 + b * 114) / 1000) as u8
            })
            .collect();

        self.detect_bottom_region(&gray, frame.width, frame.height)
    }

    /// 直接使用 Y 平面（已经是灰度）
    fn detect_yuv(&self, width: u32, height: u32, y_plane: &[u8]) -> TextDetectionResult {
        self.detect_bottom_region(y_plane, width, height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_frame(width: u32, height: u32, pattern: u8, frame_number: u64) -> Frame {
        let data = vec![pattern; (width * height * 4) as usize];
        Frame::new(width, height, data, 0, frame_number)
    }

    #[test]
    fn test_mock_detector_with_pattern() {
        let detector = MockTextDetector::with_pattern(|n| n % 10 == 0);

        let frame_with_text = create_test_frame(100, 100, 128, 10);
        let result = detector.detect(&frame_with_text);
        assert!(result.has_text);
        assert_eq!(result.text_region_count, 2);

        let frame_without_text = create_test_frame(100, 100, 128, 5);
        let result = detector.detect(&frame_without_text);
        assert!(!result.has_text);
        assert_eq!(result.text_region_count, 0);
    }

    #[test]
    fn test_mock_detector_with_fixed_frames() {
        let detector = MockTextDetector::with_fixed_frames(vec![5, 10, 15]);

        assert!(
            detector
                .detect(&create_test_frame(100, 100, 128, 5))
                .has_text
        );
        assert!(
            detector
                .detect(&create_test_frame(100, 100, 128, 10))
                .has_text
        );
        assert!(
            !detector
                .detect(&create_test_frame(100, 100, 128, 7))
                .has_text
        );
    }

    #[test]
    fn test_simple_detector_edges() {
        let detector = SimpleFeatureDetector::new();

        let uniform_frame = create_test_frame(64, 64, 128, 0);
        let result = detector.detect(&uniform_frame);
        assert!(!result.has_text);

        let high_contrast = create_test_frame(64, 64, 0, 0);
        let result = detector.detect(&high_contrast);
        assert!(!result.has_text);
    }
}
