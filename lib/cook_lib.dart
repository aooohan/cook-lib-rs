/// Cook Lib - Recipe extraction library with ASR and video frame analysis
library cook_lib;

// 初始化函数（用户使用这个）
export 'src/init.dart' show initCookLib;

// Native decoder (Android video/audio decoding)
export 'src/native_decoder.dart';

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
