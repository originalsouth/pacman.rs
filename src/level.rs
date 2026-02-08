use bevy::prelude::*;
use crate::components::{Pellet, PelletKind, Wall};
use crate::constants::TILE_SIZE;

#[derive(Resource, Clone)]
pub struct LevelData {
    pub grid: [[i32; 20]; 20],
    pub player_start: (usize, usize),
    pub ghost_starts: [(usize, usize); 4],
}

pub fn create_level() -> LevelData {
    let mut grid = [[0; 20]; 20];

    // Outer walls
    for x in 0..20 {
        grid[0][x] = 3;
        grid[19][x] = 3;
    }
    for y in 1..19 {
        grid[y][0] = 3;
        grid[y][19] = 3;
    }

    // Inner walls - create rooms
    for x in 5..15 {
        grid[5][x] = 3;
        grid[10][x] = 3;
        grid[15][x] = 3;
    }
    for y in 6..10 {
        grid[y][5] = 3;
        grid[y][14] = 3;
    }

    // Ghost pen walls
    for x in 9..12 {
        grid[8][x] = 3;
        grid[12][x] = 3;
    }
    grid[9][8] = 3;
    grid[9][12] = 3;
    grid[10][8] = 3;
    grid[10][12] = 3;
    grid[11][8] = 3;
    grid[11][12] = 3;

    // Fill with dots
    for y in 1..19 {
        for x in 1..19 {
            if grid[y][x] == 0 {
                grid[y][x] = 1;
            }
        }
    }

    // Power dots at corners
    grid[1][1] = 2;
    grid[1][18] = 2;
    grid[18][1] = 2;
    grid[18][18] = 2;

    let player_start = (10, 15);
    let ghost_starts = [
        (9, 9),   // Blinky
        (10, 9),  // Pinky
        (10, 10), // Inky
        (11, 10), // Clyde
    ];

    LevelData {
        grid,
        player_start,
        ghost_starts,
    }
}

pub fn setup_level(commands: &mut Commands, level_data: &LevelData) {
    for y in 0..20 {
        for x in 0..20 {
            match level_data.grid[y][x] {
                0 => {} // Empty space
                1 => {
                    // Normal pellet
                    commands.spawn(SpriteBundle {
                        sprite: Sprite {
                            color: Color::rgb(1.0, 1.0, 0.8),
                            custom_size: Some(Vec2::new(TILE_SIZE * 0.3, TILE_SIZE * 0.3)),
                            ..default()
                        },
                        transform: Transform {
                            translation: Vec3::new(
                                x as f32 * TILE_SIZE + TILE_SIZE * 0.5,
                                y as f32 * TILE_SIZE + TILE_SIZE * 0.5,
                                0.0,
                            ),
                            ..default()
                        },
                        ..default()
                    })
                    .insert(Pellet {
                        kind: PelletKind::Normal,
                    });
                }
                2 => {
                    // Power pellet
                    commands.spawn(SpriteBundle {
                        sprite: Sprite {
                            color: Color::rgb(1.0, 0.8, 0.2),
                            custom_size: Some(Vec2::new(TILE_SIZE * 0.6, TILE_SIZE * 0.6)),
                            ..default()
                        },
                        transform: Transform {
                            translation: Vec3::new(
                                x as f32 * TILE_SIZE + TILE_SIZE * 0.5,
                                y as f32 * TILE_SIZE + TILE_SIZE * 0.5,
                                0.0,
                            ),
                            ..default()
                        },
                        ..default()
                    })
                    .insert(Pellet {
                        kind: PelletKind::Power,
                    });
                }
                3 => {
                    // Wall
                    commands.spawn(SpriteBundle {
                        sprite: Sprite {
                            color: Color::rgb(0.2, 0.4, 1.0),
                            custom_size: Some(Vec2::new(TILE_SIZE, TILE_SIZE)),
                            ..default()
                        },
                        transform: Transform {
                            translation: Vec3::new(
                                x as f32 * TILE_SIZE,
                                y as f32 * TILE_SIZE,
                                0.0,
                            ),
                            ..default()
                        },
                        ..default()
                    })
                    .insert(Wall);
                }
                _ => {}
            }
        }
    }
}

pub fn count_pellets(level_data: &LevelData) -> usize {
    level_data
        .grid
        .iter()
        .flat_map(|row| row.iter())
        .filter(|&&cell| cell == 1 || cell == 2)
        .count()
}
