use std::io::ErrorKind;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::process::{Command, Stdio};

use crate::{Config, Message};
use crate::daemon::daemon;

pub fn bootstrap(config: &Config) -> crate::Result<()> {
    let address = socket_address(config);
    let listener = TcpListener::bind(address)?;
    println!("Listening on: {}", address);

    for conn in listener.incoming() {
        let conn = conn?;
        let message: Message = serde_json::from_reader(conn)?;
        println!("{:?}", message);

        match message {
            Message::Stop => break,
            Message::Play { path, .. } => return daemon(config, listener, path),
            _ => (),
        }
    }

    Ok(())
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

fn socket_address(config: &Config) -> SocketAddr {
    let string = format!("127.0.0.1:{}", config.port);
    string.parse().unwrap()
}
