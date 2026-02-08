// This stub exists so IDE can resolve symbols before codegen.
// Generated bindings will replace these signatures.

Future<void> initSherpa({required String modelPath}) async {}
Future<String> transcribeAudio({
  required String path,
  String? language,
}) async => '';
Future<String> transcribePcm({
  required List<double> pcm,
  required int sampleRate,
  String? language,
}) async => '';
