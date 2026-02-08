/// Cook Lib - Recipe extraction library with ASR and video frame analysis
library cook_lib;

// Native decoder (Android video/audio decoding)
export 'src/native_decoder.dart';

// FRB initialization (必须暴露，用于初始化)
export 'src/rust/frb_generated.dart' show RustLib;

// Audio Processor (音频处理器 - Dart 整合层)
export 'src/audio_processor.dart';

// Video Processor (视频处理器 - Dart 整合层)
export 'src/video_processor.dart';

// Rust 层（高级用户直接使用）
export 'src/rust/api/audio.dart' show AudioRecognizer;
export 'src/rust/api/video.dart' show VideoFrameExtractor;

// Video types
export 'src/rust/core/video/manager.dart' show YFrameData, FrameExtractedInfo, ExtractionStats;

// XHS API (小红书解析)
export 'src/rust/api/xhs.dart';

// Models
export 'src/rust/api/models/xhs.dart';

// Error types
export 'src/rust/core/audio/error.dart';
