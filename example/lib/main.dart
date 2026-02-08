import 'package:flutter/material.dart';
import 'package:cook_lib/cook_lib.dart';

void main() {
  runApp(const MyApp());
}

class MyApp extends StatelessWidget {
  const MyApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Cook Lib Example',
      home: const TestPage(),
    );
  }
}

class TestPage extends StatefulWidget {
  const TestPage({super.key});

  @override
  State<TestPage> createState() => _TestPageState();
}

class _TestPageState extends State<TestPage> {
  String _status = 'Ready';

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('Cook Lib Test')),
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
