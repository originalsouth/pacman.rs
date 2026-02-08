use bevy::prelude::*;
use crate::components::{Pacman, Direction, Pellet, PelletKind};
use crate::constants::TILE_SIZE;

pub fn setup_pacman(commands: &mut Commands, start_pos: (usize, usize)) {
    let pacman_pos = Vec3::new(
        start_pos.0 as f32 * TILE_SIZE,
        start_pos.1 as f32 * TILE_SIZE,
        0.0,
    );
    
    commands
        .spawn(SpriteBundle {
            sprite: Sprite {
                color: Color::rgb(1.0, 1.0, 0.0),
                custom_size: Some(Vec2::new(TILE_SIZE * 0.8, TILE_SIZE * 0.8)),
                ..default()
            },
            transform: Transform {
                translation: pacman_pos,
                ..default()
            },
            ..default()
        })
        .insert(Pacman {
            direction: Direction::Right,
            next_direction: None,
            speed: 150.0,
        });
}

pub fn pacman_movement(
    mut pacman_query: Query<(&mut Transform, &mut Pacman)>,
    time: Res<Time>,
) {
    for (mut transform, mut pacman) in pacman_query.iter_mut() {
        if let Some(direction) = pacman.next_direction {
            pacman.direction = direction;
            pacman.next_direction = None;
        }

        let movement = match pacman.direction {
            Direction::Up => Vec3::new(0.0, pacman.speed, 0.0),
            Direction::Down => Vec3::new(0.0, -pacman.speed, 0.0),
            Direction::Left => Vec3::new(-pacman.speed, 0.0, 0.0),
            Direction::Right => Vec3::new(pacman.speed, 0.0, 0.0),
            Direction::None => Vec3::ZERO,
        };

        transform.translation += movement * time.delta_seconds();
    }
}

pub fn input_handler(
    mut pacman_query: Query<&mut Pacman>,
    keyboard_input: Res<Input<KeyCode>>,
) {
    if let Ok(mut pacman) = pacman_query.get_single_mut() {
        if keyboard_input.pressed(KeyCode::Up) {
            pacman.next_direction = Some(Direction::Up);
        } else if keyboard_input.pressed(KeyCode::Down) {
            pacman.next_direction = Some(Direction::Down);
        } else if keyboard_input.pressed(KeyCode::Left) {
            pacman.next_direction = Some(Direction::Left);
        } else if keyboard_input.pressed(KeyCode::Right) {
            pacman.next_direction = Some(Direction::Right);
        }
    }
}

pub fn collision_with_pellets(
    mut commands: Commands,
    pacman_query: Query<&Transform, With<Pacman>>,
    pellet_query: Query<(Entity, &Transform, &Pellet), Without<Pacman>>,
    mut game_state: ResMut<crate::GameState>,
) {
    for pacman_transform in pacman_query.iter() {
        for (entity, pellet_transform, pellet) in pellet_query.iter() {
            let distance = pacman_transform.translation.distance(pellet_transform.translation);
            if distance < TILE_SIZE * 0.5 {
                commands.entity(entity).despawn();
                
                match pellet.kind {
                    PelletKind::Normal => {
                        game_state.score += 10;
                    }
                    PelletKind::Power => {
                        game_state.score += 50;
                        game_state.power_mode_timer = 10.0;
                    }
                }
                
                game_state.dots_remaining = game_state.dots_remaining.saturating_sub(1);
                
                if game_state.dots_remaining == 0 {
                    game_state.level += 1;
                }
            }
        }
    }
}
