use std::ffi::OsStr;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use regex::Regex;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::{StandardTagKey, Value};
use symphonia::core::probe::Hint;

#[derive(Debug, Default)]
pub struct Metadata {
    pub artist: Option<String>,
    pub title: Option<String>,
    pub album: Option<String>,
    pub origin: Option<Origin>,
}

#[derive(Debug)]
pub struct Origin {
    pub name: String,
    pub link: String,
}

pub fn find_metadata(path: &Path) -> Metadata {
    let candidates = [
        osu(path),
        stepmania(path),
        file_tags(path),
    ];

    for candidate in candidates {
        if let Some(metadata) = candidate {
            return metadata;
        }
    }

    // Default metadata.
    let title = path.file_stem().unwrap().to_string_lossy();
    let title = Some(title.to_string());
    Metadata { title, ..Metadata::default() }
}

fn find_extension(extension: &str, directory: &Path) -> Option<PathBuf> {
    let extension = OsStr::new(extension);
    for element in directory.read_dir().ok()? {
        let path = element.ok()?.path();
        if path.extension() == Some(extension) {
            return Some(path);
        }
    }

    None
}

fn read_file_string(path: &Path) -> Option<String> {
    let mut string = String::new();
    let mut file = File::open(path).ok()?;
    file.read_to_string(&mut string).ok()?;
    Some(string)
}

fn find_regex_match(pattern: &str, text: &str) -> Option<String> {
    let re = Regex::new(pattern).unwrap();
    let string = re.captures(text)?.get(1)?;
    let string = string.as_str().trim();
    match string.is_empty() {
        false => Some(string.to_string()),
        true => None,
    }
}

/// https://osu.ppy.sh/home
fn osu(path: &Path) -> Option<Metadata> {
    let directory = path.parent()?;
    let path = find_extension("osu", directory)?;
    let string = &read_file_string(&path)?;

    let parent = directory.file_stem().unwrap();
    let origin = Option::or(
        find_regex_match(r"BeatmapSetID:([^\n]+)", string),
        find_regex_match(r"(\d+)", &parent.to_string_lossy()),
    );

    let origin = origin.map(|origin| Origin {
        name: "osu! Beatmap".to_string(),
        link: format!("https://osu.ppy.sh/beatmapsets/{}", origin),
    });

    Some(Metadata {
        artist: find_regex_match(r"Artist:([^\n]+)", string),
        title: find_regex_match(r"Title:([^\n]+)", string),
        album: None,
        origin,
    })
}

/// https://www.stepmania.com
fn stepmania(path: &Path) -> Option<Metadata> {
    let directory = path.parent()?;
    let path = find_extension("sm", directory)?;
    let string = &read_file_string(&path)?;

    Some(Metadata {
        artist: find_regex_match(r"#ARTIST:([^;]+);", string),
        title: find_regex_match(r"#TITLE:([^;]+);", string),
        album: None,
        origin: None,
    })
}

/// Audio metadata tags.
fn file_tags(path: &Path) -> Option<Metadata> {
    let file = File::open(path).ok()?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    // Construct file hint.
    let mut hint = Hint::new();
    if let Some(extension) = path.extension() {
        if let Some(extension) = extension.to_str() {
            hint.with_extension(extension);
        }
    }

    // Get latest metadata revision.
    let probe = symphonia::default::get_probe();
    let result = probe.format(&hint, mss, &Default::default(), &Default::default()).ok()?;
    let metadata = result.metadata.current()?;

    // Search for tag.
    let find_tag = |key: StandardTagKey| {
        for tag in metadata.tags() {
            // FIXME: https://github.com/pdeljanov/Symphonia/pull/93
            if let Some(std_key) = tag.std_key {
                let std_key = std::mem::discriminant(&std_key);
                let key = std::mem::discriminant(&key);
                if std_key == key {
                    if let Value::String(tag) = &tag.value {
                        return Some(tag.to_string());
                    }
                }
            }
        }

        return None;
    };

    // Metadata must contain title.
    let title = find_tag(StandardTagKey::TrackTitle)?;
    let artist = find_tag(StandardTagKey::Artist);
    let album = find_tag(StandardTagKey::Album);

    // Construct metadata.
    Some(Metadata {
        artist,
        title: Some(title),
        album,
        origin: None,
    })
}
