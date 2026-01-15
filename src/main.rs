mod game;
mod local_player_data;
mod network;
mod sync;

use bevy::prelude::*;
use crossbeam_channel;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread;
use std::time::Duration;

use game::player::{IsJumping, Velocity, spawn_player};
use game::{
    apply_physics, apply_velocity, handle_jump_events, handle_jump_input, handle_local_input,
};
use local_player_data::LocalPlayerMarker;
use network::{
    GameState, NorayConfig, noray_client::wait_for_connection, register_only, register_udp_socket,
    send_game_state, start_udp_relay,
};
use sync::{
    RemotePlayerData, RemoteUpdateReceiver, receive_remote_updates, update_remote_player_transforms,
};

#[derive(Resource)]
struct FrameCounter(Arc<AtomicU32>);

#[derive(Resource)]
struct SyncChannel(crossbeam_channel::Sender<GameState>);

#[derive(Resource)]
struct SyncReceiver(crossbeam_channel::Receiver<GameState>);

#[derive(Resource, Clone)]
pub struct PlayerRegistrationInfo {
    pub oid: String,
    pub pid: String,
}

#[derive(Resource, Default)]
struct NetworkingState {
    connected: bool,
    error_message: String,
}

fn setup_game(mut commands: Commands) {
    commands.spawn(Camera2dBundle::default());
}

fn spawn_local_player(
    mut commands: Commands,
    registration: Res<PlayerRegistrationInfo>,
    local_players: Query<(), (With<game::player::Player>, With<LocalPlayerMarker>)>,
) {
    if local_players.is_empty() {
        let local_player = spawn_player(
            &mut commands,
            registration.oid.clone(),
            true,
            Vec3::new(0.0, 100.0, 0.0),
            Color::srgb(0.0, 0.0, 1.0),
        );
        commands.entity(local_player).insert(LocalPlayerMarker);
    }
}

