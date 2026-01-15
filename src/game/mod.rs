pub mod local_input;
pub mod player;

pub use local_input::{JumpEvent, handle_jump_input, handle_local_input};
pub use player::{
    IsJumping, Player, Velocity, apply_physics, apply_velocity, handle_jump_events, jump,
    spawn_player,
};
