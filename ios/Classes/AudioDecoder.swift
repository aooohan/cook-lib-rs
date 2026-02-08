import AVFoundation
import Foundation

enum AudioDecoderError: Error, LocalizedError {
    case fileNotFound(String)
    case noAudioTrack
    case exportFailed(String)
    case invalidURL

    var errorDescription: String? {
        switch self {
        case .fileNotFound(let path):
            return "File not found: \(path)"
        case .noAudioTrack:
            return "No audio track found in media file"
        case .exportFailed(let reason):
            return "Export failed: \(reason)"
        case .invalidURL:
            return "Invalid URL"
        }
    }
}

class AudioDecoder {

    /// Decode audio from video/audio file to WAV format
    /// Returns the path to the decoded WAV file
    static func decodeToWav(inputPath: String) throws -> String {
        let inputURL: URL

        if inputPath.hasPrefix("http://") || inputPath.hasPrefix("https://") {
            guard let url = URL(string: inputPath) else {
                throw AudioDecoderError.invalidURL
            }
            inputURL = url
        } else {
            inputURL = URL(fileURLWithPath: inputPath)
            guard FileManager.default.fileExists(atPath: inputPath) else {
                throw AudioDecoderError.fileNotFound(inputPath)
            }
        }

        let asset = AVAsset(url: inputURL)

        // Check for audio track
        let audioTracks = asset.tracks(withMediaType: .audio)
        guard !audioTracks.isEmpty else {
            throw AudioDecoderError.noAudioTrack
        }

        // Create output WAV file
        let tempDir = FileManager.default.temporaryDirectory
        let outputURL = tempDir.appendingPathComponent("decoded_\(UUID().uuidString).wav")

        // Remove existing file if any
        try? FileManager.default.removeItem(at: outputURL)

        // Use AVAssetReader to read audio and write to WAV
        try exportToWav(asset: asset, outputURL: outputURL)

        return outputURL.path
    }

    private static func exportToWav(asset: AVAsset, outputURL: URL) throws {
        let semaphore = DispatchSemaphore(value: 0)
        var exportError: Error?

        // Get audio track
        guard let audioTrack = asset.tracks(withMediaType: .audio).first else {
            throw AudioDecoderError.noAudioTrack
        }

        // Create asset reader
        let assetReader = try AVAssetReader(asset: asset)

        // Configure output settings for PCM
        let outputSettings: [String: Any] = [
            AVFormatIDKey: kAudioFormatLinearPCM,
            AVSampleRateKey: 16000,
            AVNumberOfChannelsKey: 1,
            AVLinearPCMBitDepthKey: 16,
            AVLinearPCMIsFloatKey: false,
            AVLinearPCMIsBigEndianKey: false,
            AVLinearPCMIsNonInterleaved: false
        ]

        let readerOutput = AVAssetReaderTrackOutput(track: audioTrack, outputSettings: outputSettings)
        readerOutput.alwaysCopiesSampleData = false

        guard assetReader.canAdd(readerOutput) else {
            throw AudioDecoderError.exportFailed("Cannot add reader output")
        }
        assetReader.add(readerOutput)

        // Start reading
        guard assetReader.startReading() else {
            throw AudioDecoderError.exportFailed(assetReader.error?.localizedDescription ?? "Unknown error")
        }

        // Collect all PCM data
        var pcmData = Data()

        while let sampleBuffer = readerOutput.copyNextSampleBuffer() {
            if let blockBuffer = CMSampleBufferGetDataBuffer(sampleBuffer) {
                var length = 0
                var dataPointer: UnsafeMutablePointer<Int8>?

                CMBlockBufferGetDataPointer(blockBuffer, atOffset: 0, lengthAtOffsetOut: nil, totalLengthOut: &length, dataPointerOut: &dataPointer)

                if let dataPointer = dataPointer {
                    pcmData.append(UnsafeBufferPointer(start: dataPointer, count: length))
                }
            }
            CMSampleBufferInvalidate(sampleBuffer)
        }

        if assetReader.status == .failed {
            throw AudioDecoderError.exportFailed(assetReader.error?.localizedDescription ?? "Unknown error")
        }

        // Write WAV file
        try writeWavFile(pcmData: pcmData, sampleRate: 16000, channels: 1, bitsPerSample: 16, to: outputURL)
    }

    private static func writeWavFile(pcmData: Data, sampleRate: Int, channels: Int, bitsPerSample: Int, to url: URL) throws {
        var header = Data()

        let byteRate = sampleRate * channels * bitsPerSample / 8
        let blockAlign = channels * bitsPerSample / 8
        let dataSize = pcmData.count
        let fileSize = 36 + dataSize

        // RIFF header
        header.append(contentsOf: "RIFF".utf8)
        header.append(contentsOf: withUnsafeBytes(of: UInt32(fileSize).littleEndian) { Array($0) })
        header.append(contentsOf: "WAVE".utf8)

        // fmt chunk
        header.append(contentsOf: "fmt ".utf8)
        header.append(contentsOf: withUnsafeBytes(of: UInt32(16).littleEndian) { Array($0) }) // Subchunk1Size
        header.append(contentsOf: withUnsafeBytes(of: UInt16(1).littleEndian) { Array($0) }) // AudioFormat (PCM)
        header.append(contentsOf: withUnsafeBytes(of: UInt16(channels).littleEndian) { Array($0) })
        header.append(contentsOf: withUnsafeBytes(of: UInt32(sampleRate).littleEndian) { Array($0) })
        header.append(contentsOf: withUnsafeBytes(of: UInt32(byteRate).littleEndian) { Array($0) })
        header.append(contentsOf: withUnsafeBytes(of: UInt16(blockAlign).littleEndian) { Array($0) })
        header.append(contentsOf: withUnsafeBytes(of: UInt16(bitsPerSample).littleEndian) { Array($0) })

        // data chunk
        header.append(contentsOf: "data".utf8)
        header.append(contentsOf: withUnsafeBytes(of: UInt32(dataSize).littleEndian) { Array($0) })

        // Write to file
        var fileData = header
        fileData.append(pcmData)
        try fileData.write(to: url)
    }
}
