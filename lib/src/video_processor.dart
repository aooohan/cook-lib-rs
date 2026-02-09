import 'dart:async';
import 'dart:typed_data';

import 'native_decoder.dart';
import 'rust/api/video.dart';
import 'rust/core/video/manager.dart';

/// 视频处理进度
class VideoProcessingProgress {
  /// 处理进度 (0.0 - 1.0)
  final double progress;

  /// 本批次提取的帧
  final List<FrameExtractedInfo> frames;

  /// 是否完成
  final bool isComplete;

  VideoProcessingProgress({
    required this.progress,
    required this.frames,
    this.isComplete = false,
  });
}

/// 视频处理结果
class VideoProcessingResult {
  /// 所有提取的帧
  final List<FrameExtractedInfo> frames;

  /// 处理统计
  final ExtractionStats stats;

  VideoProcessingResult({
    required this.frames,
    required this.stats,
  });
}

/// 视频处理器 - 整合原生解码器 + Rust 帧提取
///
/// ```dart
/// final processor = VideoProcessor.create();
///
/// // 方式1: 监听进度流
/// await for (final progress in processor.process(videoPath)) {
///   print('Progress: ${progress.progress}');
///   allFrames.addAll(progress.frames);
/// }
///
/// // 方式2: 直接获取结果
/// final result = await processor.processAll(videoPath);
/// print('Extracted ${result.frames.length} frames');
///
/// processor.dispose();
/// ```
class VideoProcessor {
  final VideoFrameExtractor _extractor;
  bool _isProcessing = false;
  StreamSubscription<VideoFrameEvent>? _subscription;

  VideoProcessor._(this._extractor);

  /// 创建视频处理器
  static VideoProcessor create() {
    final extractor = VideoFrameExtractor.create();
    return VideoProcessor._(extractor);
  }

  /// 是否正在处理
  bool get isProcessing => _isProcessing;

  /// 获取统计信息
  ExtractionStats get stats => _extractor.stats;

  /// 处理视频，返回进度流
  ///
  /// 每当有新帧提取出来时，会 yield 一个 [VideoProcessingProgress]
  Stream<VideoProcessingProgress> process(String videoPath) {
    if (_isProcessing) {
      throw StateError('Already processing a video');
    }

    _isProcessing = true;
    _extractor.reset();

    // 使用 StreamController 以支持外部取消
    final controller = StreamController<VideoProcessingProgress>();
    final List<_PendingFrame> batch = [];
    const int batchSize = 30;

    _subscription = MediaNativeDecoder.extractVideoFrames(videoPath).listen(
      (event) async {
        if (!_isProcessing) return;

        if (event.isFrame && event.yPlane != null) {
          batch.add(_PendingFrame(
            width: event.width!,
            height: event.height!,
            yPlane: event.yPlane!,
            timestampMs: event.timestampMs!,
            frameNumber: event.frameNumber!,
          ));

          // 批量处理
          if (batch.length >= batchSize) {
            // 暂停订阅，防止并发处理
            _subscription?.pause();
            final batchToProcess = List<_PendingFrame>.from(batch);
            batch.clear();

            final extracted = await _processBatch(batchToProcess);

            controller.add(VideoProcessingProgress(
              progress: event.progress ?? 0.0,
              frames: extracted,
            ));
            _subscription?.resume();
          }
        } else if (event.isProgress) {
          // 只有进度更新，无新帧
          controller.add(VideoProcessingProgress(
            progress: event.progress ?? 0.0,
            frames: [],
          ));
        } else if (event.isComplete) {
          // 处理剩余帧
          if (batch.isNotEmpty) {
            final batchToProcess = List<_PendingFrame>.from(batch);
            batch.clear();

            final extracted = await _processBatch(batchToProcess);

            controller.add(VideoProcessingProgress(
              progress: 1.0,
              frames: extracted,
            ));
          }

          controller.add(VideoProcessingProgress(
            progress: 1.0,
            frames: [],
            isComplete: true,
          ));
          controller.close();
        }
      },
      onError: (error) {
        controller.addError(error);
        controller.close();
      },
      onDone: () {
        _isProcessing = false;
        _subscription = null;
        if (!controller.isClosed) {
          controller.close();
        }
      },
    );

    // 当外部取消订阅时，也取消原生流
    controller.onCancel = () {
      _subscription?.cancel();
      _subscription = null;
      _isProcessing = false;
    };

    return controller.stream;
  }

  /// 处理视频，等待完成后返回结果
  Future<VideoProcessingResult> processAll(String videoPath) async {
    final allFrames = <FrameExtractedInfo>[];

    await for (final progress in process(videoPath)) {
      allFrames.addAll(progress.frames);
    }

    return VideoProcessingResult(
      frames: allFrames,
      stats: _extractor.stats,
    );
  }

  /// 停止当前处理
  ///
  /// 会立即取消原生流订阅，触发 EventChannel 的 onCancel
  void stop() {
    _subscription?.cancel();
    _subscription = null;
    _isProcessing = false;
  }

  /// 重置状态
  void reset() {
    _extractor.reset();
  }

  /// 释放资源
  void dispose() {
    stop();
    _extractor.dispose();
  }

  Future<List<FrameExtractedInfo>> _processBatch(List<_PendingFrame> batch) async {
    final yFrames = batch.map((f) => YFrameData(
      width: f.width,
      height: f.height,
      yPlane: f.yPlane,
      timestampMs: BigInt.from(f.timestampMs),
      frameNumber: BigInt.from(f.frameNumber),
    )).toList();

    return await _extractor.processBatch(frames: yFrames);
  }
}

class _PendingFrame {
  final int width;
  final int height;
  final Uint8List yPlane;
  final int timestampMs;
  final int frameNumber;

  _PendingFrame({
    required this.width,
    required this.height,
    required this.yPlane,
    required this.timestampMs,
    required this.frameNumber,
  });
}
