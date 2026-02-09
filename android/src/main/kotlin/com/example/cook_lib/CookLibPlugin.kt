package com.example.cook_lib

import android.content.Context
import androidx.annotation.NonNull
import io.flutter.embedding.engine.plugins.FlutterPlugin
import io.flutter.plugin.common.EventChannel
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel
import io.flutter.plugin.common.MethodChannel.MethodCallHandler
import io.flutter.plugin.common.MethodChannel.Result
import java.util.concurrent.Executors

class CookLibPlugin: FlutterPlugin, MethodCallHandler, EventChannel.StreamHandler {
    private lateinit var methodChannel: MethodChannel
    private lateinit var eventChannel: EventChannel
    private lateinit var context: Context
    private var eventSink: EventChannel.EventSink? = null
    private val executor = Executors.newSingleThreadExecutor()

    override fun onAttachedToEngine(@NonNull flutterPluginBinding: FlutterPlugin.FlutterPluginBinding) {
        context = flutterPluginBinding.applicationContext

        // 音频解码通道 - 用于一次性解码请求
        methodChannel = MethodChannel(flutterPluginBinding.binaryMessenger, "cook_lib/audio/decoder")
        methodChannel.setMethodCallHandler(this)

        // 视频帧流通道 - 用于流式帧数据传输
        eventChannel = EventChannel(flutterPluginBinding.binaryMessenger, "cook_lib/video/frame_stream")
        eventChannel.setStreamHandler(this)
    }

    override fun onMethodCall(@NonNull call: MethodCall, @NonNull result: Result) {
        when (call.method) {
            "decodeToWav" -> {
                val inputPath = call.argument<String>("inputPath")
                if (inputPath == null) {
                    result.error("INVALID_ARGUMENT", "inputPath is required", null)
                    return
                }
                executor.execute {
                    try {
                        val wavPath = AudioDecoder.decodeToWav(context, inputPath)
                        android.os.Handler(android.os.Looper.getMainLooper()).post {
                            result.success(wavPath)
                        }
                    } catch (e: Exception) {
                        android.os.Handler(android.os.Looper.getMainLooper()).post {
                            result.error("DECODE_ERROR", e.message, null)
                        }
                    }
                }
            }
            else -> {
                result.notImplemented()
            }
        }
    }

    override fun onListen(arguments: Any?, events: EventChannel.EventSink?) {
        eventSink = events

        // 从参数中获取 videoPath，立即开始提取
        val args = arguments as? Map<*, *>
        val videoPath = args?.get("videoPath") as? String
        if (videoPath != null) {
            executor.execute {
                VideoFrameExtractor.extractFrames(videoPath, eventSink)
            }
        }
    }

    override fun onCancel(arguments: Any?) {
        // 取消订阅时自动停止提取
        VideoFrameExtractor.stopExtraction()
        eventSink = null
    }

    override fun onDetachedFromEngine(@NonNull binding: FlutterPlugin.FlutterPluginBinding) {
        methodChannel.setMethodCallHandler(null)
        eventChannel.setStreamHandler(null)
        executor.shutdown()
    }
}
