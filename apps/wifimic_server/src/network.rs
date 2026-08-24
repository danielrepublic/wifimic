use std::io;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::time::Duration;

/// The fixed UDP port shared by the server's control and audio datagrams.
pub const SERVER_PORT: u16 = 6_902;
/// The configured Wi-Fi address of the Linux audio server.
pub const LINUX_SERVER_IP: Ipv4Addr = Ipv4Addr::new(192, 168, 0, 210);
const MAX_DATAGRAM_BYTES: usize = u16::MAX as usize;

/// Returns the exact local address that owns WiFiMic's outbound UDP source IP.
#[must_use]
pub const fn server_bind_address() -> SocketAddr {
    SocketAddr::V4(SocketAddrV4::new(LINUX_SERVER_IP, SERVER_PORT))
}

/// The one Windows peer permitted to send control and audio datagrams.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowsPeerIp(Ipv4Addr);

impl WindowsPeerIp {
    /// Returns the fixed Windows peer configured for this server.
    #[must_use]
    pub const fn configured() -> Self {
        Self(Ipv4Addr::new(192, 168, 0, 200))
    }

    /// Returns whether a source address has the exact configured IPv4 address.
    #[must_use]
    pub fn accepts(self, source: SocketAddr) -> bool {
        match source {
            SocketAddr::V4(address) => *address.ip() == self.0,
            SocketAddr::V6(_) => false,
        }
    }
}

/// A datagram that passed the server's source-IP boundary.
#[derive(Debug, PartialEq, Eq)]
pub struct ReceivedDatagram {
    /// The source address, including its untrusted UDP source port.
    pub source: SocketAddr,
    /// The control or audio bytes for later protocol processing.
    pub payload: Vec<u8>,
}

/// The Linux server's shared UDP control/audio socket.
#[derive(Debug)]
pub struct UdpServerSocket {
    socket: UdpSocket,
    approved_peer: WindowsPeerIp,
    receive_buffer: Vec<u8>,
}

impl UdpServerSocket {
    /// Binds the server socket on every IPv4 interface at UDP port 6902.
    ///
    /// Only datagrams from the fixed Windows peer `192.168.0.200` are exposed
    /// to later control/audio consumers. The source port is intentionally not
    /// part of the trust boundary.
    ///
    /// # Errors
    ///
    /// Returns the operating-system error if the socket cannot be bound.
    pub fn bind() -> io::Result<Self> {
        Self::bind_at(server_bind_address())
    }

    fn bind_at(bind_address: SocketAddr) -> io::Result<Self> {
        Ok(Self {
            socket: UdpSocket::bind(bind_address)?,
            approved_peer: WindowsPeerIp::configured(),
            receive_buffer: vec![0; MAX_DATAGRAM_BYTES],
        })
    }

    #[cfg(test)]
    fn bind_for_test() -> io::Result<Self> {
        Self::bind_at(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
    }

    /// Receives one datagram, returning no value when its source IP is not the
    /// configured Windows peer. Rejected control and audio bytes are discarded
    /// before any later consumer can inspect them.
    ///
    /// # Errors
    ///
    /// Returns the operating-system error from the UDP receive operation.
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

    /// Sets the bounded receive wait used to service control-plane timers.
    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.socket.set_read_timeout(timeout)
    }

    /// Sends one control response to the source port of the accepted command.
    pub fn send_to(&self, payload: &[u8], destination: SocketAddr) -> io::Result<usize> {
        self.socket.send_to(payload, destination)
    }
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, SocketAddr, UdpSocket};

    use super::{
        server_bind_address, UdpServerSocket, WindowsPeerIp, LINUX_SERVER_IP, SERVER_PORT,
    };

    const CONTROL_TAG: u8 = 0x01;
    const AUDIO_TAG: u8 = 0x00;

    #[test]
    fn network_binds_the_configured_linux_peer_address() {
        assert_eq!(
            server_bind_address(),
            SocketAddr::from((LINUX_SERVER_IP, SERVER_PORT))
        );
    }

    #[test]
    fn network_accepts_approved_peer() {
        // Given
        let peer = WindowsPeerIp::configured();
        let sources = [
            SocketAddr::from((Ipv4Addr::new(192, 168, 0, 200), 40_000)),
            SocketAddr::from((Ipv4Addr::new(192, 168, 0, 200), 40_001)),
        ];

        // When
        let mut accepted = sources.into_iter().map(|source| peer.accepts(source));

        // Then
        assert!(accepted.all(|is_accepted| is_accepted));
    }

    #[test]
    fn network_drops_datagram_from_unapproved_source() {
        // Given
        let mut server = UdpServerSocket::bind_for_test().expect("ephemeral server must bind");
        let sender = UdpSocket::bind("127.0.0.1:0").expect("ephemeral sender must bind");
        let destination = server
            .socket
            .local_addr()
            .expect("server address must be readable");

        // When
        sender
            .send_to(&[CONTROL_TAG, 0x01], destination)
            .expect("control datagram must be sent");
        sender
            .send_to(&[AUDIO_TAG, 0x01], destination)
            .expect("audio datagram must be sent");

        // Then
        assert_eq!(
            server.receive_once().expect("control receive must succeed"),
            None
        );
        assert_eq!(
            server.receive_once().expect("audio receive must succeed"),
            None
        );
    }

    #[test]
    fn network_rejects_ipv6_and_other_ipv4_sources() {
        // Given
        let peer = WindowsPeerIp::configured();

        // When
        let other_ipv4 = peer.accepts(SocketAddr::from((Ipv4Addr::new(192, 168, 0, 201), 40_000)));
        let ipv6 = peer.accepts(SocketAddr::from(([0, 0, 0, 0, 0, 0, 0, 1], 40_000)));

        // Then
        assert!(!other_ipv4);
        assert!(!ipv6);
    }
}
