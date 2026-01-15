use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct NorayConfig {
    pub host: String,
    pub tcp_port: u16,
    pub udp_port: u16,
}

impl Default for NorayConfig {
    fn default() -> Self {
        Self {
            host: String::from("127.0.0.1"),
            tcp_port: 8890,
            udp_port: 8809,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RegistrationInfo {
    pub oid: String,
    pub pid: String,
}

pub fn register_only(config: &NorayConfig) -> Result<(RegistrationInfo, TcpStream), String> {
    let tcp_addr = format!("{}:{}", config.host, config.tcp_port);
    println!("[TCP] Connecting to {}", tcp_addr);
    let mut stream =
        TcpStream::connect(&tcp_addr).map_err(|e| format!("Failed to connect: {}", e))?;

    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| format!("Failed to set timeout: {}", e))?;

    println!("[TCP] Sending register-host");
    stream
        .write_all(b"register-host\n")
        .map_err(|e| format!("Failed to send: {}", e))?;

    let stream_clone = stream
        .try_clone()
        .map_err(|e| format!("Failed to clone stream: {}", e))?;

    let mut reader = BufReader::new(&stream);

    let mut oid = None;
    let mut pid = None;

    for i in 0..10 {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(n) if n > 0 => {
                let line = line.trim();
                println!("[TCP] Received: {}", line);

                if line.starts_with("set-oid ") {
                    oid = Some(line[8..].to_string());
                } else if line.starts_with("set-pid ") {
                    pid = Some(line[8..].to_string());
                }

                if oid.is_some() && pid.is_some() {
                    break;
                }
            }
            Ok(_) => {
                if i >= 9 {
                    return Err("Timeout waiting for registration".to_string());
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if i >= 9 {
                    return Err("Timeout waiting for registration".to_string());
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => {
                return Err(format!("Read error: {}", e));
            }
        }
    }

    match (oid, pid) {
        (Some(oid), Some(pid)) => Ok((RegistrationInfo { oid, pid }, stream_clone)),
        _ => Err("Failed to receive oid/pid".to_string()),
    }
}

pub fn wait_for_connection(mut stream: TcpStream, host: String) -> Result<(u16, String), String> {
    println!("[TCP] Waiting for noray response on existing connection...");

    stream
        .set_read_timeout(Some(Duration::from_secs(60)))
        .map_err(|e| format!("Failed to set timeout: {}", e))?;

    let mut reader = BufReader::new(stream);

    for _ in 0..600 {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(n) if n > 0 => {
                let line = line.trim();
                println!(
                    "[TCP] Received raw: '{}' (bytes: {:?})",
                    line,
                    line.as_bytes()
                );

                if line.starts_with("connect-relay") {
                    let port_str = line.trim_start_matches("connect-relay").trim();
                    println!(
                        "[DEBUG] Port string: '{}' (len={})",
                        port_str,
                        port_str.len()
                    );
                    match port_str.parse::<u16>() {
                        Ok(port) => return Ok((port, host)),
                        Err(_) => return Err(format!("Invalid port format: '{}'", port_str)),
                    }
                } else if line.starts_with("ERROR") {
                    return Err(format!("Server error: {}", line));
                }
            }
            Ok(_) => {
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => {
                return Err(format!("Read error: {}", e));
            }
        }
    }

    Err("Timeout waiting for response".to_string())
}

pub fn connect_to_relay(config: &NorayConfig, host_oid: &str) -> Result<(u16, String), String> {
    let tcp_addr = format!("{}:{}", config.host, config.tcp_port);
    println!("[TCP] Connecting to {}", tcp_addr);

    let mut stream =
        TcpStream::connect(&tcp_addr).map_err(|e| format!("Failed to connect: {}", e))?;

    stream
        .set_read_timeout(Some(Duration::from_secs(15)))
        .map_err(|e| format!("Failed to set timeout: {}", e))?;

    let cmd = format!("connect-relay {}\n", host_oid);
    println!("[TCP] Sending: {}", cmd.trim());
    stream
        .write_all(cmd.as_bytes())
        .map_err(|e| format!("Failed to send: {}", e))?;

    let mut reader = BufReader::new(&stream);

    println!("[TCP] Waiting for noray response...");

    for i in 0..150 {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(n) if n > 0 => {
                let line = line.trim();
                println!(
                    "[TCP] Received raw: '{}' (bytes: {:?})",
                    line,
                    line.as_bytes()
                );

                if line.starts_with("connect-relay") {
                    let port_str = line.trim_start_matches("connect-relay").trim();
                    println!(
                        "[DEBUG] Port string: '{}' (len={})",
                        port_str,
                        port_str.len()
                    );
                    match port_str.parse::<u16>() {
                        Ok(port) => return Ok((port, config.host.clone())),
                        Err(_) => return Err(format!("Invalid port format: '{}'", port_str)),
                    }
                } else if line.starts_with("ERROR") || line.contains("Unknown") {
                    return Err(format!("Server error: {}", line));
                }
            }
            Ok(_) => {
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => {
                return Err(format!("Read error: {}", e));
            }
        }
    }

    Err("Timeout waiting for response".to_string())
}

pub fn connect_to_relay_with_stream(
    mut stream: TcpStream,
    host_oid: &str,
) -> Result<(u16, String), String> {
    println!("[TCP] Using existing connection for connect-relay");

    stream
        .set_read_timeout(Some(Duration::from_secs(15)))
        .map_err(|e| format!("Failed to set timeout: {}", e))?;

    let cmd = format!("connect-relay {}\n", host_oid);
    println!("[TCP] Sending: {}", cmd.trim());
    stream
        .write_all(cmd.as_bytes())
        .map_err(|e| format!("Failed to send: {}", e))?;

    let mut reader = BufReader::new(&stream);

    println!("[TCP] Waiting for noray response...");

    for _i in 0..150 {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(n) if n > 0 => {
                let line = line.trim();
                println!(
                    "[TCP] Received raw: '{}' (bytes: {:?})",
                    line,
                    line.as_bytes()
                );

                if line.starts_with("connect-relay") {
                    let port_str = line.trim_start_matches("connect-relay").trim();
                    println!(
                        "[DEBUG] Port string: '{}' (len={})",
                        port_str,
                        port_str.len()
                    );
                    match port_str.parse::<u16>() {
                        Ok(port) => return Ok((port, "127.0.0.1".to_string())),
                        Err(_) => return Err(format!("Invalid port format: '{}'", port_str)),
                    }
                } else if line.starts_with("ERROR") || line.contains("Unknown") {
                    return Err(format!("Server error: {}", line));
                }
            }
            Ok(_) => {
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => {
                return Err(format!("Read error: {}", e));
            }
        }
    }

    Err("Timeout waiting for response".to_string())
}
