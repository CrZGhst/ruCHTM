use std::path::Path;

use lofty::file::TaggedFileExt;
use lofty::prelude::{Accessor, ItemKey};
use lofty::tag::Tag;

/// Tag metadata + embedded cover extracted from an input audio file.
///
/// Everything is best-effort: a file with no tags still yields a usable
/// `display_name` derived from the filename.
#[derive(Debug, Default)]
pub struct SongMetadata {
    /// Name used for the folder and the `song.ini` `name` field.
    /// ID3/Vorbis title if present, otherwise the file stem.
    pub display_name: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub year: Option<u32>,
    /// Raw bytes of the first embedded cover picture, if any.
    pub cover: Option<Vec<u8>>,
}

/// Read metadata from `path`. Never fails: tag-reading problems just fall back
/// to filename-derived defaults.
pub fn extract(path: &Path) -> SongMetadata {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(str::to_owned)
        .unwrap_or_else(|| "Unknown Song".to_string());

    let mut meta = SongMetadata {
        display_name: stem.clone(),
        ..Default::default()
    };

    let Ok(tagged) = lofty::read_from_path(path) else {
        return meta;
    };

    let Some(tag) = tagged.primary_tag().or_else(|| tagged.first_tag()) else {
        return meta;
    };

    if let Some(title) = non_empty(tag.title()) {
        meta.display_name = title;
    }
    meta.artist = non_empty(tag.artist());
    meta.album = non_empty(tag.album());
    meta.genre = non_empty(tag.genre());
    meta.year = tag.year().or_else(|| parse_year(tag));
    meta.cover = tag.pictures().first().map(|p| p.data().to_vec());

    meta
}

/// Some formats only store the year inside a full recording date
/// (e.g. `2003-08-12`); pull the leading year out of that.
fn parse_year(tag: &Tag) -> Option<u32> {
    let raw = tag.get_string(&ItemKey::RecordingDate)?;
    raw.get(0..4)?.parse::<u32>().ok()
}

/// Turn an `Option<Cow<str>>` into an owned `String`, dropping blanks.
fn non_empty(value: Option<std::borrow::Cow<'_, str>>) -> Option<String> {
    value
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn falls_back_to_filename_stem_when_unreadable() {
        // No file exists at this path, so tag reading fails and we should fall
        // back to the filename stem with everything else empty.
        let meta = extract(Path::new("/nonexistent/Metallica - One.mp3"));
        assert_eq!(meta.display_name, "Metallica - One");
        assert!(meta.artist.is_none());
        assert!(meta.cover.is_none());
        assert!(meta.year.is_none());
    }
}
