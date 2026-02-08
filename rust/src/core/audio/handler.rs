//! Sherpa-NCNN ASR handler

use super::error::AudioError;
use log::{debug, error, info};
use sherpa_ncnn::{Recognizer, RecognizerConfig};

/// NCNN Recognizer 实例（非全局，由 RecipeProcessor 持有）
pub struct NcnnHandle {
    recognizer: Recognizer,
}

impl NcnnHandle {
    /// Initialize the NCNN recognizer with model files
    ///
    /// # Arguments
    /// * `model_dir` - Directory containing ncnn model files:
    ///   - encoder_jit_trace-pnnx.ncnn.param/bin
    ///   - decoder_jit_trace-pnnx.ncnn.param/bin
    ///   - joiner_jit_trace-pnnx.ncnn.param/bin
    ///   - tokens.txt
    pub fn new(model_dir: &str) -> Result<Self, AudioError> {
        info!("🔧 Loading Sherpa-NCNN model from: {}", model_dir);

        let num_threads = num_cpus::get().min(4) as i32;
        debug!("Using {} threads for NCNN", num_threads);

        let config = RecognizerConfig::new(model_dir).with_num_threads(num_threads);
        let recognizer = Recognizer::new(config).map_err(|e| {
            error!("❌ Failed to create NCNN recognizer: {}", e);
            AudioError::SherpaNcnn(format!("Failed to create recognizer: {}", e))
        })?;

        info!("✅ Sherpa-NCNN model loaded successfully");
        Ok(Self { recognizer })
    }

    /// Transcribe audio samples using NCNN
    ///
    /// # Arguments
    /// * `samples` - Audio samples as f32 array (normalized to [-1.0, 1.0])
    /// * `sample_rate` - Sample rate in Hz (must be 16000)
    /// * `_language` - Language hint (unused, kept for API compatibility)
    pub fn transcribe(
        &self,
        samples: &[f32],
        sample_rate: u32,
        _language: Option<&str>,
    ) -> Result<String, AudioError> {
        debug!(
            "🎤 Transcribing {} samples at {}Hz",
            samples.len(),
            sample_rate
        );

        if sample_rate != 16000 {
            error!(
                "❌ Invalid sample rate: {}Hz (NCNN requires 16000Hz)",
                sample_rate
            );
            return Err(AudioError::SherpaNcnn(format!(
                "Invalid sample rate: {}Hz (expected 16000Hz)",
                sample_rate
            )));
        }

        let result = self.recognizer.transcribe(samples, sample_rate as f32).map_err(|e| {
            error!("❌ Transcription failed: {}", e);
            AudioError::SherpaNcnn(e.to_string())
        })?;

        info!(
            "✅ Transcription complete! Length: {} chars",
            result.len()
        );
        debug!("Transcribed text: {}", result);

        Ok(result)
    }
}

impl Drop for NcnnHandle {
    fn drop(&mut self) {
        info!("🗑️ NcnnHandle: releasing NCNN recognizer");
    }
}
