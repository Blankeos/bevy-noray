use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::thread;
use std::time::Duration;

fn main() {
    let host = "127.0.0.1";
    let port = 8890;

    println!("=== Testing Noray Protocol ===\n");

    // Step 1: Host registers
    println!("[1] Host registering...");
    let host_stream = TcpStream::connect(&format!("{}:{}", host, port)).expect("Failed to connect");
    host_stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("Failed to set timeout");

    let mut host_stream = host_stream;
    host_stream
        .write_all(b"register-host\n")
        .expect("Failed to send");
    println!("   Sent: register-host");

    let mut host_oid = None;
    {
        let mut reader = BufReader::new(&host_stream);
        for i in 0..10 {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(n) if n > 0 => {
                    let line = line.trim();
                    println!("   Received: {}", line);
                    if line.starts_with("set-oid ") {
                        host_oid = Some(line[8..].to_string());
                    }
                    if host_oid.is_some() {
                        break;
                    }
                }
                Ok(_) => {
                    if i >= 9 {
                        panic!("Timeout waiting for host registration");
                    }
                }
                Err(e) => {
                    if i >= 9 {
                        panic!("Timeout: {:?}", e);
                    }
                    thread::sleep(Duration::from_millis(100));
                }
            }
        }
    }

    let host_oid = host_oid.expect("No host OID");
    println!("   Host registered: OID={}", host_oid);

    // Step 2: Joiner registers
    println!("\n[2] Joiner registering...");
    let joiner_stream =
        TcpStream::connect(&format!("{}:{}", host, port)).expect("Failed to connect");
    joiner_stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("Failed to set timeout");

    let mut joiner_stream = joiner_stream;
    joiner_stream
        .write_all(b"register-host\n")
        .expect("Failed to send");
    println!("   Sent: register-host");

    let mut joiner_oid = None;
    {
        let mut reader = BufReader::new(&joiner_stream);
        for i in 0..10 {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(n) if n > 0 => {
                    let line = line.trim();
                    println!("   Received: {}", line);
                    if line.starts_with("set-oid ") {
                        joiner_oid = Some(line[8..].to_string());
                    }
                    if joiner_oid.is_some() {
                        break;
                    }
                }
                Ok(_) => {
                    if i >= 9 {
                        panic!("Timeout waiting for joiner registration");
                    }
                }
                Err(e) => {
                    if i >= 9 {
                        panic!("Timeout: {:?}", e);
                    }
                    thread::sleep(Duration::from_millis(100));
                }
            }
        }
    }

    let joiner_oid = joiner_oid.expect("No joiner OID");
    println!("   Joiner registered: OID={}", joiner_oid);

    // Step 3: Joiner requests connect-relay
    println!(
        "\n[3] Joiner requesting connect-relay to host OID: {}",
        host_oid
    );
    let cmd = format!("connect-relay {}\n", host_oid);
    joiner_stream
        .write_all(cmd.as_bytes())
        .expect("Failed to send");
    println!("   Sent: connect-relay {}", host_oid);

    // Step 4: Joiner waits for response
    println!("\n[4] Joiner waiting for relay port...");
    joiner_stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("Failed to set timeout");

    let mut found_relay = false;
    {
        let mut reader = BufReader::new(&joiner_stream);
        for _i in 0..100 {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(n) if n > 0 => {
                    let line = line.trim();
                    println!("   Joiner received: '{}'", line);
                    if line.starts_with("connect-relay ") {
                        let port_str = &line[14..];
                        println!("   Joiner relay port: {}", port_str);
                        found_relay = true;
                        break;
                    } else if line.starts_with("ERROR") {
                        println!("   ERROR: {}", line);
                        break;
                    }
                }
                Ok(_) => {
                    thread::sleep(Duration::from_millis(100));
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(100));
                }
                Err(e) => {
                    println!("   Joiner read error: {:?}", e);
                    break;
                }
            }
        }
    }

    // Step 5: Host waits for relay info
    println!("\n[5] Host waiting for relay port...");
    host_stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("Failed to set timeout");

    {
        let mut reader = BufReader::new(&host_stream);
        for _i in 0..100 {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(n) if n > 0 => {
                    let line = line.trim();
                    println!("   Host received: '{}'", line);
                    if line.starts_with("connect-relay ") {
                        let port_str = &line[14..];
                        println!("   Host relay port: {}", port_str);
                        break;
                    } else if line.starts_with("ERROR") {
                        println!("   ERROR: {}", line);
                        break;
                    }
                }
                Ok(_) => {
                    thread::sleep(Duration::from_millis(100));
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(100));
                }
                Err(e) => {
                    println!("   Host read error: {:?}", e);
                    break;
                }
            }
        }
    }

    println!("\n=== Test Complete ===");
    if found_relay {
        println!("SUCCESS: Relay connection established!");
    } else {
        println!("FAILED: No relay port received");
    }
}
