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
        (Entity, &mut Transform),
        (
            With<Player>,
            Without<crate::local_player_data::LocalPlayerMarker>,
        ),
    >,
) {
    let mut seen_oids = std::collections::HashSet::new();

    let entities_to_update: Vec<(Entity, std::string::String)> = remote_query
        .iter()
        .map(|(entity, _)| (entity, String::new()))
        .collect();

    for (oid, (x, y, _vx, _vy, _is_jumping)) in remote_data.players.iter() {
        if !seen_oids.contains(oid) {
            spawn_player(
                &mut commands,
                oid.clone(),
                false,
                Vec3::new(*x, *y, 0.0),
                Color::srgb(1.0, 0.0, 0.0),
            );
            seen_oids.insert(oid.clone());
        }
    }

    for (entity, _) in entities_to_update {
        if let Some((ox, oy, _, _, _)) = remote_data.players.get("remote") {
            if let Ok(mut transform) = remote_query.get_mut(entity) {
                transform.1.translation.x = *ox;
                transform.1.translation.y = *oy;
            }
        }
    }
}
