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
│       ├── frb_generated.rs        # flutter_rust_bridge 生成（自动）
│       ├── api/                    # 公开 API（暴露给 Dart）
│       │   ├── audio.rs            # ASR/VAD 接口
│       │   ├── video.rs            # 视频抽帧接口
│       │   ├── xhs.rs              # 小红书解析接口
│       │   ├── simple.rs           # 简单测试接口
│       │   └── models/             # 公开数据模型（DTO）
│       │       └── xhs.rs          # XhsArticle, XhsVideo 等
│       └── core/                   # 核心实现（内部，不对外暴露）
│           ├── audio/              # 音频处理
│           │   ├── error.rs        # AudioError
│           │   ├── handler.rs      # NcnnHandle (ASR)
│           │   ├── utils.rs        # 音频工具函数
│           │   └── vad.rs          # VadHandle (VAD)
│           ├── video/              # 视频处理
│           │   ├── deduplicator.rs # 帧去重
│           │   ├── diff_filter.rs  # 帧差异过滤
│           │   ├── frame.rs        # 帧数据结构
│           │   ├── pipeline.rs     # 抽帧管线
│           │   ├── state_machine.rs# 状态机
│           │   └── text_detector.rs# 文字检测
│           └── xhs/                # 小红书解析
│               ├── mod.rs          # XhsParser
│               └── parser.rs       # HTML 解析逻辑
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

项目使用 `just` 作为构建工具，需要先安装：`cargo install just`

```bash
# 查看所有可用命令
just --list

# ===== 本地开发 =====

# Rust 检查/格式化/lint
just check                      # cargo check
just fmt                        # cargo fmt
just clippy                     # cargo clippy

# 生成 Dart 绑定（修改 Rust API 后必须执行）
just generate-bindings

# ===== Android 构建 =====

just build-android arm64-v8a    # 构建单个架构
just build-android-all          # 构建所有 4 个架构

# ===== iOS 构建 =====

just build-ios                  # 构建所有 iOS target
just package-ios                # 创建 XCFramework
just build-ios-all              # 构建 + 打包

# ===== 完整构建 =====

just build-all                  # Android + iOS + bindings
just build-mobile               # 仅 Android + iOS（用于 CI）

# ===== 清理 =====

just clean                      # 清理所有构建产物
just clean-rust                 # 仅清理 Rust target
just clean-android              # 仅清理 jniLibs
just clean-ios                  # 仅清理 Frameworks
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
