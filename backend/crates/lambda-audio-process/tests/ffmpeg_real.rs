//! Exercises `FfmpegTranscoder` against a real `ffmpeg` binary, located via
//! `FFMPEG_PATH` or `PATH` (`docs/design/media.md` §12). If
//! neither resolves, the test logs a note and skips — unless `REQUIRE_FFMPEG=1`
//! is set (CI sets it after installing ffmpeg), in which case it panics.
//!
//! The input is generated here, not a binary fixture: a short WAV file
//! containing one second of a 440 Hz sine tone, written with a few lines of
//! Rust.

use audio_process::transcode::{AudioTranscoder, FfmpegTranscoder};

/// A mono 8kHz, 16-bit PCM WAV of `seconds` of a 440 Hz sine tone.
fn sine_wav(seconds: f32) -> Vec<u8> {
    let sample_rate: u32 = 8000;
    let num_samples = (sample_rate as f32 * seconds) as u32;
    let data_len = num_samples * 2; // 16-bit mono

    let mut wav = Vec::new();
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&1u16.to_le_bytes()); // mono
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&(sample_rate * 2).to_le_bytes()); // byte rate
    wav.extend_from_slice(&2u16.to_le_bytes()); // block align
    wav.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());

    for i in 0..num_samples {
        let t = i as f32 / sample_rate as f32;
        let sample = (t * 440.0 * std::f32::consts::TAU).sin() * i16::MAX as f32 * 0.8;
        wav.extend_from_slice(&(sample as i16).to_le_bytes());
    }
    wav
}

/// Finds `ffmpeg`, or skips/panics per the module doc.
fn require_ffmpeg_or_skip() -> Option<String> {
    match audio_process::transcode::find_ffmpeg() {
        Some(path) => Some(path),
        None if std::env::var("REQUIRE_FFMPEG").as_deref() == Ok("1") => {
            panic!("REQUIRE_FFMPEG=1 but no ffmpeg binary was found via FFMPEG_PATH or PATH")
        }
        None => {
            eprintln!("skipping: no ffmpeg binary found via FFMPEG_PATH or PATH");
            None
        }
    }
}

#[test]
fn decodes_a_sine_tone_to_the_right_duration() {
    let Some(ffmpeg_path) = require_ffmpeg_or_skip() else {
        return;
    };
    let transcoder = FfmpegTranscoder::new(ffmpeg_path);
    let wav = sine_wav(2.0);

    let samples = transcoder
        .decode_samples(&wav)
        .expect("ffmpeg decodes the wav");
    let duration_ms = audio_process::process::duration_ms(
        samples.len(),
        audio_process::transcode::DECODE_SAMPLE_RATE_HZ,
    );
    // Allow a little slack for ffmpeg's own framing.
    assert!(
        (1_900..=2_100).contains(&duration_ms),
        "duration was {duration_ms}ms"
    );
    assert!(samples.iter().any(|&s| s != 0), "not all-silent");
}

#[test]
fn transcodes_to_a_nonempty_opus_webm() {
    let Some(ffmpeg_path) = require_ffmpeg_or_skip() else {
        return;
    };
    let transcoder = FfmpegTranscoder::new(ffmpeg_path);
    let wav = sine_wav(1.0);

    let webm = transcoder
        .transcode_opus(&wav)
        .expect("ffmpeg transcodes the wav");
    assert!(!webm.is_empty());
    // The EBML header every WebM/Matroska file starts with.
    assert_eq!(&webm[..4], &[0x1A, 0x45, 0xDF, 0xA3]);
}

#[test]
fn an_oversized_duration_request_is_bounded_by_the_decode_budget() {
    let Some(ffmpeg_path) = require_ffmpeg_or_skip() else {
        return;
    };
    let transcoder = FfmpegTranscoder::new(ffmpeg_path);
    // A clip longer than MAX_AUDIO_SECONDS + 2: the decode pass's `-t` flag
    // should cap how much of it ffmpeg actually reads.
    let long_seconds = (shared::config::MAX_AUDIO_SECONDS + 10) as f32;
    let wav = sine_wav(long_seconds);

    let samples = transcoder
        .decode_samples(&wav)
        .expect("ffmpeg decodes the wav");
    let duration_ms = audio_process::process::duration_ms(
        samples.len(),
        audio_process::transcode::DECODE_SAMPLE_RATE_HZ,
    );
    let bound_ms = (u64::from(shared::config::MAX_AUDIO_SECONDS) + 3) * 1000;
    assert!(
        duration_ms <= bound_ms,
        "decode pass should be bounded by -t, got {duration_ms}ms"
    );
}
