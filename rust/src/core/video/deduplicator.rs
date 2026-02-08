use super::frame::Frame;
use super::text_detector::CookingTextDetector;
use std::collections::VecDeque;

/// 区域感知去重器 - 针对做菜视频优化
/// 分上/中/下三区计算哈希，字幕区（底部）权重最高
pub struct FrameDeduplicator {
    /// 最近的关键帧历史（用于时序比较）
    history: VecDeque<RegionHashes>,
    /// 字幕区（底部）汉明距离阈值
    text_threshold: u32,
    /// 配料区（顶部）汉明距离阈值
    ingredient_threshold: u32,
    /// 动作区（中部）汉明距离阈值
    action_threshold: u32,
    /// 保底时间间隔（毫秒）
    min_interval_ms: u64,
    /// 最后保留帧的时间戳
    last_keyframe_time_ms: u64,
    /// 锁定的字幕区域（Y坐标，高度）
    locked_subtitle_region: Option<(usize, usize)>,
    /// 区域浮动范围（像素）
    region_flex: usize,
}

/// 分区域哈希结构
#[derive(Debug, Clone, Copy)]
pub struct RegionHashes {
    pub top: u64,              // 配料区 (0-33%)
    pub mid: u64,              // 动作区 (33-67%)
    pub bot: u64,              // 字幕区 (67-100%)
    pub subtitle_band: u64,    // 字幕条带哈希（核心去重依据）
    pub has_subtitle: bool,    // 是否有字幕条带
    pub timestamp_ms: u64,
    pub width: u32,
    pub height: u32,
}

/// 去重决策结果
#[derive(Debug, Clone)]
pub struct DedupDecision {
    pub is_duplicate: bool,
    pub reason: DedupReason,
    pub similarity: f32,      // 0.0-1.0, 越高越相似
    pub text_distance: u32,   // 字幕区汉明距离
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DedupReason {
    NewScene,           // 新场景，保留
    TextChanged,        // 字幕变化，保留
    IngredientChanged,  // 配料变化，保留
    TooSimilar,         // 太相似，去重
    ForceInterval,      // 强制保底，保留
}

impl FrameDeduplicator {
    pub fn new() -> Self {
        Self {
            history: VecDeque::with_capacity(3),
            text_threshold: 10,
            ingredient_threshold: 14,
            action_threshold: 20,
            min_interval_ms: 400,
            last_keyframe_time_ms: 0,
            locked_subtitle_region: None,
            region_flex: 10,
        }
    }

    pub fn with_threshold(text_thresh: u32) -> Self {
        Self {
            history: VecDeque::with_capacity(3),
            text_threshold: text_thresh,
            ingredient_threshold: text_thresh + 4,
            action_threshold: text_thresh + 10,
            min_interval_ms: 250,
            last_keyframe_time_ms: 0,
            locked_subtitle_region: None,
            region_flex: 10,
        }
    }

    /// 兼容旧接口
    pub fn check_duplicate(&mut self, regions: &RegionHashes) -> DedupDecision {
        // 简化为直接比较传入的 regions
        let time_since_last = regions.timestamp_ms.saturating_sub(self.last_keyframe_time_ms);
        if time_since_last >= self.min_interval_ms {
            self.add_keyframe(*regions);
            return DedupDecision {
                is_duplicate: false,
                reason: DedupReason::ForceInterval,
                similarity: 0.0,
                text_distance: 64,
            };
        }

        if let Some(last) = self.history.back() {
            let text_dist = Self::hamming_distance(regions.subtitle_band, last.subtitle_band);
            let text_sim = 1.0 - (text_dist as f32 / 64.0);

            if text_dist > self.text_threshold {
                self.add_keyframe(*regions);
                return DedupDecision {
                    is_duplicate: false,
                    reason: DedupReason::TextChanged,
                    similarity: text_sim,
                    text_distance: text_dist,
                };
            }

            if text_sim > 0.75 {
                return DedupDecision {
                    is_duplicate: true,
                    reason: DedupReason::TooSimilar,
                    similarity: text_sim,
                    text_distance: text_dist,
                };
            }
        }

        self.add_keyframe(*regions);
        DedupDecision {
            is_duplicate: false,
            reason: DedupReason::NewScene,
            similarity: 0.0,
            text_distance: 64,
        }
    }

