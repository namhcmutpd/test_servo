use esp32_receiver::WifiReceiver;
use esp_idf_hal::gpio::PinDriver;
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_hal::rmt::config::TransmitConfig;
use esp_idf_hal::rmt::{FixedLengthSignal, PinState, Pulse, TxRmtDriver};
use log::info;
use std::thread;
use std::time::Duration;

// 12 colors representing 12 mosquito species flags (0 to 11)
const COLORS: [(u8, u8, u8); 12] = [
    (255, 0, 0),     // 0: Red
    (0, 255, 0),     // 1: Green
    (0, 0, 255),     // 2: Blue
    (255, 255, 0),   // 3: Yellow
    (0, 255, 255),   // 4: Cyan
    (255, 0, 255),   // 5: Magenta
    (255, 128, 0),   // 6: Orange
    (128, 255, 0),   // 7: Lime/Chartreuse
    (0, 255, 128),   // 8: Spring Green
    (0, 128, 255),   // 9: Azure
    (128, 0, 255),   // 10: Violet/Purple
    (255, 0, 128),   // 11: Rose/Pink
];

/// Encodes a color as 24-bit G-R-B and sends it to the WS2812 LED using the RMT driver
fn set_color(tx: &mut TxRmtDriver, color: (u8, u8, u8)) -> anyhow::Result<()> {
    let r = color.0;
    let g = color.1;
    let b = color.2;

    // WS2812 expects G-R-B format: Green MSB first, then Red, then Blue
    let grb: u32 = ((g as u32) << 16) | ((r as u32) << 8) | (b as u32);

    let ticks_hz = tx.counter_clock()?;
    
    // Pulse durations according to WS2812 datasheet
    let t0h = Pulse::new_with_duration(ticks_hz, PinState::High, &Duration::from_nanos(350))?;
    let t0l = Pulse::new_with_duration(ticks_hz, PinState::Low, &Duration::from_nanos(800))?;
    let t1h = Pulse::new_with_duration(ticks_hz, PinState::High, &Duration::from_nanos(700))?;
    let t1l = Pulse::new_with_duration(ticks_hz, PinState::Low, &Duration::from_nanos(600))?;

    let mut signal = FixedLengthSignal::<24>::new();
    for i in 0..24 {
        let bit = (grb >> (23 - i)) & 1;
        if bit == 1 {
            signal.set(i, &(t1h, t1l))?;
        } else {
            signal.set(i, &(t0h, t0l))?;
        }
    }

    tx.write_items(&signal, true)?;
    Ok(())
}

fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    info!("Starting ESP32 UDP Receiver with RGB & Button control...");

    let ssid = "Loc";
    let password = "66666666";
    let port = 8080;

    let peripherals = Peripherals::take()?;
    let sys_loop = esp_idf_svc::eventloop::EspSystemEventLoop::take()?;
    let nvs = esp_idf_svc::nvs::EspDefaultNvsPartition::take()?;

    // Initialize RGB LED on GPIO 48
    let led = peripherals.pins.gpio48;
    let channel = peripherals.rmt.channel0;
    let config = TransmitConfig::new().clock_divider(1);
    let mut tx = TxRmtDriver::new(channel, led, &config)?;

    // Initialize BOOT button on GPIO 0
    let mut button = PinDriver::input(peripherals.pins.gpio0)?;
    button.set_pull(esp_idf_hal::gpio::Pull::Up)?;

    // Start wifi connection
    let wifi = esp_idf_svc::wifi::EspWifi::new(
        peripherals.modem,
        sys_loop.clone(),
        Some(nvs),
    )?;

    let receiver = WifiReceiver::new(wifi, ssid, password, port)?;
    info!("Listening for UDP packets on port {}...", port);

    // Initial state: turn off LED (black)
    let _ = set_color(&mut tx, (0, 0, 0));

    let mut mode_is_auto = true;
    let mut last_button_pressed = false;
    let mut pi_addr: Option<std::net::SocketAddr> = None;

    loop {
        // 1. Process incoming flag signals from Raspberry Pi
        if let Some((msg, src)) = receiver.receive_packet() {
            info!("Received message: '{}' from Pi at {}", msg, src);
            pi_addr = Some(src);

            let flag_str = if msg.starts_with("FLAG:") {
                &msg[5..]
            } else {
                &msg
            };

            if let Ok(flag_idx) = flag_str.trim().parse::<usize>() {
                if flag_idx < 12 {
                    let color = COLORS[flag_idx];
                    info!("Flag parsed: {}. Setting LED color to R:{}, G:{}, B:{}", flag_idx, color.0, color.1, color.2);
                    if let Err(e) = set_color(&mut tx, color) {
                        log::error!("Error setting color: {:?}", e);
                    }
                } else {
                    log::warn!("Parsed index out of range: {}", flag_idx);
                }
            }
        }

        // 2. Poll the BOOT button (GPIO 0)
        let current_pressed = button.is_low(); // BOOT is active low
        if current_pressed && !last_button_pressed {
            // Button pressed transition detected
            mode_is_auto = !mode_is_auto;
            info!("BOOT Button Pressed! Switched mode. Auto Mode: {}", mode_is_auto);

            // Send notification to Pi if we know its address
            if let Some(addr) = pi_addr {
                let notify_msg = if mode_is_auto { "MODE:AUTO" } else { "MODE:MANUAL" };
                match receiver.send_to(notify_msg.as_bytes(), addr) {
                    Ok(_) => info!("Sent mode notification '{}' to Pi at {}", notify_msg, addr),
                    Err(e) => log::error!("Failed to send notification to Pi: {:?}", e),
                }
            } else {
                info!("No Raspberry Pi address stored yet. Toggle mode locally.");
            }

            // Small delay for button debouncing
            thread::sleep(Duration::from_millis(200));
        }
        last_button_pressed = current_pressed;

        thread::sleep(Duration::from_millis(10));
    }
}
