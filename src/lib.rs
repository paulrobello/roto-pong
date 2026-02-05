//! Roto Pong - A circular arena arcade game
//!
//! Core modules:
//! - `sim`: Deterministic simulation (physics, collisions, game state)
//! - `renderer`: WebGPU rendering pipeline
//! - `platform`: Browser/native platform abstraction
//! - `persistence`: Save/load with integrity verification
//! - `tuning`: Data-driven game balance

pub mod highscores;
pub mod persistence;
pub mod platform;
pub mod renderer;
pub mod settings;
pub mod sim;
pub mod tuning;
pub mod ui;

pub use highscores::HighScores;
pub use settings::{QualityPreset, Settings};

use glam::Vec2;

/// Game configuration constants
pub mod consts {
    /// Fixed simulation timestep (120 Hz for smooth physics)
    pub const SIM_DT: f32 = 1.0 / 120.0;
    /// Maximum substeps per frame to prevent spiral of death
    pub const MAX_SUBSTEPS: u32 = 8;

    /// Arena dimensions
    pub const ARENA_OUTER_RADIUS: f32 = 400.0;
    pub const BLACK_HOLE_RADIUS: f32 = 40.0;
    pub const BLACK_HOLE_LOSS_RADIUS: f32 = 35.0;

    /// Paddle defaults - paddle orbits INSIDE, defending the black hole
    pub const PADDLE_RADIUS: f32 = 47.5; // Back edge at event horizon (40 + 15/2)
    pub const PADDLE_THICKNESS: f32 = 15.0;
    pub const PADDLE_ARC_WIDTH: f32 = 1.21; // radians (~69 degrees) - another 10% bigger

    /// Ball defaults
    pub const BALL_RADIUS: f32 = 8.0;
    pub const BALL_START_SPEED: f32 = 200.0;
    /// Minimum ball speed (gravity can't slow it below this)
    pub const BALL_MIN_SPEED: f32 = 150.0;
    /// Maximum ball speed
    pub const BALL_MAX_SPEED: f32 = 400.0;

    /// Black hole gravity (acceleration toward center, pixels/s²)
    pub const BLACK_HOLE_GRAVITY: f32 = 120.0;
    /// Speed boost when ball hits paddle (multiplicative)
    pub const PADDLE_BOOST: f32 = 1.15;

    /// Block defaults
    pub const BLOCK_THICKNESS: f32 = 24.0;
}

/// Normalized angle to [-π, π)
#[inline]
pub fn normalize_angle(mut angle: f32) -> f32 {
    use std::f32::consts::PI;
    while angle >= PI {
        angle -= 2.0 * PI;
    }
    while angle < -PI {
        angle += 2.0 * PI;
    }
    angle
}

/// Convert polar (r, theta) to cartesian (x, y)
#[inline]
pub fn polar_to_cartesian(r: f32, theta: f32) -> Vec2 {
    Vec2::new(r * theta.cos(), r * theta.sin())
}

/// Convert cartesian (x, y) to polar (r, theta)
#[inline]
pub fn cartesian_to_polar(pos: Vec2) -> (f32, f32) {
    (pos.length(), pos.y.atan2(pos.x))
}
