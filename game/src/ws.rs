use std::{error::Error, io, net::TcpStream};

use tungstenite::{connect, stream::MaybeTlsStream, Message, WebSocket};

pub struct Connection {
    socket: Option<WebSocket<MaybeTlsStream<TcpStream>>>,
}

impl Connection {
    pub fn new() -> Self {
        Self { socket: None }
    }

    pub fn send(&mut self, msg: Vec<u8>) -> Result<(), Box<dyn Error>> {
        let socket = self
            .socket
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotConnected, "No socket connection"))?;
        socket.write_message(Message::Binary(msg))?;

        Ok(())
    }

    pub fn connect(&mut self, url: &str) {
        match connect(url) {
            Ok((mut socket, _)) => {
                if let MaybeTlsStream::Plain(s) = socket.get_mut() {
                    s.set_nonblocking(true).unwrap();
                }
                self.socket = Some(socket);
            }
            Err(err) => {
                log::error!("Failed to connect: {}, retrying...", err);
                self.connect(url);
            }
        }
    }

    pub fn poll(&mut self) -> Option<Vec<u8>> {
        if let Some(socket) = &mut self.socket {
            if let Ok(msg) = socket.read_message() {
                if let Message::Binary(buf) = msg {
                    return Some(buf);
                }
            }
        }
        None
    }
}
