use std::path::PathBuf;

use thiserror::Error;

/// Domain errors produced by the library parts of the tool.
///
/// `main` and `builder` use `anyhow` to add human-readable context on top of
/// these; these variants describe *what* went wrong in a typed way.
#[derive(Debug, Error)]
pub enum ChtmError {
    #[error("input file does not exist: {0}")]
    InputNotFound(PathBuf),

    #[error("input file has no extension, cannot determine its format: {0}")]
    NoExtension(PathBuf),

    #[error("unsupported audio format '{0}' (supported: mp3, wav, flac, ogg)")]
    UnsupportedFormat(String),

    #[error(
        "`ffmpeg` was not found on your PATH. Install it (e.g. `sudo apt install ffmpeg`) and try again"
    )]
    FfmpegNotFound,

    #[error("ffmpeg failed to convert the audio (exit status: {status}):\n{stderr}")]
    FfmpegFailed { status: String, stderr: String },

    #[error("no decodable audio track found in {0}")]
    NoAudioTrack(PathBuf),

    #[error("output folder already exists: {0}\n  (re-run with --force to overwrite it)")]
    OutputExists(PathBuf),

    #[error("could not analyze audio: {0}")]
    Analyze(String),

    #[error("image error: {0}")]
    Image(#[from] image::ImageError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
