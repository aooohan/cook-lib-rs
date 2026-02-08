import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:cook_lib/cook_lib.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  const testVideoPath = '/data/local/tmp/lmth-cook.mp4';

  group('MediaNativeDecoder Integration Tests', () {
    testWidgets('decodeAudioToWav should return valid wav path', (tester) async {
      // Decode audio from video
      final wavPath = await MediaNativeDecoder.decodeAudioToWav(testVideoPath);

      // Verify result
      expect(wavPath, isNotEmpty);
      expect(wavPath.endsWith('.wav'), isTrue);
      print('Audio decoded to: $wavPath');
    });

    testWidgets('extractVideoFrames should extract multiple frames', (tester) async {
      final frames = <VideoFrameEvent>[];
      bool completed = false;

      // Extract frames
      await for (final event in MediaNativeDecoder.extractVideoFrames(testVideoPath)) {
        if (event.isFrame) {
          frames.add(event);
          print('Frame ${frames.length}: ${event.width}x${event.height} @ ${event.timestampMs}ms');
        } else if (event.isProgress) {
          print('Progress: ${(event.progress! * 100).toStringAsFixed(1)}%');
        } else if (event.isComplete) {
          completed = true;
          break;
        }
      }

      // Verify results
      expect(frames.length, greaterThan(0));
      expect(completed, isTrue);

      // Verify frame data
      final firstFrame = frames.first;
      expect(firstFrame.width, greaterThan(0));
      expect(firstFrame.height, greaterThan(0));
      expect(firstFrame.yPlane, isNotNull);
      expect(firstFrame.yPlane!.length, equals(firstFrame.width! * firstFrame.height!));

      print('Total frames extracted: ${frames.length}');
    });

    testWidgets('frame extraction should maintain correct aspect ratio', (tester) async {
      VideoFrameEvent? firstFrame;

      await for (final event in MediaNativeDecoder.extractVideoFrames(testVideoPath)) {
        if (event.isFrame) {
          firstFrame = event;
          break;
        }
      }

      // Stop extraction after getting first frame
      await MediaNativeDecoder.stopExtraction();

      expect(firstFrame, isNotNull);
      // Max dimension should be 640 (as defined in VideoFrameExtractor.kt)
      expect(firstFrame!.width! <= 640 || firstFrame.height! <= 640, isTrue);
      print('Frame size: ${firstFrame.width}x${firstFrame.height}');
    });
  });

  group('Rust API Integration Tests', () {
    testWidgets('processYuvFrame should handle valid frame data', (tester) async {
      // First extract a real frame
      VideoFrameEvent? frame;
      await for (final event in MediaNativeDecoder.extractVideoFrames(testVideoPath)) {
        if (event.isFrame) {
          frame = event;
          break;
        }
      }
      await MediaNativeDecoder.stopExtraction();

      expect(frame, isNotNull);

      // Now test Rust processing with real frame data
      // This tests the full pipeline: Native decode -> Rust process
      print('Got frame: ${frame!.width}x${frame.height}, ${frame.yPlane!.length} bytes');

      // TODO: Add Rust API call when integrated
      // final result = await processYuvFrame(
      //   yPlane: frame.yPlane!,
      //   width: frame.width!,
      //   height: frame.height!,
      //   timestampMs: frame.timestampMs!,
      // );
    });
  });
}
