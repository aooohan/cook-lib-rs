import 'dart:typed_data';
import 'dart:ui' as ui;
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'dart:io';
import 'dart:async';
import 'package:path_provider/path_provider.dart';
import 'package:cook_lib/cook_lib.dart';

class VideoFrameExtractorPage extends StatefulWidget {
  const VideoFrameExtractorPage({super.key});

  @override
  State<VideoFrameExtractorPage> createState() =>
      _VideoFrameExtractorPageState();
}

class _VideoFrameExtractorPageState extends State<VideoFrameExtractorPage> {
  final _videoPathController = TextEditingController(
    text: '/data/local/tmp/lmth-cook.mp4',
  );
  String _status = '就绪';
  bool _busy = false;
  double _progress = 0.0;
  int _processedFrames = 0;
  int _extractedFrames = 0;
  int _totalFrameBytes = 0;
  List<ExtractedFrameInfo> _extractedFramesList = [];
  DateTime? _startTime;
  double _elapsedTime = 0.0;
  Timer? _timer;
  FrameExtractorManager? _extractor;

  static const EventChannel _frameEventChannel = EventChannel(
    'com.example.cook_lib/video_frame_events',
  );
  StreamSubscription? _frameSubscription;

  static const int _batchSize = 8;
  final List<YFrameData> _frameBatch = [];

  @override
  void initState() {
    super.initState();
    _initExtractor();
  }

  void _initExtractor() async {
    _extractor = FrameExtractorManager();
    _extractor?.reset();
    print('FrameExtractor initialized with CookingTextDetector');
  }

  Future<void> _startExtraction() async {
    if (_busy) return;

    String videoPath = _videoPathController.text.trim();
    if (videoPath.isEmpty) {
      setState(() => _status = '请输入视频路径');
      return;
    }

    if (!File(videoPath).existsSync()) {
      setState(() => _status = '视频文件不存在: $videoPath');
      return;
    }

    _extractor?.reset();
    _frameSubscription?.cancel();
    _frameBatch.clear();

    setState(() {
      _busy = true;
      _status = '开始解码视频...';
      _progress = 0.0;
      _processedFrames = 0;
      _extractedFrames = 0;
      _totalFrameBytes = 0;
      _extractedFramesList = [];
      _startTime = DateTime.now();
      _elapsedTime = 0.0;
    });

    _timer = Timer.periodic(const Duration(milliseconds: 100), (timer) {
      if (_startTime != null) {
        setState(() {
          _elapsedTime =
              DateTime.now().difference(_startTime!).inMilliseconds / 1000.0;
        });
      }
    });

    try {
      _frameSubscription = _frameEventChannel
          .receiveBroadcastStream({'videoPath': videoPath})
          .listen(
            _onFrameEvent,
            onError: _onFrameError,
            onDone: _onFrameDone,
            cancelOnError: false,
          );
    } catch (e) {
      setState(() {
        _status = '错误: $e';
        _busy = false;
      });
      _timer?.cancel();
    }
  }

  void _onFrameEvent(dynamic event) {
    if (event is Map) {
      final type = event['type'] as String?;

      if (type == 'frame') {
        _collectFrame(event);
      } else if (type == 'progress') {
        setState(() {
          _progress = (event['progress'] as num?)?.toDouble() ?? 0.0;
          _status = '处理中... ${(_progress * 100).toStringAsFixed(1)}%';
        });
      } else if (type == 'complete') {
        _flushBatch();
        final totalKB = (_totalFrameBytes / 1024).toStringAsFixed(1);
        setState(() {
          _status = '完成 - 提取 $_extractedFrames 帧，总大小 ${totalKB}KB';
          _busy = false;
          _progress = 1.0;
        });
        _timer?.cancel();
        _frameSubscription?.cancel();
      }
    }
  }

  void _onFrameError(dynamic error) {
    setState(() {
      _status = '错误: $error';
      _busy = false;
    });
    _timer?.cancel();
  }

  void _onFrameDone() {
    _flushBatch();
    setState(() {
      _busy = false;
    });
    _timer?.cancel();
  }

  void _collectFrame(Map event) {
    final width = event['width'] as int? ?? 0;
    final height = event['height'] as int? ?? 0;
    final yPlane = event['yPlane'] as Uint8List?;
    final timestampMs = event['timestampMs'] as int? ?? 0;
    final frameNumber = event['frameNumber'] as int? ?? 0;

    if (width > 0 && height > 0 && yPlane != null) {
      _frameBatch.add(
        YFrameData(
          width: width,
          height: height,
          yPlane: yPlane,
          timestampMs: BigInt.from(timestampMs),
          frameNumber: BigInt.from(frameNumber),
        ),
      );

      if (_frameBatch.length >= _batchSize) {
        _flushBatch();
      }
    }
  }

