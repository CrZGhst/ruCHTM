use std::path::PathBuf;

use clap::{Parser, ValueEnum};

/// Turn any audio file into a ready-to-play Clone Hero song folder.
#[derive(Parser, Debug)]
#[command(name = "clonehero-maker", version, about, long_about = None)]
pub struct Cli {
    /// Input audio file (mp3, wav, flac, ogg).
    pub input: PathBuf,

    /// Directory the generated song folder is written into.
    #[arg(short, long, default_value = "Output")]
    pub output: PathBuf,

    /// Highest difficulty to chart. Every difficulty up to and including this
    /// one is generated, so the song is always playable in-game.
    #[arg(short, long, value_enum, default_value_t = Difficulty::Expert)]
    pub difficulty: Difficulty,

    /// Overwrite the song folder if it already exists.
    #[arg(short, long)]
    pub force: bool,
}

/// Standard Clone Hero / Guitar Hero guitar difficulties.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Difficulty {
    Easy,
    Medium,
    Hard,
    Expert,
}

impl Difficulty {
    /// Lowest MIDI note for this difficulty's 5-fret lane (green fret).
    /// Frets green..orange occupy `base..=base + 4`.
    pub fn base_note(self) -> u8 {
        match self {
            Difficulty::Easy => 60,
            Difficulty::Medium => 72,
            Difficulty::Hard => 84,
            Difficulty::Expert => 96,
        }
    }

    /// Ordering rank, used to select "every difficulty up to this one".
    pub fn rank(self) -> u8 {
        match self {
            Difficulty::Easy => 0,
            Difficulty::Medium => 1,
            Difficulty::Hard => 2,
            Difficulty::Expert => 3,
        }
    }

    /// How many notes to place per beat for this difficulty.
    pub fn notes_per_beat(self) -> f64 {
        match self {
            Difficulty::Easy => 0.5,
            Difficulty::Medium => 1.0,
            Difficulty::Hard => 2.0,
            Difficulty::Expert => 2.0,
        }
    }

    /// `diff_guitar` intensity rating shown in Clone Hero's song browser.
    pub fn rating(self) -> u8 {
        match self {
            Difficulty::Easy => 2,
            Difficulty::Medium => 3,
            Difficulty::Hard => 4,
            Difficulty::Expert => 5,
        }
    }

    /// All difficulties up to and including `self`, easy → hardest.
    pub fn up_to(self) -> Vec<Difficulty> {
        [
            Difficulty::Easy,
            Difficulty::Medium,
            Difficulty::Hard,
            Difficulty::Expert,
        ]
        .into_iter()
        .filter(|d| d.rank() <= self.rank())
        .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn up_to_includes_all_lower_difficulties() {
        assert_eq!(Difficulty::Easy.up_to(), vec![Difficulty::Easy]);
        assert_eq!(
            Difficulty::Hard.up_to(),
            vec![Difficulty::Easy, Difficulty::Medium, Difficulty::Hard]
        );
        assert_eq!(Difficulty::Expert.up_to().len(), 4);
    }

    #[test]
    fn base_notes_match_clone_hero_lanes() {
        assert_eq!(Difficulty::Easy.base_note(), 60);
        assert_eq!(Difficulty::Medium.base_note(), 72);
        assert_eq!(Difficulty::Hard.base_note(), 84);
        assert_eq!(Difficulty::Expert.base_note(), 96);
    }

    #[test]
    fn clap_args_parse() {
        use clap::Parser;
        let cli = Cli::parse_from(["clonehero-maker", "song.mp3"]);
        assert_eq!(cli.difficulty, Difficulty::Expert); // default
        let cli = Cli::parse_from(["clonehero-maker", "song.mp3", "-d", "easy", "-o", "out"]);
        assert_eq!(cli.difficulty, Difficulty::Easy);
        assert_eq!(cli.output.to_str(), Some("out"));
    }
}
