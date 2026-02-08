//! Silero VAD using sherpa-ncnn C API
//!
//! Uses the real Silero VAD model for accurate speech detection.

use super::error::AudioError;
use log::{debug, info};
use sherpa_ncnn::{Vad, VadConfig};

/// Chunk size for VAD processing (16000 samples = 1 second at 16kHz)
const VAD_CHUNK_SIZE: usize = 16000;

#[derive(Debug, Clone)]
pub struct SpeechSegment {
    pub start: f32,
    pub end: f32,
}

/// VAD 实例（非全局，由 RecipeProcessor 持有）
pub struct VadHandle {
    vad: Vad,
}

impl VadHandle {
    /// Initialize VAD with Silero model
    pub fn new(model_path: &str) -> Result<Self, AudioError> {
        info!("🔧 Initializing Silero VAD with model: {}", model_path);

        let config = VadConfig::new(model_path).with_num_threads(2);
        let vad = Vad::new(config, 60.0).map_err(|e| {
            AudioError::SherpaNcnn(format!("Failed to create VAD: {}", e))
        })?;

        info!("✅ Silero VAD initialized successfully");
        Ok(Self { vad })
    }

    /// Detect speech segments using Silero VAD
    pub fn detect_speech_segments(
        &mut self,
        samples: &[f32],
        sample_rate: u32,
    ) -> Result<Vec<SpeechSegment>, AudioError> {
        if sample_rate != 16000 {
            return Err(AudioError::Resample(format!(
                "VAD requires 16000Hz sample rate, got {}Hz",
                sample_rate
            )));
        }

        let total_samples = samples.len();
        let duration_secs = total_samples as f32 / sample_rate as f32;
        info!("🔍 Running Silero VAD on {:.1}s audio ({} samples)", duration_secs, total_samples);

        // Reset VAD state for new audio
        self.vad.reset();
        self.vad.clear();

        // Feed audio in larger chunks for efficiency
        let num_chunks = (total_samples + VAD_CHUNK_SIZE - 1) / VAD_CHUNK_SIZE;
        debug!("📦 Processing {} chunks of up to {} samples", num_chunks, VAD_CHUNK_SIZE);

        for i in 0..num_chunks {
            let start = i * VAD_CHUNK_SIZE;
            let end = (start + VAD_CHUNK_SIZE).min(total_samples);
            let chunk = &samples[start..end];
            self.vad.accept_waveform(chunk);
        }

        // Flush to detect the last segment
        self.vad.flush();

        // Collect all speech segments
        let raw_segments = self.vad.get_all_segments();
        let total_duration = samples.len() as f32 / sample_rate as f32;

        info!("📊 Raw VAD segments: {}", raw_segments.len());

        // Convert to time-based segments
        let segments: Vec<SpeechSegment> = raw_segments
            .into_iter()
            .map(|seg| {
                let start = seg.start as f32 / sample_rate as f32;
                let end = (seg.start + seg.samples.len() as i32) as f32 / sample_rate as f32;
                SpeechSegment {
                    start,
                    end: end.min(total_duration),
                }
            })
            .collect();

        info!(
            "✅ Silero VAD: {} speech segments found",
            segments.len()
        );

        for (i, seg) in segments.iter().enumerate() {
            debug!(
                "   Segment {}: {:.2}s - {:.2}s ({:.2}s)",
                i + 1,
                seg.start,
                seg.end,
                seg.end - seg.start
            );
        }

        // Fallback if no segments detected
        if segments.is_empty() {
            info!("⚠️  No speech detected, using full audio as single segment");
            return Ok(vec![SpeechSegment {
                start: 0.0,
                end: total_duration,
            }]);
        }

        Ok(segments)
    }

    pub fn extract_segment(samples: &[f32], sample_rate: u32, segment: &SpeechSegment) -> Vec<f32> {
        let start_sample = (segment.start * sample_rate as f32) as usize;
        let end_sample = (segment.end * sample_rate as f32) as usize;

        let start = start_sample.min(samples.len());
        let end = end_sample.min(samples.len());

        samples[start..end].to_vec()
    }
}

impl Drop for VadHandle {
    fn drop(&mut self) {
        info!("🗑️ VadHandle: releasing Silero VAD");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_segment() {
        let samples: Vec<f32> = (0..16000).map(|i| i as f32 / 16000.0).collect();
        let segment = SpeechSegment {
            start: 0.5,
            end: 1.0,
        };

        let extracted = VadHandle::extract_segment(&samples, 16000, &segment);
        assert_eq!(extracted.len(), 8000);
    }
}
