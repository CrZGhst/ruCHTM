use std::path::Path;
use std::process::Command;

use symphonia::core::audio::{SampleBuffer, SignalSpec};
use symphonia::core::codecs::{CODEC_TYPE_NULL, DecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use crate::error::ChtmError;

/// Result of analyzing the source audio.
#[derive(Debug, Clone)]
pub struct AudioInfo {
    pub duration_secs: f64,
    pub bpm: f32,
    /// Detected onset (transient) times in seconds. Used to place notes that
    /// follow the music; empty when nothing usable was found.
    pub onsets: Vec<f64>,
}

/// Hop size (in mono samples) for the onset-strength envelope used by the
/// tempo estimator. ~512 gives ~3 BPM resolution at 44.1 kHz, which is plenty
/// for an auto-generated chart.
const ENVELOPE_HOP: usize = 512;
const FALLBACK_BPM: f32 = 120.0;

/// Center of the perceptual tempo prior (BPM). Tempi near this are favored,
/// which resolves the octave ambiguity inherent in autocorrelation.
const TEMPO_PRIOR_CENTER_BPM: f32 = 120.0;
/// Width of the (log-normal) tempo prior, in octaves.
const TEMPO_PRIOR_STD_OCT: f32 = 1.0;

/// Convert any supported input into an Ogg/Vorbis `song.ogg` by shelling out
/// to the system `ffmpeg`. The cover-art / video stream is dropped (`-vn`).
pub fn convert_to_ogg(input: &Path, output: &Path) -> Result<(), ChtmError> {
    let result = Command::new("ffmpeg")
        .arg("-y") // overwrite without prompting
        .arg("-loglevel")
        .arg("error")
        .arg("-i")
        .arg(input)
        .arg("-vn") // strip any embedded cover/video stream
        .arg("-c:a")
        .arg("libvorbis")
        .arg("-q:a")
        .arg("5") // ~160 kbps VBR, transparent enough for gameplay
        .arg(output)
        .output();

    let out = match result {
        Ok(out) => out,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(ChtmError::FfmpegNotFound);
        }
        Err(e) => return Err(ChtmError::Io(e)),
    };

    if !out.status.success() {
        return Err(ChtmError::FfmpegFailed {
            status: out.status.to_string(),
            stderr: String::from_utf8_lossy(&out.stderr).trim().to_string(),
        });
    }

    Ok(())
}

/// Decode the file once to determine its length and estimate its tempo.
pub fn analyze(path: &Path) -> Result<AudioInfo, ChtmError> {
    let file = std::fs::File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| ChtmError::Analyze(e.to_string()))?;

    let mut format = probed.format;

    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| ChtmError::NoAudioTrack(path.to_path_buf()))?;

    let track_id = track.id;
    let sample_rate = track.codec_params.sample_rate.unwrap_or(44_100);
    // Prefer the container's frame count for an accurate length; fall back to
    // counting decoded frames below if it isn't advertised.
    let declared_frames = track.codec_params.n_frames;

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| ChtmError::Analyze(e.to_string()))?;

    let mut decoded_frames: u64 = 0;
    let mut envelope: Vec<f32> = Vec::new();
    let mut acc_sq: f64 = 0.0;
    let mut acc_n: usize = 0;

    // Iterate until `next_packet` errors — typically a clean EOF, at which
    // point we use whatever we have decoded so far.
    while let Ok(packet) = format.next_packet() {
        if packet.track_id() != track_id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            // A single corrupt packet shouldn't abort the whole analysis.
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(_) => break,
        };

        let spec: SignalSpec = *decoded.spec();
        let frames = decoded.frames();
        if frames == 0 {
            continue;
        }
        decoded_frames += frames as u64;

        let channels = spec.channels.count().max(1);
        let mut sample_buf = SampleBuffer::<f32>::new(frames as u64, spec);
        sample_buf.copy_interleaved_ref(decoded);
        let samples = sample_buf.samples();

        for frame in 0..frames {
            let mut sum = 0.0f32;
            for ch in 0..channels {
                sum += samples[frame * channels + ch];
            }
            let mono = sum / channels as f32;
            acc_sq += (mono * mono) as f64;
            acc_n += 1;
            if acc_n == ENVELOPE_HOP {
                envelope.push((acc_sq / ENVELOPE_HOP as f64).sqrt() as f32);
                acc_sq = 0.0;
                acc_n = 0;
            }
        }
    }

    let frames_for_len = match declared_frames {
        Some(n) if n > 0 => n,
        _ => decoded_frames,
    };
    let duration_secs = frames_for_len as f64 / sample_rate as f64;

    let fps = sample_rate as f32 / ENVELOPE_HOP as f32;
    let onset_strength = onset_strength(&envelope);
    let bpm = estimate_bpm(&onset_strength, fps).unwrap_or(FALLBACK_BPM);
    let onsets = detect_onsets(&onset_strength, fps);

    Ok(AudioInfo {
        duration_secs,
        bpm,
        onsets,
    })
}

