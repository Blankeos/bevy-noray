use bevy::prelude::*;
use std::collections::HashMap;

pub use crate::game::player::{Player, spawn_player};

#[derive(Resource, Default)]
pub struct RemotePlayerData {
    pub players: HashMap<String, (f32, f32, f32, f32, bool)>,
    pub initialized: bool,
}

pub fn update_remote_player_transforms(
    mut commands: Commands,
    remote_data: Res<RemotePlayerData>,
    mut remote_query: Query<
        (Entity, &Player, &mut Transform),
        (
            With<Player>,
            Without<crate::local_player_data::LocalPlayerMarker>,
        ),
    >,
) {
    let mut oid_to_entity = HashMap::new();

    for (entity, player, _transform) in remote_query.iter() {
        oid_to_entity.insert(player.oid.clone(), entity);
    }

    for (oid, (x, y, _vx, _vy, _is_jumping)) in remote_data.players.iter() {
        if let Some(entity) = oid_to_entity.get(oid) {
            if let Ok((_, _, mut transform)) = remote_query.get_mut(*entity) {
                transform.translation.x = *x;
                transform.translation.y = *y;
            }
        } else {
            spawn_player(
                &mut commands,
                oid.clone(),
                false,
                Vec3::new(*x, *y, 0.0),
                Color::srgb(1.0, 0.0, 0.0),
            );
        }
    }
}
