use crate::core::audio_error::AudioError;
use crate::core::audio_utils::{load_wav_mono_f32, resample_to_16k_mono};
use crate::core::ncnn_handler::NcnnHandle;
use crate::core::ncnn_vad::{SpeechSegment, VadHandle};
use flutter_rust_bridge::frb;
use log::{debug, error, info};

/// Format seconds to HH:MM:SS:mm
fn format_timestamp(seconds: f32) -> String {
    let hours = (seconds / 3600.0) as u32;
    let minutes = ((seconds % 3600.0) / 60.0) as u32;
    let secs = (seconds % 60.0) as u32;
    let millis = ((seconds % 1.0) * 100.0) as u32;
    format!("{:02}:{:02}:{:02}:{:02}", hours, minutes, secs, millis)
}

#[frb(sync)]
pub fn initSherpa(modelPath: String) -> Result<(), AudioError> {
    info!(
        "🎙️  Starting Sherpa initialization with model: {}",
        modelPath
    );
    crate::init_logging();

    match NcnnHandle::init(modelPath.clone()) {
        Ok(_) => {
            info!("✅ Sherpa initialized successfully");
            Ok(())
        }
        Err(e) => {
            error!("❌ Sherpa initialization failed: {}", e);
            Err(e)
        }
    }
}

#[frb(sync)]
pub fn initVad(vadModelPath: String) -> Result<(), AudioError> {
    info!("🔧 Starting VAD initialization with model: {}", vadModelPath);
    
    match VadHandle::init(vadModelPath) {
        Ok(_) => {
            info!("✅ VAD initialized successfully");
            Ok(())
        }
        Err(e) => {
            error!("❌ VAD initialization failed: {}", e);
            Err(e)
        }
    }
}

#[frb(dart_async)]
pub async fn transcribeAudio(path: String, language: Option<String>) -> Result<String, AudioError> {
    info!("🎵 Loading WAV file: {}", path);

    match load_wav_mono_f32(&path) {
        Ok(pcm) => {
            info!("📊 WAV loaded: {} samples", pcm.len());
            debug!("Language: {:?}", language);
            transcribePcm(pcm, 16_000, language).await
        }
        Err(e) => {
            error!("❌ Failed to load WAV: {}", e);
            Err(e)
        }
    }
}

#[frb(dart_async)]
pub async fn transcribePcm(
    pcm: Vec<f32>,
    sampleRate: u32,
    language: Option<String>,
) -> Result<String, AudioError> {
    info!(
        "🔄 Starting VAD-based transcription: {} samples at {} Hz",
        pcm.len(),
        sampleRate
    );

    let pcm_16k = if sampleRate == 16_000 {
        info!("✓ Already 16kHz, skipping resample");
        pcm
    } else {
        info!("🔧 Resampling from {} Hz to 16 kHz...", sampleRate);
        match resample_to_16k_mono(&pcm, sampleRate) {
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
    let speech_segments = match VadHandle::detect_speech_segments(&pcm_16k, 16_000) {
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
        
        debug!("   Extracted {} samples for segment {}", segment_samples.len(), index + 1);

        match NcnnHandle::transcribe(&segment_samples, 16_000, language.as_deref()) {
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
