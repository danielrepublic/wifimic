use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};

use wifimic_protocol::DEFAULT_PORT;

/// The only Linux peer allowed to affect this client's control or audio state.
pub const APPROVED_SERVER_IP: Ipv4Addr = Ipv4Addr::new(192, 168, 0, 210);
const RECEIVE_BUFFER_BYTES: usize = u16::MAX as usize;

/// The transport seam used by the control state machine.
pub trait DatagramTransport {
    /// Sends bytes to the configured Linux peer.
    fn send_to_peer(&mut self, payload: &[u8]) -> io::Result<()>;

    /// Receives one datagram when the transport provides an inbound socket.
    fn receive_once(&mut self) -> io::Result<Option<ReceivedDatagram>> {
        Ok(None)
    }
}

/// A source address that passed the client's exact IPv4 boundary.
#[derive(Debug, PartialEq, Eq)]
pub struct ReceivedDatagram {
    /// The source address, including its untrusted UDP source port.
    pub source: SocketAddr,
    /// The datagram bytes for protocol processing.
    pub payload: Vec<u8>,
}

/// The fixed Linux peer address used by the Windows client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinuxPeerIp(Ipv4Addr);

impl LinuxPeerIp {
    /// Returns the one configured Linux peer.
    #[must_use]
    pub const fn configured() -> Self {
        Self(APPROVED_SERVER_IP)
    }

    /// Compares only the source IP; the source port is deliberately ignored.
    #[must_use]
    pub fn accepts(self, source: SocketAddr) -> bool {
        source.ip() == IpAddr::V4(self.0)
    }
}

/// The client's bidirectional UDP socket.
#[derive(Debug)]
pub struct UdpClientSocket {
    socket: UdpSocket,
    peer: SocketAddr,
    approved_peer: LinuxPeerIp,
    receive_buffer: Vec<u8>,
}

impl UdpClientSocket {
    /// Binds UDP 6902 on every local IPv4 interface.
    pub fn bind() -> io::Result<Self> {
        Self::bind_at(SocketAddr::from((Ipv4Addr::UNSPECIFIED, DEFAULT_PORT)))
    }

    /// Binds a socket at an explicit address for deterministic integration tests.
    pub fn bind_at(bind_address: SocketAddr) -> io::Result<Self> {
        Ok(Self {
            socket: UdpSocket::bind(bind_address)?,
            peer: SocketAddr::from((APPROVED_SERVER_IP, DEFAULT_PORT)),
            approved_peer: LinuxPeerIp::configured(),
            receive_buffer: vec![0; RECEIVE_BUFFER_BYTES],
        })
    }

    /// Receives one datagram after applying the exact source-IP allow-list.
    pub fn receive_once(&mut self) -> io::Result<Option<ReceivedDatagram>> {
        let (received, source) = self.socket.recv_from(&mut self.receive_buffer)?;
        if !self.approved_peer.accepts(source) {
            return Ok(None);
        }
        Ok(Some(ReceivedDatagram {
            source,
            payload: self.receive_buffer[..received].to_vec(),
        }))
    }

    /// Applies a bounded read timeout so the caller can service control timers.
    pub fn set_read_timeout(&self, timeout: Option<std::time::Duration>) -> io::Result<()> {
        self.socket.set_read_timeout(timeout)
    }

    /// Returns the local address selected by the operating system.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.socket.local_addr()
    }
}

impl DatagramTransport for UdpClientSocket {
    fn send_to_peer(&mut self, payload: &[u8]) -> io::Result<()> {
        self.socket.send_to(payload, self.peer).map(|_| ())
    }

    fn receive_once(&mut self) -> io::Result<Option<ReceivedDatagram>> {
        Self::receive_once(self)
    }
}
