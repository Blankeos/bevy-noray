use std::sync::Arc;

use bevy::prelude::*;
use crossbeam_channel::Receiver;

use super::RemotePlayerData;
use crate::network::GameState;

#[derive(Resource)]
pub struct RemoteUpdateReceiver {
    pub receiver: Arc<Receiver<GameState>>,
}

pub fn receive_remote_updates(
    mut remote_data: ResMut<RemotePlayerData>,
    receiver: Option<Res<RemoteUpdateReceiver>>,
) {
    if let Some(rx) = receiver {
        while let Ok(state) = rx.receiver.try_recv() {
            remote_data.players.insert(
                "remote".to_string(),
                (state.x, state.y, state.vx, state.vy, state.is_jumping),
            );
            remote_data.initialized = true;
        }
    }
}
