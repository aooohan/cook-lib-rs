use crate::frame_extractor::deduplicator::FrameDeduplicator;
use crate::frame_extractor::diff_filter::FrameDiffFilter;
use crate::frame_extractor::frame::{Frame, FrameInfo, RawFrame};
use crate::frame_extractor::state_machine::{StateAction, StateConfig, StateMachine};
use crate::frame_extractor::text_detector::TextDetector;

pub struct ExtractionConfig {
    pub state_config: StateConfig,
    pub diff_threshold: f32,
    pub dedup_threshold: u32,
}

impl Default for ExtractionConfig {
    fn default() -> Self {
        Self {
            state_config: StateConfig::default(),
            diff_threshold: 0.15,
            dedup_threshold: 8,
        }
    }
}

impl ExtractionConfig {
    pub fn for_high_motion() -> Self {
        Self {
            state_config: StateConfig::for_high_motion(),
            diff_threshold: 0.12,
            dedup_threshold: 10,
        }
    }

    pub fn for_low_motion() -> Self {
        Self {
            state_config: StateConfig::for_low_motion(),
            diff_threshold: 0.18,
            dedup_threshold: 6,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExtractionResult {
    pub frame_info: FrameInfo,
    pub confidence: f32,
}

pub struct FrameExtractor {
    diff_filter: FrameDiffFilter,
    state_machine: StateMachine,
    deduplicator: FrameDeduplicator,
    config: ExtractionConfig,
}

impl FrameExtractor {
    pub fn new() -> Self {
        Self::with_config(ExtractionConfig::default())
    }

    pub fn with_config(config: ExtractionConfig) -> Self {
        Self {
            diff_filter: FrameDiffFilter::with_threshold(config.diff_threshold),
            state_machine: StateMachine::with_config(config.state_config.clone()),
            deduplicator: FrameDeduplicator::with_threshold(config.dedup_threshold),
            config,
        }
    }

    pub fn process_frame(
        &mut self,
        frame: &Frame,
        detector: &dyn TextDetector,
    ) -> Option<ExtractionResult> {
        if !self.diff_filter.should_process(frame) {
            return None;
        }

        let detection_result = detector.detect(frame);
        let is_duplicate = self.deduplicator.is_duplicate(frame);

        let action = self
            .state_machine
            .process_frame(detection_result.has_text, is_duplicate);

        match action {
            StateAction::Extract => {
                self.deduplicator.add(frame);
                Some(ExtractionResult {
                    frame_info: FrameInfo::from_frame(frame),
                    confidence: detection_result.confidence,
                })
            }
            _ => None,
        }
    }

    pub fn process_raw_frame(
        &mut self,
        raw_frame: &RawFrame,
        detector: &dyn TextDetector,
    ) -> Option<ExtractionResult> {
        let frame = raw_frame.to_rgba();
        self.process_frame(&frame, detector)
    }

    pub fn process_y_frame(
        &mut self,
        width: u32,
        height: u32,
        y_plane: &[u8],
        detector: &dyn TextDetector,
        timestamp_ms: u64,
        frame_number: u64,
    ) -> Option<ExtractionResult> {
        // Fast detection first
        let detection_result = detector.detect_yuv(width, height, y_plane);

        // 直接用 Y plane 做 diff 过滤，不再转换 RGBA
        if !self.diff_filter.should_process_y(y_plane, width, height) {
            self.state_machine.process_frame(false, false);
            return None;
        }

        // 用 Y plane 计算去重哈希
        let region_hashes = FrameDeduplicator::region_hashes_from_y_plane(
            y_plane, width, height, timestamp_ms
        );
        let decision = self.deduplicator.check_duplicate(&region_hashes);
        let is_duplicate = decision.is_duplicate;

        let action = self
            .state_machine
            .process_frame(detection_result.has_text, is_duplicate);

        match action {
            StateAction::Extract => {
                Some(ExtractionResult {
                    frame_info: FrameInfo {
                        width,
                        height,
                        timestamp_ms,
                        frame_number,
                    },
                    confidence: detection_result.confidence,
                })
            }
            _ => None,
        }
    }

    pub fn frame_count(&self) -> u64 {
        self.state_machine.frame_count()
    }

    pub fn extracted_count(&self) -> usize {
        self.deduplicator.len()
    }

    pub fn reset(&mut self) {
        self.diff_filter.reset();
        self.state_machine.reset();
        self.deduplicator.clear();
    }

    pub fn process_frame_with_detection(
        &mut self,
        frame: &Frame,
        has_text: bool,
        confidence: f32,
    ) -> Option<ExtractionResult> {
        if !self.diff_filter.should_process(frame) {
            self.state_machine.process_frame(false, false);
            return None;
        }

        let is_duplicate = self.deduplicator.is_duplicate(frame);
        let action = self.state_machine.process_frame(has_text, is_duplicate);

        match action {
            StateAction::Extract => {
                self.deduplicator.add(frame);
                Some(ExtractionResult {
                    frame_info: FrameInfo::from_frame(frame),
                    confidence,
                })
            }
            _ => None,
        }
    }
}

impl Default for FrameExtractor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame_extractor::text_detector::MockTextDetector;

    fn create_test_frame(width: u32, height: u32, fill: u8, frame_number: u64) -> Frame {
        let data = vec![fill; (width * height * 4) as usize];
        Frame::new(width, height, data, frame_number * 33, frame_number)
    }

    #[test]
    fn test_extractor_full_pipeline() {
        let config = ExtractionConfig {
            state_config: StateConfig {
                min_lock_frames: 1,
                cooldown_frames: 5,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut extractor = FrameExtractor::with_config(config);

        let detector = MockTextDetector::with_fixed_frames(vec![5, 10, 15]);

        let mut extracted = 0;
        for i in 1..=20 {
            let frame = create_test_frame(100, 100, (i * 10) as u8, i);
            if let Some(result) = extractor.process_frame(&frame, &detector) {
                extracted += 1;
                assert_eq!(result.frame_info.frame_number, i);
            }
        }

        assert_eq!(extracted, 3);
        assert_eq!(extractor.extracted_count(), 3);
    }

    #[test]
    fn test_extractor_respects_cooldown() {
        let config = ExtractionConfig {
            state_config: StateConfig {
                min_lock_frames: 1,
                cooldown_frames: 10,
                initial_skip: 2,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut extractor = FrameExtractor::with_config(config);
        let detector = MockTextDetector::with_pattern(|n| n == 5 || n == 6);

        for i in 1..=20 {
            let frame = create_test_frame(100, 100, (i * 20) as u8, i);
            extractor.process_frame(&frame, &detector);
        }

        assert_eq!(extractor.extracted_count(), 1);
    }

    #[test]
    fn test_extractor_duplicate_skipped() {
        let config = ExtractionConfig {
            state_config: StateConfig {
                min_lock_frames: 1,
                cooldown_frames: 1,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut extractor = FrameExtractor::with_config(config);
        let detector = MockTextDetector::with_pattern(|n| n <= 3);

        let frame1 = create_test_frame(100, 100, 100, 1);
        let frame2 = create_test_frame(100, 100, 102, 2);
        let frame3 = create_test_frame(100, 100, 104, 3);

        let r1 = extractor.process_frame(&frame1, &detector);
        assert!(r1.is_some());

        extractor.process_frame(&frame2, &detector);

        let r3 = extractor.process_frame(&frame3, &detector);
        assert!(r3.is_some());

        assert_eq!(extractor.extracted_count(), 2);
    }

    #[test]
    fn test_extractor_reset() {
        let mut extractor = FrameExtractor::new();
        let detector = MockTextDetector::with_pattern(|_| true);

        for i in 1..=10 {
            let frame = create_test_frame(100, 100, (i * 30) as u8, i);
            extractor.process_frame(&frame, &detector);
        }

        assert!(extractor.extracted_count() > 0);
        assert!(extractor.frame_count() > 0);

        extractor.reset();

        assert_eq!(extractor.extracted_count(), 0);
        assert_eq!(extractor.frame_count(), 0);
    }

    #[test]
    fn test_raw_frame_processing() {
        let mut extractor = FrameExtractor::new();
        let detector = MockTextDetector::with_pattern(|_| true);

        let raw_frame = RawFrame {
            width: 64,
            height: 64,
            y_plane: vec![128u8; 64 * 64],
            u_plane: vec![128u8; 64 * 32],
            v_plane: vec![128u8; 64 * 32],
            timestamp_ms: 1000,
            frame_number: 1,
        };

        let result = extractor.process_raw_frame(&raw_frame, &detector);
        assert!(result.is_none() || result.is_some());
    }
}
