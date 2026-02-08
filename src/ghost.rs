use bevy::prelude::*;
use crate::components::{Ghost, Pacman, Direction};
use crate::constants::TILE_SIZE;
use crate::level::LevelData;
use rand::Rng;

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum GhostKind {
    Blinky,
    Pinky,
    Inky,
    Clyde,
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum GhostState {
    Chase,
    Scatter,
    Frightened,
    Eyes,
}

#[derive(Component)]
pub struct GhostMovement {
    pub speed: f32,
    pub move_timer: f32,
    pub move_interval: f32,
    pub last_direction: Direction,
}

pub fn spawn_ghosts(commands: &mut Commands, level_data: &LevelData) {
    let colors = [
        Color::rgb(1.0, 0.0, 0.0),   // Blinky - Red
        Color::rgb(1.0, 0.7, 1.0),   // Pinky - Pink
        Color::rgb(0.0, 1.0, 1.0),   // Inky - Cyan
        Color::rgb(1.0, 0.7, 0.4),   // Clyde - Orange
    ];

    let kinds = [
        GhostKind::Blinky,
        GhostKind::Pinky,
        GhostKind::Inky,
        GhostKind::Clyde,
    ];

    for (i, &kind) in kinds.iter().enumerate() {
        let (x, y) = level_data.ghost_starts[i];
        let pos = Vec3::new(x as f32 * TILE_SIZE, y as f32 * TILE_SIZE, 0.0);

        commands
            .spawn(SpriteBundle {
                sprite: Sprite {
                    color: colors[i],
                    custom_size: Some(Vec2::new(TILE_SIZE * 0.8, TILE_SIZE * 0.8)),
                    ..default()
                },
                transform: Transform {
                    translation: pos,
                    ..default()
                },
                ..default()
            })
            .insert(Ghost {
                kind,
                state: GhostState::Chase,
            })
            .insert(GhostMovement {
                speed: 100.0,
                move_timer: 0.0,
                move_interval: 0.5,
                last_direction: Direction::Left,
            });
    }
}

pub fn ghost_movement(
    mut ghost_query: Query<(&mut Transform, &mut GhostMovement), With<Ghost>>,
    time: Res<Time>,
) {
    for (mut transform, mut movement) in ghost_query.iter_mut() {
        movement.move_timer += time.delta_seconds();

        if movement.move_timer >= movement.move_interval {
            movement.move_timer = 0.0;

            let direction_movement = match movement.last_direction {
                Direction::Up => Vec3::new(0.0, movement.speed * movement.move_interval, 0.0),
                Direction::Down => Vec3::new(0.0, -movement.speed * movement.move_interval, 0.0),
                Direction::Left => Vec3::new(-movement.speed * movement.move_interval, 0.0, 0.0),
                Direction::Right => Vec3::new(movement.speed * movement.move_interval, 0.0, 0.0),
                Direction::None => Vec3::ZERO,
            };

            transform.translation += direction_movement;
        }
    }
}

pub fn ghost_ai(
    mut ghost_query: Query<(&Transform, &Ghost, &mut GhostMovement)>,
    pacman_query: Query<&Transform, With<Pacman>>,
) {
    let pacman_pos = pacman_query.single().translation;

    for (ghost_transform, ghost, mut movement) in ghost_query.iter_mut() {
        let ghost_pos = ghost_transform.translation;

        // Simple AI: chase pacman
        let dx = pacman_pos.x - ghost_pos.x;
        let dy = pacman_pos.y - ghost_pos.y;

        movement.last_direction = if dx.abs() > dy.abs() {
            if dx > 0.0 {
                Direction::Right
            } else {
                Direction::Left
            }
        } else {
            if dy > 0.0 {
                Direction::Up
            } else {
                Direction::Down
            }
        };

        // Scatter behavior occasionally
        if ghost.state == GhostState::Scatter {
            let mut rng = rand::thread_rng();
            movement.last_direction = match rng.gen_range(0..4) {
                0 => Direction::Up,
                1 => Direction::Down,
                2 => Direction::Left,
                _ => Direction::Right,
            };
        }
    }
}
