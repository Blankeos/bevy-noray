mod game;
mod local_player_data;
mod network;
mod sync;

use bevy::prelude::*;
use crossbeam_channel;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread;

use game::player::{IsJumping, Velocity, spawn_player};
use game::{
    apply_physics, apply_velocity, handle_jump_events, handle_jump_input, handle_local_input,
};
use local_player_data::LocalPlayerMarker;
use network::{
    GameState, NorayConfig, register_only, register_udp_socket, send_game_state, start_udp_relay,
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
    println!("1. Host a game (2 players)");
    println!("2. Host a game (3 players)");
    println!("3. Host a game (4 players)");
    println!("4. Join a game");
    println!();

    let mut choice = String::new();
    std::io::stdin()
        .read_line(&mut choice)
        .expect("Failed to read input");
    let choice = choice.trim();

    let num_players = match choice {
        "1" => 2,
        "2" => 3,
        "3" => 4,
        "4" => {
            println!("\nEnter host's OpenID:");
            let mut host_oid = String::new();
            std::io::stdin()
                .read_line(&mut host_oid)
                .expect("Failed to read OID");
            let host_oid = host_oid.trim().to_string();

            if host_oid.is_empty() {
                eprintln!("Empty OID, exiting");
                std::process::exit(1);
            }

            run_joiner(&config, &host_oid);
            return;
        }
        _ => {
            eprintln!("Invalid choice");
            std::process::exit(1);
        }
    };

    run_host(&config, num_players);
}

fn run_host(config: &NorayConfig, num_players: u32) {
    println!("\n[1/3] Registering with noray...");
    let (reg, stream) = match register_only(config) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("[ERROR] Registration failed: {}", e);
            std::process::exit(1);
        }
    };
    let player_oid = reg.oid.clone();
    let player_pid = reg.pid.clone();
    println!("[OK] Your OpenID: {}", player_oid);

    println!("\n[2/3] Registering UDP (required before relay)...");
    let udp_for_relay = match register_udp_socket(config, &player_pid) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[ERROR] UDP registration failed: {}", e);
            std::process::exit(1);
        }
    };
    println!("[OK] UDP registered");

    println!("\n[3/3] Waiting for {} players...", num_players - 1);
    println!("Your OID: {}", player_oid);
    println!("\nTell players your OID and keep this terminal open!");

    let host = config.host.clone();
    let peers = match network::noray_client::wait_for_connections(stream, host, num_players - 1) {
        Ok(peers) => peers,
        Err(e) => {
            eprintln!("[ERROR] Failed to wait for connections: {}", e);
            std::process::exit(1);
        }
    };

    println!("\n[OK] All {} players connected!", num_players - 1);

    let relay_port = peers[0].port;
    let relay_host = peers[0].host.clone();
    let relay_addr = format!("{}:{}", relay_host, relay_port);

    println!("\n[UDP] Registering UDP socket...");
    let socket_for_udp = udp_for_relay;
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

    let receiver = start_udp_relay(socket_for_udp, relay_port);
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

fn run_joiner(config: &NorayConfig, host_oid: &str) {
    println!("\n[1/3] Registering with noray...");
    let (reg, joiner_stream) = match register_only(config) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("[ERROR] Registration failed: {}", e);
            std::process::exit(1);
        }
    };
    let player_oid = reg.oid.clone();
    let player_pid = reg.pid.clone();
    println!("[OK] Your OpenID: {}", player_oid);

    println!("\n[2/3] Registering UDP (required before connect-relay)...");
    let udp_socket = match register_udp_socket(config, &player_pid) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[ERROR] UDP registration failed: {}", e);
            std::process::exit(1);
        }
    };
    println!("[OK] UDP registered");

    println!("\n[3/3] Connecting to host: {}...", host_oid);
    let result = network::noray_client::connect_to_relay_with_stream(joiner_stream, host_oid);

    let (relay_port, relay_host) = match result {
        Ok((port, host)) => (port, host),
        Err(e) => {
            eprintln!("[ERROR] Connection failed: {}", e);
            std::process::exit(1);
        }
    };

    println!("\n[OK] Got relay port: {}", relay_port);
    let relay_addr = format!("{}:{}", relay_host, relay_port);

    println!("\n[UDP] Registering UDP socket...");
    println!("[OK] UDP ready");

    println!("\n[NETWORK] Starting UDP relay to {}...", relay_addr);

    let (sync_tx, sync_rx) = crossbeam_channel::bounded(100);
    let frame_counter = Arc::new(AtomicU32::new(0));

    let socket_clone = udp_socket.try_clone().expect("Failed to clone socket");
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

    let receiver = start_udp_relay(udp_socket, relay_port);
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
    registration: Res<PlayerRegistrationInfo>,
) {
    for (transform, velocity, is_jumping) in query.iter() {
        let frame = counter.0.fetch_add(1, Ordering::SeqCst);

        let state = GameState {
            oid: registration.oid.clone(),
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
