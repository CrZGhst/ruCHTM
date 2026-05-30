use std::path::Path;

use midly::num::{u4, u7, u15, u24, u28};
use midly::{Format, Header, MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind};

use crate::cli::Difficulty;
use crate::error::ChtmError;

/// Ticks per quarter note. 480 is a common, widely-compatible resolution.
const TICKS_PER_BEAT: u16 = 480;
const NOTE_VELOCITY: u8 = 100;
const FRETS: u8 = 5; // green, red, yellow, blue, orange

/// Minimum number of detected onsets before we chart to the music. Below this
/// we fall back to an even beat grid so a song still gets a playable chart.
const MIN_ONSETS: usize = 8;

/// Write a valid `notes.mid`:
///   * Track 0 — tempo/sync map (track name, tempo, 4/4 time signature)
///   * Track 1 — `PART GUITAR` with notes for every difficulty up to `difficulty`
///
/// Notes are placed on detected `onsets` (transients) so the chart follows the
/// music; if too few onsets are available it falls back to an evenly-spaced
/// beat grid. Either way the result is enough for Clone Hero to load and play
/// the song without errors.
pub fn write(
    out: &Path,
    song_name: &str,
    bpm: f32,
    duration_secs: f64,
    onsets: &[f64],
    difficulty: Difficulty,
) -> Result<(), ChtmError> {
    build(song_name, bpm, duration_secs, onsets, difficulty)
        .save(out)
        .map_err(ChtmError::Io)
}

/// Assemble the in-memory MIDI structure. Split out from `write` so it can be
/// unit-tested without touching the filesystem.
fn build<'a>(
    song_name: &'a str,
    bpm: f32,
    duration_secs: f64,
    onsets: &[f64],
    difficulty: Difficulty,
) -> Smf<'a> {
    let bpm = if bpm.is_finite() && bpm > 20.0 {
        bpm as f64
    } else {
        120.0
    };

    let header = Header::new(Format::Parallel, Timing::Metrical(u15::new(TICKS_PER_BEAT)));
    let mut smf = Smf::new(header);
    smf.tracks.push(build_tempo_track(song_name, bpm));
    smf.tracks
        .push(build_guitar_track(bpm, duration_secs, onsets, difficulty));
    smf
}

fn build_tempo_track(song_name: &str, bpm: f64) -> Vec<TrackEvent<'_>> {
    let micros_per_beat = (60_000_000.0 / bpm).round() as u32;
    vec![
        meta(MetaMessage::TrackName(song_name.as_bytes())),
        meta(MetaMessage::Tempo(u24::new(micros_per_beat))),
        // 4/4: numerator 4, denominator 2^2, 24 MIDI clocks/click, 8 32nds/beat.
        meta(MetaMessage::TimeSignature(4, 2, 24, 8)),
        meta(MetaMessage::EndOfTrack),
    ]
}

fn build_guitar_track<'a>(
    bpm: f64,
    duration_secs: f64,
    onsets: &[f64],
    difficulty: Difficulty,
) -> Vec<TrackEvent<'a>> {
    // (absolute_tick, is_note_on, key)
    let mut events: Vec<(u32, bool, u8)> = Vec::new();
    let mut rng = Lcg::seed(bpm.to_bits() ^ 0xC0FF_EE00);

    let use_onsets = onsets.len() >= MIN_ONSETS;

    for diff in difficulty.up_to() {
        let base = diff.base_note();
        if use_onsets {
            place_onset_notes(&mut events, &mut rng, base, bpm, onsets, diff);
        } else {
            place_grid_notes(&mut events, &mut rng, base, bpm, duration_secs, diff);
        }
    }

    // Sort by time; at equal ticks emit note-offs (false) before note-ons (true).
    events.sort_by_key(|&(tick, is_on, _)| (tick, is_on));

    let mut track: Vec<TrackEvent<'a>> = Vec::with_capacity(events.len() + 2);
    track.push(meta(MetaMessage::TrackName(b"PART GUITAR")));

    let mut last_tick = 0u32;
    for (tick, is_on, key) in events {
        let delta = tick - last_tick;
        last_tick = tick;
        let message = if is_on {
            MidiMessage::NoteOn {
                key: u7::new(key),
                vel: u7::new(NOTE_VELOCITY),
            }
        } else {
            MidiMessage::NoteOff {
                key: u7::new(key),
                vel: u7::new(0),
            }
        };
        track.push(TrackEvent {
            delta: u28::new(delta),
            kind: TrackEventKind::Midi {
                channel: u4::new(0),
                message,
            },
        });
    }

    track.push(meta(MetaMessage::EndOfTrack));
    track
}

