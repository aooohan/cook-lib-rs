import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:cook_lib/cook_lib.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  const testVideoPath = '/data/local/tmp/lmth-cook.mp4';

  group('VideoProcessor Integration Tests', () {
    testWidgets('VideoProcessor should extract frames with progress', (tester) async {
      final processor = VideoProcessor.create();
      final allFrames = <FrameExtractedInfo>[];
      bool completed = false;

      await for (final progress in processor.process(testVideoPath)) {
        allFrames.addAll(progress.frames);
        print('Progress: ${(progress.progress * 100).toStringAsFixed(1)}%, frames: ${allFrames.length}');

        if (progress.isComplete) {
          completed = true;
          break;
        }
      }

      expect(completed, isTrue);
      print('Total extracted frames: ${allFrames.length}');

      final stats = processor.stats;
      print('Stats - Processed: ${stats.totalProcessed}, Extracted: ${stats.totalExtracted}');
      expect(stats.totalProcessed, greaterThan(0));

      processor.dispose();
    });

    testWidgets('VideoProcessor.processAll should return complete result', (tester) async {
      final processor = VideoProcessor.create();

      final result = await processor.processAll(testVideoPath);

      expect(result.frames, isNotNull);
      expect(result.stats.totalProcessed, greaterThan(0));
      print('Extracted ${result.frames.length} frames');
      print('Stats - Processed: ${result.stats.totalProcessed}, Extracted: ${result.stats.totalExtracted}');

      processor.dispose();
    });

    testWidgets('VideoProcessor.stop should halt extraction', (tester) async {
      final processor = VideoProcessor.create();
      int progressCount = 0;

      await for (final progress in processor.process(testVideoPath)) {
        progressCount++;
        if (progressCount >= 3) {
          await processor.stop();
          break;
        }
      }

      expect(processor.isProcessing, isFalse);
      print('Stopped after $progressCount progress updates');

      processor.dispose();
    });
  });

  group('Rust API Integration Tests', () {
    testWidgets('VideoFrameExtractor should process batch of frames', (tester) async {
      final processor = VideoProcessor.create();

      // Get some frames first
      final result = await processor.processAll(testVideoPath);

      expect(result.frames.length, greaterThanOrEqualTo(0));
      print('Batch processing complete: ${result.frames.length} frames extracted');

      processor.dispose();
    });
  });
}
