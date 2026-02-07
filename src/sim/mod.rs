//! Deterministic simulation module
//!
//! All gameplay logic lives here. This module must be pure and deterministic:
//! - Fixed timestep only
//! - Seeded RNG only
//! - Stable iteration order (by entity ID)
//! - No rendering or platform dependencies

pub mod arc;
pub mod collision;
pub mod sdf;
pub mod state;
pub mod tick;

pub use arc::ArcSegment;
pub use collision::{CollisionResult, ball_arc_collision};
pub use sdf::{check_sdf_collision, raymarch_collision, reflect, sd_arc, sd_arena_wall, sd_circle};
pub use state::{
    ARENA_GROWTH_PER_WAVE, ARENA_GROWTH_START_WAVE, BASE_ARENA_RADIUS, Ball, BallState, Block,
    BlockKind, GameEvent, GamePhase, GameState, INNER_MARGIN, LAYER_SPACING, MAX_ARENA_RADIUS,
    Paddle, PickupKind, WALL_MARGIN,
};
pub use tick::{TickInput, generate_wave, tick};
