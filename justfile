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

# ============ 发布 ============

# 发布目录
publish_dir := "dist/cook_lib"

# 打包发布版本（只包含必要文件）
package-publish version="0.0.0-dev":
    #!/usr/bin/env bash
    set -euo pipefail

    echo "📦 Creating publish package v{{version}}..."
    rm -rf dist
    mkdir -p {{publish_dir}}/android/src/main
    mkdir -p {{publish_dir}}/ios
    mkdir -p {{publish_dir}}/lib/src

    # Dart 代码
    cp lib/cook_lib.dart {{publish_dir}}/lib/
    cp lib/src/native_decoder.dart {{publish_dir}}/lib/src/
    cp -r lib/src/rust {{publish_dir}}/lib/src/

    # Android 插件
    cp android/build.gradle {{publish_dir}}/android/
    cp android/settings.gradle {{publish_dir}}/android/ 2>/dev/null || true
    cp android/src/main/AndroidManifest.xml {{publish_dir}}/android/src/main/
    cp -r android/src/main/kotlin {{publish_dir}}/android/src/main/
    cp -r android/src/main/jniLibs {{publish_dir}}/android/src/main/ 2>/dev/null || echo "⚠️  No jniLibs (run build-android-all first)"

    # iOS 插件
    cp ios/cook_lib.podspec {{publish_dir}}/ios/ 2>/dev/null || true
    cp -r ios/Classes {{publish_dir}}/ios/ 2>/dev/null || true
    cp -r ios/Frameworks {{publish_dir}}/ios/ 2>/dev/null || echo "⚠️  No Frameworks (run build-ios-all first)"

    # 根目录文件
    sed "s/^version: .*/version: {{version}}/" pubspec.yaml > {{publish_dir}}/pubspec.yaml
    cp LICENSE {{publish_dir}}/ 2>/dev/null || true
    cp README.md {{publish_dir}}/ 2>/dev/null || true
    cp CHANGELOG.md {{publish_dir}}/ 2>/dev/null || true

    # 清理
    find {{publish_dir}} -name ".DS_Store" -delete 2>/dev/null || true
    find {{publish_dir}} -name "*.log" -delete 2>/dev/null || true

    echo ""
    echo "✅ Package created at {{publish_dir}}"
    echo "📊 Size: $(du -sh {{publish_dir}} | cut -f1)"
    echo ""
    echo "Contents:"
    find {{publish_dir}} -type f | head -30
    echo "..."

# 创建发布压缩包
package-archive version="0.0.0-dev": (package-publish version)
    #!/usr/bin/env bash
    set -euo pipefail

    cd dist
    echo "📦 Creating archives..."
    tar -czvf cook_lib-v{{version}}.tar.gz cook_lib/
    zip -r cook_lib-v{{version}}.zip cook_lib/

    echo ""
    echo "✅ Archives created:"
    ls -lh cook_lib-v{{version}}.*

# 完整构建 + 打包 (本地测试发布流程)
release-local version="0.0.0-dev": build-all (package-archive version)
    @echo ""
    @echo "🎉 Local release complete!"
    @echo "   dist/cook_lib-v{{version}}.tar.gz"
    @echo "   dist/cook_lib-v{{version}}.zip"

# 清理发布目录
clean-publish:
    rm -rf dist/
