import 'dart:io';
import 'package:flutter/material.dart';
import 'package:cook_lib/cook_lib.dart';
import 'pages/video_frame_extractor_page.dart';
import 'pages/transcribe_demo_page.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  if (Platform.isAndroid || Platform.isIOS) {
    await initCookLib();
    print('CookLib initialized');
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
            subtitle: const Text('AudioProcessor 语音识别'),
            onTap: () => Navigator.push(
              context,
              MaterialPageRoute(builder: (_) => const TranscribeDemoPage()),
            ),
          ),
          const Divider(),
          ListTile(
            leading: const Icon(Icons.video_library),
            title: const Text('视频帧提取'),
            subtitle: const Text('VideoProcessor 关键帧提取'),
            onTap: () => Navigator.push(
              context,
              MaterialPageRoute(
                builder: (_) => const VideoFrameExtractorPage(),
              ),
            ),
          ),
        ],
      ),
    );
  }
}
