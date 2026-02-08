use bevy::prelude::Component;

use crate::ghost::{GhostKind, GhostState};

// Components
#[derive(Component)]
pub struct Pacman {
    pub direction: Direction,
    pub next_direction: Option<Direction>,
    pub speed: f32,
}

#[derive(Component)]
pub struct Ghost {
    pub kind: GhostKind,
    pub state: GhostState,
}

#[derive(Component)]
pub struct Wall;

#[derive(Component)]
pub struct Pellet {
    pub kind: PelletKind,
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum PelletKind {
    Normal,
    Power,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
    None,
}
