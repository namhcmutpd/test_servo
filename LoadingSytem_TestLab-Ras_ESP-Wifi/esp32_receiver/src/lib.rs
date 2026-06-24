use esp_idf_svc::wifi::{AuthMethod, ClientConfiguration, Configuration, EspWifi};
use log::info;
use std::net::UdpSocket;
use std::time::Duration;

pub struct WifiReceiver<'a> {
    _wifi: EspWifi<'a>,
    socket: UdpSocket,
}

impl<'a> WifiReceiver<'a> {
    /// Connect to Wi-Fi and bind a UDP socket on the specified port
    pub fn new(
        mut wifi: EspWifi<'a>,
        ssid: &str,
        password: &str,
        port: u16,
    ) -> anyhow::Result<Self> {

        wifi.set_configuration(&Configuration::Client(ClientConfiguration {
            ssid: ssid.try_into().unwrap(),
            password: password.try_into().unwrap(),
            auth_method: AuthMethod::WPA2Personal,
            ..Default::default()
        }))?;

        info!("Starting wifi...");
        wifi.start()?;

        info!("Connecting wifi...");
        wifi.connect()?;

        info!("Waiting for IP address...");
        while !wifi.is_connected()? || !wifi.sta_netif().is_up()? {
            std::thread::sleep(Duration::from_millis(500));
        }

        let ip_info = wifi.sta_netif().get_ip_info()?;
        info!("Wifi Connected! IP Address: {:?}", ip_info.ip);

        // Bind UDP Socket
        let bind_addr = format!("0.0.0.0:{}", port);
        let socket = UdpSocket::bind(&bind_addr)?;
        info!("UDP Socket bound to {}", bind_addr);

        // Optional: Set a read timeout
        socket.set_read_timeout(Some(Duration::from_millis(100)))?;

        Ok(Self {
            _wifi: wifi,
            socket,
        })
    }

    /// Receives a packet and returns the string content and sender address
    pub fn receive_packet(&self) -> Option<(String, std::net::SocketAddr)> {
        let mut buf = [0u8; 1024];
        match self.socket.recv_from(&mut buf) {
            Ok((amt, src)) => {
                let msg = String::from_utf8_lossy(&buf[..amt]).into_owned();
                Some((msg, src))
            }
            Err(e) => {
                // Ignore would block (timeout) errors
                if e.kind() != std::io::ErrorKind::WouldBlock {
                    log::error!("UDP receive error: {:?}", e);
                }
                None
            }
        }
    }

    /// Sends a packet to the specified destination address
    pub fn send_to(&self, data: &[u8], dest: std::net::SocketAddr) -> std::io::Result<usize> {
        self.socket.send_to(data, dest)
    }
}