    /// 主去重逻辑 - 基于锁定的字幕区域
    /// 首帧检测字幕位置并锁定，后续只比较锁定区域（±浮动）
    pub fn check_duplicate_with_y_plane(
        &mut self,
        y_plane: &[u8],
        width: u32,
        height: u32,
        timestamp_ms: u64,
    ) -> DedupDecision {
        // 策略1：保底机制
        let time_since_last = timestamp_ms.saturating_sub(self.last_keyframe_time_ms);
        if time_since_last >= self.min_interval_ms {
            let region_hash = self.compute_locked_region_hash(y_plane, width, height);
            self.add_keyframe(region_hash);
            return DedupDecision {
                is_duplicate: false,
                reason: DedupReason::ForceInterval,
                similarity: 0.0,
                text_distance: 64,
            };
        }

        // 策略2：计算锁定区域的哈希并比较
        let current_hash = self.compute_locked_region_hash(y_plane, width, height);

        if let Some(last) = self.history.back() {
            let text_dist = Self::hamming_distance(current_hash.subtitle_band, last.subtitle_band);
            let text_sim = 1.0 - (text_dist as f32 / 64.0);

            // 字幕区变化大 → 保留
            if text_dist > self.text_threshold {
                self.add_keyframe(current_hash);
                return DedupDecision {
                    is_duplicate: false,
                    reason: DedupReason::TextChanged,
                    similarity: text_sim,
                    text_distance: text_dist,
                };
            }

            // 字幕区几乎相同 → 去重
            if text_sim > 0.75 {
                return DedupDecision {
                    is_duplicate: true,
                    reason: DedupReason::TooSimilar,
                    similarity: text_sim,
                    text_distance: text_dist,
                };
            }
        }

        // 默认保留
        self.add_keyframe(current_hash);
        DedupDecision {
            is_duplicate: false,
            reason: DedupReason::NewScene,
            similarity: 0.0,
            text_distance: 64,
        }
    }

    /// 计算锁定字幕区域的哈希
    fn compute_locked_region_hash(&mut self, y_plane: &[u8], width: u32, height: u32) -> RegionHashes {
        let h = height as usize;
        let w = width as usize;

        // 如果没有锁定字幕区域，先检测并锁定
        if self.locked_subtitle_region.is_none() {
            let detector = CookingTextDetector::new();
            if let Some((_, band_y, band_height)) = detector.subtitle_band_hash(y_plane, width, height) {
                self.locked_subtitle_region = Some((band_y, band_height));
                println!("🔒 字幕区域锁定: Y={}, H={}", band_y, band_height);
            }
        }

        // 使用锁定区域（±浮动）计算哈希
        let (y, hgt) = self.locked_subtitle_region.unwrap_or_else(|| {
            // 默认底部30%
            let default_y = h * 7 / 10;
            let default_h = h * 3 / 10;
            (default_y, default_h)
        });

        // 应用浮动
        let flex = self.region_flex;
        let y_start = y.saturating_sub(flex);
        let y_end = (y + hgt + flex).min(h);
        let actual_height = y_end - y_start;

        // 计算该区域的哈希
        let subtitle_hash = Self::phash_y_region(y_plane, w, h, 0, y_start, w, actual_height);

        // 同时计算完整三区的哈希（兼容旧逻辑）
        let top_h = h / 3;
        let mid_start = top_h;
        let bot_start = mid_start + h / 3;

        let top_hash = Self::phash_y_region(y_plane, w, h, 0, 0, w, top_h);
        let mid_hash = Self::phash_y_region(y_plane, w, h, 0, mid_start, w, h / 3);
        let bot_hash = Self::phash_y_region(y_plane, w, h, 0, bot_start, w, h - bot_start);

        RegionHashes {
            top: top_hash,
            mid: mid_hash,
            bot: bot_hash,
            subtitle_band: subtitle_hash,
            has_subtitle: self.locked_subtitle_region.is_some(),
            timestamp_ms: 0, // 需要外部更新
            width,
            height,
        }
    }

    /// 兼容旧接口 - 直接检查哈希
    pub fn is_hash_duplicate(&self, hash: u64) -> bool {
        // 简单检查是否与历史任意帧相似
        for prev in &self.history {
            let dist = Self::hamming_distance(hash, prev.bot); // 用字幕区比较
            if dist < self.text_threshold {
                return true;
            }
        }
        false
    }

    /// 兼容旧接口 - 检查完整帧
    pub fn is_duplicate(&mut self, frame: &Frame) -> bool {
        let regions = Self::compute_region_hashes(frame);
        let decision = self.check_duplicate(&regions);
        decision.is_duplicate
    }

    pub fn add(&mut self, frame: &Frame) {
        let regions = Self::compute_region_hashes(frame);
        self.add_keyframe(regions);
    }

    pub fn add_hash(&mut self, hash: u64) {
        // 简化：作为全区域相同的哈希添加
        let regions = RegionHashes {
            top: hash,
            mid: hash,
            bot: hash,
            subtitle_band: hash,
            has_subtitle: false,
            timestamp_ms: self.last_keyframe_time_ms,
            width: 0,
            height: 0,
        };
        self.add_keyframe(regions);
    }

