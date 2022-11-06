use crate::tcpstream::{create_tcpstream_connection, ConnectFuture};
use async_recursion::async_recursion;
use futures::future;
use macroquad::prelude::coroutines::wait_seconds;
use mio::net;
use std::{error::Error, io, net::ToSocketAddrs, sync::Mutex};
use tungstenite::{
    client::{client, IntoClientRequest},
    handshake::MidHandshake,
    ClientHandshake, HandshakeError, Message, WebSocket,
};

pub struct Connection {
    socket: Mutex<Option<WebSocket<net::TcpStream>>>,
}

impl Default for Connection {
    fn default() -> Self {
        Self::new()
    }
}

impl Connection {
    pub fn new() -> Self {
        Self {
            socket: Mutex::new(None),
        }
    }

    pub fn restart(&self) {
        let mut socket_lock = self.socket.lock().unwrap();
        *socket_lock = None;
    }

    #[async_recursion]
    pub async fn connect(&self, url: &str) -> Result<(), Box<dyn Error>> {
        let req = url.into_client_request()?;
        let uri = req.uri().clone();
        let host = uri.host().ok_or(tungstenite::Error::Url(
            tungstenite::error::UrlError::NoHostName,
        ))?;
        let port = uri.port_u16().unwrap_or(80);

        let addresses = (host, port).to_socket_addrs()?;
        let stream_futures = addresses
            .map(create_tcpstream_connection)
            .collect::<io::Result<Vec<ConnectFuture>>>()?;

        if let Err(err) = self.connect_internal(stream_futures, url).await {
            log::error!(
                "Failed to connect to {}, attempting again in 1 second: {}",
                url,
                err
            );
            wait_seconds(1.0).await;
            self.connect(url).await?
        }

        log::info!("Connection established successfully");

        Ok(())
    }

    async fn connect_internal(
        &self,
        connect_futures: Vec<ConnectFuture>,
        url: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let streams = future::join_all(connect_futures).await;
        let stream = streams.into_iter().find_map(|s| s.ok()).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("Failed to connect to {}", url),
            )
        })?;

        let socket = match client(url, stream) {
            Ok((socket, _)) => Ok(socket),
            Err(err) => {
                if let HandshakeError::Interrupted(mid_handshake) = err {
                    retry_handshake(mid_handshake)
                } else {
                    Err(err)
                }
            }
        }?;

        let mut socket_lock = self
            .socket
            .lock()
            .map_err(|err| io::Error::new(io::ErrorKind::Other, format!("{}", err)))?;

        *socket_lock = Some(socket);

        Ok(())
    }

    pub fn poll(&self) -> Option<Vec<u8>> {
        if let Ok(mut socket_lock) = self.socket.try_lock() {
            if let Some(socket) = socket_lock.as_mut() {
                if let Ok(Message::Binary(buf)) = socket.read_message() {
                    return Some(buf);
                }
            }
        }
        None
    }

    pub fn send(&self, msg: Vec<u8>) -> tungstenite::Result<()> {
        if let Ok(mut socket_lock) = self.socket.try_lock() {
            let socket = socket_lock.as_mut().ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotConnected, "No socket connection")
            })?;

            socket.write_message(Message::Binary(msg))?;
        }

        Ok(())
    }
}

pub fn retry_handshake(
    mid_handshake: MidHandshake<ClientHandshake<net::TcpStream>>,
) -> tungstenite::Result<WebSocket<net::TcpStream>, HandshakeError<ClientHandshake<net::TcpStream>>>
{
    match mid_handshake.handshake() {
        Ok((socket, _)) => Ok(socket),
        Err(err) => match err {
            HandshakeError::Interrupted(mid_handshake) => retry_handshake(mid_handshake),
            HandshakeError::Failure(_) => Err(err),
        },
    }
}
