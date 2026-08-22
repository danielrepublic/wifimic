pub mod capture;
pub mod control;
mod network;

use std::io;
use std::time::{Duration, Instant};

use wifimic_diagnostics::EventContext;

use crate::capture::CaptureHandle;
use crate::control::{ControlError, ControlPlane};

fn main() -> std::io::Result<()> {
    let mut socket = network::UdpServerSocket::bind()?;
    socket.set_read_timeout(Some(Duration::from_millis(100)))?;
    let diagnostics = EventContext::logging(Instant::now());
    let mut control = ControlPlane::new(CaptureHandle::new(), diagnostics);

    loop {
        match socket.receive_once() {
            Ok(Some(datagram)) => {
                match control.handle_datagram(&datagram.payload, Instant::now()) {
                    Ok(Some(ack)) => {
                        socket.send_to(&ack, datagram.source)?;
                    }
                    Ok(None) => {}
                    Err(ControlError::Protocol(_) | ControlError::UnexpectedAck { .. }) => {}
                    Err(error) => return Err(io::Error::other(error)),
                }
            }
            Ok(None) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                ) => {}
            Err(error) => return Err(error),
        }
        control.advance(Instant::now()).map_err(io::Error::other)?;
    }
}