/// Append a single note (NoteOn + NoteOff) with a deterministically-chosen fret.
fn push_note(events: &mut Vec<(u32, bool, u8)>, rng: &mut Lcg, base: u8, tick: u32, len: u32) {
    let fret = (rng.next() % FRETS as u32) as u8;
    let key = base + fret;
    events.push((tick, true, key));
    events.push((tick + len.max(1), false, key));
}

/// Place notes on detected onsets (converted to ticks via the tempo), thinned
/// per difficulty so lower difficulties are sparser. Note lengths are clamped
/// to half the gap to the next note so notes never overlap.
fn place_onset_notes(
    events: &mut Vec<(u32, bool, u8)>,
    rng: &mut Lcg,
    base: u8,
    bpm: f64,
    onsets: &[f64],
    diff: Difficulty,
) {
    let ticks_per_second = bpm * TICKS_PER_BEAT as f64 / 60.0;
    let ticks: Vec<u32> = onsets
        .iter()
        .enumerate()
        .filter(|(i, _)| keep_for_difficulty(*i, diff))
        .map(|(_, &t)| (t * ticks_per_second).round() as u32)
        .collect();

    let max_len = (TICKS_PER_BEAT / 2) as u32;
    let default_len = (TICKS_PER_BEAT / 4) as u32;
    for i in 0..ticks.len() {
        let len = if i + 1 < ticks.len() {
            (ticks[i + 1].saturating_sub(ticks[i]) / 2).clamp(1, max_len)
        } else {
            default_len
        };
        push_note(events, rng, base, ticks[i], len);
    }
}

/// Thin the onset list per difficulty: Expert keeps everything, lower
/// difficulties keep progressively fewer notes.
fn keep_for_difficulty(index: usize, diff: Difficulty) -> bool {
    match diff {
        Difficulty::Expert => true,
        Difficulty::Hard => index % 4 != 3,
        Difficulty::Medium => index.is_multiple_of(2),
        Difficulty::Easy => index.is_multiple_of(4),
    }
}

/// Fallback when onset detection found too little: evenly-spaced notes over a
/// beat grid, with density per difficulty.
fn place_grid_notes(
    events: &mut Vec<(u32, bool, u8)>,
    rng: &mut Lcg,
    base: u8,
    bpm: f64,
    duration_secs: f64,
    diff: Difficulty,
) {
    let total_beats = (duration_secs * bpm / 60.0).max(4.0);
    let step_beats = 1.0 / diff.notes_per_beat();
    let note_len = ((step_beats * 0.5) * TICKS_PER_BEAT as f64).round().max(1.0) as u32;

    let mut beat = 0.0f64;
    while beat < total_beats {
        let tick = (beat * TICKS_PER_BEAT as f64).round() as u32;
        push_note(events, rng, base, tick, note_len);
        beat += step_beats;
    }
}

