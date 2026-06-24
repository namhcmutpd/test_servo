use std::net::UdpSocket;
use std::time::Duration;

pub struct WifiSender {
    socket: UdpSocket,
    target_addr: String,
}

impl WifiSender {
    /// Creates a new WifiSender bound to an arbitrary local port.
    /// `target_addr` should be in the format "IP:PORT", e.g., "192.168.1.100:1234".
    pub fn new(target_addr: &str) -> std::io::Result<Self> {
        // Bind to any available local port on all interfaces
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        
        // Optional: Set a timeout for socket operations
        socket.set_read_timeout(Some(Duration::from_secs(2)))?;
        socket.set_write_timeout(Some(Duration::from_secs(2)))?;

        Ok(Self {
            socket,
            target_addr: target_addr.to_string(),
        })
    }

    /// Sends a plain text message to the target address via UDP.
    pub fn send_message(&self, message: &str) -> std::io::Result<usize> {
        let bytes_sent = self.socket.send_to(message.as_bytes(), &self.target_addr)?;
        Ok(bytes_sent)
    }

    /// Clones the internal UdpSocket for sharing across threads.
    pub fn clone_socket(&self) -> std::io::Result<UdpSocket> {
        self.socket.try_clone()
    }
}
