import Flutter
import UIKit

public class CookLibPlugin: NSObject, FlutterPlugin, FlutterStreamHandler {
    private var eventSink: FlutterEventSink?
    private let queue = DispatchQueue(label: "com.example.cook_lib.decoder", qos: .userInitiated)

    public static func register(with registrar: FlutterPluginRegistrar) {
        // 音频解码通道 - 用于一次性解码请求
        let audioChannel = FlutterMethodChannel(name: "cook_lib/audio/decoder", binaryMessenger: registrar.messenger())
        // 视频帧流通道 - 用于流式帧数据传输
        let videoFrameChannel = FlutterEventChannel(name: "cook_lib/video/frame_stream", binaryMessenger: registrar.messenger())

        let instance = CookLibPlugin()
        registrar.addMethodCallDelegate(instance, channel: audioChannel)
        videoFrameChannel.setStreamHandler(instance)
    }

    public func handle(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
        switch call.method {
        case "decodeToWav":
            guard let args = call.arguments as? [String: Any],
                  let inputPath = args["inputPath"] as? String else {
                result(FlutterError(code: "INVALID_ARGUMENT", message: "inputPath is required", details: nil))
                return
            }

            queue.async {
                do {
                    let wavPath = try AudioDecoder.decodeToWav(inputPath: inputPath)
                    DispatchQueue.main.async {
                        result(wavPath)
                    }
                } catch {
                    DispatchQueue.main.async {
                        result(FlutterError(code: "DECODE_ERROR", message: error.localizedDescription, details: nil))
                    }
                }
            }

        default:
            result(FlutterMethodNotImplemented)
        }
    }

    // MARK: - FlutterStreamHandler

    public func onListen(withArguments arguments: Any?, eventSink events: @escaping FlutterEventSink) -> FlutterError? {
        self.eventSink = events

        // 从参数中获取 videoPath，立即开始提取
        if let args = arguments as? [String: Any],
           let videoPath = args["videoPath"] as? String {
            queue.async { [weak self] in
                VideoFrameExtractor.extractFrames(videoPath: videoPath, eventSink: self?.eventSink)
            }
        }

        return nil
    }

    public func onCancel(withArguments arguments: Any?) -> FlutterError? {
        // 停止当前实例的提取
        VideoFrameExtractor.stopExtraction()
        self.eventSink = nil
        return nil
    }
}
