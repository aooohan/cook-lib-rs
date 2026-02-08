use thiserror::Error;

#[derive(Debug, Error)]
pub enum AudioError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("WAV format error: {0}")]
    Wav(#[from] hound::Error),
    #[error("Resample error: {0}")]
    Resample(String),
    #[error("Model not initialized")]
    NotInitialized,
    #[error("Model load failed: {0}")]
    ModelLoadFailed(String),
    #[error("Sherpa-NCNN error: {0}")]
    SherpaNcnn(String),
}
