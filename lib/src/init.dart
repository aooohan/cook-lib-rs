import 'dart:io';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';
import 'rust/frb_generated.dart';

/// 初始化 cook_lib
///
/// iOS: 使用静态库，符号已链接到主程序
/// Android: 使用动态库 (.so)
Future<void> initCookLib() async {
  if (Platform.isIOS) {
    // iOS 静态库：符号已链接到主程序，使用 process()
    await RustLib.init(
      externalLibrary: ExternalLibrary.process(iKnowHowToUseIt: true),
    );
  } else if (Platform.isAndroid) {
    // Android 动态库：正常加载
    await RustLib.init();
  }
  // 其他平台暂不支持
}