/// Onset-strength signal: the half-wave-rectified first difference of the
/// energy envelope (rises in energy = likely note attacks). Shared by both the
/// tempo estimator and the onset detector.
fn onset_strength(envelope: &[f32]) -> Vec<f32> {
    if envelope.is_empty() {
        return Vec::new();
    }
    let mut onset = Vec::with_capacity(envelope.len());
    onset.push(0.0f32);
    for i in 1..envelope.len() {
        onset.push((envelope[i] - envelope[i - 1]).max(0.0));
    }
    onset
}

/// Pick onset times (seconds) from the onset-strength signal via local-maximum
/// peak picking with an adaptive threshold and a minimum inter-onset gap.
fn detect_onsets(onset: &[f32], fps: f32) -> Vec<f64> {
    if onset.len() < 8 {
        return Vec::new();
    }
    let max = onset.iter().copied().fold(0.0f32, f32::max);
    if max <= 0.0 {
        return Vec::new();
    }

    let window = ((fps * 0.1).round() as usize).max(1); // ±100 ms
    let min_gap = ((fps * 0.12).round() as usize).max(1); // ≥120 ms between onsets
    let delta = 0.06f32; // how far above the local mean a peak must rise

    let mut onsets = Vec::new();
    let mut last_idx = 0usize;
    let mut have_last = false;

    for i in 1..onset.len() - 1 {
        let v = onset[i] / max;
        // Must be a strict-ish local maximum.
        if v <= onset[i - 1] / max || v < onset[i + 1] / max {
            continue;
        }
        let lo = i.saturating_sub(window);
        let hi = (i + window + 1).min(onset.len());
        let local_mean = onset[lo..hi].iter().sum::<f32>() / (hi - lo) as f32 / max;
        if v < local_mean + delta {
            continue;
        }
        if have_last && i - last_idx < min_gap {
            continue;
        }
        onsets.push(i as f64 / fps as f64);
        last_idx = i;
        have_last = true;
    }

    onsets
}

/// Tempo search range (BPM) and resolution.
const MIN_BPM: f32 = 50.0;
const MAX_BPM: f32 = 210.0;
const BPM_STEP: f32 = 0.5;
/// If half the chosen tempo still has at least this fraction of its spectral
/// magnitude, fold down to it — the slower one is the true fundamental.
const SUBHARMONIC_FOLD_RATIO: f32 = 0.5;

/// Estimate tempo from the onset-strength signal.
///
/// `onset` is the half-wave-rectified energy flux from [`onset_strength`];
/// `fps` is the envelope frame rate (`sample_rate / ENVELOPE_HOP`).
///
/// Method: scan candidate tempi (50–210 BPM) and score each by the onset
/// signal's spectral magnitude at that beat frequency (computed with the
/// Goertzel algorithm — one DFT bin per candidate, no FFT dependency),
/// weighted by a log-normal prior centered on [`TEMPO_PRIOR_CENTER_BPM`].
///
/// Working in the frequency domain avoids the half-tempo ("octave") error that
/// plagues autocorrelation: a beat at f Hz has spectral energy at f and its
/// harmonics (2f, 3f, …) but **not** at f/2, so the sub-octave is never a
/// competing peak. Returns `None` if the signal is too short or too flat.
fn estimate_bpm(onset: &[f32], fps: f32) -> Option<f32> {
    if onset.len() < 64 {
        return None;
    }

    // Remove the mean so DC / slow swells don't dominate.
    let mut signal = onset.to_vec();
    let mean = signal.iter().sum::<f32>() / signal.len() as f32;
    for v in &mut signal {
        *v -= mean;
    }

    let mut best_bpm = 0.0f32;
    let mut best_score = 0.0f32;
    let mut bpm = MIN_BPM;
    while bpm <= MAX_BPM {
        let magnitude = goertzel_magnitude(&signal, bpm / 60.0, fps);
        let score = magnitude * tempo_prior(bpm);
        if score > best_score {
            best_score = score;
            best_bpm = bpm;
        }
        bpm += BPM_STEP;
    }

    if best_score <= 0.0 || best_bpm <= 0.0 {
        return None;
    }

    // Fold down to the fundamental: if half the chosen tempo carries comparable
    // spectral energy, the peak we found was a harmonic of the real beat.
    loop {
        let half = best_bpm / 2.0;
        if half < MIN_BPM {
            break;
        }
        let mag_best = goertzel_magnitude(&signal, best_bpm / 60.0, fps);
        let mag_half = goertzel_magnitude(&signal, half / 60.0, fps);
        if mag_half >= SUBHARMONIC_FOLD_RATIO * mag_best {
            best_bpm = half;
        } else {
            break;
        }
    }

    Some(best_bpm.round())
}

