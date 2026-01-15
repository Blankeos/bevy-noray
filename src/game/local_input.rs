use crate::game::player::{Player, Velocity};
use bevy::prelude::*;

pub fn handle_local_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut query: Query<&mut Velocity, With<Player>>,
) {
    for mut velocity in query.iter_mut() {
        const SPEED: f32 = 300.0;

        if keyboard.pressed(KeyCode::KeyA) {
            velocity.x = -SPEED;
        } else if keyboard.pressed(KeyCode::KeyD) {
            velocity.x = SPEED;
        } else {
            velocity.x = 0.0;
        }
    }
}

pub fn handle_jump_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    query: Query<Entity, With<Player>>,
    mut jump_events: EventWriter<JumpEvent>,
) {
    if keyboard.just_pressed(KeyCode::Space) {
        for entity in query.iter() {
            jump_events.send(JumpEvent(entity));
        }
    }
}

#[derive(Event)]
pub struct JumpEvent(pub Entity);
