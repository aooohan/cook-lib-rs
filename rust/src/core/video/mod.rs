pub mod deduplicator;
pub mod diff_filter;
pub mod frame;
pub mod manager;
pub mod pipeline;
pub mod state_machine;
pub mod text_detector;

pub use deduplicator::FrameDeduplicator;
pub use frame::{Frame, FrameInfo, RawFrame};
pub use manager::{ExtractionStats, FrameExtractedInfo, FrameExtractorManager, YFrameData};
pub use pipeline::{ExtractionConfig, ExtractionResult, FrameExtractor};
pub use state_machine::ExtractionState;
