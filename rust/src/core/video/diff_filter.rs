use super::frame::Frame;

pub struct FrameDiffFilter {
    sample_size: (u32, u32),
    threshold: f32,
    last_hash: Option<u64>,
    last_histogram: Option<[u32; 64]>,
    // Y plane 版本的缓存
    last_y_hash: Option<u64>,
    last_y_histogram: Option<[u32; 64]>,
}

impl FrameDiffFilter {
    pub fn new() -> Self {
        Self {
            sample_size: (8, 8),
            threshold: 0.10,
            last_hash: None,
            last_histogram: None,
            last_y_hash: None,
            last_y_histogram: None,
        }
    }

    pub fn with_threshold(threshold: f32) -> Self {
        Self {
            sample_size: (8, 8),
            threshold,
            last_hash: None,
            last_histogram: None,
            last_y_hash: None,
            last_y_histogram: None,
        }
    }

    pub fn should_process(&mut self, frame: &Frame) -> bool {
        let resized = frame.resize_to(self.sample_size.0, self.sample_size.1);
        let (gray, mean) = Self::to_grayscale(&resized);

        let current_hash = Self::phash(&gray, mean);
        let current_histogram = Self::color_histogram(&resized);

        let should_process =
            if let (Some(last_hash), Some(last_hist)) = (self.last_hash, self.last_histogram) {
                let hash_diff = Self::hamming_distance(current_hash, last_hash) as f32 / 64.0;
                let hist_diff = Self::histogram_similarity(&current_histogram, &last_hist);

                let combined_score = hash_diff * 0.5 + (1.0 - hist_diff) * 0.5;
                combined_score > self.threshold
            } else {
                true
            };

        self.last_hash = Some(current_hash);
        self.last_histogram = Some(current_histogram);

        should_process
    }

    fn to_grayscale(frame: &Frame) -> (Vec<u8>, u8) {
        let mut sum = 0u32;
        let gray: Vec<u8> = frame
            .data
            .chunks_exact(4)
            .map(|rgba| {
                let val =
                    (rgba[0] as u32 * 299 + rgba[1] as u32 * 587 + rgba[2] as u32 * 114) / 1000;
                sum += val;
                val as u8
            })
            .collect();
        let mean = (sum / gray.len() as u32) as u8;
        (gray, mean)
    }

    fn phash(gray: &[u8], mean: u8) -> u64 {
        let mut hash: u64 = 0;
        for (i, &val) in gray.iter().enumerate().take(56) {
            if val > mean {
                hash |= 1 << i;
            }
        }

        let brightness = (mean as u64) << 56;
        hash | brightness
    }

    fn color_histogram(frame: &Frame) -> [u32; 64] {
        let mut hist = [0u32; 64];

        for chunk in frame.data.chunks_exact(4) {
            let gray = ((chunk[0] as u32 * 299 + chunk[1] as u32 * 587 + chunk[2] as u32 * 114)
                / 1000) as u8;
            let idx = (gray >> 2) as usize;
            hist[idx] += 1;
        }

        hist
    }

    fn hamming_distance(a: u64, b: u64) -> u32 {
        (a ^ b).count_ones()
    }

    fn histogram_similarity(h1: &[u32; 64], h2: &[u32; 64]) -> f32 {
        let dot: u32 = h1.iter().zip(h2.iter()).map(|(a, b)| a.min(b)).sum();
        let sum1: u32 = h1.iter().sum();
        let sum2: u32 = h2.iter().sum();

        if sum1 == 0 || sum2 == 0 {
            return 0.0;
        }

        dot as f32 / sum1.max(sum2) as f32
    }

    /// Y plane 版本的 should_process，避免 RGBA 转换
    pub fn should_process_y(&mut self, y_plane: &[u8], width: u32, height: u32) -> bool {
        let (gray, mean) = Self::downsample_y_plane(y_plane, width, height, self.sample_size.0, self.sample_size.1);

        let current_hash = Self::phash(&gray, mean);
        let current_histogram = Self::y_histogram(&gray);

        let should_process = if let (Some(last_hash), Some(last_hist)) = (self.last_y_hash, self.last_y_histogram) {
            let hash_diff = Self::hamming_distance(current_hash, last_hash) as f32 / 64.0;
            let hist_diff = Self::histogram_similarity(&current_histogram, &last_hist);

            let combined_score = hash_diff * 0.5 + (1.0 - hist_diff) * 0.5;
            combined_score > self.threshold
        } else {
            true
        };

        self.last_y_hash = Some(current_hash);
        self.last_y_histogram = Some(current_histogram);

        should_process
    }

    /// 下采样 Y plane 到指定大小
    fn downsample_y_plane(y_plane: &[u8], width: u32, height: u32, target_w: u32, target_h: u32) -> (Vec<u8>, u8) {
        let w = width as usize;
        let h = height as usize;
        let tw = target_w as usize;
        let th = target_h as usize;

        let block_w = w / tw;
        let block_h = h / th;

        let mut result = Vec::with_capacity(tw * th);
        let mut sum = 0u32;

        for by in 0..th {
            for bx in 0..tw {
                let mut block_sum = 0u32;
                let mut count = 0u32;

                let y_start = by * block_h;
                let y_end = ((by + 1) * block_h).min(h);
                let x_start = bx * block_w;
                let x_end = ((bx + 1) * block_w).min(w);

                for py in y_start..y_end {
                    let row_offset = py * w;
                    for px in x_start..x_end {
                        let idx = row_offset + px;
                        if idx < y_plane.len() {
                            block_sum += y_plane[idx] as u32;
                            count += 1;
                        }
                    }
                }

                let avg = if count > 0 { (block_sum / count) as u8 } else { 0 };
                result.push(avg);
                sum += avg as u32;
            }
        }

        let mean = if !result.is_empty() { (sum / result.len() as u32) as u8 } else { 0 };
        (result, mean)
    }

    /// Y plane 直方图（64 bins）
    fn y_histogram(gray: &[u8]) -> [u32; 64] {
        let mut hist = [0u32; 64];
        for &val in gray {
            let idx = (val >> 2) as usize;
            hist[idx] += 1;
        }
        hist
    }

    pub fn reset(&mut self) {
        self.last_hash = None;
        self.last_histogram = None;
        self.last_y_hash = None;
        self.last_y_histogram = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_frame(width: u32, height: u32, fill: u8) -> Frame {
        let data = vec![fill; (width * height * 4) as usize];
        Frame::new(width, height, data, 0, 0)
    }

    #[test]
    fn test_identical_frames() {
        let mut filter = FrameDiffFilter::new();
        let frame1 = create_test_frame(100, 100, 128);
        let frame2 = create_test_frame(100, 100, 128);

        assert!(filter.should_process(&frame1));
        assert!(!filter.should_process(&frame2));
    }

    #[test]
    fn test_different_frames() {
        let mut filter = FrameDiffFilter::new();
        let frame1 = create_test_frame(100, 100, 0);
        let frame2 = create_test_frame(100, 100, 255);

        assert!(filter.should_process(&frame1));
        assert!(filter.should_process(&frame2));
    }

    #[test]
    fn test_histogram_similarity() {
        let h1 = [1u32; 64];
        let h2 = [1u32; 64];
        let sim = FrameDiffFilter::histogram_similarity(&h1, &h2);
        assert!((sim - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_hamming_distance() {
        assert_eq!(FrameDiffFilter::hamming_distance(0b0, 0b0), 0);
        assert_eq!(FrameDiffFilter::hamming_distance(0b0, 0b1), 1);
        assert_eq!(FrameDiffFilter::hamming_distance(0b1111, 0b0000), 4);
    }
}
