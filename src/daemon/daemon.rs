use std::collections::VecDeque;
use std::fs::File;
use std::io::BufReader;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, mpsc};
use std::sync::mpsc::Sender;
use std::thread;
use std::time::{Duration, SystemTime};

use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::daemon::discord::{clear_presence, discord_client, set_discord_presence};
use crate::daemon::lastfm::{lastfm_client, lastfm_now_playing, try_scrobble};
use crate::metadata::{find_metadata, Metadata};

#[derive(Debug, Serialize, Deserialize)]
pub enum Message {
    Stop,
    Pause,
    Play {
        path: PathBuf,
        now: bool,
    },
    Skip,
    Next,
}

pub struct CurrentSong {
    // The path to the audio file.
    pub path: PathBuf,
    // The audio sink for this song.
    pub sink: Arc<Sink>,
    // Song metadata.
    pub metadata: Metadata,
    // The time elapsed into the playback of this
    // song when the player was last paused.
    last_elapsed: Duration,
    // The timestamp when this song was last resumed.
    last_resume: SystemTime,
}

impl CurrentSong {
    pub fn elapsed(&self) -> Duration {
        let delta = SystemTime::now().duration_since(self.last_resume);
        self.last_elapsed + delta.unwrap()
    }
}

pub fn daemon(config: &Config, listener: TcpListener, path: PathBuf) -> crate::Result<()> {
    // Play song immediately.
    let (_stream, stream_handle) = OutputStream::try_default()?;
    let mut song = play_song(config, &stream_handle, path)?;

    // Load deferred services.
    let queue = &mut VecDeque::new();
    let discord = &mut discord_client();
    let lastfm = &lastfm_client(config);
    let (tx, rx) = mpsc::channel::<Message>();
    socket_listener(listener, tx.clone());

    let register_song = &mut |song: &CurrentSong, discord: &mut _| {
        sink_finished_listener(tx.clone(), song.sink.clone());
        set_discord_presence(discord, song);
        lastfm_now_playing(lastfm, song);
    };

    register_song(&song, discord);
    for message in rx {
        println!("{:?}", message);
        match message {
            Message::Stop => break,
            Message::Pause => match song.sink.is_paused() {
                true => {
                    // Resume playback.
                    song.sink.play();
                    song.last_resume = SystemTime::now();
                    set_discord_presence(discord, &song);
                }
                false => {
                    // Pause playback.
                    song.sink.pause();
                    song.last_elapsed = song.elapsed();
                    clear_presence(discord);
                }
            },
            Message::Play { path, now } => match now {
                false => queue.push_back(path),
                true => {
                    queue.clear();
                    queue.push_back(path);
                    tx.send(Message::Skip)?;
                }
            },
            Message::Skip => {
                // Immediately stop current song.
                song.sink.stop();
            }
            Message::Next => {
                let path = match (queue.pop_front(), config.loop_last) {
                    (Some(path), _) => path,
                    (None, true) => song.path.clone(),
                    (None, false) => break,
                };

                // Play next song immediately.
                let next = play_song(config, &stream_handle, path)?;
                let previous = std::mem::replace(&mut song, next);
                register_song(&song, discord);

                // Scrobble previous song.
                try_scrobble(config, lastfm, &previous);
            }
        }
    }

    try_scrobble(config, lastfm, &song);
    Ok(())
}

fn play_song(
    config: &Config,
    stream_handle: &OutputStreamHandle,
    path: PathBuf,
) -> crate::Result<CurrentSong> {
    // Load audio file.
    let file = BufReader::new(File::open(&path)?);
    let source = Decoder::new(file)?;

    // Load audio sink.
    let sink = audio_sink(config, &stream_handle)?;
    sink.append(source);

    // Construct song.
    let metadata = find_metadata(&path);
    Ok(CurrentSong {
        path,
        sink,
        metadata,
        last_elapsed: Duration::ZERO,
        last_resume: SystemTime::now(),
    })
}

fn audio_sink(config: &Config, handle: &OutputStreamHandle) -> crate::Result<Arc<Sink>> {
    let sink = Arc::new(Sink::try_new(handle)?);
    sink.set_volume(config.volume);
    Ok(sink)
}

fn sink_finished_listener(tx: Sender<Message>, sink: Arc<Sink>) {
    thread::spawn(move || {
        sink.sleep_until_end();
        tx.send(Message::Next)?;
        crate::Result::Ok(())
    });
}

fn socket_listener(listener: TcpListener, tx: Sender<Message>) {
    thread::spawn(move || {
        for conn in listener.incoming() {
            let conn = conn?;
            let message: Message = serde_json::from_reader(conn)?;
            tx.send(message)?;
        }

        crate::Result::Ok(())
    });
}

