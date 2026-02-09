#
# To learn more about a Podspec see http://guides.cocoapods.org/syntax/podspec.html.
# Run `pod lib lint cook_lib.podspec` to validate before publishing.
#
Pod::Spec.new do |s|
  s.name             = 'cook_lib'
  s.version          = '0.0.1'
  s.summary          = 'A new Flutter plugin project.'
  s.description      = <<-DESC
A new Flutter plugin project.
                       DESC
  s.homepage         = 'http://example.com'
  s.license          = { :file => '../LICENSE' }
  s.author           = { 'Your Company' => 'email@example.com' }
  s.source           = { :path => '.' }
  s.source_files = 'Classes/**/*'
  s.dependency 'Flutter'
  s.platform = :ios, '13.0'

  # 静态库
  s.vendored_frameworks = 'Frameworks/cook_lib.xcframework'
  s.static_framework = true

  # Flutter.framework does not contain a i386 slice.
  s.pod_target_xcconfig = {
    'DEFINES_MODULE' => 'YES',
    'EXCLUDED_ARCHS[sdk=iphonesimulator*]' => 'i386'
  }

  # 传递到主应用的链接器标志（关键！静态库符号需要 force_load）
  s.user_target_xcconfig = {
    'OTHER_LDFLAGS[sdk=iphoneos*]' => '-force_load "${PODS_ROOT}/../.symlinks/plugins/cook_lib/ios/Frameworks/cook_lib.xcframework/ios-arm64/libcook_lib.a" -lc++',
    'OTHER_LDFLAGS[sdk=iphonesimulator*]' => '-force_load "${PODS_ROOT}/../.symlinks/plugins/cook_lib/ios/Frameworks/cook_lib.xcframework/ios-arm64_x86_64-simulator/libcook_lib.a" -lc++'
  }

  s.swift_version = '5.0'

  # If your plugin requires a privacy manifest, for example if it uses any
  # required reason APIs, update the PrivacyInfo.xcprivacy file to describe your
  # plugin's privacy impact, and then uncomment this line. For more information,
  # see https://developer.apple.com/documentation/bundleresources/privacy_manifest_files
  # s.resource_bundles = {'cook_lib_privacy' => ['Resources/PrivacyInfo.xcprivacy']}
end
