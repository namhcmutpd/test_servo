use pi_sender::WifiSender;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

fn main() {
    // IP address and port of your ESP32
    let esp32_ip = "172.20.10.2:8080";
    
    println!("Starting Pi UDP Sender...");
    println!("Target ESP32 address: {}", esp32_ip);

    let sender = match WifiSender::new(esp32_ip) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to create UDP socket: {}", e);
            return;
        }
    };

    // Shared state: mode is manual (true) or auto (false)
    let is_manual = Arc::new(AtomicBool::new(false));
    let is_manual_clone = Arc::clone(&is_manual);

    // Spawn a background thread to listen for mode notifications back from ESP32
    let socket = sender.clone_socket().expect("Failed to clone UDP socket for listening");
    thread::spawn(move || {
        let mut buf = [0u8; 1024];
        println!("Pi UDP Receiver thread running. Listening for mode updates from ESP32...");
        loop {
            match socket.recv_from(&mut buf) {
                Ok((amt, src)) => {
                    let msg = String::from_utf8_lossy(&buf[..amt]);
                    let clean_msg = msg.trim();
                    println!("Received message from ESP32 ({}): '{}'", src, clean_msg);
                    if clean_msg == "MODE:AUTO" {
                        is_manual_clone.store(false, Ordering::SeqCst);
                        println!(">>> Mode Switched to AUTO (1s interval)");
                    } else if clean_msg == "MODE:MANUAL" {
                        is_manual_clone.store(true, Ordering::SeqCst);
                        println!(">>> Mode Switched to MANUAL (5s interval)");
                    }
                }
                Err(e) => {
                    eprintln!("Error in background receiver thread: {:?}", e);
                    thread::sleep(Duration::from_millis(500));
                }
            }
        }
    });

    let mut flag_idx = 0;

    loop {
        // Send the current flag to ESP32
        let message = format!("FLAG:{}", flag_idx);
        let mode_str = if is_manual.load(Ordering::SeqCst) { "MANUAL" } else { "AUTO" };
        
        match sender.send_message(&message) {
            Ok(bytes) => {
                println!("[{}] Sent Flag {} ({} bytes) to ESP32", mode_str, flag_idx, bytes);
            }
            Err(e) => {
                eprintln!("[{}] Failed to send Flag {}: {}", mode_str, flag_idx, e);
            }
        }

        // Move to the next mosquito species (12 species total: index 0 to 11)
        flag_idx = (flag_idx + 1) % 12;

        // Sleep according to the current mode
        // We poll the mode state in small steps to react immediately to button presses
        let start_mode = is_manual.load(Ordering::SeqCst);
        let target_delay = if start_mode { 5.0 } else { 1.0 };
        
        let mut elapsed = 0.0;
        while elapsed < target_delay {
            thread::sleep(Duration::from_millis(50));
            elapsed += 0.05;
            
            // If mode switched mid-sleep, break early to adjust
            if is_manual.load(Ordering::SeqCst) != start_mode {
                break;
            }
        }
    }
}
