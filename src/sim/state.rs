//! Game state and core simulation types
//!
//! All state that must be persisted for Continue/determinism lives here.

use glam::Vec2;
use rand::SeedableRng;
use rand_pcg::Pcg32;
use serde::{Deserialize, Serialize};

use super::arc::ArcSegment;
use crate::consts::*;
use crate::{normalize_angle, polar_to_cartesian};

/// Current phase of gameplay
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GamePhase {
    /// Ball attached to paddle, waiting for launch input
    Serve,
    /// Active gameplay
    Playing,
    /// Between-wave rest period (5 seconds)
    Breather,
    /// Game is paused
    Paused,
    /// Run ended
    GameOver,
}

/// Ball state - attached to paddle or free-moving
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum BallState {
    /// Ball is attached to paddle at given angular offset from paddle center
    Attached { offset: f32 },
    /// Ball is free-moving
    Free,
    /// Ball is being consumed by black hole (spaghettification!)
    Dying { timer: f32, start_pos: (f32, f32) },
}

/// Trail point for ball rendering
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TrailPoint {
    pub pos: Vec2,
    pub speed: f32,
}

/// Maximum number of trail points to store
pub const TRAIL_LENGTH: usize = 20;

/// A ball entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ball {
    pub id: u32,
    pub pos: Vec2,
    pub vel: Vec2,
    pub radius: f32,
    pub state: BallState,
    /// Piercing mode (passes through blocks without reflecting)
    pub piercing: bool,
    /// Cooldown ticks before paddle can be hit again (prevents sticking)
    #[serde(default)]
    pub paddle_cooldown: u32,
    /// Portal block IDs the ball is currently inside (for exit-only damage)
    #[serde(default)]
    pub inside_portals: Vec<u32>,
    /// Trail history for rendering (newest first)
    #[serde(skip)]
    pub trail: Vec<TrailPoint>,
}

impl Ball {
    pub fn new(id: u32) -> Self {
        Self {
            id,
            pos: Vec2::ZERO,
            vel: Vec2::ZERO,
            radius: BALL_RADIUS,
            state: BallState::Attached { offset: 0.0 },
            piercing: false,
            paddle_cooldown: 0,
            inside_portals: Vec::new(),
            trail: Vec::with_capacity(TRAIL_LENGTH),
        }
    }

    /// Record current position to trail (call each tick when free)
    pub fn record_trail(&mut self) {
        let speed = self.vel.length();
        self.trail.insert(0, TrailPoint { pos: self.pos, speed });
        if self.trail.len() > TRAIL_LENGTH {
            self.trail.pop();
        }
    }

    /// Clear trail (on respawn/attach)
    pub fn clear_trail(&mut self) {
        self.trail.clear();
    }

    /// Update attached ball position based on paddle
    pub fn update_attached(&mut self, paddle: &Paddle) {
        if let BallState::Attached { offset } = self.state {
            let theta = paddle.theta + offset;
            // Position ball just outside paddle outer edge
            let r = PADDLE_RADIUS + PADDLE_THICKNESS / 2.0 + self.radius + 2.0;
            self.pos = polar_to_cartesian(r, theta);
        }
    }

    /// Launch the ball from attached state
    pub fn launch(&mut self, paddle: &Paddle, base_speed: f32, english_factor: f32) {
        if let BallState::Attached { offset } = self.state {
            let launch_theta = paddle.theta + offset;
            // Base direction: radially outward
            let radial_dir = Vec2::new(launch_theta.cos(), launch_theta.sin());
            // Add small tangential component from paddle angular velocity
            let tangent = Vec2::new(-launch_theta.sin(), launch_theta.cos());
            let english = (paddle.angular_vel * english_factor).clamp(-0.3, 0.3);

            self.vel = (radial_dir + tangent * english).normalize() * base_speed;
            self.state = BallState::Free;
        }
    }
}

/// The player's paddle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Paddle {
    /// Current angle (radians, center of paddle arc)
    pub theta: f32,
    /// Angular width of paddle (radians)
    pub arc_width: f32,
    /// Angular velocity (for "english" on ball)
    pub angular_vel: f32,
}

impl Default for Paddle {
    fn default() -> Self {
        Self {
            theta: -std::f32::consts::FRAC_PI_2, // Start at bottom
            arc_width: PADDLE_ARC_WIDTH,
            angular_vel: 0.0,
        }
    }
}

impl Paddle {
    /// Get the paddle as an ArcSegment for collision detection
    pub fn as_arc(&self) -> ArcSegment {
        ArcSegment::new(
            PADDLE_RADIUS,
            PADDLE_THICKNESS,
            self.theta - self.arc_width / 2.0,
            self.theta + self.arc_width / 2.0,
        )
    }

    /// Update paddle angle toward target (with smoothing)
    pub fn move_toward(&mut self, target_theta: f32, dt: f32, max_speed: f32) {
        let target = normalize_angle(target_theta);
        let current = normalize_angle(self.theta);

        let mut delta = target - current;
        // Handle wraparound
        if delta > std::f32::consts::PI {
            delta -= std::f32::consts::TAU;
        } else if delta < -std::f32::consts::PI {
            delta += std::f32::consts::TAU;
        }

        // Clamp to max angular speed
        let max_delta = max_speed * dt;
        let clamped_delta = delta.clamp(-max_delta, max_delta);

        self.angular_vel = clamped_delta / dt;
        self.theta = normalize_angle(self.theta + clamped_delta);
    }
}

