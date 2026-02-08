package com.example.cook_lib

import android.content.Context
import android.media.*
import android.net.Uri
import androidx.core.net.toUri
import java.io.File
import java.io.FileOutputStream
import java.nio.ByteBuffer

// Decode MP4 (H.264 + AAC) audio track to a PCM16 WAV temp file.
// WAV sampleRate/channels follow decoder output (e.g., 44.1k/48k, stereo). Downmix/resample is done in Rust.
// Returns the WAV file path.
object AudioDecoder {
    fun decodeToWav(context: Context, inputPath: String): String {
        val extractor = MediaExtractor()

        try {
            // Support both local file paths and network URLs
            val uri = if (inputPath.startsWith("http://") || inputPath.startsWith("https://")) {
                // Network URL
                Uri.parse(inputPath)
            } else {
                // Local file path
                File(inputPath).toUri()
            }

            extractor.setDataSource(context, uri, null)

            val trackIndex = selectAudioTrack(extractor)
            if (trackIndex < 0) throw IllegalArgumentException("No AAC audio track found")
            extractor.selectTrack(trackIndex)

            val format = extractor.getTrackFormat(trackIndex)
            val mime = format.getString(MediaFormat.KEY_MIME) ?: throw IllegalStateException("MIME missing")
            require(mime.startsWith("audio/")) { "Not an audio track" }

            // Force PCM output
            format.setInteger(MediaFormat.KEY_PCM_ENCODING, AudioFormat.ENCODING_PCM_16BIT)

            val codec = MediaCodec.createDecoderByType(mime)
            codec.configure(format, null, null, 0)
            codec.start()

            val tempWav = File.createTempFile("decoded_", ".wav", context.cacheDir)
            FileOutputStream(tempWav).use { fos -> decodeLoop(extractor, codec, fos) }

            codec.stop(); codec.release(); extractor.release()
            return tempWav.absolutePath
        } catch (e: Exception) {
            extractor.release()
            throw e
        }
    }

    private fun decodeLoop(extractor: MediaExtractor, codec: MediaCodec, fos: FileOutputStream) {
        val bufferInfo = MediaCodec.BufferInfo()
        var inputDone = false
        var outputDone = false
        var wav: WavWriter? = null
        var outputSampleRate = 44_100
        var outputChannels = 2

        while (!outputDone) {
            if (!inputDone) {
                val inIndex = codec.dequeueInputBuffer(10_000)
                if (inIndex >= 0) {
                    val inputBuffer = codec.getInputBuffer(inIndex)!!
                    val sampleSize = extractor.readSampleData(inputBuffer, 0)
                    if (sampleSize < 0) {
                        codec.queueInputBuffer(inIndex, 0, 0, 0L, MediaCodec.BUFFER_FLAG_END_OF_STREAM)
                        inputDone = true
                    } else {
                        val presentationTimeUs = extractor.sampleTime
                        codec.queueInputBuffer(inIndex, 0, sampleSize, presentationTimeUs, 0)
                        extractor.advance()
                    }
                }
            }

            val outIndex = codec.dequeueOutputBuffer(bufferInfo, 10_000)
            when {
                outIndex >= 0 -> {
                    if (wav == null) {
                        val fmt = codec.outputFormat
                        outputSampleRate = fmt.getInteger(MediaFormat.KEY_SAMPLE_RATE)
                        outputChannels = fmt.getInteger(MediaFormat.KEY_CHANNEL_COUNT)
                        wav = WavWriter(fos, outputSampleRate, outputChannels)
                    }
                    val outputBuffer = codec.getOutputBuffer(outIndex)!!
                    if (bufferInfo.size > 0) {
                        outputBuffer.position(bufferInfo.offset)
                        outputBuffer.limit(bufferInfo.offset + bufferInfo.size)
                        wav?.writePcm16Le(outputBuffer)
                    }
                    codec.releaseOutputBuffer(outIndex, false)
                    if (bufferInfo.flags and MediaCodec.BUFFER_FLAG_END_OF_STREAM != 0) {
                        outputDone = true
                    }
                }
                outIndex == MediaCodec.INFO_OUTPUT_FORMAT_CHANGED -> {
                    val fmt = codec.outputFormat
                    if (fmt.containsKey(MediaFormat.KEY_SAMPLE_RATE)) {
                        outputSampleRate = fmt.getInteger(MediaFormat.KEY_SAMPLE_RATE)
                    }
                    if (fmt.containsKey(MediaFormat.KEY_CHANNEL_COUNT)) {
                        outputChannels = fmt.getInteger(MediaFormat.KEY_CHANNEL_COUNT)
                    }
                }
            }
        }

        wav?.close()
    }

    private fun selectAudioTrack(extractor: MediaExtractor): Int {
        for (i in 0 until extractor.trackCount) {
            val format = extractor.getTrackFormat(i)
            val mime = format.getString(MediaFormat.KEY_MIME) ?: continue
            if (mime.startsWith("audio/")) return i
        }
        return -1
    }
}

// Minimal WAV writer for 16-bit PCM; assumes decoder outputs 16-bit PCM.
// Downmix/resample is handled in Rust.
class WavWriter(private val fos: FileOutputStream, private val sampleRate: Int, private val channels: Int) : AutoCloseable {
    private var totalPcmBytes: Int = 0
    private val headerSize = 44

    init {
        // Write placeholder header
        val header = ByteArray(headerSize) { 0 }
        fos.write(header)
    }

    fun writePcm16Le(buf: ByteBuffer) {
        val bytes = ByteArray(buf.remaining())
        buf.get(bytes)
        fos.write(bytes)
        totalPcmBytes += bytes.size
    }

    override fun close() {
        fos.flush()
        fos.channel.position(0)
        fos.write(buildHeader(totalPcmBytes))
        fos.flush()
        fos.close()
    }

    private fun buildHeader(dataSize: Int): ByteArray {
        val totalDataLen = dataSize + 36
        val header = ByteArray(headerSize)
        fun putIntLE(offset: Int, value: Int) {
            header[offset] = (value and 0xff).toByte()
            header[offset + 1] = ((value shr 8) and 0xff).toByte()
            header[offset + 2] = ((value shr 16) and 0xff).toByte()
            header[offset + 3] = ((value shr 24) and 0xff).toByte()
        }
        fun putShortLE(offset: Int, value: Int) {
            header[offset] = (value and 0xff).toByte()
            header[offset + 1] = ((value shr 8) and 0xff).toByte()
        }

        header[0] = 'R'.code.toByte(); header[1] = 'I'.code.toByte(); header[2] = 'F'.code.toByte(); header[3] = 'F'.code.toByte()
        putIntLE(4, totalDataLen)
        header[8] = 'W'.code.toByte(); header[9] = 'A'.code.toByte(); header[10] = 'V'.code.toByte(); header[11] = 'E'.code.toByte()
        header[12] = 'f'.code.toByte(); header[13] = 'm'.code.toByte(); header[14] = 't'.code.toByte(); header[15] = ' '.code.toByte()
        putIntLE(16, 16) // Subchunk1Size
        putShortLE(20, 1) // PCM
        putShortLE(22, channels)
        putIntLE(24, sampleRate)
        putIntLE(28, sampleRate * channels * 2)
        putShortLE(32, channels * 2)
        putShortLE(34, 16)
        header[36] = 'd'.code.toByte(); header[37] = 'a'.code.toByte(); header[38] = 't'.code.toByte(); header[39] = 'a'.code.toByte()
        putIntLE(40, dataSize)
        return header
    }
}
