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

        methodChannel = MethodChannel(flutterPluginBinding.binaryMessenger, "cook_lib/methods")
        methodChannel.setMethodCallHandler(this)

        eventChannel = EventChannel(flutterPluginBinding.binaryMessenger, "cook_lib/video_frames")
        eventChannel.setStreamHandler(this)
    }

    override fun onMethodCall(@NonNull call: MethodCall, @NonNull result: Result) {
        when (call.method) {
            "decodeAudioToWav" -> {
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
            "extractVideoFrames" -> {
                val videoPath = call.argument<String>("videoPath")
                if (videoPath == null) {
                    result.error("INVALID_ARGUMENT", "videoPath is required", null)
                    return
                }
                executor.execute {
                    VideoFrameExtractor.extractFrames(videoPath, eventSink)
                }
                result.success(null)
            }
            "stopExtraction" -> {
                VideoFrameExtractor.stopExtraction()
                result.success(null)
            }
            else -> {
                result.notImplemented()
            }
        }
    }

    override fun onListen(arguments: Any?, events: EventChannel.EventSink?) {
        eventSink = events
    }

    override fun onCancel(arguments: Any?) {
        eventSink = null
    }

    override fun onDetachedFromEngine(@NonNull binding: FlutterPlugin.FlutterPluginBinding) {
        methodChannel.setMethodCallHandler(null)
        eventChannel.setStreamHandler(null)
        executor.shutdown()
    }
}
