use std::fs;

use anyhow::{Context, Result};

use crate::cli::Cli;
use crate::error::ChtmError;
use crate::{album_art, audio, metadata, midi, song_ini};

/// Input extensions we accept (lowercased).
const SUPPORTED: &[&str] = &["mp3", "wav", "flac", "ogg"];

/// Run the full pipeline: validate input → analyze → create folder → write
/// song.ogg, guitar.ogg, album.png, notes.mid and song.ini.
pub fn run(cli: Cli) -> Result<()> {
    let input = &cli.input;

    if !input.exists() {
        return Err(ChtmError::InputNotFound(input.clone()).into());
    }

    let ext = input
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .ok_or_else(|| ChtmError::NoExtension(input.clone()))?;
    if !SUPPORTED.contains(&ext.as_str()) {
        return Err(ChtmError::UnsupportedFormat(ext).into());
    }

    println!("→ Reading metadata …");
    let meta = metadata::extract(input);
    let name = meta.display_name.clone();
    let folder_name = sanitize_folder_name(&name);

    println!("→ Analyzing audio (tempo & length) …");
    let info = audio::analyze(input)
        .with_context(|| format!("analyzing {}", input.display()))?;
    println!(
        "   detected ≈{:.0} BPM, {:.1} s, {} onsets",
        info.bpm,
        info.duration_secs,
        info.onsets.len()
    );

    let song_dir = cli.output.join(&folder_name);
    if song_dir.exists() && !cli.force {
        return Err(ChtmError::OutputExists(song_dir).into());
    }
    fs::create_dir_all(&song_dir)
        .with_context(|| format!("creating output folder {}", song_dir.display()))?;

    let song_ogg = song_dir.join("song.ogg");
    println!("→ Converting audio → song.ogg …");
    audio::convert_to_ogg(input, &song_ogg)
        .with_context(|| format!("converting {} to Ogg/Vorbis", input.display()))?;

    let guitar_ogg = song_dir.join("guitar.ogg");
    println!("→ Writing guitar.ogg (copy of song.ogg) …");
    fs::copy(&song_ogg, &guitar_ogg)
        .with_context(|| format!("writing {}", guitar_ogg.display()))?;

    println!("→ Writing album.png …");
    album_art::write(meta.cover.as_deref(), &name, &song_dir.join("album.png"))
        .with_context(|| "writing album.png")?;

    println!("→ Writing notes.mid …");
    midi::write(
        &song_dir.join("notes.mid"),
        &name,
        info.bpm,
        info.duration_secs,
        &info.onsets,
        cli.difficulty,
    )
    .with_context(|| "writing notes.mid")?;

    println!("→ Writing song.ini …");
    let ini = song_ini::render(&meta, &name, info.duration_secs, cli.difficulty);
    fs::write(song_dir.join("song.ini"), ini).with_context(|| "writing song.ini")?;

    println!("\n✓ Done. Copy this folder into your Clone Hero songs directory:");
    println!("  {}", song_dir.display());
    Ok(())
}

/// Make a string safe to use as a folder name across platforms.
fn sanitize_folder_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            // Control characters would break some filesystems/UIs.
            c if c.is_control() => '_',
            c => c,
        })
        .collect();

    let trimmed = cleaned.trim().trim_matches('.').trim();
    if trimmed.is_empty() {
        "Untitled Song".to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_illegal_characters() {
        assert_eq!(sanitize_folder_name("AC/DC: Back?"), "AC_DC_ Back_");
        assert_eq!(sanitize_folder_name(r#"a<b>c|d"e"#), "a_b_c_d_e");
    }

    #[test]
    fn keeps_normal_names() {
        assert_eq!(sanitize_folder_name("Metallica - One"), "Metallica - One");
    }

    #[test]
    fn blank_or_dot_only_names_get_a_default() {
        assert_eq!(sanitize_folder_name("   ...   "), "Untitled Song");
        assert_eq!(sanitize_folder_name(""), "Untitled Song");
    }

    #[test]
    fn supported_extensions_are_lowercase() {
        // Guards against accidentally adding an uppercase entry that the
        // lowercased comparison in `run` would never match.
        assert!(SUPPORTED.iter().all(|e| e == &e.to_lowercase()));
    }
}
