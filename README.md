# Bevy Noray

A multiplayer demo showcasing how to build real-time multiplayer games using [Bevy](https://bevyengine.org/) and [Noray](https://github.com/michaelfairley/noray).

<video width="800" height="450" src="https://raw.githubusercontent.com/Blankeos/bevy-noray/main/noray-bevy-simple-1.mp4"></video>

## What is Noray?

Noray is a minimal peer-to-peer hole punching relay server. It helps two peers behind NATs or firewalls establish a direct UDP connection by:

1. Both peers register with the Noray server
2. The server facilitates a handshake between peers
3. Once connected, peers communicate directly via UDP (the relay is no longer needed)

## Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│                      Player 1                           │
│  ┌─────────────┐    ┌─────────────┐    ┌────────────┐  │
│  │   Bevy App  │───>│  SyncState  │───>│ UDP Socket │  │
│  └─────────────┘    └─────────────┘    └────────────┘  │
│         │                                     │         │
│         │ (TCP handshake)                     │ (UDP)   │
│         v                                     v         │
│  ┌──────────────────────────────────────────────┐       │
│  │              Noray Server                     │       │
│  └──────────────────────────────────────────────┘       │
│         ^                                     ^         │
│         │ (TCP handshake)                     │ (UDP)   │
│         │                                     │         │
│  ┌─────────────┐    ┌─────────────┐    ┌────────────┐  │
│  │   Bevy App  │<───│ ReceiveSync │<───│ UDP Socket │  │
│  └─────────────┘    └─────────────┘    └────────────┘  │
│                      Player 2                           │
└─────────────────────────────────────────────────────────┘
```

## How It Works

### 1. Registration Phase (TCP)

Both players connect to the Noray server via TCP and register themselves:

**Host Flow (`src/main.rs:88-128`):**

```
1. Register with Noray → Get OpenID (oid) and PlayerID (pid)
2. Register UDP socket → Tell server our UDP endpoint
3. Wait for peer to connect via TCP relay
```

**Joiner Flow (`src/main.rs:129-179`):**

```
1. Register with Noray → Get own OpenID and PlayerID
2. Register UDP socket → Tell server our UDP endpoint
3. Connect to host's OpenID via TCP relay
```

### 2. Connection Established

When both peers have registered, the Noray server facilitates a connection by:

- Sharing each peer's public IP/port with the other
- Allowing a direct UDP connection to be established

### 3. Game Loop (Bevy ECS)

The game uses Bevy's Entity Component System:

**Local Player (`src/game/player.rs`):**

- `Player` component - identity and local/remote flag
- `Velocity` component - x,y velocity for physics
- `IsJumping` component - jump state
- `Transform` - position in world space

**Physics (`src/game/player.rs:53-82`):**

```rust
// Gravity applied each frame
velocity.y -= GRAVITY * delta_seconds();

// Ground collision
if position.y <= GROUND_LEVEL {
    position.y = GROUND_LEVEL;
    velocity.y = 0.0;
    is_jumping = false;
}
```

### 4. State Synchronization

**Sending Updates (`src/main.rs:263-282`):**

```rust
fn sync_local_state(
    query: Query<(&Transform, &Velocity, &IsJumping>, With<LocalPlayerMarker>>,
    sync_tx: Res<SyncChannel>,
) {
    // Send position, velocity, and jump state to network thread
    let state = GameState {
        frame: counter++,
        x: transform.translation.x,
        y: transform.translation.y,
        vx: velocity.x,
        vy: velocity.y,
        is_jumping: is_jumping.0,
    };
    sync_tx.0.send(state);
}
```

**Network Thread (`src/network/packet_handler.rs:127-152`):**

```
1. Receive GameState from channel
2. Serialize with bincode
3. Send via UDP to peer
```

**Receiving Updates (`src/sync/receive.rs:14-27`):**

```rust
pub fn receive_remote_updates(
    mut remote_data: ResMut<RemotePlayerData>,
    receiver: Option<Res<RemoteUpdateReceiver>>,
) {
    while let Ok(state) = rx.receiver.try_recv() {
        remote_data.players.insert(
            "remote".to_string(),
            (state.x, state.y, state.vx, state.vy, state.is_jumping),
        );
    }
}
```

### 5. Remote Player Rendering (`src/sync/remote_player.rs`)

Remote players are rendered by:

1. Receiving GameState packets via UDP
2. Updating a `RemotePlayerData` resource
3. Spawning/removing entities based on received data

## Key Files

| File                            | Purpose                                            |
| ------------------------------- | -------------------------------------------------- |
| `src/main.rs`                   | Entry point, game setup, networking initialization |
| `src/game/mod.rs`               | Game logic (input, physics, player spawning)       |
| `src/game/player.rs`            | Player components and physics                      |
| `src/network/mod.rs`            | Network module exports                             |
| `src/network/noray_client.rs`   | TCP communication with Noray server                |
| `src/network/packet_handler.rs` | UDP packet serialization/deserialization           |
| `src/sync/mod.rs`               | Sync module exports                                |
| `src/sync/receive.rs`           | Receiving remote player updates                    |
| `src/sync/remote_player.rs`     | Remote player rendering                            |

## Running the Demo

```bash
# Terminal 1 - Host a game
cargo run
# Choose option 1
# Copy your OpenID displayed

# Terminal 2 - Join a game
cargo run
# Choose option 2
# Enter the host's OpenID
```

## Controls

- **A/D** - Move left/right
- **Space** - Jump

## Key Data Structures

### GameState (`src/network/packet_handler.rs:10-18`)

```rust
struct GameState {
    frame: u32,        // Frame counter for interpolation
    x: f32,           // Position X
    y: f32,           // Position Y
    vx: f32,          // Velocity X
    vy: f32,          // Velocity Y
    is_jumping: bool, // Jump state
}
```

### NorayConfig

```rust
struct NorayConfig {
    host: String,      // Noray server address
    tcp_port: u16,     // TCP port for registration
    udp_port: u16,     // UDP port for hole punching
}
```

## Networking Flow Summary

```
Host                          Noray Server                      Joiner
  │                               │                                │
  ├─ TCP: register() ────────────>│                                │
  │<────── oid: "host123" ────────┤                                │
  │                               │                                │
  ├─ UDP: register_udp(pid) ────>│                                │
  │                               │                                │
  │   [wait for peer]            │                                │
  │                               │<── TCP: register() ───────────┤
  │                               │<──── oid: "joiner1" ──────────┤
  │                               │                                │
  │                               │── TCP: connect("host123") ───>│
  │<── TCP: accept() (with port)──│                                │
  │                               │                                │
  │   [UDP hole punched]          │                                │
  │<──────────── UDP packets ────────────────────────>             │
  │   [Direct P2P connection established]                         │
```

## Technical Notes

- **Packet size**: Fixed at 21 bytes (bincode serialized GameState)
- **Sync channel**: Bounded channel with capacity 100
- **Frame counter**: Atomic counter for ordering updates
- **No interpolation**: For simplicity, direct position updates are used
- **No prediction**: Client authoritative (not production-ready)

## Learning Resources

To understand this codebase:

1. Start with `src/main.rs` to see the initialization flow
2. Read `src/game/player.rs` to understand Bevy ECS patterns
3. Study `src/network/packet_handler.rs` for UDP networking
4. Review `src/sync/receive.rs` for receiving remote updates

## Dependencies

- `bevy 0.14` - Game engine
- `tokio` - Async runtime for TCP networking
- `bincode` - Binary serialization
- `serde` - Serialization framework
- `crossbeam-channel` - Thread-safe channels
