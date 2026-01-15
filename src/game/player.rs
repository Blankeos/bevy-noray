use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Component, Clone)]
pub struct Player {
    pub oid: String,
    pub is_local: bool,
}

#[derive(Component, Reflect, Default, Clone, Copy, Serialize, Deserialize)]
#[reflect(Component, Default)]
pub struct Velocity {
    pub x: f32,
    pub y: f32,
}

#[derive(Component, Default, Clone, Copy)]
pub struct IsJumping(pub bool);

pub fn spawn_player(
    commands: &mut Commands,
    oid: String,
    is_local: bool,
    position: Vec3,
    color: Color,
) -> Entity {
    commands
        .spawn((
            Player { oid, is_local },
            Velocity { x: 0.0, y: 0.0 },
            IsJumping(false),
            Transform::from_translation(position),
            GlobalTransform::default(),
        ))
        .with_children(|parent| {
            parent.spawn(SpriteBundle {
                sprite: Sprite {
                    color,
                    custom_size: Some(Vec2::new(50.0, 50.0)),
                    ..default()
                },
                ..default()
            });
        })
        .id()
}

const GRAVITY: f32 = 900.0;
const GROUND_LEVEL: f32 = 25.0;
const MOVE_SPEED: f32 = 300.0;
const JUMP_FORCE: f32 = 400.0;

pub fn apply_velocity(mut query: Query<(&mut Transform, &Velocity)>, time: Res<Time>) {
    for (mut transform, velocity) in query.iter_mut() {
        transform.translation.x += velocity.x * time.delta_seconds();
        transform.translation.y += velocity.y * time.delta_seconds();
    }
}

pub fn apply_physics(
    mut query: Query<(&mut Velocity, &mut IsJumping, &mut Transform)>,
    time: Res<Time>,
) {
    for (mut velocity, mut is_jumping, mut transform) in query.iter_mut() {
        velocity.y -= GRAVITY * time.delta_seconds();

        if transform.translation.y <= GROUND_LEVEL {
            transform.translation.y = GROUND_LEVEL;
            velocity.y = 0.0;
            is_jumping.0 = false;
        }
    }
}

pub fn jump(mut query: Query<(&mut Velocity, &mut IsJumping)>) {
    for (mut velocity, mut is_jumping) in query.iter_mut() {
        if !is_jumping.0 {
            velocity.y = JUMP_FORCE;
            is_jumping.0 = true;
        }
    }
}

pub fn handle_jump_events(
    mut events: EventReader<crate::game::local_input::JumpEvent>,
    mut query: Query<(&mut Velocity, &mut IsJumping)>,
) {
    for event in events.read() {
        if let Ok((mut velocity, mut is_jumping)) = query.get_mut(event.0) {
            if !is_jumping.0 {
                velocity.y = JUMP_FORCE;
                is_jumping.0 = true;
            }
        }
    }
}
