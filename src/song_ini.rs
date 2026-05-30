use std::fmt::Write as _;

use crate::cli::Difficulty;
use crate::metadata::SongMetadata;

/// Render the `song.ini` contents.
///
/// `song_length` (milliseconds) is added on top of the requested fields because
/// Clone Hero uses it for the song-browser timer and end-of-song handling.
pub fn render(
    meta: &SongMetadata,
    name: &str,
    duration_secs: f64,
    difficulty: Difficulty,
) -> String {
    let year = meta.year.map(|y| y.to_string()).unwrap_or_default();
    let song_length_ms = (duration_secs * 1000.0).round().max(0.0) as u64;

    let mut ini = String::new();
    // `writeln!` into a String is infallible, so ignoring the result is safe
    // and keeps us free of `unwrap()`.
    let _ = writeln!(ini, "[song]");
    let _ = writeln!(ini, "name = {name}");
    let _ = writeln!(ini, "artist = {}", meta.artist.as_deref().unwrap_or(""));
    let _ = writeln!(ini, "charter = clonehero-maker");
    let _ = writeln!(ini, "album = {}", meta.album.as_deref().unwrap_or(""));
    let _ = writeln!(ini, "genre = {}", meta.genre.as_deref().unwrap_or(""));
    let _ = writeln!(ini, "year = {year}");
    let _ = writeln!(ini, "diff_guitar = {}", difficulty.rating());
    let _ = writeln!(ini, "preview_start_time = 0");
    let _ = writeln!(ini, "song_length = {song_length_ms}");
    ini
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_all_fields() {
        let meta = SongMetadata {
            display_name: "One".into(),
            artist: Some("Metallica".into()),
            album: Some("Reload".into()),
            genre: Some("Metal".into()),
            year: Some(1997),
            cover: None,
        };
        let ini = render(&meta, "One", 10.0, Difficulty::Hard);

        assert!(ini.starts_with("[song]\n"));
        assert!(ini.contains("name = One\n"));
        assert!(ini.contains("artist = Metallica\n"));
        assert!(ini.contains("charter = clonehero-maker\n"));
        assert!(ini.contains("album = Reload\n"));
        assert!(ini.contains("genre = Metal\n"));
        assert!(ini.contains("year = 1997\n"));
        assert!(ini.contains("diff_guitar = 4\n")); // Hard rating
        assert!(ini.contains("preview_start_time = 0\n"));
        assert!(ini.contains("song_length = 10000\n"));
    }

    #[test]
    fn missing_optionals_render_empty() {
        let meta = SongMetadata {
            display_name: "x".into(),
            ..Default::default()
        };
        let ini = render(&meta, "x", 0.0, Difficulty::Expert);

        assert!(ini.lines().any(|l| l == "artist = "));
        assert!(ini.lines().any(|l| l == "album = "));
        assert!(ini.lines().any(|l| l == "year = "));
        assert!(ini.contains("diff_guitar = 5\n")); // Expert rating
        assert!(ini.contains("song_length = 0\n"));
    }
}
