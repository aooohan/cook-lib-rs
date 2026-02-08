package com.example.cook_lib

import android.media.MediaCodec
import android.media.MediaExtractor
import android.media.MediaFormat
import android.os.Handler
import android.os.Looper
import io.flutter.plugin.common.EventChannel
import java.nio.ByteBuffer

class VideoFrameExtractor {
    companion object {
        private const val VIDEO_MIME_PREFIX = "video/"
        private const val TIMEOUT_US = 10000L
        // 固定每秒2帧
        private const val TARGET_FPS = 2
        private const val FRAME_INTERVAL_US = 500_000L  // 500ms

        // 检测和预览都用640px，保证文字识别率和预览清晰度
        private const val MAX_DIMENSION = 640

        @JvmStatic
        fun extractFrames(
            videoPath: String,
            eventSink: EventChannel.EventSink?
        ) {
            val extractor = MediaExtractor()
            var decoder: MediaCodec? = null
            var trackIndex = -1
            var format: MediaFormat? = null

            try {
                extractor.setDataSource(videoPath)

                // Find video track
                for (i in 0 until extractor.trackCount) {
                    val fmt = extractor.getTrackFormat(i)
                    val mime = fmt.getString(MediaFormat.KEY_MIME) ?: ""
                    if (mime.startsWith(VIDEO_MIME_PREFIX)) {
                        trackIndex = i
                        format = fmt
                        break
                    }
                }

                if (trackIndex < 0 || format == null) {
                    throw RuntimeException("No video track found")
                }

                extractor.selectTrack(trackIndex)

                val mime = format.getString(MediaFormat.KEY_MIME)!!
                decoder = MediaCodec.createDecoderByType(mime)

                val width = format.getInteger(MediaFormat.KEY_WIDTH)
                val height = format.getInteger(MediaFormat.KEY_HEIGHT)
                val durationUs = format.getLong(MediaFormat.KEY_DURATION, 0)

                decoder.configure(format, null, null, 0)
                decoder.start()

                var isEOS = false
                var frameCount = 0L
                var outputDone = false
                var lastProcessedTimeUs = -FRAME_INTERVAL_US  // 确保第一帧被处理

                val bufferInfo = MediaCodec.BufferInfo()
                val inputBuffers = decoder.inputBuffers

                while (!outputDone) {
                    // Feed input
                    if (!isEOS) {
                        val inputBufferId = decoder.dequeueInputBuffer(TIMEOUT_US)
                        if (inputBufferId >= 0) {
                            val inputBuffer = inputBuffers[inputBufferId]
                            val sampleSize = extractor.readSampleData(inputBuffer, 0)

                            if (sampleSize < 0) {
                                decoder.queueInputBuffer(inputBufferId, 0, 0, 0, MediaCodec.BUFFER_FLAG_END_OF_STREAM)
                                isEOS = true
                            } else {
                                val presentationTimeUs = extractor.sampleTime
                                decoder.queueInputBuffer(inputBufferId, 0, sampleSize, presentationTimeUs, 0)
                                extractor.advance()
                            }
                        }
                    }

                    // Drain output
                    val outputBufferId = decoder.dequeueOutputBuffer(bufferInfo, TIMEOUT_US)
                    if (outputBufferId >= 0) {
                        if (bufferInfo.flags and MediaCodec.BUFFER_FLAG_END_OF_STREAM != 0) {
                            outputDone = true
                        }

                        // Process frame
                        if (bufferInfo.size > 0) {
                            val image = decoder.getOutputImage(outputBufferId)
                            if (image != null) {
                                frameCount++

                                // 时间驱动：每500ms抽一帧
                                val shouldProcess = bufferInfo.presentationTimeUs - lastProcessedTimeUs >= FRAME_INTERVAL_US
                                if (shouldProcess) {
                                    lastProcessedTimeUs = bufferInfo.presentationTimeUs
                                    val yPlane = image.planes[0]

                                    // Calculate scaled dimensions - use smaller size for faster processing
                                    val scale = Math.min(
                                        MAX_DIMENSION.toFloat() / width,
                                        MAX_DIMENSION.toFloat() / height
                                    ).coerceAtMost(1.0f)
                                    val scaledWidth = (width * scale).toInt()
                                    val scaledHeight = (height * scale).toInt()

                                    // Only extract Y plane (grayscale) - sufficient for text detection
                                    val yBuffer = if (scale < 1.0f) {
                                        scalePlane(yPlane, scaledWidth, scaledHeight)
                                    } else {
                                        ByteArray(yPlane.buffer.remaining()).apply {
                                            yPlane.buffer.get(this)
                                        }
                                    }

                                    // Close image immediately after copying data to free native memory
                                    image.close()

                                    // Calculate progress
                                    val progress = if (durationUs > 0) {
                                        bufferInfo.presentationTimeUs.toFloat() / durationUs.toFloat()
                                    } else {
                                        0f
                                    }

                                    // Send frame data to Flutter - only Y plane for efficiency
                                    val frameData = hashMapOf(
                                        "type" to "frame",
                                        "width" to scaledWidth,
                                        "height" to scaledHeight,
                                        "yPlane" to yBuffer,
                                        "timestampMs" to (bufferInfo.presentationTimeUs / 1000),
                                        "frameNumber" to frameCount,
                                        "progress" to progress
                                    )

                                    Handler(Looper.getMainLooper()).post {
                                        eventSink?.success(frameData)
                                    }

                                    // Send progress update
                                    val progressData = hashMapOf(
                                        "type" to "progress",
                                        "progress" to progress
                                    )
                                    Handler(Looper.getMainLooper()).post {
                                        eventSink?.success(progressData)
                                    }
                                } else {
                                    // Skip frame - just close the image
                                    image.close()
                                }
                            }
                        }

                        decoder.releaseOutputBuffer(outputBufferId, false)
                    } else if (outputBufferId == MediaCodec.INFO_OUTPUT_FORMAT_CHANGED) {
                        // Format changed, can handle if needed
                    }
                }

                // Send completion
                Handler(Looper.getMainLooper()).post {
                    eventSink?.success(hashMapOf("type" to "complete"))
                }

            } catch (e: Exception) {
                Handler(Looper.getMainLooper()).post {
                    eventSink?.error("EXTRACT_ERROR", e.message, null)
                }
            } finally {
                decoder?.stop()
                decoder?.release()
                extractor.release()
            }
        }

        /**
         * Simple nearest-neighbor downscale for YUV plane data
         */
        private fun scalePlane(plane: android.media.Image.Plane, targetWidth: Int, targetHeight: Int): ByteArray {
            val srcBuffer = plane.buffer
            val rowStride = plane.rowStride
            val pixelStride = plane.pixelStride

            // Determine source dimensions from buffer size and strides
            val srcHeight = srcBuffer.remaining() / rowStride
            val srcWidth = if (pixelStride == 1) rowStride else (rowStride / pixelStride)

            val result = ByteArray(targetWidth * targetHeight)
            val scaleX = srcWidth.toFloat() / targetWidth
            val scaleY = srcHeight.toFloat() / targetHeight

            for (y in 0 until targetHeight) {
                val srcY = (y * scaleY).toInt().coerceIn(0, srcHeight - 1)
                for (x in 0 until targetWidth) {
                    val srcX = (x * scaleX).toInt().coerceIn(0, srcWidth - 1)
                    val srcIndex = srcY * rowStride + srcX * pixelStride
                    result[y * targetWidth + x] = srcBuffer.get(srcIndex)
                }
            }

            return result
        }

        @JvmStatic
        fun stopExtraction() {
            // Signal cancellation - actual implementation needs shared state
        }
    }
}
