import 'dart:async';
import 'dart:typed_data';
import 'package:flutter/services.dart';

/// Native video/audio decoder for Android
class NativeDecoder {
  static const MethodChannel _methodChannel = MethodChannel('cook_lib/methods');
  static const EventChannel _eventChannel = EventChannel('cook_lib/video_frames');

  /// Decode audio from video/audio file to WAV format
  /// Returns the path to the decoded WAV file
  static Future<String> decodeAudioToWav(String inputPath) async {
    final result = await _methodChannel.invokeMethod<String>(
      'decodeAudioToWav',
      {'inputPath': inputPath},
    );
    return result!;
  }

  /// Start extracting video frames
  /// Returns a stream of frame events
  static Stream<VideoFrameEvent> extractVideoFrames(String videoPath) {
    _methodChannel.invokeMethod('extractVideoFrames', {'videoPath': videoPath});
    return _eventChannel.receiveBroadcastStream().map((event) {
      final map = Map<String, dynamic>.from(event as Map);
      return VideoFrameEvent.fromMap(map);
    });
  }

  /// Stop frame extraction
  static Future<void> stopExtraction() async {
    await _methodChannel.invokeMethod('stopExtraction');
  }
}

/// Video frame event from native decoder
class VideoFrameEvent {
  final String type; // 'frame', 'progress', 'complete'
  final int? width;
  final int? height;
  final Uint8List? yPlane;
  final int? timestampMs;
  final int? frameNumber;
  final double? progress;

  VideoFrameEvent({
    required this.type,
    this.width,
    this.height,
    this.yPlane,
    this.timestampMs,
    this.frameNumber,
    this.progress,
  });

  factory VideoFrameEvent.fromMap(Map<String, dynamic> map) {
    return VideoFrameEvent(
      type: map['type'] as String,
      width: map['width'] as int?,
      height: map['height'] as int?,
      yPlane: map['yPlane'] != null ? Uint8List.fromList(List<int>.from(map['yPlane'])) : null,
      timestampMs: map['timestampMs'] as int?,
      frameNumber: (map['frameNumber'] as num?)?.toInt(),
      progress: (map['progress'] as num?)?.toDouble(),
    );
  }

  bool get isFrame => type == 'frame';
  bool get isProgress => type == 'progress';
  bool get isComplete => type == 'complete';
}