fn meta(message: MetaMessage<'_>) -> TrackEvent<'_> {
    TrackEvent {
        delta: u28::new(0),
        kind: TrackEventKind::Meta(message),
    }
}

/// Tiny deterministic LCG so the fret pattern is varied but reproducible.
struct Lcg(u64);

impl Lcg {
    fn seed(seed: u64) -> Self {
        Lcg(seed | 1)
    }

    fn next(&mut self) -> u32 {
        // Numerical Recipes constants.
        self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::ops::RangeInclusive;

    /// Build a chart, round-trip it through midly's parser, and assert it is a
    /// valid SMF with balanced notes inside the expected lane range.
    fn parse_and_check(difficulty: Difficulty, expected_keys: RangeInclusive<u8>) {
        // Empty onsets -> grid fallback path.
        let smf = build("Test Song", 120.0, 12.0, &[], difficulty);

        // Serialize via the same `save` path used in production, then read back.
        let path = std::env::temp_dir().join(format!("chtm_midi_test_{difficulty:?}.mid"));
        smf.save(&path).expect("save");
        let bytes = std::fs::read(&path).expect("read");
        let _ = std::fs::remove_file(&path);

        let parsed = Smf::parse(&bytes).expect("notes.mid must be a valid SMF");
        assert_eq!(parsed.tracks.len(), 2, "tempo track + PART GUITAR");

        let mut note_on = 0i32;
        let mut note_off = 0i32;
        let mut keys = BTreeSet::new();
        let mut guitar_name: Option<Vec<u8>> = None;

        for event in &parsed.tracks[1] {
            match event.kind {
                TrackEventKind::Meta(MetaMessage::TrackName(name)) => {
                    guitar_name = Some(name.to_vec());
                }
                TrackEventKind::Midi {
                    message: MidiMessage::NoteOn { key, vel },
                    ..
                } => {
                    if vel.as_int() > 0 {
                        note_on += 1;
                        keys.insert(key.as_int());
                    } else {
                        note_off += 1;
                    }
                }
                TrackEventKind::Midi {
                    message: MidiMessage::NoteOff { .. },
                    ..
                } => note_off += 1,
                _ => {}
            }
        }

        assert_eq!(guitar_name.as_deref(), Some(&b"PART GUITAR"[..]));
        assert!(note_on > 0, "chart must contain notes");
        assert_eq!(note_on, note_off, "every NoteOn needs a matching NoteOff");
        for key in &keys {
            assert!(
                expected_keys.contains(key),
                "key {key} outside expected lane {expected_keys:?}"
            );
        }
    }

    #[test]
    fn expert_chart_uses_all_lanes_and_is_balanced() {
        parse_and_check(Difficulty::Expert, 60..=100);
    }

    #[test]
    fn easy_chart_only_uses_easy_lane() {
        parse_and_check(Difficulty::Easy, 60..=64);
    }

    #[test]
    fn expert_charts_one_note_per_onset() {
        // 16 onsets, 0.5 s apart (>= MIN_ONSETS) -> onset path, Expert keeps all.
        let onsets: Vec<f64> = (0..16).map(|i| 0.5 * i as f64).collect();
        let smf = build("Onset Song", 120.0, 8.0, &onsets, Difficulty::Expert);

        // Count only Expert-lane (96..=100) note-ons; the chart also contains
        // the thinned Easy/Medium/Hard lanes.
        let expert_note_ons = smf.tracks[1]
            .iter()
            .filter(|e| {
                matches!(
                    e.kind,
                    TrackEventKind::Midi {
                        message: MidiMessage::NoteOn { key, vel },
                        ..
                    } if vel.as_int() > 0 && (96..=100).contains(&key.as_int())
                )
            })
            .count();
        assert_eq!(
            expert_note_ons,
            onsets.len(),
            "Expert lane should chart one note per onset"
        );
    }

    #[test]
    fn tempo_track_has_tempo_and_time_signature() {
        let smf = build("X", 150.0, 5.0, &[], Difficulty::Medium);
        let tempo = &smf.tracks[0];
        assert!(tempo.iter().any(|e| matches!(
            e.kind,
            TrackEventKind::Meta(MetaMessage::Tempo(_))
        )));
        assert!(tempo.iter().any(|e| matches!(
            e.kind,
            TrackEventKind::Meta(MetaMessage::TimeSignature(..))
        )));
    }

    #[test]
    fn invalid_bpm_falls_back_without_panicking() {
        // 0 / NaN BPM must not produce an out-of-range tempo or crash.
        let smf = build("X", f32::NAN, 5.0, &[], Difficulty::Hard);
        assert_eq!(smf.tracks.len(), 2);
    }
}
