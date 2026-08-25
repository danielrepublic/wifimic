use std::io;
use std::net::{Ipv4Addr, SocketAddr, ToSocketAddrs, UdpSocket};
use std::num::ParseIntError;
use std::time::{Duration, Instant, SystemTime, SystemTimeError, UNIX_EPOCH};

use thiserror::Error;
use wifimic_protocol::{
    decode_control, encode_control, ControlMessage, AUDIO_PACKET_BYTES, HEARTBEAT_TAG, START_TAG,
    STOP_TAG,
};

const READ_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Error)]
enum SmokeError {
    #[error("usage: wifimic_control_smoke HOST PORT")]
    Usage,
    #[error("invalid port {value:?}: {source}")]
    InvalidPort {
        value: String,
        #[source]
        source: ParseIntError,
    },
    #[error("could not resolve {host}:{port}: {source}")]
    Resolve {
        host: String,
        port: u16,
        #[source]
        source: io::Error,
    },
    #[error("no address resolved for {host}:{port}")]
    NoResolvedAddress { host: String, port: u16 },
    #[error("system clock is before the Unix epoch: {0}")]
    Clock(#[from] SystemTimeError),
    #[error("system clock value does not fit in a session ID")]
    SessionIdOverflow,
    #[error(transparent)]
    Transport(#[from] io::Error),
    #[error(transparent)]
    Protocol(#[from] wifimic_protocol::ProtocolError),
    #[error("received UDP response from unexpected peer {peer}")]
    UnexpectedSource { peer: SocketAddr },
    #[error(
        "mismatched acknowledgement: expected session {expected_session_id} and tag {expected_kind:#04x}, got session {actual_session_id} and tag {actual_kind:#04x}"
    )]
    MismatchedAck {
        expected_session_id: u64,
        expected_kind: u8,
        actual_session_id: u64,
        actual_kind: u8,
    },
    #[error("unexpected control response: {0:?}")]
    UnexpectedResponse(ControlMessage),
}

#[derive(Debug)]
struct CliArgs {
    host: String,
    port: u16,
}

#[derive(Debug)]
struct SmokeClient<'a> {
    socket: &'a UdpSocket,
    peer: SocketAddr,
    read_timeout: Duration,
}

impl<'a> SmokeClient<'a> {
    fn new(socket: &'a UdpSocket, peer: SocketAddr, read_timeout: Duration) -> Self {
        Self {
            socket,
            peer,
            read_timeout,
        }
    }

    fn run(&self, session_id: u64) -> Result<(), SmokeError> {
        self.exchange(ControlMessage::Start { session_id }, START_TAG)?;
        self.exchange(ControlMessage::Heartbeat { session_id }, HEARTBEAT_TAG)?;
        self.exchange(ControlMessage::Stop { session_id }, STOP_TAG)
    }

    fn exchange(&self, request: ControlMessage, expected_kind: u8) -> Result<(), SmokeError> {
        let session_id = match request {
            ControlMessage::Start { session_id }
            | ControlMessage::Heartbeat { session_id }
            | ControlMessage::Stop { session_id }
            | ControlMessage::Ack { session_id, .. } => session_id,
        };
        self.socket.send_to(&encode_control(&request), self.peer)?;
        let deadline = Instant::now() + self.read_timeout;
        let mut packet = [0_u8; AUDIO_PACKET_BYTES];
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::Error::from(io::ErrorKind::TimedOut).into());
            }
            self.socket.set_read_timeout(Some(remaining))?;
            let (received, source) = self.socket.recv_from(&mut packet)?;
            if source != self.peer {
                return Err(SmokeError::UnexpectedSource { peer: source });
            }
            match decode_control(&packet[..received]) {
                Ok(ControlMessage::Ack {
                    session_id: actual_session_id,
                    acked_kind: actual_kind,
                }) if actual_session_id == session_id && actual_kind == expected_kind => {
                    return Ok(())
                }
                Ok(ControlMessage::Ack {
                    session_id: actual_session_id,
                    acked_kind: actual_kind,
                }) => {
                    return Err(SmokeError::MismatchedAck {
                        expected_session_id: session_id,
                        expected_kind,
                        actual_session_id,
                        actual_kind,
                    });
                }
                Ok(response) => return Err(SmokeError::UnexpectedResponse(response)),
                Err(_) => {}
            }
        }
    }
}

fn parse_cli(mut arguments: impl Iterator<Item = String>) -> Result<CliArgs, SmokeError> {
    let Some(_program) = arguments.next() else {
        return Err(SmokeError::Usage);
    };
    let Some(host) = arguments.next() else {
        return Err(SmokeError::Usage);
    };
    let Some(port_text) = arguments.next() else {
        return Err(SmokeError::Usage);
    };
    if arguments.next().is_some() {
        return Err(SmokeError::Usage);
    }
    let port = port_text
        .parse::<u16>()
        .map_err(|source| SmokeError::InvalidPort {
            value: port_text,
            source,
        })?;
    Ok(CliArgs { host, port })
}

fn resolve_peer(args: &CliArgs) -> Result<SocketAddr, SmokeError> {
    let mut addresses = (args.host.as_str(), args.port)
        .to_socket_addrs()
        .map_err(|source| SmokeError::Resolve {
            host: args.host.clone(),
            port: args.port,
            source,
        })?;
    addresses
        .next()
        .ok_or_else(|| SmokeError::NoResolvedAddress {
            host: args.host.clone(),
            port: args.port,
        })
}

fn current_session_id() -> Result<u64, SmokeError> {
    let elapsed = SystemTime::now().duration_since(UNIX_EPOCH)?;
    u64::try_from(elapsed.as_millis()).map_err(|_| SmokeError::SessionIdOverflow)
}

fn main() -> Result<(), SmokeError> {
    let args = parse_cli(std::env::args())?;
    let peer = resolve_peer(&args)?;
    let socket = UdpSocket::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0_u16)))?;
    SmokeClient::new(&socket, peer, READ_TIMEOUT).run(current_session_id()?)?;
    println!("wifimic-control-smoke: PASS");
    Ok(())
}

#[cfg(test)]
#[path = "../wifimic_control_smoke_tests.rs"]
mod tests;
