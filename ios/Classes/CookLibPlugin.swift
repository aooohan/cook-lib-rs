import Flutter
import UIKit

public class CookLibPlugin: NSObject, FlutterPlugin, FlutterStreamHandler {
    private var eventSink: FlutterEventSink?
    private let queue = DispatchQueue(label: "com.example.cook_lib.decoder", qos: .userInitiated)

    public static func register(with registrar: FlutterPluginRegistrar) {
        let methodChannel = FlutterMethodChannel(name: "cook_lib/methods", binaryMessenger: registrar.messenger())
        let eventChannel = FlutterEventChannel(name: "cook_lib/video_frames", binaryMessenger: registrar.messenger())

        let instance = CookLibPlugin()
        registrar.addMethodCallDelegate(instance, channel: methodChannel)
        eventChannel.setStreamHandler(instance)
    }

    public func handle(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
        switch call.method {
        case "decodeAudioToWav":
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

        case "extractVideoFrames":
            guard let args = call.arguments as? [String: Any],
                  let videoPath = args["videoPath"] as? String else {
                result(FlutterError(code: "INVALID_ARGUMENT", message: "videoPath is required", details: nil))
                return
            }

            queue.async { [weak self] in
                VideoFrameExtractor.extractFrames(videoPath: videoPath, eventSink: self?.eventSink)
            }
            result(nil)

        case "stopExtraction":
            VideoFrameExtractor.stopExtraction()
            result(nil)

        default:
            result(FlutterMethodNotImplemented)
        }
    }

    // MARK: - FlutterStreamHandler

    public func onListen(withArguments arguments: Any?, eventSink events: @escaping FlutterEventSink) -> FlutterError? {
        self.eventSink = events
        return nil
    }

    public func onCancel(withArguments arguments: Any?) -> FlutterError? {
        self.eventSink = nil
        return nil
    }
}
