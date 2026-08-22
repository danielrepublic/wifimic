use std::io;
use std::net::{SocketAddr, UdpSocket};

use wifimic_protocol::{decode_control, encode_control, ControlMessage};

pub(super) fn receive_control(socket: &UdpSocket) -> io::Result<(SocketAddr, ControlMessage)> {
    let mut packet = [0_u8; 64];
    let (received, source) = socket.recv_from(&mut packet)?;
    let message = decode_control(&packet[..received])
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
    Ok((source, message))
}

pub(super) struct LiveSessionGuard<'socket> {
    socket: &'socket UdpSocket,
    server: SocketAddr,
    session_id: u64,
    active: bool,
}

impl<'socket> LiveSessionGuard<'socket> {
    pub(super) fn new(socket: &'socket UdpSocket, server: SocketAddr, session_id: u64) -> Self {
        Self {
            socket,
            server,
            session_id,
            active: true,
        }
    }

    pub(super) fn stop(&mut self) -> io::Result<ControlMessage> {
        let packet = encode_control(&ControlMessage::Stop {
            session_id: self.session_id,
        });
        self.socket.send_to(&packet, self.server)?;
        let (_, message) = receive_control(self.socket)?;
        self.active = false;
        Ok(message)
    }
}

impl Drop for LiveSessionGuard<'_> {
    fn drop(&mut self) {
        if self.active {
            let packet = encode_control(&ControlMessage::Stop {
                session_id: self.session_id,
            });
            let _ = self.socket.send_to(&packet, self.server);
        }
    }
}