/// Block types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BlockKind {
    #[default]
    Glass,
    Armored,
    Explosive,
    Invincible, // Cannot be destroyed, doesn't count for wave clear
    Prism,
    Portal { pair_id: u32 },
    Jello, // Wobbly block that ripples when hit
    Pulse,
    Magnet,
    PowerUpCapsule,
}

/// A block entity (curved arc)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub id: u32,
    pub kind: BlockKind,
    pub hp: u8,
    pub arc: ArcSegment,
    /// Rotation speed in radians/sec (0 = stationary)
    #[serde(default)]
    pub rotation_speed: f32,
    /// Wobble intensity (0-1, decays over time) for Jello blocks
    #[serde(default)]
    pub wobble: f32,
}

impl Block {
    /// Rotate the block by its rotation speed * dt, decay wobble
    pub fn rotate(&mut self, dt: f32) {
        if self.rotation_speed != 0.0 {
            let delta = self.rotation_speed * dt;
            self.arc.theta_start = normalize_angle(self.arc.theta_start + delta);
            self.arc.theta_end = normalize_angle(self.arc.theta_end + delta);
        }
        // Decay wobble over time (fast decay for snappy feel)
        if self.wobble > 0.0 {
            self.wobble = (self.wobble - dt * 2.0).max(0.0);
        }
    }
    
    /// Trigger wobble (for Jello blocks when hit)
    pub fn trigger_wobble(&mut self) {
        if self.kind == BlockKind::Jello {
            self.wobble = 1.0;
        }
    }
    
    /// Returns true if this block must be destroyed to clear the wave
    pub fn counts_for_clear(&self) -> bool {
        self.kind != BlockKind::Invincible
    }
}

/// Power-up types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PickupKind {
    MultiBall,
    Slow,
    Piercing,
    WidenPaddle,
    Shield,
}

/// A pickup entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pickup {
    pub id: u32,
    pub kind: PickupKind,
    pub pos: Vec2,
    pub vel: Vec2,
    pub ttl_ticks: u32,
}

/// Active power-up effects
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ActiveEffects {
    pub slow_ticks: u32,
    pub piercing_ticks: u32,
    pub widen_ticks: u32,
    pub shield_active: bool,
}

/// A particle for visual effects
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Particle {
    pub pos: Vec2,
    pub vel: Vec2,
    pub color: u32, // Block kind for color lookup
    pub life: f32,  // 0-1, decreases over time
    pub size: f32,
}

/// Maximum particles
pub const MAX_PARTICLES: usize = 256;

/// RNG state wrapper for serialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RngState {
    pub seed: u64,
    pub stream: u64,
}

impl RngState {
    pub fn new(seed: u64) -> Self {
        Self { seed, stream: 0 }
    }

    pub fn to_rng(&self) -> Pcg32 {
        Pcg32::seed_from_u64(self.seed)
    }
}

/// Complete game state (deterministic, serializable)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    /// Run seed for reproducibility
    pub seed: u64,
    /// RNG state
    pub rng_state: RngState,
    /// Current wave index (0-based)
    pub wave_index: u32,
    /// Player lives
    pub lives: u8,
    /// Score
    pub score: u64,
    /// Combo counter
    pub combo: u32,
    /// Simulation tick counter
    pub time_ticks: u64,
    /// Current phase
    pub phase: GamePhase,
    /// Breather timer (ticks remaining)
    pub breather_ticks: u32,
    /// Player paddle
    pub paddle: Paddle,
    /// Active balls (sorted by id for determinism)
    pub balls: Vec<Ball>,
    /// Active blocks (sorted by id for determinism)
    pub blocks: Vec<Block>,
    /// Active pickups (sorted by id for determinism)
    pub pickups: Vec<Pickup>,
    /// Active power-up effects
    pub effects: ActiveEffects,
    /// Visual particles (not gameplay-affecting)
    #[serde(skip)]
    pub particles: Vec<Particle>,
    /// Next entity ID
    next_id: u32,
}

impl GameState {
    /// Create a new game state with the given seed
    pub fn new(seed: u64) -> Self {
        let mut state = Self {
            seed,
            rng_state: RngState::new(seed),
            wave_index: 0,
            lives: 3,
            score: 0,
            combo: 0,
            time_ticks: 0,
            phase: GamePhase::Serve,
            breather_ticks: 0,
            paddle: Paddle::default(),
            balls: Vec::new(),
            blocks: Vec::new(),
            pickups: Vec::new(),
            effects: ActiveEffects::default(),
            particles: Vec::new(),
            next_id: 1,
        };

        // Spawn initial ball attached to paddle
        state.spawn_ball_attached();

        state
    }

    /// Allocate a new entity ID
    pub fn next_entity_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Spawn a ball attached to the paddle
    pub fn spawn_ball_attached(&mut self) {
        let id = self.next_entity_id();
        let mut ball = Ball::new(id);
        ball.state = BallState::Attached { offset: 0.0 };
        ball.update_attached(&self.paddle);
        self.balls.push(ball);
    }

    /// Ensure balls are sorted by ID for deterministic iteration
    pub fn normalize_order(&mut self) {
        self.balls.sort_by_key(|b| b.id);
        self.blocks.sort_by_key(|b| b.id);
        self.pickups.sort_by_key(|p| p.id);
    }
}

/// Breather phase duration in ticks (2 seconds at 120 Hz)
pub const BREATHER_DURATION_TICKS: u32 = 2 * 120;