    fn add_keyframe(&mut self, regions: RegionHashes) {
        self.history.push_back(regions);
        if self.history.len() > 3 {
            self.history.pop_front();
        }
        self.last_keyframe_time_ms = regions.timestamp_ms;
    }

    pub fn clear(&mut self) {
        self.history.clear();
        self.last_keyframe_time_ms = 0;
        self.locked_subtitle_region = None;
    }

    pub fn len(&self) -> usize {
        self.history.len()
    }

    pub fn is_empty(&self) -> bool {
        self.history.is_empty()
    }

    /// 计算分区域感知哈希
    pub fn compute_region_hashes(frame: &Frame) -> RegionHashes {
        let w = frame.width as usize;
        let h = frame.height as usize;

        // 分区边界（竖屏视频 9:16）
        let top_h = h / 3;      // 上区：配料/标题
        let mid_start = top_h;  // 中区：动作
        let mid_h = h / 3;
        let bot_start = mid_start + mid_h; // 下区：字幕

        // 分别计算三区的 pHash
        let top_hash = Self::phash_region(&frame.data, w, h, 0, 0, w, top_h);
        let mid_hash = Self::phash_region(&frame.data, w, h, 0, mid_start, w, mid_h);
        let bot_hash = Self::phash_region(&frame.data, w, h, 0, bot_start, w, h - bot_start);

        // 转换为灰度计算字幕条带哈希
        let gray: Vec<u8> = frame
            .data
            .chunks_exact(4)
            .map(|rgba| {
                let r = rgba[0] as u32;
                let g = rgba[1] as u32;
                let b = rgba[2] as u32;
                ((r * 299 + g * 587 + b * 114) / 1000) as u8
            })
            .collect();

        let detector = CookingTextDetector::new();
        let (subtitle_hash, has_subtitle) =
            if let Some((hash, _, _)) = detector.subtitle_band_hash(&gray, frame.width, frame.height) {
                (hash, true)
            } else {
                (bot_hash, false)
            };

        RegionHashes {
            top: top_hash,
            mid: mid_hash,
            bot: bot_hash,
            subtitle_band: subtitle_hash,
            has_subtitle,
            timestamp_ms: 0, // 需要外部设置
            width: frame.width,
            height: frame.height,
        }
    }

    /// 从 Y 平面直接计算区域哈希（更高效）
    /// 包含字幕条带检测和哈希
    pub fn region_hashes_from_y_plane(
        y_plane: &[u8],
        width: u32,
        height: u32,
        timestamp_ms: u64,
    ) -> RegionHashes {
        let w = width as usize;
        let h = height as usize;

        let top_h = h / 3;
        let mid_start = top_h;
        let mid_h = h / 3;
        let bot_start = mid_start + mid_h;

        let top_hash = Self::phash_y_region(y_plane, w, h, 0, 0, w, top_h);
        let mid_hash = Self::phash_y_region(y_plane, w, h, 0, mid_start, w, mid_h);
        let bot_hash = Self::phash_y_region(y_plane, w, h, 0, bot_start, w, h - bot_start);

        // 计算字幕条带哈希
        let detector = CookingTextDetector::new();
        let (subtitle_hash, has_subtitle) =
            if let Some((hash, _, _)) = detector.subtitle_band_hash(y_plane, width, height) {
                (hash, true)
            } else {
                (bot_hash, false) // 没检测到字幕条带，用底部区域哈希兜底
            };

        RegionHashes {
            top: top_hash,
            mid: mid_hash,
            bot: bot_hash,
            subtitle_band: subtitle_hash,
            has_subtitle,
            timestamp_ms,
            width,
            height,
        }
    }