/// Goertzel single-bin DFT magnitude (normalized) at `freq` Hz for a signal
/// sampled at `fps` frames per second.
fn goertzel_magnitude(signal: &[f32], freq: f32, fps: f32) -> f32 {
    let omega = std::f32::consts::TAU * freq / fps;
    let coeff = 2.0 * omega.cos();
    let mut s_prev = 0.0f32;
    let mut s_prev2 = 0.0f32;
    for &x in signal {
        let s = x + coeff * s_prev - s_prev2;
        s_prev2 = s_prev;
        s_prev = s;
    }
    let power = s_prev2 * s_prev2 + s_prev * s_prev - coeff * s_prev * s_prev2;
    power.max(0.0).sqrt() / signal.len() as f32
}

/// Log-normal weighting that favors tempi near [`TEMPO_PRIOR_CENTER_BPM`].
fn tempo_prior(bpm: f32) -> f32 {
    let z = (bpm / TEMPO_PRIOR_CENTER_BPM).log2() / TEMPO_PRIOR_STD_OCT;
    (-0.5 * z * z).exp()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fps_at(sample_rate: u32) -> f32 {
        sample_rate as f32 / ENVELOPE_HOP as f32
    }

    /// A pulse train spaced for `target_bpm`, used as a synthetic onset signal.
    fn pulse_train(fps: f32, target_bpm: f32, beats: usize) -> (Vec<f32>, usize) {
        let period = (fps * 60.0 / target_bpm).round() as usize;
        let mut signal = vec![0.0f32; period * beats];
        for i in (0..signal.len()).step_by(period) {
            signal[i] = 1.0;
        }
        (signal, period)
    }

    #[test]
    fn detects_a_periodic_pulse_train() {
        let fps = fps_at(44_100);
        let (signal, _) = pulse_train(fps, 120.0, 80);
        let bpm = estimate_bpm(&signal, fps).expect("pulse train should yield a tempo");
        assert!((bpm - 120.0).abs() <= 5.0, "expected ~120, got {bpm}");
    }

    #[test]
    fn resolves_octave_error_toward_plausible_tempo() {
        // A 150 BPM pulse train: plain ACF peaks just as strongly at 75 BPM
        // (every other pulse). The prior must keep us near 150, not 75.
        let fps = fps_at(44_100);
        let (signal, _) = pulse_train(fps, 150.0, 80);
        let bpm = estimate_bpm(&signal, fps).expect("should detect a tempo");
        assert!((bpm - 150.0).abs() <= 10.0, "expected ~150, got {bpm}");
    }

    #[test]
    fn folds_double_tempo_to_fundamental() {
        // An 80 BPM pulse train: its 2nd harmonic (160 BPM) sits closer to the
        // 120 BPM prior center, so without folding we'd report 160. The
        // sub-harmonic fold must pull us back to ~80.
        let fps = fps_at(44_100);
        let (signal, _) = pulse_train(fps, 80.0, 80);
        let bpm = estimate_bpm(&signal, fps).expect("should detect a tempo");
        assert!((bpm - 80.0).abs() <= 8.0, "expected ~80, got {bpm}");
    }

    #[test]
    fn tempo_prior_peaks_at_center() {
        assert!((tempo_prior(TEMPO_PRIOR_CENTER_BPM) - 1.0).abs() < 1e-6);
        // One octave up/down is symmetric and below the peak.
        assert!(tempo_prior(60.0) < 1.0);
        assert!((tempo_prior(60.0) - tempo_prior(240.0)).abs() < 1e-6);
    }

    #[test]
    fn flat_or_short_signal_returns_none() {
        let fps = fps_at(44_100);
        // Constant signal -> mean-removed to zero -> no tempo.
        assert!(estimate_bpm(&vec![0.5f32; 1000], fps).is_none());
        // Too short to analyze.
        assert!(estimate_bpm(&[0.1, 0.9, 0.2], fps).is_none());
    }

    #[test]
    fn onset_strength_is_half_wave_rectified() {
        let env = [0.0, 1.0, 0.5, 0.8];
        // diffs: +1.0, -0.5 -> 0, +0.3 ; first element is 0 by construction.
        let os = onset_strength(&env);
        assert_eq!(os.len(), env.len());
        assert!(os.iter().all(|&v| v >= 0.0));
        assert!((os[1] - 1.0).abs() < 1e-6);
        assert_eq!(os[2], 0.0);
    }

    #[test]
    fn detects_onsets_in_a_pulse_train() {
        let fps = fps_at(44_100);
        let (signal, _) = pulse_train(fps, 120.0, 40);
        let onsets = detect_onsets(&signal, fps);
        // ~39 interior pulses (the index-0 pulse is at the boundary).
        assert!(onsets.len() >= 37 && onsets.len() <= 40, "got {}", onsets.len());
        let gap = onsets[1] - onsets[0];
        assert!((gap - 0.5).abs() < 0.05, "expected ~0.5s spacing, got {gap}");
    }
}
