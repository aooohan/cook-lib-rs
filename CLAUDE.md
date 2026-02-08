# Cook Lib - Flutter Plugin 项目记忆

## 项目目标

Flutter plugin，封装菜谱视频处理功能：
- **ASR 语音识别** - 使用 sherpa-ncnn（离线、实时）
- **VAD 语音活动检测** - 使用 Silero VAD
- **视频抽帧分析** - 基于文字检测的智能去重
- **小红书解析** - 解析菜谱链接

## 项目结构

```
cook_lib/
├── rust/                           # Rust 源码
│   ├── Cargo.toml
│   ├── build.rs
│   └── src/
│       ├── lib.rs                  # 入口，模块声明
│       ├── frb_generated.rs        # flutter_rust_bridge 生成
│       ├── api/                    # 公开 API（暴露给 Dart）
│       │   ├── audio.rs            # ASR/VAD 接口
│       │   ├── frame_extractor.rs  # 视频抽帧接口
│       │   ├── xhs.rs              # 小红书解析
│       │   └── simple.rs           # 简单测试接口
│       ├── core/                   # 核心实现
│       │   ├── ncnn_handler.rs     # ASR 处理器
│       │   ├── ncnn_vad.rs         # VAD 处理器
│       │   └── audio_error.rs      # 错误类型
│       ├── frame_extractor/        # 视频帧处理
│       │   ├── pipeline.rs
│       │   ├── deduplicator.rs
│       │   └── ...
│       └── models/                 # 数据模型
├── lib/                            # Dart 代码
│   ├── cook_lib.dart               # 主入口，导出所有 API
│   ├── src/native_decoder.dart     # 原生解码器 API
│   └── src/rust/                   # flutter_rust_bridge 生成的绑定
│       ├── frb_generated.dart
│       ├── api/
│       └── ...
├── android/
│   ├── src/main/kotlin/.../
│   │   ├── CookLibPlugin.kt        # Flutter 插件入口
│   │   ├── AudioDecoder.kt         # 音频解码 (AAC → WAV)
│   │   └── VideoFrameExtractor.kt  # 视频帧提取 (MediaCodec)
│   └── src/main/jniLibs/           # Android 原生库
│       ├── arm64-v8a/
│       ├── armeabi-v7a/
│       ├── x86_64/
│       └── x86/
├── ios/
│   └── Frameworks/                 # iOS XCFramework
├── example/                        # 测试 App
│   ├── lib/
│   │   ├── main.dart               # 主菜单
│   │   └── pages/
│   │       ├── video_frame_extractor_page.dart  # 视频帧提取测试
│   │       └── transcribe_demo_page.dart        # 语音转写测试
│   └── integration_test/
│       └── plugin_test.dart        # 自动化集成测试
├── .github/workflows/
│   └── release.yml                 # CI 构建和发布
├── flutter_rust_bridge.yaml        # FRB 配置
└── pubspec.yaml                    # ffiPlugin: true + pluginClass
```

## 技术栈

| 组件 | 技术 |
|------|------|
| 跨语言桥接 | flutter_rust_bridge 2.11.1 |
| ASR 引擎 | sherpa-ncnn（ncnn 推理） |
| VAD 模型 | Silero VAD |
| 图像处理 | image crate |
| 音频处理 | hound + rubato |
| HTTP | reqwest |
| HTML 解析 | scraper |

## 关键依赖

```toml
# Cargo.toml
flutter_rust_bridge = "=2.11.1"
sherpa-ncnn = { git = "https://github.com/aooohan/sherpa-ncnn-rs", default-features = false }
```

- `sherpa-ncnn-rs` 是自己实现的 crate，自动下载预编译库
- 支持 Android（4 架构）和 iOS（device + simulator）

## 构建命令

### 本地开发

```bash
# Rust 检查
cd rust && cargo check

# Android 构建（需要 NDK）
cd rust
cargo ndk -o ../android/src/main/jniLibs -t arm64-v8a build --release

# iOS 构建
cd rust
cargo build --release --target aarch64-apple-ios

# 重新生成 Dart 绑定
flutter_rust_bridge_codegen generate
```

### CI 发布

GitHub Actions 手动触发：
1. Actions → "Build and Release"
2. 输入版本号（如 `0.1.0`）
3. 自动构建所有平台并提交

## Flutter 项目使用

```yaml
# pubspec.yaml
dependencies:
  cook_lib:
    git:
      url: git@github.com:aooohan/cook_lib.git
      ref: v0.1.0
```

## API 概览

### ASR

```dart
import 'package:cook_lib/cook_lib.dart';

// 初始化
await initSherpa(modelPath: '/path/to/model');
await initVad(vadModelPath: '/path/to/silero');

// 转录音频文件
String text = await transcribeAudio(path: '/path/to/audio.wav');

// 转录 PCM 数据
String text = await transcribePcm(samples: floatList, sampleRate: 16000);
```

### 视频抽帧

```dart
// 处理 YUV 帧
FrameResult? result = await processYuvFrame(
  yPlane: yData,
  width: 720,
  height: 1280,
  timestampMs: 1000,
);
```

## 原生库说明

### Android jniLibs 结构

```
jniLibs/{arch}/
├── libcook_lib.so              # 主库（cargo-ndk 输出）
├── libsherpa-ncnn-c-api.so     # sherpa-ncnn C API
└── libncnn.so                  # ncnn 推理引擎
```

### iOS Frameworks

```
Frameworks/
└── cook_lib.xcframework/       # 包含 device 和 simulator
```

## CI 流程

```
workflow_dispatch（手动触发）
        ↓
┌───────────────────────────────────────┐
│  build-android（matrix: 4 架构）      │
│  build-ios（device + sim）            │
│  generate-bindings                    │
└───────────────────────────────────────┘
        ↓
┌───────────────────────────────────────┐
│  commit-and-release                   │
│  - 下载 artifacts                     │
│  - 更新 pubspec.yaml 版本             │
│  - git commit & push                  │
│  - 创建 tag 和 GitHub Release         │
└───────────────────────────────────────┘
```

## 注意事项

1. **sherpa-ncnn-rs** 未发布到 crates.io，通过 git 依赖
2. **模型文件** 需要在 Flutter 端管理（下载/解压到本地）
3. **采样率** 必须是 16000Hz
4. **flutter_rust_bridge 版本** Rust 和 Dart 必须一致（2.11.1）

## 从原项目迁移

本项目从 `cook-follow/cook_lib_exp` 迁移而来：
- 原项目是完整 Flutter 应用
- 本项目是独立 Flutter plugin
- Rust 代码完全相同，仅调整了 crate 名称

## 相关项目

- [sherpa-ncnn-rs](https://github.com/aooohan/sherpa-ncnn-rs) - Rust 绑定
- [sherpa-ncnn](https://github.com/k2-fsa/sherpa-ncnn) - 原始 C++ 实现
