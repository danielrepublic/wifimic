use std::io;
use std::net::{Ipv4Addr, SocketAddr, ToSocketAddrs, UdpSocket};
use std::num::ParseIntError;
use std::time::{Duration, SystemTime, SystemTimeError, UNIX_EPOCH};

use thiserror::Error;
use wifimic_protocol::{
    decode_control, encode_control, ControlMessage, HEARTBEAT_TAG, START_TAG, STOP_TAG,
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
        self.socket.set_read_timeout(Some(self.read_timeout))?;
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
        let mut packet = [0_u8; 64];
        let (received, source) = self.socket.recv_from(&mut packet)?;
        if source != self.peer {
            return Err(SmokeError::UnexpectedSource { peer: source });
        }
        match decode_control(&packet[..received])? {
            ControlMessage::Ack {
                session_id: actual_session_id,
                acked_kind: actual_kind,
            } if actual_session_id == session_id && actual_kind == expected_kind => Ok(()),
            ControlMessage::Ack {
                session_id: actual_session_id,
                acked_kind: actual_kind,
            } => Err(SmokeError::MismatchedAck {
                expected_session_id: session_id,
                expected_kind,
                actual_session_id,
                actual_kind,
            }),
            response => Err(SmokeError::UnexpectedResponse(response)),
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
mod tests {
    use std::io;
    use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
    use std::thread::{self, JoinHandle};
    use std::time::Duration;

    use wifimic_protocol::{decode_control, encode_control, ControlMessage};

    use super::{SmokeClient, SmokeError};

    #[derive(Debug, Clone, Copy)]
    enum ResponderMode {
        Matching,
        Missing,
        Mismatched,
    }

    fn spawn_responder(mode: ResponderMode) -> (SocketAddr, JoinHandle<()>) {
        let socket = UdpSocket::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0_u16)))
            .expect("test responder must bind");
        let address = socket
            .local_addr()
            .expect("test responder must have an address");
        let handle = thread::spawn(move || {
            let exchanges = match mode {
                ResponderMode::Matching => 3,
                ResponderMode::Missing | ResponderMode::Mismatched => 1,
            };
            for _ in 0..exchanges {
                let mut packet = [0_u8; 64];
                let (received, source) = socket
                    .recv_from(&mut packet)
                    .expect("test responder must receive a request");
                let request = decode_control(&packet[..received]).expect("request must decode");
                let (session_id, acked_kind) = match request {
                    ControlMessage::Start { session_id } => {
                        (session_id, wifimic_protocol::START_TAG)
                    }
                    ControlMessage::Heartbeat { session_id } => {
                        (session_id, wifimic_protocol::HEARTBEAT_TAG)
                    }
                    ControlMessage::Stop { session_id } => (session_id, wifimic_protocol::STOP_TAG),
                    ControlMessage::Ack { .. } => panic!("smoke client must not send Ack"),
                };
                let response = match mode {
                    ResponderMode::Matching => Some(ControlMessage::Ack {
                        session_id,
                        acked_kind,
                    }),
                    ResponderMode::Missing => None,
                    ResponderMode::Mismatched => Some(ControlMessage::Ack {
                        session_id: session_id.saturating_add(1),
                        acked_kind,
                    }),
                };
                if let Some(response) = response {
                    socket
                        .send_to(&encode_control(&response), source)
                        .expect("test responder must send an acknowledgement");
                }
            }
        });
        (address, handle)
    }

    fn run_test(mode: ResponderMode, session_id: u64) -> Result<(), SmokeError> {
        let (peer, responder) = spawn_responder(mode);
        let socket = UdpSocket::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0_u16)))
            .expect("smoke client must bind");
        let result = SmokeClient::new(&socket, peer, Duration::from_millis(20)).run(session_id);
        responder.join().expect("test responder must finish");
        result
    }

    #[test]
    fn smoke_succeeds_with_three_matching_acknowledgements() {
        // Given
        // When
        let result = run_test(ResponderMode::Matching, 41);

        // Then
        assert!(result.is_ok());
    }

    #[test]
    fn smoke_fails_when_acknowledgement_is_missing() {
        // Given
        // When
        let result = run_test(ResponderMode::Missing, 42);

        // Then
        assert!(matches!(
            result,
            Err(SmokeError::Transport(error))
                if matches!(
                    error.kind(),
                    io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                )
        ));
    }

    #[test]
    fn smoke_fails_when_acknowledgement_session_mismatches() {
        // Given
        // When
        let result = run_test(ResponderMode::Mismatched, 43);

        // Then
        assert!(matches!(result, Err(SmokeError::MismatchedAck { .. })));
    }
}
