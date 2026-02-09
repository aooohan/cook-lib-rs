import 'dart:async';
import 'package:flutter/services.dart';

/// 原生视频/音频解码器
///
/// 通过 Platform Channel 调用 iOS/Android 原生解码能力：
/// - MethodChannel: 用于音频解码等一次性调用
/// - EventChannel: 用于视频帧提取的流式数据传输
class MediaNativeDecoder {
  /// 音频解码通道 - 用于一次性解码请求
  static const MethodChannel _audioChannel = MethodChannel('cook_lib/audio/decoder');

  /// 视频帧流通道 - 用于流式帧数据传输
  static const EventChannel _videoFrameChannel = EventChannel('cook_lib/video/frame_stream');

  /// 解码音频为 WAV 格式
  ///
  /// [inputPath] 输入文件路径（视频或音频）
  /// 返回解码后的 WAV 文件路径
  static Future<String> decodeAudioToWav(String inputPath) async {
    final result = await _audioChannel.invokeMethod<String>(
      'decodeToWav',
      {'inputPath': inputPath},
    );
    return result!;
  }

  /// 提取视频帧
  ///
  /// 架构说明：
  /// - 参数通过 EventChannel.receiveBroadcastStream 传递
  /// - 原生端在 onListen 时读取参数并立即开始提取
  /// - 这是原子操作，避免了 MethodChannel + EventChannel 分离导致的竞态条件
  /// - 取消 Stream 订阅时，原生端 onCancel 会自动停止提取
  ///
  /// [videoPath] 视频文件路径
  /// 返回帧事件流（frame/progress/complete）
  static Stream<VideoFrameEvent> extractVideoFrames(String videoPath) {
    return _videoFrameChannel.receiveBroadcastStream({'videoPath': videoPath}).map((event) {
      final map = Map<String, dynamic>.from(event as Map);
      return VideoFrameEvent.fromMap(map);
    });
  }
}

/// 视频帧事件
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
