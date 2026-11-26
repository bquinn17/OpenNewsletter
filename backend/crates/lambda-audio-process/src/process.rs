//! Pure audio logic (no AWS, no subprocess) — duration maths and waveform
//! peak bucketing (`docs/design/media.md` §12, decided semantics).

use shared::config::{AUDIO_DURATION_SLACK_MS, MAX_AUDIO_SECONDS};

/// Duration in milliseconds of `sample_count` mono samples at `sample_rate`.
pub fn duration_ms(sample_count: usize, sample_rate: u32) -> u64 {
    (sample_count as u64) * 1000 / sample_rate as u64
}

/// Whether a decoded clip's duration is within the cap plus slack
/// (`docs/design/media.md` §12's `AUDIO_DURATION_SLACK_MS`).
pub fn duration_within_cap(duration_ms: u64) -> bool {
    duration_ms <= u64::from(MAX_AUDIO_SECONDS) * 1000 + AUDIO_DURATION_SLACK_MS
}

/// Splits `samples` into `peak_count` equal-size buckets (the last bucket
/// absorbs any remainder), takes each bucket's maximum absolute amplitude,
/// then scales so the loudest bucket is 255. An all-silent clip (every
/// bucket's max is 0) returns all zeros rather than dividing by zero.
///
/// `samples.is_empty()` returns `peak_count` zeros — the decode pass never
/// reaches here with zero samples (it errors first), but this keeps the
/// function total.
pub fn compute_peaks(samples: &[i16], peak_count: usize) -> Vec<u8> {
    if peak_count == 0 {
        return Vec::new();
    }
    if samples.is_empty() {
        return vec![0; peak_count];
    }

    let bucket_size = samples.len().div_ceil(peak_count).max(1);
    let mut bucket_max: Vec<i32> = Vec::with_capacity(peak_count);
    for chunk in samples.chunks(bucket_size) {
        let max_abs = chunk
            .iter()
            .map(|&s| i32::from(s).unsigned_abs() as i32)
            .max()
            .unwrap_or(0);
        bucket_max.push(max_abs);
    }
    // `chunks` can yield fewer than `peak_count` buckets when `samples` is
    // short; pad with silence so the output is always exactly `peak_count`.
    bucket_max.resize(peak_count, 0);

    let loudest = *bucket_max.iter().max().unwrap_or(&0);
    if loudest == 0 {
        return vec![0; peak_count];
    }
    bucket_max
        .into_iter()
        .map(|m| ((m as f64 / loudest as f64) * 255.0).round() as u8)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_ms_divides_sample_count_by_rate() {
        assert_eq!(duration_ms(8000, 8000), 1000);
        assert_eq!(duration_ms(4000, 8000), 500);
        assert_eq!(duration_ms(0, 8000), 0);
    }

    #[test]
    fn duration_within_cap_allows_the_slack() {
        let cap_ms = u64::from(MAX_AUDIO_SECONDS) * 1000;
        assert!(duration_within_cap(cap_ms));
        assert!(duration_within_cap(cap_ms + AUDIO_DURATION_SLACK_MS));
        assert!(!duration_within_cap(cap_ms + AUDIO_DURATION_SLACK_MS + 1));
    }

    #[test]
    fn peaks_has_exactly_peak_count_entries() {
        let samples = vec![100i16; 1000];
        assert_eq!(compute_peaks(&samples, 48).len(), 48);
        // Fewer samples than buckets still pads to the requested count.
        assert_eq!(compute_peaks(&[1, 2, 3], 48).len(), 48);
    }

    #[test]
    fn an_all_silent_clip_is_all_zero_peaks() {
        let samples = vec![0i16; 1000];
        assert_eq!(compute_peaks(&samples, 48), vec![0u8; 48]);
    }

    #[test]
    fn the_loudest_bucket_scales_to_255() {
        // One very loud bucket, the rest quiet.
        let mut samples = vec![10i16; 4800];
        for s in samples.iter_mut().take(100) {
            *s = i16::MAX;
        }
        let peaks = compute_peaks(&samples, 48);
        assert_eq!(peaks.iter().copied().max(), Some(255));
        assert!(peaks.iter().any(|&p| p < 255));
    }

    #[test]
    fn a_uniformly_loud_clip_is_all_255() {
        let samples = vec![i16::MAX; 4800];
        assert_eq!(compute_peaks(&samples, 48), vec![255u8; 48]);
    }
}
