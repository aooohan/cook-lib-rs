import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:cook_lib/cook_lib.dart';
import 'package:dio/dio.dart';
import 'package:path_provider/path_provider.dart';
import 'package:permission_handler/permission_handler.dart';
import 'dart:io';
import 'dart:async';

class TranscribeDemoPage extends StatefulWidget {
  const TranscribeDemoPage({super.key});

  @override
  State<TranscribeDemoPage> createState() => _TranscribeDemoPageState();
}

class _TranscribeDemoPageState extends State<TranscribeDemoPage> {
  final _videoPathController = TextEditingController(
    text: Platform.isIOS
        ? '/Users/lihan/workplace/cook-follow/cook-video-test/lmth-cook.mp4'
        : '/data/local/tmp/lmth-cook.mp4',
  );
  String _modelPath = '初始化中...';
  String _status = '初始化中...';
  String _transcript = '';
  bool _busy = false;
  DateTime? _startTime;
  double _elapsedTime = 0.0;
  Timer? _timer;

  AudioProcessor? _audioProcessor;

  @override
  void initState() {
    super.initState();
    _requestPermissionsAndInitialize();
  }

  @override
  void dispose() {
    _timer?.cancel();
    _audioProcessor?.dispose();
    super.dispose();
  }

  Future<void> _requestPermissionsAndInitialize() async {
    try {
      await Permission.storage.request();
      await _initializeModel();
    } catch (e) {
      setState(() {
        _status = '权限请求失败: $e';
      });
    }
  }

  Future<void> _initializeModel() async {
    try {
      final docDir = await getApplicationDocumentsDirectory();
      final modelsDir = docDir.path;

      // Copy Sherpa-NCNN model
      final sherpaDir = Directory('$modelsDir/sherpa-ncnn');
      if (!sherpaDir.existsSync()) {
        setState(() {
          _status = '复制ASR模型文件中...';
        });
        sherpaDir.createSync(recursive: true);

        const modelPrefix = 'models/zipformer-ncnn';
        final files = [
          'encoder_jit_trace-pnnx.ncnn.param',
          'encoder_jit_trace-pnnx.ncnn.bin',
          'decoder_jit_trace-pnnx.ncnn.param',
          'decoder_jit_trace-pnnx.ncnn.bin',
          'joiner_jit_trace-pnnx.ncnn.param',
          'joiner_jit_trace-pnnx.ncnn.bin',
          'tokens.txt',
        ];

        for (final fileName in files) {
          print('Copying $fileName...');
          final data = await rootBundle.load('$modelPrefix/$fileName');
          await File('${sherpaDir.path}/$fileName')
              .writeAsBytes(data.buffer.asUint8List());
        }
        print('ASR model files copied');
      }

      // Copy Silero VAD model
      final vadDir = Directory('$modelsDir/silero-vad');
      if (!vadDir.existsSync()) {
        setState(() {
          _status = '复制VAD模型文件中...';
        });
        vadDir.createSync(recursive: true);

        final vadFiles = ['silero.ncnn.param', 'silero.ncnn.bin'];
        for (final fileName in vadFiles) {
          print('Copying VAD model: $fileName...');
          final data = await rootBundle.load('models/silero-vad/$fileName');
          await File('${vadDir.path}/$fileName')
              .writeAsBytes(data.buffer.asUint8List());
        }
        print('Silero VAD model copied');
      }

      setState(() {
        _modelPath = modelsDir;
        _status = '初始化AudioProcessor...';
      });

      print('Creating AudioProcessor with models: $modelsDir');
      _audioProcessor = await AudioProcessor.create(modelsDir: modelsDir);
      print('AudioProcessor created');

      setState(() {
        _status = '已就绪，请输入视频路径';
      });
    } catch (e) {
      setState(() {
        _status = '初始化失败: $e';
      });
    }
  }

  Future<String> _downloadVideo(String url) async {
    try {
      setState(() {
        _status = '下载视频中...';
      });

      final tempDir = await getTemporaryDirectory();
      final fileName = url.split('/').last.split('?').first;
      if (fileName.isEmpty) {
        throw Exception('无效的视频URL');
      }
      final savePath = '${tempDir.path}/$fileName';

      final dio = Dio();
      await dio.download(
        url,
        savePath,
        onReceiveProgress: (received, total) {
          if (total != -1) {
            final progress = (received / total * 100).toStringAsFixed(0);
            setState(() {
              _status = '下载中: $progress%';
            });
          }
        },
      );

      return savePath;
    } catch (e) {
      throw Exception('下载失败: $e');
    }
  }

  Future<void> _run() async {
    if (_busy) return;
    if (_audioProcessor == null) {
      setState(() {
        _status = 'AudioProcessor未初始化，请等待初始化完成';
      });
      return;
    }

    setState(() {
      _busy = true;
      _status = '处理中...';
      _transcript = '';
      _startTime = DateTime.now();
      _elapsedTime = 0.0;
    });

    _timer = Timer.periodic(const Duration(seconds: 1), (timer) {
      if (_startTime != null) {
        setState(() {
          _elapsedTime =
              DateTime.now().difference(_startTime!).inMilliseconds / 1000.0;
        });
      }
    });

    try {
      String videoPath = _videoPathController.text.trim();
      if (videoPath.isEmpty) {
        throw Exception('请先输入要转写的视频路径');
      }

      if (videoPath.startsWith('http')) {
        print('Downloading video from URL...');
        videoPath = await _downloadVideo(videoPath);
        print('Video downloaded to: $videoPath');
      }

      if (!File(videoPath).existsSync()) {
        throw Exception('视频文件不存在: $videoPath');
      }

      // Use AudioProcessor's Stream-based API for progress updates
      await for (final progress in _audioProcessor!.process(
        videoPath,
        language: 'zh',
      )) {
        setState(() {
          _status = progress.message;
        });
      }

      final result = _audioProcessor!.lastResult;
      if (result != null) {
        setState(() {
          _status = '完成';
          _transcript = result.text;
        });
      }
    } catch (e) {
      print('Error: $e');
      setState(() {
        _status = '出错: $e';
      });
    } finally {
      _timer?.cancel();
      if (_startTime != null) {
        _elapsedTime =
            DateTime.now().difference(_startTime!).inMilliseconds / 1000.0;
      }
      setState(() => _busy = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('语音转写测试')),
      body: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text('模型路径: $_modelPath'),
            const SizedBox(height: 16),
            TextField(
              controller: _videoPathController,
              decoration: const InputDecoration(
                labelText: '视频路径或URL',
                helperText: '网络URL (http://...)',
                border: OutlineInputBorder(),
              ),
            ),
            const SizedBox(height: 12),
            ElevatedButton(
              onPressed: _busy ? null : _run,
              child: const Text('转写视频'),
            ),
            const SizedBox(height: 12),
            Text('状态: $_status'),
            const SizedBox(height: 12),
            Text('耗时: ${_elapsedTime.toStringAsFixed(2)} s'),
            const SizedBox(height: 12),
            Expanded(
              child: Container(
                width: double.infinity,
                padding: const EdgeInsets.all(12),
                decoration: BoxDecoration(
                  border: Border.all(color: Colors.grey.shade400),
                  borderRadius: BorderRadius.circular(8),
                ),
                child: SingleChildScrollView(
                  child: Text(
                    _transcript.isEmpty ? '暂无结果' : _transcript,
                    softWrap: true,
                    style: const TextStyle(height: 1.5),
                  ),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