    /// 计算指定区域的 pHash（从 RGBA 数据）
    fn phash_region(
        rgba_data: &[u8],
        img_w: usize,
        img_h: usize,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
    ) -> u64 {
        // 下采样到 8x8
        let block_w = w.max(1) / 8;
        let block_h = h.max(1) / 8;

        let mut samples = [0u32; 64];
        let mut sum = 0u32;

        for by in 0..8 {
            for bx in 0..8 {
                let mut block_sum = 0u32;
                let mut count = 0u32;

                let y_start = (y + by * block_h).min(img_h);
                let y_end = (y + (by + 1) * block_h).min(img_h);
                let x_start = (x + bx * block_w).min(img_w);
                let x_end = (x + (bx + 1) * block_w).min(img_w);

                for py in y_start..y_end {
                    for px in x_start..x_end {
                        let idx = (py * img_w + px) * 4;
                        if idx + 2 < rgba_data.len() {
                            // RGB to grayscale
                            let gray = (rgba_data[idx] as u32 * 299
                                + rgba_data[idx + 1] as u32 * 587
                                + rgba_data[idx + 2] as u32 * 114)
                                / 1000;
                            block_sum += gray;
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

        // 高16位存储平均亮度，用于快速过滤亮度差异大的帧
        let brightness = ((mean & 0xFFFF) as u64) << 48;
        hash | brightness
    }

    /// 计算指定区域的 pHash（从 Y 平面）
    fn phash_y_region(
        y_plane: &[u8],
        img_w: usize,
        img_h: usize,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
    ) -> u64 {
        let block_w = w.max(1) / 8;
        let block_h = h.max(1) / 8;

        let mut samples = [0u32; 64];
        let mut sum = 0u32;

        for by in 0..8 {
            for bx in 0..8 {
                let mut block_sum = 0u32;
                let mut count = 0u32;

                let y_start = (y + by * block_h).min(img_h);
                let y_end = (y + (by + 1) * block_h).min(img_h);
                let x_start = (x + bx * block_w).min(img_w);
                let x_end = (x + (bx + 1) * block_w).min(img_w);

                for py in y_start..y_end {
                    let row_start = py * img_w;
                    for px in x_start..x_end {
                        let idx = row_start + px;
                        if idx < y_plane.len() {
                            block_sum += y_plane[idx] as u32;
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

        let brightness = ((mean & 0xFFFF) as u64) << 48;
        hash | brightness
    }

    pub fn hamming_distance(a: u64, b: u64) -> u32 {
        (a ^ b).count_ones()
    }

    /// 兼容旧接口 - 计算全图 pHash
    pub fn phash(frame: &Frame) -> u64 {
        Self::compute_region_hashes(frame).bot // 返回字幕区哈希
    }

    /// 兼容旧接口 - 从 Y 平面计算哈希
    pub fn phash_from_y_plane(y_plane: &[u8], width: u32, height: u32) -> u64 {
        Self::region_hashes_from_y_plane(y_plane, width, height, 0).bot
    }
}

impl Default for FrameDeduplicator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_frame(width: u32, height: u32, fill: u8, frame_number: u64) -> Frame {
        let data = vec![fill; (width * height * 4) as usize];
        Frame::new(width, height, data, 0, frame_number)
    }

    #[test]
    fn test_time_based_force_keep() {
        let mut dedup = FrameDeduplicator::new();

        // 第一帧，时间 0
        let mut regions = FrameDeduplicator::compute_region_hashes(&create_test_frame(100, 100, 128, 0));
        regions.timestamp_ms = 0;
        let decision = dedup.check_duplicate(&regions);
        assert!(!decision.is_duplicate);
        assert_eq!(decision.reason, DedupReason::ForceInterval);

        // 100ms 后，太接近，应该去重（如果相似）
        regions.timestamp_ms = 100;
        let decision = dedup.check_duplicate(&regions);
        assert!(decision.is_duplicate);
    }

    #[test]
    fn test_text_region_change_keeps() {
        let mut dedup = FrameDeduplicator::new();

        // 创建上半部分亮、下半部分暗的帧
        let mut frame1 = create_test_frame(100, 100, 100, 0);
        // 修改下半部分为白色（模拟字幕）
        for y in 67..100 {
            for x in 0..100 {
                let idx = ((y * 100 + x) * 4) as usize;
                frame1.data[idx] = 255;
                frame1.data[idx + 1] = 255;
                frame1.data[idx + 2] = 255;
            }
        }

        let mut regions1 = FrameDeduplicator::compute_region_hashes(&frame1);
        regions1.timestamp_ms = 0;
        dedup.check_duplicate(&regions1);

        // 创建字幕区不同的帧
        let mut frame2 = create_test_frame(100, 100, 100, 0);
        for y in 67..100 {
            for x in 0..100 {
                let idx = ((y * 100 + x) * 4) as usize;
                frame2.data[idx] = 200; // 不同的字幕亮度
                frame2.data[idx + 1] = 200;
                frame2.data[idx + 2] = 200;
            }
        }

        let mut regions2 = FrameDeduplicator::compute_region_hashes(&frame2);
        regions2.timestamp_ms = 100; // 很接近的时间
        let decision = dedup.check_duplicate(&regions2);

        // 字幕区变化大，应该保留
        assert!(!decision.is_duplicate);
        assert_eq!(decision.reason, DedupReason::TextChanged);
    }

    #[test]
    fn test_hamming_distance() {
        assert_eq!(FrameDeduplicator::hamming_distance(0b0, 0b0), 0);
        assert_eq!(FrameDeduplicator::hamming_distance(0b1111, 0b0000), 4);
    }
}
