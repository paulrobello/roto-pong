//! WebGPU rendering module
//!
//! Handles all visual output:
//! - Arena, paddle, balls, blocks
//! - Black hole effect
//! - Particles and post-processing
//! - HUD overlay

pub mod pipeline;
pub mod sdf_pipeline;
pub mod shapes;
pub mod vertex;

pub use pipeline::RenderState;
pub use sdf_pipeline::SdfRenderState;
pub use shapes::{arc_segment, ball_trail, circle, ring};
pub use vertex::{Vertex, colors};

use glam::Vec2;

use crate::consts::*;
use crate::sim::{BlockKind, GameState};

/// Generate all vertices for the current game state
pub fn generate_frame(state: &GameState) -> Vec<Vertex> {
    let mut vertices = Vec::with_capacity(4096);

    // Arena outer wall (ring)
    vertices.extend(ring(
        Vec2::ZERO,
        ARENA_OUTER_RADIUS - 4.0,
        ARENA_OUTER_RADIUS,
        colors::ARENA_WALL,
        64,
    ));

    // Black hole
    vertices.extend(circle(
        Vec2::ZERO,
        BLACK_HOLE_RADIUS,
        colors::BLACK_HOLE,
        32,
    ));
    // Black hole ring
    vertices.extend(ring(
        Vec2::ZERO,
        BLACK_HOLE_RADIUS,
        BLACK_HOLE_RADIUS + 3.0,
        colors::BLACK_HOLE_RING,
        32,
    ));

    // Blocks
    for block in &state.blocks {
        let color = match block.kind {
            BlockKind::Glass => colors::BLOCK_GLASS,
            BlockKind::Armored => colors::BLOCK_ARMORED,
            BlockKind::Explosive => colors::BLOCK_EXPLOSIVE,
            BlockKind::Invincible => colors::BLOCK_INVINCIBLE,
            _ => colors::BLOCK_GLASS,
        };
        vertices.extend(arc_segment(&block.arc, color, 16.0));
    }

    // Paddle
    let paddle_arc = state.paddle.as_arc();
    vertices.extend(arc_segment(&paddle_arc, colors::PADDLE, 24.0));

    // Ball trails (render before balls so ball appears on top)
    for ball in &state.balls {
        vertices.extend(ball_trail(&ball.trail, ball.radius));
    }

    // Balls
    for ball in &state.balls {
        vertices.extend(circle(ball.pos, ball.radius, colors::BALL, 16));
    }

    vertices
}
