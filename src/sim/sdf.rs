//! SDF-based collision detection
//!
//! Uses signed distance fields for accurate collision detection and response.

use glam::Vec2;
use std::f32::consts::TAU;

/// Signed distance to a circle
#[inline]
pub fn sd_circle(p: Vec2, center: Vec2, radius: f32) -> f32 {
    (p - center).length() - radius
}

/// Signed distance to an arc segment
/// Returns distance to the arc band (inner to outer radius, theta_start to theta_end)
pub fn sd_arc(p: Vec2, theta_start: f32, theta_end: f32, radius: f32, thickness: f32) -> f32 {
    let r = p.length();
    let angle = p.y.atan2(p.x);

    // Normalize angle difference
    let mut angle_diff = angle - theta_start;
    angle_diff = angle_diff - (angle_diff / TAU).round() * TAU;

    // Arc span
    let mut span = theta_end - theta_start;
    span = span - (span / TAU).round() * TAU;
    if span <= 0.0 {
        span += TAU;
    }

    // Normalize to positive
    if angle_diff < 0.0 {
        angle_diff += TAU;
    }

    let in_arc = angle_diff <= span;
    let half_thick = thickness * 0.5;

    if in_arc {
        // Distance to inner/outer radius
        (r - radius).abs() - half_thick
    } else {
        // Distance to arc endpoints
        let p1 = Vec2::new(theta_start.cos(), theta_start.sin()) * radius;
        let p2 = Vec2::new(theta_end.cos(), theta_end.sin()) * radius;
        let d1 = (p - p1).length() - half_thick;
        let d2 = (p - p2).length() - half_thick;
        d1.min(d2)
    }
}

/// Signed distance to arena outer wall
#[inline]
pub fn sd_arena_wall(p: Vec2, arena_radius: f32) -> f32 {
    p.length() - arena_radius
}

/// Compute SDF gradient (surface normal) using central differences
pub fn sdf_gradient<F>(p: Vec2, sdf: F) -> Vec2
where
    F: Fn(Vec2) -> f32,
{
    let eps = 0.5;
    let dx = sdf(p + Vec2::new(eps, 0.0)) - sdf(p - Vec2::new(eps, 0.0));
    let dy = sdf(p + Vec2::new(0.0, eps)) - sdf(p - Vec2::new(0.0, eps));
    Vec2::new(dx, dy).normalize_or_zero()
}

/// Result of SDF collision check
#[derive(Debug, Clone)]
pub struct SdfCollision {
    pub hit: bool,
    pub distance: f32,
    pub normal: Vec2,
    pub penetration: f32,
}

impl SdfCollision {
    pub fn miss() -> Self {
        Self {
            hit: false,
            distance: f32::MAX,
            normal: Vec2::ZERO,
            penetration: 0.0,
        }
    }
}

/// Check collision between ball and an SDF shape
pub fn check_sdf_collision<F>(ball_pos: Vec2, ball_radius: f32, sdf: F) -> SdfCollision
where
    F: Fn(Vec2) -> f32,
{
    let dist = sdf(ball_pos);

    if dist < ball_radius {
        let normal = sdf_gradient(ball_pos, &sdf);
        SdfCollision {
            hit: true,
            distance: dist,
            normal,
            penetration: ball_radius - dist,
        }
    } else {
        SdfCollision::miss()
    }
}

/// Reflect velocity off a surface with given normal
#[inline]
pub fn reflect(vel: Vec2, normal: Vec2) -> Vec2 {
    vel - 2.0 * vel.dot(normal) * normal
}

/// Raymarch along a path to find first collision point
/// Returns (hit, t, normal) where t is the fraction along the path
pub fn raymarch_collision<F>(
    start: Vec2,
    end: Vec2,
    ball_radius: f32,
    max_steps: usize,
    sdf: F,
) -> Option<(f32, Vec2)>
where
    F: Fn(Vec2) -> f32,
{
    let dir = end - start;
    let total_dist = dir.length();
    if total_dist < 0.001 {
        return None;
    }
    let dir_norm = dir / total_dist;

    let mut t = 0.0;

    for _ in 0..max_steps {
        let p = start + dir_norm * t;
        let d = sdf(p);

        if d < ball_radius {
            // Hit! Compute normal
            let normal = sdf_gradient(p, &sdf);
            return Some((t / total_dist, normal));
        }

        // Step by distance to surface (sphere tracing)
        let step = (d - ball_radius * 0.5).max(0.5);
        t += step;

        if t >= total_dist {
            break;
        }
    }

    None
}
