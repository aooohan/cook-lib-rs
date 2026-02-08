# Cook Lib - Flutter Plugin

Flutter plugin，封装菜谱APP底层处理功能（ASR、VAD、视频抽帧、小红书解析）。

## 目录规则

|------|------|
| `rust/src/api/` | 公开 API，暴露给 Dart 的接口 |
| `rust/src/core/` | 核心实现，内部逻辑，不对外暴露 |
| `lib/` | Dart 代码，主入口和原生解码器 |
| `lib/src/rust/` | flutter_rust_bridge 自动生成，勿手动修改 |
| `android/src/main/kotlin/` | Android 平台代码（Kotlin） |
| `android/src/main/jniLibs/` | Android 原生库（.so），构建生成 |
| `ios/Frameworks/` | iOS XCFramework，构建生成 |
| `example/` | 测试 App |

## 构建命令

所有构建操作统一使用 `just`：

```bash
just --list    # 查看所有可用命令
```

## 关键约束

1. **flutter_rust_bridge 版本**：Rust 和 Dart 必须一致（2.11.1）
2. **修改 Rust API 后**：必须执行 `just generate-bindings`

## AI 行为规则

1. **Bash 执行结果**：只关注成功/失败，忽略正常日志输出（避免上下文膨胀）
