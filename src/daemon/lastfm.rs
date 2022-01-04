use rustfm_scrobble::{Scrobble, Scrobbler};

use crate::Config;
use crate::daemon::CurrentSong;

const API_KEY: &str = "45528d82eb7bd7afb15dcd9042bdd948";
const SHARED_SECRET: &str = "b694310aec977a220b87ffc219ada4eb";

pub struct Lastfm(Option<Scrobbler>);

pub fn lastfm_client(config: &Config) -> Lastfm {
    Lastfm((|| {
        let username = &config.lastfm_username;
        let password = &config.lastfm_password;
        if username.is_empty() || password.is_empty() {
            // Return if credentials are empty.
            return None;
        }

        // Connect to Last.fm.
        let mut lastfm = Scrobbler::new(API_KEY, SHARED_SECRET);
        lastfm.authenticate_with_password(username, password).ok()?;
        println!("Authenticated to Last.fm.");
        Some(lastfm)
    })())
}

pub fn lastfm_now_playing(Lastfm(lastfm): &Lastfm, song: &CurrentSong) {
    if let Some(lastfm) = lastfm {
        if let Some(scrobble) = create_scrobble(song) {
            let _ = lastfm.now_playing(&scrobble);
        }
    }
}

pub fn try_scrobble(config: &Config, Lastfm(lastfm): &Lastfm, song: &CurrentSong) {
    if let Some(lastfm) = lastfm {
        if song.elapsed().as_secs() >= config.lastfm_threshold_seconds {
            if let Some(scrobble) = create_scrobble(song) {
                let _ = lastfm.scrobble(&scrobble);
            }
        }
    }
}

fn create_scrobble(song: &CurrentSong) -> Option<Scrobble> {
    let artist = song.metadata.artist.as_ref()?;
    let title = song.metadata.title.as_ref()?;
    let album = match &song.metadata.album {
        Some(album) => album.as_str(),
        None => "",
    };

    Some(Scrobble::new(artist, title, album))
}
