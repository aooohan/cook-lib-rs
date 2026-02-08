import 'dart:io';
import 'package:flutter/material.dart';
import 'package:cook_lib/cook_lib.dart';
import 'pages/video_frame_extractor_page.dart';
import 'pages/transcribe_demo_page.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  if (Platform.isAndroid || Platform.isIOS) {
    await RustLib.init();
    print('RustLib initialized');
  }
  runApp(const MyApp());
}

class MyApp extends StatelessWidget {
  const MyApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Cook Lib Example',
      theme: ThemeData(
        colorScheme: ColorScheme.fromSeed(seedColor: Colors.deepPurple),
        useMaterial3: true,
      ),
      home: const MainMenuPage(),
    );
  }
}

class MainMenuPage extends StatelessWidget {
  const MainMenuPage({super.key});

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('CookLib Demo')),
      body: ListView(
        children: [
          ListTile(
            leading: const Icon(Icons.mic),
            title: const Text('语音转写'),
            subtitle: const Text('Sherpa + Paraformer 语音识别'),
            onTap: () => Navigator.push(
              context,
              MaterialPageRoute(builder: (_) => const TranscribeDemoPage()),
            ),
          ),
          const Divider(),
          ListTile(
            leading: const Icon(Icons.video_library),
            title: const Text('视频帧提取'),
            subtitle: const Text('文字检测 + 关键帧提取'),
            onTap: () => Navigator.push(
              context,
              MaterialPageRoute(
                builder: (_) => const VideoFrameExtractorPage(),
              ),
            ),
          ),
          const Divider(),
          ListTile(
            leading: const Icon(Icons.play_arrow),
            title: const Text('简单测试'),
            subtitle: const Text('快速测试原生解码 API'),
            onTap: () => Navigator.push(
              context,
              MaterialPageRoute(builder: (_) => const SimpleTestPage()),
            ),
          ),
        ],
      ),
    );
  }
}

class SimpleTestPage extends StatefulWidget {
  const SimpleTestPage({super.key});

  @override
  State<SimpleTestPage> createState() => _SimpleTestPageState();
}

class _SimpleTestPageState extends State<SimpleTestPage> {
  String _status = 'Ready';

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('简单测试')),
      body: Center(
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Text(_status),
            const SizedBox(height: 20),
            ElevatedButton(
              onPressed: _testAudioDecode,
              child: const Text('Test Audio Decode'),
            ),
            const SizedBox(height: 10),
            ElevatedButton(
              onPressed: _testVideoFrames,
              child: const Text('Test Video Frames'),
            ),
          ],
        ),
      ),
    );
  }

  Future<void> _testAudioDecode() async {
    setState(() => _status = 'Decoding audio...');
    try {
      final wavPath = await NativeDecoder.decodeAudioToWav(
        '/data/local/tmp/lmth-cook.mp4',
      );
      setState(() => _status = 'Audio decoded: $wavPath');
    } catch (e) {
      setState(() => _status = 'Error: $e');
    }
  }

  Future<void> _testVideoFrames() async {
    setState(() => _status = 'Extracting frames...');
    int frameCount = 0;
    try {
      await for (final event in NativeDecoder.extractVideoFrames(
        '/data/local/tmp/lmth-cook.mp4',
      )) {
        if (event.isFrame) {
          frameCount++;
          setState(() => _status = 'Frame $frameCount: ${event.width}x${event.height}');
        } else if (event.isComplete) {
          setState(() => _status = 'Complete: $frameCount frames');
          break;
        }
      }
    } catch (e) {
      setState(() => _status = 'Error: $e');
    }
  }
}
