import AVFoundation
import CoreVideo
import Flutter
import Foundation

class VideoFrameExtractor {
    private static let targetFPS = 2
    private static let frameIntervalSeconds: Double = 0.5  // 500ms
    private static let maxDimension: CGFloat = 640

    private static var shouldStop = false

    static func extractFrames(videoPath: String, eventSink: FlutterEventSink?) {
        shouldStop = false

        let inputURL: URL
        if videoPath.hasPrefix("http://") || videoPath.hasPrefix("https://") {
            guard let url = URL(string: videoPath) else {
                DispatchQueue.main.async {
                    eventSink?(FlutterError(code: "INVALID_URL", message: "Invalid URL", details: nil))
                }
                return
            }
            inputURL = url
        } else {
            inputURL = URL(fileURLWithPath: videoPath)
            guard FileManager.default.fileExists(atPath: videoPath) else {
                DispatchQueue.main.async {
                    eventSink?(FlutterError(code: "FILE_NOT_FOUND", message: "File not found: \(videoPath)", details: nil))
                }
                return
            }
        }

        let asset = AVAsset(url: inputURL)

        guard let videoTrack = asset.tracks(withMediaType: .video).first else {
            DispatchQueue.main.async {
                eventSink?(FlutterError(code: "NO_VIDEO_TRACK", message: "No video track found", details: nil))
            }
            return
        }

        do {
            let assetReader = try AVAssetReader(asset: asset)

            // Get video dimensions
            let naturalSize = videoTrack.naturalSize
            let transform = videoTrack.preferredTransform
            let videoSize = naturalSize.applying(transform)
            let width = abs(videoSize.width)
            let height = abs(videoSize.height)

            // Calculate scaled dimensions
            let scale = min(maxDimension / width, maxDimension / height, 1.0)
            let scaledWidth = Int(width * scale)
            let scaledHeight = Int(height * scale)

            // Configure output settings for raw video frames
            let outputSettings: [String: Any] = [
                kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange,
                kCVPixelBufferWidthKey as String: scaledWidth,
                kCVPixelBufferHeightKey as String: scaledHeight
            ]

            let readerOutput = AVAssetReaderTrackOutput(track: videoTrack, outputSettings: outputSettings)
            readerOutput.alwaysCopiesSampleData = false

            guard assetReader.canAdd(readerOutput) else {
                DispatchQueue.main.async {
                    eventSink?(FlutterError(code: "READER_ERROR", message: "Cannot add reader output", details: nil))
                }
                return
            }
            assetReader.add(readerOutput)

            guard assetReader.startReading() else {
                DispatchQueue.main.async {
                    eventSink?(FlutterError(code: "READER_ERROR", message: assetReader.error?.localizedDescription ?? "Failed to start reading", details: nil))
                }
                return
            }

            let duration = asset.duration.seconds
            var frameCount: Int64 = 0
            var lastProcessedTime: Double = -frameIntervalSeconds

            while let sampleBuffer = readerOutput.copyNextSampleBuffer() {
                if shouldStop {
                    CMSampleBufferInvalidate(sampleBuffer)
                    break
                }

                let presentationTime = CMSampleBufferGetPresentationTimeStamp(sampleBuffer).seconds
                frameCount += 1

                // Time-driven: extract one frame every 500ms
                if presentationTime - lastProcessedTime >= frameIntervalSeconds {
                    lastProcessedTime = presentationTime

                    if let imageBuffer = CMSampleBufferGetImageBuffer(sampleBuffer) {
                        CVPixelBufferLockBaseAddress(imageBuffer, .readOnly)

                        // Extract Y plane (grayscale)
                        let yPlaneAddress = CVPixelBufferGetBaseAddressOfPlane(imageBuffer, 0)
                        let yBytesPerRow = CVPixelBufferGetBytesPerRowOfPlane(imageBuffer, 0)
                        let yHeight = CVPixelBufferGetHeightOfPlane(imageBuffer, 0)
                        let yWidth = CVPixelBufferGetWidthOfPlane(imageBuffer, 0)

                        var yData = Data()
                        if let yPlaneAddress = yPlaneAddress {
                            // Copy Y plane data row by row to handle stride
                            for row in 0..<yHeight {
                                let rowStart = yPlaneAddress.advanced(by: row * yBytesPerRow)
                                yData.append(Data(bytes: rowStart, count: yWidth))
                            }
                        }

                        CVPixelBufferUnlockBaseAddress(imageBuffer, .readOnly)

                        let progress = duration > 0 ? Float(presentationTime / duration) : 0
                        let timestampMs = Int64(presentationTime * 1000)

                        let frameData: [String: Any] = [
                            "type": "frame",
                            "width": yWidth,
                            "height": yHeight,
                            "yPlane": FlutterStandardTypedData(bytes: yData),
                            "timestampMs": timestampMs,
                            "frameNumber": frameCount,
                            "progress": progress
                        ]

                        DispatchQueue.main.async {
                            eventSink?(frameData)
                        }

                        // Send progress update
                        let progressData: [String: Any] = [
                            "type": "progress",
                            "progress": progress
                        ]
                        DispatchQueue.main.async {
                            eventSink?(progressData)
                        }
                    }
                }

                CMSampleBufferInvalidate(sampleBuffer)
            }

            if assetReader.status == .failed {
                DispatchQueue.main.async {
                    eventSink?(FlutterError(code: "READER_ERROR", message: assetReader.error?.localizedDescription ?? "Unknown error", details: nil))
                }
                return
            }

            // Send completion
            DispatchQueue.main.async {
                eventSink?(["type": "complete"])
            }

        } catch {
            DispatchQueue.main.async {
                eventSink?(FlutterError(code: "EXTRACT_ERROR", message: error.localizedDescription, details: nil))
            }
        }
    }

    static func stopExtraction() {
        shouldStop = true
    }
}
