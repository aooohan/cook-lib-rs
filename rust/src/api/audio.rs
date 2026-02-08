//! 音频识别器 - ASR + VAD

use crate::core::audio::{AudioError, NcnnHandle, SpeechSegment, VadHandle};
use crate::core::audio::{load_wav_mono_f32, resample_to_16k_mono};
use flutter_rust_bridge::frb;
use log::{debug, error, info};
use std::path::Path;
use std::sync::Mutex;

/// 音频识别器 - 封装 ASR + VAD
///
/// ```dart
/// final recognizer = await AudioRecognizer.create(modelsDir: "/path/to/models");
/// final text = await recognizer.transcribeAudio(path: wavPath);
/// ```
#[frb(opaque)]
pub struct AudioRecognizer {
    models_dir: String,
    ncnn: NcnnHandle,
    vad: Mutex<VadHandle>,
}

impl AudioRecognizer {
    /// 创建音频识别器并加载模型
    ///
    /// models_dir 下需要包含：
    /// - sherpa-ncnn/ (ASR 模型)
    /// - silero-vad/ (VAD 模型)
    #[frb(dart_async)]
    pub async fn create(models_dir: String) -> Result<Self, AudioError> {
        info!("🎙️ AudioRecognizer: initializing with models_dir: {}", models_dir);
        crate::init_logging();

        // 初始化 Sherpa-NCNN (ASR)
        let sherpa_path = Path::new(&models_dir).join("sherpa-ncnn");
        let ncnn = if sherpa_path.exists() {
            info!("🎙️ Loading Sherpa-NCNN from {:?}", sherpa_path);
            NcnnHandle::new(&sherpa_path.to_string_lossy())?
        } else {
            return Err(AudioError::ModelLoadFailed(format!(
                "sherpa-ncnn model not found at {:?}",
                sherpa_path
            )));
        };

        // 初始化 Silero-VAD
        let vad_path = Path::new(&models_dir).join("silero-vad");
        let vad = if vad_path.exists() {
            info!("🔇 Loading Silero-VAD from {:?}", vad_path);
            VadHandle::new(&vad_path.to_string_lossy())?
        } else {
            return Err(AudioError::ModelLoadFailed(format!(
                "silero-vad model not found at {:?}",
                vad_path
            )));
        };

        info!("✅ AudioRecognizer initialized successfully");
        Ok(Self {
            models_dir,
            ncnn,
            vad: Mutex::new(vad),
        })
    }

    /// 转录音频文件（WAV 格式）
    #[frb(dart_async)]
    pub async fn transcribe_audio(&self, path: String, language: Option<String>) -> Result<String, AudioError> {
        info!("🎵 Loading WAV file: {}", path);

        match load_wav_mono_f32(&path) {
            Ok(pcm) => {
                info!("📊 WAV loaded: {} samples", pcm.len());
                debug!("Language: {:?}", language);
                self.transcribe_pcm(pcm, 16_000, language).await
            }
            Err(e) => {
                error!("❌ Failed to load WAV: {}", e);
                Err(e)
            }
        }
    }

    /// 转录 PCM 数据 (内部使用)
    async fn transcribe_pcm(
        &self,
        pcm: Vec<f32>,
        sample_rate: u32,
        language: Option<String>,
    ) -> Result<String, AudioError> {
        info!(
            "🔄 Starting VAD-based transcription: {} samples at {} Hz",
            pcm.len(),
            sample_rate
        );

        let pcm_16k = if sample_rate == 16_000 {
            info!("✓ Already 16kHz, skipping resample");
            pcm
        } else {
            info!("🔧 Resampling from {} Hz to 16 kHz...", sample_rate);
            match resample_to_16k_mono(&pcm, sample_rate) {
                Ok(resampled) => {
                    info!("✓ Resampled: {} -> {} samples", pcm.len(), resampled.len());
                    resampled
                }
                Err(e) => {
                    error!("❌ Resample failed: {}", e);
                    return Err(e);
                }
            }
        };

        info!("🔍 Running Silero VAD to detect speech segments...");
        let speech_segments = {
            let mut vad = self.vad.lock().map_err(|e| {
                AudioError::SherpaNcnn(format!("VAD lock poisoned: {}", e))
            })?;
            match vad.detect_speech_segments(&pcm_16k, 16_000) {
                Ok(segments) => segments,
                Err(e) => {
                    error!("❌ VAD detection failed: {}", e);
                    error!("⚠️  Falling back to fixed-time chunking");
                    let duration = pcm_16k.len() as f32 / 16_000.0;
                    vec![SpeechSegment {
                        start: 0.0,
                        end: duration,
                    }]
                }
            }
        };

        info!(
            "🎙️  Running ASR on {} speech segments...",
            speech_segments.len()
        );

        let mut lines: Vec<String> = Vec::new();

        for (index, segment) in speech_segments.iter().enumerate() {
            info!(
                "📦 Segment {}: {:.2}s - {:.2}s (duration: {:.2}s)",
                index + 1,
                segment.start,
                segment.end,
                segment.end - segment.start
            );

            let segment_samples = VadHandle::extract_segment(&pcm_16k, 16_000, segment);

            debug!(
                "   Extracted {} samples for segment {}",
                segment_samples.len(),
                index + 1
            );

            match self.ncnn.transcribe(&segment_samples, 16_000, language.as_deref()) {
                Ok(result) => {
                    info!("✅ Segment {} complete ({} chars)", index + 1, result.len());
                    debug!("   Text: {}", result);

                    if !result.trim().is_empty() {
                        let start_time = format_timestamp(segment.start);
                        let end_time = format_timestamp(segment.end);
                        let line = format!("{} - {}  --  {}", start_time, end_time, result.trim());
                        lines.push(line);
                    }
                }
                Err(e) => {
                    error!("❌ Segment {} failed: {}", index + 1, e);
                    continue;
                }
            }
        }

        let result = lines.join("\n");
        info!(
            "🎯 All segments processed, total lines: {}",
            lines.len()
        );
        debug!("Result:\n{}", result);
        Ok(result)
    }

    /// 获取模型目录
    #[frb(sync, getter)]
    pub fn models_dir(&self) -> String {
        self.models_dir.clone()
    }
}

impl Drop for AudioRecognizer {
    fn drop(&mut self) {
        info!("🗑️ AudioRecognizer: releasing resources (NCNN + VAD)");
    }
}

/// Format seconds to HH:MM:SS:mm
fn format_timestamp(seconds: f32) -> String {
    let hours = (seconds / 3600.0) as u32;
    let minutes = ((seconds % 3600.0) / 60.0) as u32;
    let secs = (seconds % 60.0) as u32;
    let millis = ((seconds % 1.0) * 100.0) as u32;
    format!("{:02}:{:02}:{:02}:{:02}", hours, minutes, secs, millis)
}
