use std::{thread, time};
use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufReader, ErrorKind};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, mpsc};
use std::sync::mpsc::Sender;
use std::time::{Duration, SystemTime};

use discord_rich_presence::activity::{Activity, Assets, Button, Timestamps};
use discord_rich_presence::DiscordIpc;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::metadata::{find_metadata, Metadata};

const DISCORD_CLIENT_ID: &str = "927041178103332965";

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

#[derive(Debug)]
pub struct Song {
    // Song metadata.
    metadata: Metadata,
    // The time elapsed into the playback of this song.
    elapsed: Duration,
    // The timestamp when this song was last resumed.
    last_resume: SystemTime,
}

pub fn send(config: &Config, message: &Message) -> crate::Result<()> {
    let address = &socket_address(config);
    let conn = match TcpStream::connect(address) {
        Ok(conn) => conn,
        Err(error) => match error.kind() {
            ErrorKind::ConnectionRefused => {
                // Spawn daemon if not running.
                spawn_daemon()?;
                loop {
                    // Wait for daemon to start.
                    match TcpStream::connect(address) {
                        Ok(conn) => break conn,
                        Err(error) => match error.kind() {
                            ErrorKind::ConnectionRefused => (),
                            _ => return Err(error.into()),
                        }
                    }
                }
            }
            _ => return Err(error.into()),
        },
    };

    Ok(serde_json::to_writer(conn, message)?)
}

fn spawn_daemon() -> crate::Result<()> {
    Command::new(std::env::current_exe()?)
        .arg("daemon")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(())
}

pub fn daemon(config: &Config) -> crate::Result<()> {
    // Song state.
    let mut queue = empty_queue();
    let mut song: Option<Song> = None;

    // Auxiliary state.
    let (_stream, stream_handle) = OutputStream::try_default()?;
    let mut song_sink: Option<Arc<Sink>> = None;
    let mut discord = discord_client();

    let (tx, rx) = mpsc::channel::<Message>();
    socket_listener(config, tx.clone());
    for message in rx {
        println!("{:?}", message);
        match message {
            Message::Stop => break,
            Message::Pause => {
                if let Some(sink) = &song_sink {
                    match sink.is_paused() {
                        true => {
                            // Resume playback.
                            sink.play();
                            set_discord_presence(&mut discord, &song);
                            if let Some(song) = &mut song {
                                song.last_resume = SystemTime::now();
                            }
                        }
                        false => {
                            // Pause playback.
                            sink.pause();
                            discord.as_mut().map(|discord| {
                                // Clear presence.
                                let activity = Activity::new();
                                let _ = discord.set_activity(activity);
                            });

                            if let Some(song) = &mut song {
                                song.elapsed += SystemTime::now()
                                    .duration_since(song.last_resume)
                                    .unwrap();
                            }
                        }
                    }
                }
            }
            Message::Play { path, now } => {
                if now {
                    queue = empty_queue();
                    queue.push_back(path);
                    tx.send(Message::Skip)?;
                } else {
                    queue.push_back(path);
                }

                if song_sink.is_none() {
                    // Bootstrap initial song.
                    tx.send(Message::Next)?;
                }
            }
            Message::Skip => {
                if let Some(current_sink) = &song_sink {
                    // Immediately stop current song.
                    current_sink.stop();
                }
            }
            Message::Next => {
                if queue.len() > 1 || !config.loop_last {
                    // Remove the last played song from the queue.
                    queue.pop_front();
                }

                // Exit if queue is empty.
                let path = match queue.front() {
                    Some(path) => path,
                    None => break,
                };

                // Load audio file.
                let file = BufReader::new(File::open(&path)?);
                let source = Decoder::new(file)?;

                // Load audio sink.
                let sink = audio_sink(config, &stream_handle)?;
                sink.append(source);
                sink_finished_listener(tx.clone(), sink.clone());
                song_sink = Some(sink);

                // Load metadata.
                song = Some(Song {
                    metadata: find_metadata(path),
                    elapsed: Duration::ZERO,
                    last_resume: SystemTime::now(),
                });

                // Update presence.
                set_discord_presence(&mut discord, &song);
            }
        }
    }

    Ok(())
}

fn socket_address(config: &Config) -> SocketAddr {
    let string = format!("127.0.0.1:{}", config.port);
    string.parse().unwrap()
}

fn socket_listener(config: &Config, tx: Sender<Message>) {
    let address = socket_address(config);
    thread::spawn(move || {
        let listener = TcpListener::bind(address)?;
        println!("Ready!");

        for conn in listener.incoming() {
            let conn = conn?;
            let message: Message = serde_json::from_reader(conn)?;
            tx.send(message)?;
        }

        crate::Result::Ok(())
    });
}

fn discord_client() -> Option<Box<dyn DiscordIpc>> {
    let mut client = discord_rich_presence::new_client(DISCORD_CLIENT_ID).ok()?;
    client.connect().ok()?;
    Some(Box::new(client))
}

fn set_discord_presence(discord: &mut Option<Box<dyn DiscordIpc>>, song: &Option<Song>) {
    if let Some(song) = song {
        discord.as_mut().map(|discord| {
            let start = SystemTime::now() - song.elapsed;
            let start = start.duration_since(time::UNIX_EPOCH).unwrap();
            let mut activity = Activity::new()
                .details(song.metadata.artist.as_deref().unwrap_or("Unknown Artist"))
                .state(song.metadata.title.as_deref().unwrap_or("Unknown Title"))
                .timestamps(Timestamps::new().start(start.as_secs() as i64))
                .assets(Assets::new()
                    .large_image("icon")
                    .large_text("https://pmu.techno.fish/"));

            if let Some(origin) = &song.metadata.origin {
                let button = Button::new(&origin.name, &origin.link);
                activity = activity.buttons(vec![button]);
            }

            let _ = discord.set_activity(activity);
        });
    }
}

fn empty_queue() -> VecDeque<PathBuf> {
    // Queue always contains currently playing song.
    let mut queue = VecDeque::new();
    queue.push_back(PathBuf::new());
    queue
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
