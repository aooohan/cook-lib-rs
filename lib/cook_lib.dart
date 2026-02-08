/// Cook Lib - Recipe extraction library with ASR and video frame analysis
library cook_lib;

// Native decoder (Android video/audio decoding)
export 'src/native_decoder.dart';

// FRB initialization (必须暴露，用于初始化)
export 'src/rust/frb_generated.dart' show RustLib;

// ASR API
export 'src/rust/api/audio.dart' show initSherpa, initVad, transcribeAudio, transcribePcm;

// Video API
export 'src/rust/api/video.dart';

// XHS API
export 'src/rust/api/xhs.dart';

// Models
export 'src/rust/api/models/xhs.dart';

// Audio error
export 'src/rust/core/audio/error.dart';
