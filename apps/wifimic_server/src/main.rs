mod network;

fn main() -> std::io::Result<()> {
    let mut socket = network::UdpServerSocket::bind()?;
    loop {
        let _datagram = socket.receive_once()?;
    }
}
