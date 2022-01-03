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
use crate::daemon::discord::{clear_presence, Discord, discord_client, set_discord_presence};
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
    // The time elapsed into the playback of this song.
    pub elapsed: Duration,
    // The timestamp when this song was last resumed.
    pub last_resume: SystemTime,
}

pub fn daemon(config: &Config, listener: TcpListener, path: PathBuf) -> crate::Result<()> {
    let (tx, rx) = mpsc::channel::<Message>();
    socket_listener(listener, tx.clone());

    let discord = &mut discord_client();
    let (_stream, stream_handle) = OutputStream::try_default()?;
    let load_song = &mut |discord: &mut Discord, path| {
        let song = play_song(config, &stream_handle, path)?;
        sink_finished_listener(tx.clone(), song.sink.clone());
        set_discord_presence(discord, &song);
        crate::Result::Ok(song)
    };

    let queue = &mut VecDeque::new();
    let mut song: CurrentSong = load_song(discord, path)?;
    for message in rx {
        println!("{:?}", message);
        match message {
            Message::Stop => break,
            Message::Pause => match song.sink.is_paused() {
                true => {
                    // Resume playback.
                    song.sink.play();
                    set_discord_presence(discord, &song);
                    song.last_resume = SystemTime::now();
                }
                false => {
                    // Pause playback.
                    song.sink.pause();
                    clear_presence(discord);
                    song.elapsed += SystemTime::now()
                        .duration_since(song.last_resume)
                        .unwrap();
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

                song = load_song(discord, path)?;
            }
        }
    }

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
        elapsed: Duration::ZERO,
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

