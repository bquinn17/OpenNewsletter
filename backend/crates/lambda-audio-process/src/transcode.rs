//! Audio decode/transcode behind a small trait, so the handler can be
//! exercised in tests without a real `ffmpeg` binary
//! (`docs/design/media.md` §12, decided semantics).
//!
//! `FfmpegTranscoder` shells out to `ffmpeg` twice per clip:
//! 1. **Decode pass** — mono 8 kHz PCM, bounded to `MAX_AUDIO_SECONDS + 2`
//!    seconds of input so a maliciously long file can't run unbounded work.
//!    Gives both the duration (`samples.len() / 8000`) and the waveform data,
//!    with no separate `ffprobe` call.
//! 2. **Transcode pass** — mono 48 kHz Opus in a WebM container at 24 kbps,
//!    tuned for speech (`-application voip`), metadata stripped, seek index
//!    moved to the front (`-cues_to_front 1`).

use shared::config::MAX_AUDIO_SECONDS;
use std::path::Path;
use std::process::Command;

#[derive(Debug, thiserror::Error)]
pub enum TranscodeError {
    #[error("ffmpeg exited with a non-zero status: {0}")]
    NonZeroExit(String),
    #[error("ffmpeg produced no decodable audio samples")]
    NoSamples,
    #[error("io error running ffmpeg: {0}")]
    Io(String),
}

/// The decode/transcode operations `lambda-audio-process`'s handler needs.
/// Implemented for real by [`FfmpegTranscoder`]; tests substitute
/// `tests::fake::FakeTranscoder` (handler-level) or exercise
/// [`FfmpegTranscoder`] directly when a real `ffmpeg` binary is available.
pub trait AudioTranscoder: Send + Sync {
    /// Mono 8 kHz PCM samples decoded from `input`, bounded to
    /// `MAX_AUDIO_SECONDS + 2` seconds of source audio.
    fn decode_samples(&self, input: &[u8]) -> Result<Vec<i16>, TranscodeError>;
    /// Mono 48 kHz Opus-in-WebM bytes transcoded from `input`.
    fn transcode_opus(&self, input: &[u8]) -> Result<Vec<u8>, TranscodeError>;
}

/// The sample rate the decode pass requests — enough for a clear waveform
/// envelope and duration measurement, far below what speech needs for
/// amplitude bucketing.
pub const DECODE_SAMPLE_RATE_HZ: u32 = 8000;

pub struct FfmpegTranscoder {
    pub ffmpeg_path: String,
}

impl FfmpegTranscoder {
    pub fn new(ffmpeg_path: impl Into<String>) -> Self {
        Self {
            ffmpeg_path: ffmpeg_path.into(),
        }
    }

    fn scratch_path(&self, suffix: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("audio-process-{}{suffix}", uuid::Uuid::new_v4()))
    }
}

impl AudioTranscoder for FfmpegTranscoder {
    fn decode_samples(&self, input: &[u8]) -> Result<Vec<i16>, TranscodeError> {
        let in_path = self.scratch_path(".bin");
        std::fs::write(&in_path, input).map_err(|e| TranscodeError::Io(e.to_string()))?;
        let _guard = TempFileGuard(in_path.clone());

        let output = Command::new(&self.ffmpeg_path)
            .args(["-nostdin", "-v", "error", "-i"])
            .arg(&in_path)
            .args([
                "-vn",
                "-map",
                "0:a:0",
                "-t",
                &(MAX_AUDIO_SECONDS + 2).to_string(),
                "-ac",
                "1",
                "-ar",
                &DECODE_SAMPLE_RATE_HZ.to_string(),
                "-f",
                "s16le",
                "pipe:1",
            ])
            .output()
            .map_err(|e| TranscodeError::Io(e.to_string()))?;

        if !output.status.success() {
            return Err(TranscodeError::NonZeroExit(
                String::from_utf8_lossy(&output.stderr).into_owned(),
            ));
        }
        let samples = pcm_bytes_to_samples(&output.stdout);
        if samples.is_empty() {
            return Err(TranscodeError::NoSamples);
        }
        Ok(samples)
    }

    fn transcode_opus(&self, input: &[u8]) -> Result<Vec<u8>, TranscodeError> {
        let in_path = self.scratch_path(".bin");
        let out_path = self.scratch_path(".webm");
        std::fs::write(&in_path, input).map_err(|e| TranscodeError::Io(e.to_string()))?;
        let _in_guard = TempFileGuard(in_path.clone());
        let _out_guard = TempFileGuard(out_path.clone());

        let output = Command::new(&self.ffmpeg_path)
            .args(["-nostdin", "-v", "error", "-y", "-i"])
            .arg(&in_path)
            .args([
                "-vn",
                "-map",
                "0:a:0",
                "-map_metadata",
                "-1",
                "-ac",
                "1",
                "-ar",
                "48000",
                "-c:a",
                "libopus",
                "-b:a",
                "24k",
                "-vbr",
                "on",
                "-application",
                "voip",
                "-cues_to_front",
                "1",
                "-f",
                "webm",
            ])
            .arg(&out_path)
            .output()
            .map_err(|e| TranscodeError::Io(e.to_string()))?;

        if !output.status.success() {
            return Err(TranscodeError::NonZeroExit(
                String::from_utf8_lossy(&output.stderr).into_owned(),
            ));
        }
        std::fs::read(&out_path).map_err(|e| TranscodeError::Io(e.to_string()))
    }
}

/// Little-endian i16 PCM bytes to samples; an odd trailing byte is dropped.
fn pcm_bytes_to_samples(bytes: &[u8]) -> Vec<i16> {
    bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| i16::from_le_bytes(*c))
        .collect()
}

/// Best-effort temp-file cleanup, so a scratch file doesn't survive a short
/// Lambda invocation forever (`/tmp` persists across warm invocations).
struct TempFileGuard(std::path::PathBuf);

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Locates `ffmpeg`: `FFMPEG_PATH` if set, else the first `ffmpeg` on `PATH`.
/// `None` when neither resolves — callers decide whether that's a skip or a
/// panic (`REQUIRE_FFMPEG=1` in CI).
pub fn find_ffmpeg() -> Option<String> {
    if let Ok(path) = std::env::var("FFMPEG_PATH") {
        if Path::new(&path).exists() {
            return Some(path);
        }
    }
    let path_var = std::env::var("PATH").ok()?;
    for dir in path_var.split(':') {
        let candidate = Path::new(dir).join("ffmpeg");
        if candidate.exists() {
            return Some(candidate.to_string_lossy().into_owned());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pcm_bytes_decode_little_endian_pairs() {
        // 1 (0x0001) then -1 (0xFFFF), little-endian.
        let bytes = [0x01, 0x00, 0xFF, 0xFF];
        assert_eq!(pcm_bytes_to_samples(&bytes), vec![1, -1]);
    }

    #[test]
    fn a_trailing_odd_byte_is_dropped() {
        let bytes = [0x01, 0x00, 0xFF];
        assert_eq!(pcm_bytes_to_samples(&bytes), vec![1]);
    }
}
