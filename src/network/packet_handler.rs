use bincode::{deserialize, serialize};
use crossbeam_channel;
use serde::{Deserialize, Serialize};
use std::net::UdpSocket;
use std::thread;
use std::time::Duration;

const OID_LENGTH: usize = 32;
const PACKET_SIZE: usize = 4 + OID_LENGTH + 4 + 4 + 4 + 4 + 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    pub oid: String,
    pub frame: u32,
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub is_jumping: bool,
}

pub struct GameStatePacket(pub GameState);

impl GameStatePacket {
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        serialize(&self.0).map_err(|e| format!("Failed to serialize: {}", e))
    }

    pub fn log_send(&self) {
        let g = &self.0;
        println!(
            "[SEND] oid={} frame={} pos=({:.1},{:.1}) vel=({:.1},{:.1}) jump={}",
            g.oid, g.frame, g.x, g.y, g.vx, g.vy, g.is_jumping as u8
        );
    }

    pub fn log_receive(&self) {
        let g = &self.0;
        println!(
            "[RECV] oid={} frame={} pos=({:.1},{:.1}) vel=({:.1},{:.1}) jump={}",
            g.oid, g.frame, g.x, g.y, g.vx, g.vy, g.is_jumping as u8
        );
    }
}

pub fn register_udp_socket(
    config: &crate::network::NorayConfig,
    pid: &str,
) -> Result<UdpSocket, String> {
    let socket =
        UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("Failed to bind UDP socket: {}", e))?;

    socket
        .set_read_timeout(Some(Duration::from_millis(100)))
        .map_err(|e| format!("Failed to set UDP timeout: {}", e))?;

    let udp_addr = format!("{}:{}", config.host, config.udp_port);
    println!("Registering UDP at {}...", udp_addr);

    socket
        .send_to(pid.as_bytes(), &udp_addr)
        .map_err(|e| format!("Failed to register UDP: {}", e))?;

    let mut buf = [0u8; 1024];
    match socket.recv_from(&mut buf) {
        Ok((len, _)) => {
            let response = String::from_utf8_lossy(&buf[..len]);
            println!("UDP registration response: {}", response);
        }
        Err(_) => {
            println!("No UDP registration response (continuing)...");
        }
    }

    Ok(socket)
}

pub fn start_udp_relay(
    socket: UdpSocket,
    relay_port: u16,
) -> crossbeam_channel::Receiver<GameState> {
    println!(
        "UDP relay listening for incoming packets on port {}",
        relay_port
    );

    let (tx, rx) = crossbeam_channel::bounded::<GameState>(100);

    thread::spawn(move || {
        let socket = socket;
        println!("UDP relay listening for incoming packets");

        let mut buf = [0u8; PACKET_SIZE];

        loop {
            match socket.recv_from(&mut buf) {
                Ok((len, _addr)) => {
                    if len == PACKET_SIZE {
                        match deserialize::<GameState>(&buf[..len]) {
                            Ok(state) => {
                                let packet = GameStatePacket(state.clone());
                                packet.log_receive();

                                if tx.send(state).is_err() {
                                    println!("Receiver disconnected, stopping UDP thread");
                                    break;
                                }
                            }
                            Err(e) => {
                                println!("Failed to deserialize packet: {}", e);
                            }
                        }
                    }
                }
                Err(_) => {}
            }
        }
    });

    rx
}

pub fn send_game_state(
    socket: &UdpSocket,
    relay_addr: &str,
    state: &GameState,
) -> Result<(), String> {
    let packet = GameStatePacket(state.clone());
    packet.log_send();

    let bytes = packet
        .to_bytes()
        .map_err(|e| format!("Failed to encode packet: {}", e))?;

    if bytes.len() != PACKET_SIZE {
        return Err(format!(
            "Invalid packet size: {} (expected {})",
            bytes.len(),
            PACKET_SIZE
        ));
    }

    socket
        .send_to(&bytes, relay_addr)
        .map_err(|e| format!("Failed to send packet: {}", e))?;

    Ok(())
}
