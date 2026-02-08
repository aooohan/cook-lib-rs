# Cook Lib 构建脚本
# 使用: just --list 查看所有命令

# 默认 Android 输出目录
android_jni_dir := "android/src/main/jniLibs"
# iOS 输出目录
ios_frameworks_dir := "ios/Frameworks"

# Android 架构映射
_android_targets := "arm64-v8a:aarch64-linux-android armeabi-v7a:armv7-linux-androideabi x86_64:x86_64-linux-android x86:i686-linux-android"

# 列出所有可用命令
default:
    @just --list

# ============ Android ============

# 构建单个 Android 架构 (arm64-v8a, armeabi-v7a, x86_64, x86)
build-android arch:
    #!/usr/bin/env bash
    set -euo pipefail

    # 映射 NDK 架构到 Rust target
    case "{{arch}}" in
        arm64-v8a)     rust_target="aarch64-linux-android" ;;
        armeabi-v7a)   rust_target="armv7-linux-androideabi" ;;
        x86_64)        rust_target="x86_64-linux-android" ;;
        x86)           rust_target="i686-linux-android" ;;
        *)             echo "Unknown arch: {{arch}}"; exit 1 ;;
    esac

    echo "Building Android {{arch}} ($rust_target)..."
    cd rust
    cargo ndk -o ../{{android_jni_dir}} -t {{arch}} build --release

    # 复制依赖库
    target_dir="target/$rust_target/release"
    output_dir="../{{android_jni_dir}}/{{arch}}"

    for lib in libsherpa-ncnn-c-api.so libncnn.so; do
        if [ -f "$target_dir/$lib" ]; then
            cp -v "$target_dir/$lib" "$output_dir/"
        fi
    done

    echo "Built libraries in $output_dir:"
    ls -la "$output_dir"/*.so

# 构建所有 Android 架构
build-android-all:
    just build-android arm64-v8a
    just build-android armeabi-v7a
    just build-android x86_64
    just build-android x86

# ============ iOS ============

# 构建单个 iOS target
build-ios-target target:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "Building iOS {{target}}..."
    cd rust
    cargo build --release --target {{target}}

# 构建所有 iOS 库
build-ios:
    just build-ios-target aarch64-apple-ios
    just build-ios-target aarch64-apple-ios-sim
    just build-ios-target x86_64-apple-ios

# 创建 iOS XCFramework
package-ios:
    #!/usr/bin/env bash
    set -euo pipefail

    echo "Creating iOS XCFramework..."
    mkdir -p {{ios_frameworks_dir}}

    # 创建 simulator fat library
    mkdir -p rust/target/ios-simulator-universal
    lipo -create \
        rust/target/aarch64-apple-ios-sim/release/libcook_lib.a \
        rust/target/x86_64-apple-ios/release/libcook_lib.a \
        -output rust/target/ios-simulator-universal/libcook_lib.a

    # 删除旧的 xcframework
    rm -rf {{ios_frameworks_dir}}/cook_lib.xcframework

    # 创建 XCFramework
    xcodebuild -create-xcframework \
        -library rust/target/aarch64-apple-ios/release/libcook_lib.a \
        -library rust/target/ios-simulator-universal/libcook_lib.a \
        -output {{ios_frameworks_dir}}/cook_lib.xcframework

    echo "Created XCFramework at {{ios_frameworks_dir}}/cook_lib.xcframework"

# 构建 iOS 并打包 XCFramework
build-ios-all: build-ios package-ios

# ============ Dart 绑定 ============

# 生成 Dart 绑定
generate-bindings:
    flutter pub get
    flutter_rust_bridge_codegen generate

# ============ 完整构建 ============

# 构建所有平台 (Android + iOS + Dart bindings)
build-all: build-android-all build-ios-all generate-bindings
    @echo "All platforms built successfully!"

# 仅构建移动端 (不生成 bindings，用于 CI)
build-mobile: build-android-all build-ios-all

# ============ 清理 ============

# 清理 Rust 构建产物
clean-rust:
    cd rust && cargo clean

# 清理 Android jniLibs
clean-android:
    rm -rf {{android_jni_dir}}/*/

# 清理 iOS frameworks
clean-ios:
    rm -rf {{ios_frameworks_dir}}/

# 清理所有构建产物
clean: clean-rust clean-android clean-ios
    @echo "Cleaned all build artifacts"

# ============ 开发辅助 ============

# 检查 Rust 代码
check:
    cd rust && cargo check

# 格式化 Rust 代码
fmt:
    cd rust && cargo fmt

# Rust clippy 检查
clippy:
    cd rust && cargo clippy
