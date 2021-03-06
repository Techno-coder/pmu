use std::fs;
use std::fs::File;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    /// The port to host the daemon on.
    pub port: u16,
    /// The volume of the played songs. Normal volume is `1.0`.
    pub volume: f32,
    /// Whether to loop the last song of the queue.
    pub loop_last: bool,
    // Last.fm username for scrobbling.
    pub lastfm_username: String,
    // Last.fm password.
    pub lastfm_password: String,
    // Duration before scrobbling a track to Last.fm.
    pub lastfm_threshold_seconds: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            port: 9999,
            volume: 0.2,
            loop_last: false,
            lastfm_username: "".into(),
            lastfm_password: "".into(),
            lastfm_threshold_seconds: 110,
        }
    }
}

pub fn directory() -> PathBuf {
    let dir = dirs::config_dir().unwrap();
    dir.join("pmu")
}

pub fn load() -> crate::Result<Config> {
    let directory = directory();
    let path = &directory.join("config.json");

    if !path.exists() {
        fs::create_dir_all(directory)?;
        let file = File::create(path)?;
        let default = Config::default();
        serde_json::to_writer_pretty(file, &default)?;
    }

    let file = File::open(path)?;
    Ok(serde_json::from_reader(file)?)
}
