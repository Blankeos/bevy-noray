pub mod receive;
pub mod remote_player;

pub use receive::{RemoteUpdateReceiver, receive_remote_updates};
pub use remote_player::{RemotePlayerData, update_remote_player_transforms};