  Future<void> _flushBatch() async {
    if (_frameBatch.isEmpty || _extractor == null) return;

    try {
      final batchToProcess = _frameBatch.toList();
      _frameBatch.clear();

      final results = await _extractor!.processBatch(frames: batchToProcess);

      int batchBytes = 0;
      for (final result in results) {
        batchBytes += result.jpegData.length;
        _extractedFramesList.add(
          ExtractedFrameInfo(
            frameNumber: result.frameNumber.toInt(),
            timestampMs: result.timestampMs.toInt(),
            confidence: result.confidence,
            width: result.width,
            height: result.height,
            jpegData: Uint8List.fromList(result.jpegData),
          ),
        );
      }

      final stats = _extractor!.getStats();
      if (mounted) {
        setState(() {
          _processedFrames = stats.processedFrames.toInt();
          _extractedFrames = stats.extractedFrames.toInt();
          _totalFrameBytes += batchBytes;
        });
      }
    } catch (e) {
      print('Batch processing error: $e');
    }
  }

  Future<void> _saveExtractedFrames() async {
    if (_extractedFramesList.isEmpty) return;

    final directory = await getExternalStorageDirectory();
    final path = directory?.path ?? '';
    if (path.isEmpty) return;

    final saveDir = Directory('$path/extracted_debug');
    if (saveDir.existsSync()) {
      await saveDir.delete(recursive: true);
    }
    await saveDir.create(recursive: true);

    int savedCount = 0;
    for (var frame in _extractedFramesList) {
      final file = File('${saveDir.path}/frame_${frame.frameNumber}.jpg');
      await file.writeAsBytes(frame.jpegData);
      savedCount++;
    }

    if (mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('已保存 $savedCount 帧 (JPEG) 到 ${saveDir.path}')),
      );
    }
    print('Frames saved to: ${saveDir.path}');
  }

  void _stopExtraction() {
    _frameSubscription?.cancel();
    setState(() {
      _busy = false;
      _status = '已停止';
    });
    _timer?.cancel();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('视频帧提取测试'),
        actions: [
          IconButton(
            icon: const Icon(Icons.save),
            tooltip: '保存提取的帧',
            onPressed:
                _extractedFramesList.isEmpty ? null : _saveExtractedFrames,
          ),
          if (_busy)
            const Padding(
              padding: EdgeInsets.all(16.0),
              child: SizedBox(
                width: 20,
                height: 20,
                child: CircularProgressIndicator(
                  strokeWidth: 2,
                  color: Colors.white,
                ),
              ),
            ),
        ],
      ),
      body: Padding(
        padding: const EdgeInsets.all(16.0),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            TextField(
              controller: _videoPathController,
              decoration: const InputDecoration(
                labelText: '视频路径',
                hintText: '/data/local/tmp/lmth-cook.mp4',
                border: OutlineInputBorder(),
              ),
              enabled: !_busy,
            ),
            const SizedBox(height: 16),
            Row(
              children: [
                ElevatedButton.icon(
                  onPressed: _busy ? null : _startExtraction,
                  icon: const Icon(Icons.play_arrow),
                  label: const Text('开始提取'),
                ),
                const SizedBox(width: 16),
                if (_busy)
                  ElevatedButton.icon(
                    onPressed: _stopExtraction,
                    icon: const Icon(Icons.stop),
                    label: const Text('停止'),
                    style: ElevatedButton.styleFrom(
                      backgroundColor: Colors.red,
                    ),
                  ),
              ],
            ),
            const SizedBox(height: 24),
            LinearProgressIndicator(value: _busy ? _progress : 0.0),
            const SizedBox(height: 16),
            _buildStatCard('状态', _status),
            const SizedBox(height: 8),
            Row(
              children: [
                Expanded(child: _buildStatCard('已处理帧', '$_processedFrames')),
                const SizedBox(width: 8),
                Expanded(child: _buildStatCard('提取帧', '$_extractedFrames')),
                const SizedBox(width: 8),
                Expanded(
                  child: _buildStatCard(
                    '耗时',
                    '${_elapsedTime.toStringAsFixed(1)}s',
                  ),
                ),
              ],
            ),
            const SizedBox(height: 16),
            const Text('提取的帧:', style: TextStyle(fontWeight: FontWeight.bold)),
            const SizedBox(height: 8),
            Expanded(
              child: _extractedFramesList.isEmpty
                  ? const Center(child: Text('暂无提取的帧'))
                  : GridView.builder(
                      gridDelegate:
                          const SliverGridDelegateWithFixedCrossAxisCount(
                        crossAxisCount: 3,
                        crossAxisSpacing: 8,
                        mainAxisSpacing: 8,
                        childAspectRatio: 1.0,
                      ),
                      itemCount: _extractedFramesList.length,
                      itemBuilder: (context, index) {
                        final frame = _extractedFramesList[index];
                        return _buildFrameThumbnail(frame, index);
                      },
                    ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildFrameThumbnail(ExtractedFrameInfo frame, int index) {
    return GestureDetector(
      onTap: () => _showFrameDetail(frame),
      child: Card(
        clipBehavior: Clip.antiAlias,
        child: Stack(
          fit: StackFit.expand,
          children: [
            FrameImageWidget(frame: frame, fit: BoxFit.cover),
            Positioned(
              bottom: 0,
              left: 0,
              right: 0,
              child: Container(
                color: Colors.black54,
                padding: const EdgeInsets.symmetric(vertical: 4, horizontal: 6),
                child: Text(
                  '${(frame.timestampMs / 1000).toStringAsFixed(1)}s',
                  style: const TextStyle(color: Colors.white, fontSize: 11),
                  textAlign: TextAlign.center,
                ),
              ),
            ),
            Positioned(
              top: 4,
              right: 4,
              child: Container(
                padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 2),
                decoration: BoxDecoration(
                  color: Colors.blue,
                  borderRadius: BorderRadius.circular(4),
                ),
                child: Text(
                  '#${index + 1}',
                  style: const TextStyle(
                    color: Colors.white,
                    fontSize: 10,
                    fontWeight: FontWeight.bold,
                  ),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }

  void _showFrameDetail(ExtractedFrameInfo frame) {
    showDialog(
      context: context,
      barrierDismissible: true,
      builder: (context) => Dialog.fullscreen(
        child: Scaffold(
          appBar: AppBar(
            title: Text('帧 #${frame.frameNumber}'),
            leading: IconButton(
              icon: const Icon(Icons.close),
              onPressed: () => Navigator.pop(context),
            ),
          ),
          body: Column(
            children: [
              Expanded(
                child: InteractiveViewer(
                  minScale: 0.5,
                  maxScale: 5.0,
                  boundaryMargin: const EdgeInsets.all(20),
                  child: Center(
                    child: FrameImageWidget(frame: frame, fit: BoxFit.contain),
                  ),
                ),
              ),
              Container(
                padding: const EdgeInsets.all(16),
                decoration: BoxDecoration(
                  color: Theme.of(context).colorScheme.surface,
                  boxShadow: [
                    BoxShadow(
                      color: Colors.black.withOpacity(0.1),
                      blurRadius: 4,
                      offset: const Offset(0, -2),
                    ),
                  ],
                ),
                child: SafeArea(
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      _buildInfoRow(
                        '时间',
                        '${(frame.timestampMs / 1000).toStringAsFixed(2)}s',
                      ),
                      _buildInfoRow('帧编号', '#${frame.frameNumber}'),
                      _buildInfoRow('尺寸', '${frame.width} x ${frame.height}'),
                      _buildInfoRow(
                        '置信度',
                        '${(frame.confidence * 100).toStringAsFixed(1)}%',
                      ),
                    ],
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _buildInfoRow(String label, String value) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 4),
      child: Row(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Text(
            '$label: ',
            style: TextStyle(fontSize: 14, color: Colors.grey[600]),
          ),
          Text(
            value,
            style: const TextStyle(fontSize: 14, fontWeight: FontWeight.w600),
          ),
        ],
      ),
    );
  }

  Widget _buildStatCard(String label, String value) {
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(12.0),
        child: Column(
          children: [
            Text(
              label,
              style: TextStyle(fontSize: 12, color: Colors.grey[600]),
            ),
            const SizedBox(height: 4),
            Text(
              value,
              style: const TextStyle(fontSize: 16, fontWeight: FontWeight.bold),
            ),
          ],
        ),
      ),
    );
  }

  @override
  void dispose() {
    _timer?.cancel();
    _frameSubscription?.cancel();
    _videoPathController.dispose();
    super.dispose();
  }
}

class ExtractedFrameInfo {
  final int frameNumber;
  final int timestampMs;
  final double confidence;
  final int width;
  final int height;
  final Uint8List jpegData;
  ui.Image? _cachedImage;

  ExtractedFrameInfo({
    required this.frameNumber,
    required this.timestampMs,
    required this.confidence,
    required this.width,
    required this.height,
    required this.jpegData,
  });

  Future<ui.Image> getImage() async {
    if (_cachedImage != null) return _cachedImage!;

    final completer = Completer<ui.Image>();
    ui.decodeImageFromList(jpegData, (image) {
      _cachedImage = image;
      completer.complete(image);
    });
    return completer.future;
  }
}

class FrameImageWidget extends StatefulWidget {
  final ExtractedFrameInfo frame;
  final BoxFit fit;

  const FrameImageWidget({
    super.key,
    required this.frame,
    this.fit = BoxFit.cover,
  });

  @override
  State<FrameImageWidget> createState() => _FrameImageWidgetState();
}

class _FrameImageWidgetState extends State<FrameImageWidget> {
  ui.Image? _image;

  @override
  void initState() {
    super.initState();
    _loadImage();
  }

  @override
  void didUpdateWidget(covariant FrameImageWidget oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.frame != widget.frame) {
      _loadImage();
    }
  }

  void _loadImage() {
    widget.frame.getImage().then((image) {
      if (mounted) {
        setState(() => _image = image);
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    if (_image == null) {
      return Container(
        color: Colors.grey[300],
        child: const Center(child: CircularProgressIndicator(strokeWidth: 2)),
      );
    }
    return RawImage(
      image: _image,
      fit: widget.fit,
      filterQuality: FilterQuality.high,
    );
  }
}
