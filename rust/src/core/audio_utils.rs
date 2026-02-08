use crate::core::audio_error::AudioError;
use log::{error, info};
use rubato::{Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType};

pub fn load_wav_mono_f32(path: &str) -> Result<Vec<f32>, AudioError> {
    info!("📖 Reading WAV file: {}", path);
    let mut reader = hound::WavReader::open(path)?;
    let spec = reader.spec();
    let channels = spec.channels.max(1) as usize;
    let estimated_samples = reader.duration() as usize / channels;
    let mut samples = Vec::with_capacity(estimated_samples.max(128));

    info!(
        "📊 WAV spec: {} Hz, {} channels, {} bits",
        spec.sample_rate, spec.channels, spec.bits_per_sample
    );

    if spec.sample_rate == 0 {
        return Err(AudioError::Wav(hound::Error::FormatError(
            "invalid sample rate".into(),
        )));
    }

    if spec.channels == 1 {
        for s in reader.samples::<i16>() {
            let v = s? as f32 / i16::MAX as f32;
            samples.push(v);
        }
    } else {
        let mut iter = reader.samples::<i16>();
        loop {
            let l = match iter.next() {
                Some(v) => v?,
                None => break,
            };
            let r = match iter.next() {
                Some(v) => v?,
                None => break,
            };
            let m = ((l as f32 + r as f32) * 0.5) / i16::MAX as f32;
            samples.push(m);
        }
    }

    info!("✓ Loaded {} mono samples from file", samples.len());

    if spec.sample_rate != 16_000 {
        Ok(resample_to_16k_mono(&samples, spec.sample_rate)?)
    } else {
        Ok(samples)
    }
}

pub fn resample_to_16k_mono(input: &[f32], in_rate: u32) -> Result<Vec<f32>, AudioError> {
    if in_rate == 16_000 {
        return Ok(input.to_vec());
    }

    if in_rate % 16_000 == 0 {
        let factor = (in_rate / 16_000) as usize;
        info!(
            "⚡ Fast downsample from {} Hz to 16 kHz (factor {})",
            in_rate, factor
        );
        return Ok(downsample_by_factor(input, factor));
    }

    info!(
        "🔧 Resampling {} samples from {} Hz to 16 kHz",
        input.len(),
        in_rate
    );
    let ratio = 16_000.0 / in_rate as f64;
    let params = SincInterpolationParameters {
        sinc_len: 48,
        f_cutoff: 0.90,
        interpolation: SincInterpolationType::Cubic,
        oversampling_factor: 4,
        window: rubato::WindowFunction::BlackmanHarris2,
    };

    let mut resampler =
        SincFixedIn::<f32>::new(ratio, 1.0, params, input.len(), 1).map_err(|e| {
            error!("❌ Resample creation failed: {}", e);
            AudioError::Resample(e.to_string())
        })?;

    let mut output = vec![vec![0.0f32; input.len() * 2]];
    resampler
        .process_into_buffer(&[input], &mut output, None)
        .map_err(|e| {
            error!("❌ Resample processing failed: {}", e);
            AudioError::Resample(e.to_string())
        })?;

    let result: Vec<f32> = output.into_iter().flatten().collect();
    info!(
        "✓ Resampling complete: {} -> {} samples",
        input.len(),
        result.len()
    );
    Ok(result)
}

/// Quickly downsample by averaging consecutive frames when the ratio is an integer
fn downsample_by_factor(input: &[f32], factor: usize) -> Vec<f32> {
    debug_assert!(factor > 0);
    let mut output = Vec::with_capacity((input.len() + factor - 1) / factor);
    let mut accumulator = 0.0_f32;
    let mut count = 0;

    for &sample in input {
        accumulator += sample;
        count += 1;
        if count == factor {
            output.push(accumulator / factor as f32);
            accumulator = 0.0;
            count = 0;
        }
    }

    if count > 0 {
        output.push(accumulator / count as f32);
    }

    output
}
