use std::ffi::OsStr;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use regex::Regex;

#[derive(Debug, Default)]
pub struct Metadata {
    pub artist: Option<String>,
    pub title: Option<String>,
}

pub fn find_metadata(path: &Path) -> Metadata {
    let candidates = [
        osu(path),
        stepmania(path),
    ];

    for candidate in candidates {
        if let Some(metadata) = candidate {
            return metadata;
        }
    }

    // Default metadata.
    let song = path.file_stem().unwrap().to_string_lossy();
    let song = Some(song.to_string());
    Metadata { artist: None, title: song }
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
    Some(string.as_str().to_string())
}

/// https://osu.ppy.sh/home
fn osu(path: &Path) -> Option<Metadata> {
    let directory = path.parent()?;
    let path = find_extension("osu", directory)?;
    let string = &read_file_string(&path)?;

    Some(Metadata {
        artist: find_regex_match(r"Artist:([^\n]+)", string),
        title: find_regex_match(r"Title:([^\n]+)", string),
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
    })
}