fn main() {
    let config = NorayConfig::default();

    println!("=== Bevy + Noray Multiplayer (Localhost) ===\n");
    println!("1. Host a game");
    println!("2. Join a game");
    println!();

    let mut choice = String::new();
    std::io::stdin()
        .read_line(&mut choice)
        .expect("Failed to read input");
    let choice = choice.trim();

    let (result_tx, result_rx) = crossbeam_channel::bounded(1);
    let mut player_oid = String::new();
    let mut player_pid = String::new();
    let mut udp_socket = None;
    let mut relay_info = None;

    if choice == "1" {
        println!("\n[1/3] Registering with noray...");
        let (reg, stream) = match register_only(&config) {
            Ok(result) => result,
            Err(e) => {
                eprintln!("[ERROR] Registration failed: {}", e);
                std::process::exit(1);
            }
        };
        player_oid = reg.oid.clone();
        player_pid = reg.pid.clone();
        println!("[OK] Your OpenID: {}", player_oid);

        println!("\n[2/3] Registering UDP (required before relay)...");
        let udp_for_relay = match register_udp_socket(&config, &player_pid) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[ERROR] UDP registration failed: {}", e);
                std::process::exit(1);
            }
        };
        println!("[OK] UDP registered");
        udp_socket = Some(udp_for_relay);

        println!("\n[3/3] Waiting for peer...");
        println!("Your OID: {}", player_oid);
        println!("\nTell them your OID and keep this terminal open!");
        println!("[INFO] Waiting for joiner to connect...\n");

        let host = config.host.clone();
        thread::spawn(move || {
            let result = wait_for_connection(stream, host);
            match result {
                Ok((port, host)) => {
                    let _ = result_tx.send(Ok((port, host)));
                }
                Err(e) => {
                    let _ = result_tx.send(Err(e));
                }
            }
        });
    } else {
        println!("\nEnter host's OpenID:");
        let mut host_oid = String::new();
        std::io::stdin()
            .read_line(&mut host_oid)
            .expect("Failed to read OID");
        host_oid = host_oid.trim().to_string();

        if host_oid.is_empty() {
            eprintln!("Empty OID, exiting");
            std::process::exit(1);
        }

        println!("\n[1/3] Registering with noray...");
        let (reg, joiner_stream) = match register_only(&config) {
            Ok(result) => result,
            Err(e) => {
                eprintln!("[ERROR] Registration failed: {}", e);
                std::process::exit(1);
            }
        };
        player_oid = reg.oid.clone();
        player_pid = reg.pid.clone();
        println!("[OK] Your OpenID: {}", player_oid);

        println!("\n[2/3] Registering UDP (required before connect-relay)...");
        udp_socket = Some(match register_udp_socket(&config, &player_pid) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[ERROR] UDP registration failed: {}", e);
                std::process::exit(1);
            }
        });
        println!("[OK] UDP registered");

        println!("\n[3/3] Connecting to host: {}...", host_oid);
        let config = config.clone();
        let target_oid = host_oid;
        thread::spawn(move || {
            let result =
                network::noray_client::connect_to_relay_with_stream(joiner_stream, &target_oid);
            match result {
                Ok((port, host)) => {
                    let _ = result_tx.send(Ok((port, host)));
                }
                Err(e) => {
                    let _ = result_tx.send(Err(e));
                }
            }
        });
    }

    relay_info = Some(match result_rx.recv_timeout(Duration::from_secs(60)) {
        Ok(Ok((port, host))) => {
            println!("\n[OK] Got relay port: {}", port);
            (port, host)
        }
        Ok(Err(e)) => {
            eprintln!("\n[ERROR] Connection failed: {}", e);
            std::process::exit(1);
        }
        Err(_) => {
            eprintln!("\n[ERROR] Timeout waiting for connection");
            std::process::exit(1);
        }
    });

    let (relay_port, relay_host) = relay_info.unwrap();
    let relay_addr = format!("{}:{}", relay_host, relay_port);

    println!("\n[UDP] Registering UDP socket...");
    let socket_for_udp =
        udp_socket
            .take()
            .unwrap_or_else(|| match register_udp_socket(&config, &player_pid) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("[ERROR] UDP registration failed: {}", e);
                    std::process::exit(1);
                }
            });
    println!("[OK] UDP ready");

    println!("\n[NETWORK] Starting UDP relay to {}...", relay_addr);

    let (sync_tx, sync_rx) = crossbeam_channel::bounded(100);
    let frame_counter = Arc::new(AtomicU32::new(0));

    let socket_clone = socket_for_udp.try_clone().expect("Failed to clone socket");
    let relay_addr_clone = relay_addr.clone();
    thread::spawn(move || {
        let sync_rx = sync_rx;
        let socket = socket_clone;
        loop {
            if let Ok(state) = sync_rx.recv() {
                let _ = send_game_state(&socket, &relay_addr_clone, &state);
            }
        }
    });

    let receiver = start_udp_relay(socket_for_udp, relay_port, relay_host);
    let receiver = Arc::new(receiver);

    println!("\n=== Game Starting ===");
    println!("Controls: A/D to move, Space to jump\n");

    App::new()
        .add_plugins(DefaultPlugins)
        .insert_resource(PlayerRegistrationInfo {
            oid: player_oid,
            pid: player_pid,
        })
        .insert_resource(RemoteUpdateReceiver { receiver })
        .insert_resource(NetworkingState {
            connected: true,
            error_message: String::new(),
        })
        .insert_resource(RemotePlayerData::default())
        .insert_resource(FrameCounter(frame_counter))
        .insert_resource(SyncChannel(sync_tx))
        .insert_resource(SyncReceiver(crossbeam_channel::bounded(100).1))
        .add_event::<game::local_input::JumpEvent>()
        .add_systems(Startup, (setup_game, spawn_local_player))
        .add_systems(Update, handle_local_input)
        .add_systems(Update, handle_jump_input)
        .add_systems(Update, apply_velocity)
        .add_systems(Update, apply_physics)
        .add_systems(Update, handle_jump_events)
        .add_systems(Update, sync_local_state)
        .add_systems(Update, receive_remote_updates)
        .add_systems(Update, update_remote_player_transforms)
        .run();
}

fn sync_local_state(
    query: Query<(&Transform, &Velocity, &IsJumping), With<LocalPlayerMarker>>,
    counter: Res<FrameCounter>,
    sync_tx: Res<SyncChannel>,
) {
    if let Ok((transform, velocity, is_jumping)) = query.get_single() {
        let frame = counter.0.fetch_add(1, Ordering::SeqCst);

        let state = GameState {
            frame,
            x: transform.translation.x,
            y: transform.translation.y,
            vx: velocity.x,
            vy: velocity.y,
            is_jumping: is_jumping.0,
        };

        let _ = sync_tx.0.send(state);
    }
}
