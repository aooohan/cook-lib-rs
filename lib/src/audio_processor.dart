import 'dart:async';

import 'native_decoder.dart';
import 'rust/api/audio.dart' as rust;

/// 音频处理进度
class AudioProcessProgress {
  /// 当前阶段
  final AudioProcessStage stage;

  /// 阶段描述
  final String message;

  AudioProcessProgress({
    required this.stage,
    required this.message,
  });
}

/// 处理阶段
enum AudioProcessStage {
  /// 解码音频
  decoding,

  /// 转录中
  transcribing,

  /// 完成
  complete,
}

/// 音频处理结果
class AudioProcessResult {
  /// 转录文本
  final String text;

  /// WAV 文件路径（可用于缓存）
  final String wavPath;

  AudioProcessResult({
    required this.text,
    required this.wavPath,
  });
}

/// 音频处理器 - 整合原生解码器 + Rust ASR
///
/// ```dart
/// final processor = await AudioProcessor.create(modelsDir: modelsPath);
///
/// // 方式1: 监听进度
/// await for (final progress in processor.process(videoPath)) {
///   print('Stage: ${progress.stage}');
/// }
/// final result = processor.lastResult;
///
/// // 方式2: 直接获取结果
/// final result = await processor.processAll(videoPath);
/// print('Text: ${result.text}');
///
/// processor.dispose();
/// ```
class AudioProcessor {
  final rust.AudioRecognizer _recognizer;
  AudioProcessResult? _lastResult;

  AudioProcessor._(this._recognizer);

  /// 创建音频处理器并加载模型
  ///
  /// [modelsDir] 下需要包含：
  /// - sherpa-ncnn/ (ASR 模型)
  /// - silero-vad/ (VAD 模型)
  static Future<AudioProcessor> create({required String modelsDir}) async {
    final recognizer = await rust.AudioRecognizer.create(modelsDir: modelsDir);
    return AudioProcessor._(recognizer);
  }

  /// 获取模型目录
  String get modelsDir => _recognizer.modelsDir;

  /// 上次处理结果
  AudioProcessResult? get lastResult => _lastResult;

  /// 处理音视频文件，返回进度流
  ///
  /// 支持视频文件（自动提取音轨）或音频文件
  Stream<AudioProcessProgress> process(
    String inputPath, {
    String? language,
  }) async* {
    // 阶段1: 解码音频到 WAV
    yield AudioProcessProgress(
      stage: AudioProcessStage.decoding,
      message: '正在提取音频...',
    );

    final wavPath = await MediaNativeDecoder.decodeAudioToWav(inputPath);

    // 阶段2: 转录
    yield AudioProcessProgress(
      stage: AudioProcessStage.transcribing,
      message: '正在转录语音...',
    );

    final text = await _recognizer.transcribeAudio(
      path: wavPath,
      language: language,
    );

    _lastResult = AudioProcessResult(
      text: text,
      wavPath: wavPath,
    );

    // 完成
    yield AudioProcessProgress(
      stage: AudioProcessStage.complete,
      message: '转录完成',
    );
  }

  /// 处理音视频文件，等待完成后返回结果
  Future<AudioProcessResult> processAll(
    String inputPath, {
    String? language,
  }) async {
    await for (final _ in process(inputPath, language: language)) {
      // 等待完成
    }
    return _lastResult!;
  }

  /// 直接转录 WAV 文件（跳过解码步骤）
  Future<String> transcribeWav(String wavPath, {String? language}) async {
    return await _recognizer.transcribeAudio(
      path: wavPath,
      language: language,
    );
  }

  /// 转录 PCM 数据
  Future<String> transcribePcm({
    required List<double> pcm,
    required int sampleRate,
    String? language,
  }) async {
    return await _recognizer.transcribePcm(
      pcm: pcm,
      sampleRate: sampleRate,
      language: language,
    );
  }

  /// 释放资源
  void dispose() {
    _recognizer.dispose();
  }
}
