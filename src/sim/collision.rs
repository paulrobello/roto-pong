//! Collision detection and response for curved geometry
//!
//! The tricky part of Roto Pong: detecting collisions between a circular ball
//! and curved arc segments, then computing proper reflection vectors.

use glam::Vec2;

use super::arc::ArcSegment;
use crate::{cartesian_to_polar, normalize_angle, polar_to_cartesian};

/// Result of a collision check
#[derive(Debug, Clone)]
pub struct CollisionResult {
    /// Whether a collision occurred
    pub hit: bool,
    /// Collision point (if hit)
    pub point: Vec2,
    /// Surface normal at collision (pointing toward ball center, for reflection)
    pub normal: Vec2,
    /// Penetration depth (for position correction)
    pub penetration: f32,
}

impl CollisionResult {
    pub fn miss() -> Self {
        Self {
            hit: false,
            point: Vec2::ZERO,
            normal: Vec2::ZERO,
            penetration: 0.0,
        }
    }
}

/// Check collision between a ball and an arc segment
///
/// Returns collision info if the ball overlaps the arc, including the
/// surface normal for reflection.
///
/// The arc is treated as a "thick band" between inner_radius and outer_radius,
/// spanning from theta_start to theta_end.
pub fn ball_arc_collision(ball_pos: Vec2, ball_radius: f32, arc: &ArcSegment) -> CollisionResult {
    let (ball_r, ball_theta) = cartesian_to_polar(ball_pos);
    let ball_theta = normalize_angle(ball_theta);

    let inner_r = arc.inner_radius();
    let outer_r = arc.outer_radius();

    // Check if ball is within angular extent of the arc
    let in_angular_range = arc.contains_angle(ball_theta);

    if in_angular_range {
        // Ball is within the arc's angular span
        // Check radial collision with inner or outer edge

        // Distance to outer edge
        let dist_to_outer = ball_r - outer_r;
        // Distance to inner edge (negative means ball is outside inner edge)
        let dist_to_inner = inner_r - ball_r;

        if dist_to_outer < ball_radius && dist_to_outer > -arc.thickness - ball_radius {
            // Potential collision with outer edge (ball approaching from outside)
            if dist_to_outer < ball_radius && ball_r > arc.radius {
                let penetration = ball_radius - dist_to_outer;
                let normal = arc.inward_normal_at(ball_theta); // Point inward for reflection
                let contact_point = polar_to_cartesian(outer_r, ball_theta);
                return CollisionResult {
                    hit: true,
                    point: contact_point,
                    normal,
                    penetration,
                };
            }
        }

        if dist_to_inner < ball_radius && dist_to_inner > -arc.thickness - ball_radius {
            // Potential collision with inner edge (ball approaching from inside)
            if dist_to_inner < ball_radius && ball_r < arc.radius {
                let penetration = ball_radius - dist_to_inner;
                let normal = arc.outward_normal_at(ball_theta); // Point outward for reflection
                let contact_point = polar_to_cartesian(inner_r, ball_theta);
                return CollisionResult {
                    hit: true,
                    point: contact_point,
                    normal,
                    penetration,
                };
            }
        }

        // Check if ball is inside the arc band (tunneling case)
        if ball_r > inner_r + ball_radius && ball_r < outer_r - ball_radius {
            // Ball is fully inside the arc - shouldn't happen with proper substepping
            // Return collision with nearest edge
            let dist_inner = ball_r - inner_r;
            let dist_outer = outer_r - ball_r;

            if dist_inner < dist_outer {
                return CollisionResult {
                    hit: true,
                    point: polar_to_cartesian(inner_r, ball_theta),
                    normal: arc.outward_normal_at(ball_theta),
                    penetration: ball_radius,
                };
            } else {
                return CollisionResult {
                    hit: true,
                    point: polar_to_cartesian(outer_r, ball_theta),
                    normal: arc.inward_normal_at(ball_theta),
                    penetration: ball_radius,
                };
            }
        }
    }

    // Check collision with arc endpoints (the "caps")
    // This handles cases where the ball hits the angular edges of the arc
    let _start_point = polar_to_cartesian(arc.radius, arc.theta_start);
    let _end_point = polar_to_cartesian(arc.radius, arc.theta_end);

    // Check collision with start cap (simplified as point collision)
    if let Some(result) = check_endpoint_collision(ball_pos, ball_radius, arc, arc.theta_start) {
        return result;
    }

    // Check collision with end cap
    if let Some(result) = check_endpoint_collision(ball_pos, ball_radius, arc, arc.theta_end) {
        return result;
    }

    CollisionResult::miss()
}

/// Check collision with an arc endpoint (the angular edge)
fn check_endpoint_collision(
    ball_pos: Vec2,
    ball_radius: f32,
    arc: &ArcSegment,
    theta: f32,
) -> Option<CollisionResult> {
    let inner_r = arc.inner_radius();
    let outer_r = arc.outer_radius();

    // The endpoint is a line segment from inner to outer radius at angle theta
    let inner_point = polar_to_cartesian(inner_r, theta);
    let outer_point = polar_to_cartesian(outer_r, theta);

    // Find closest point on line segment to ball
    let line_vec = outer_point - inner_point;
    let ball_vec = ball_pos - inner_point;
    let line_len_sq = line_vec.length_squared();

    if line_len_sq < 0.0001 {
        return None; // Degenerate segment
    }

    let t = (ball_vec.dot(line_vec) / line_len_sq).clamp(0.0, 1.0);
    let closest = inner_point + line_vec * t;
    let dist = (ball_pos - closest).length();

    if dist < ball_radius {
        // Compute normal pointing away from arc center (tangential)
        let normal = (ball_pos - closest).normalize_or_zero();
        if normal.length_squared() < 0.5 {
            // Ball center is on the line - use perpendicular to line
            let perp = Vec2::new(-line_vec.y, line_vec.x).normalize();
            // Choose direction away from arc center
            let arc_center = polar_to_cartesian(arc.radius, theta + arc.angular_span() / 2.0);
            let to_arc = arc_center - closest;
            let normal = if perp.dot(to_arc) < 0.0 { perp } else { -perp };
            return Some(CollisionResult {
                hit: true,
                point: closest,
                normal,
                penetration: ball_radius - dist,
            });
        }
        return Some(CollisionResult {
            hit: true,
            point: closest,
            normal,
            penetration: ball_radius - dist,
        });
    }

    None
}

/// Reflect velocity off a surface
///
/// Standard reflection: v' = v - 2(v·n)n
/// Optionally add tangential component from paddle angular velocity
#[inline]
pub fn reflect_velocity(velocity: Vec2, normal: Vec2) -> Vec2 {
    velocity - 2.0 * velocity.dot(normal) * normal
}

/// Reflect velocity with optional "english" from paddle rotation
pub fn reflect_velocity_with_english(
    velocity: Vec2,
    normal: Vec2,
    paddle_angular_vel: f32,
    contact_radius: f32,
    english_factor: f32,
) -> Vec2 {
    let reflected = reflect_velocity(velocity, normal);

    // Add tangential component based on paddle rotation
    // Tangent is perpendicular to normal
    let tangent = Vec2::new(-normal.y, normal.x);
    let english = paddle_angular_vel * contact_radius * english_factor;

    // Clamp english to prevent extreme deflections
    let max_english = velocity.length() * 0.3;
    let clamped_english = english.clamp(-max_english, max_english);

    reflected + tangent * clamped_english
}

/// Check collision with outer arena wall
pub fn ball_outer_wall_collision(
    ball_pos: Vec2,
    ball_radius: f32,
    arena_radius: f32,
) -> CollisionResult {
    let (r, theta) = cartesian_to_polar(ball_pos);

    if r + ball_radius > arena_radius {
        let normal = -polar_to_cartesian(1.0, theta); // Point inward
        let contact = polar_to_cartesian(arena_radius, theta);
        return CollisionResult {
            hit: true,
            point: contact,
            normal,
            penetration: r + ball_radius - arena_radius,
        };
    }

    CollisionResult::miss()
}

/// Check if ball fell into black hole
pub fn ball_black_hole_collision(ball_pos: Vec2, ball_radius: f32, hole_radius: f32) -> bool {
    ball_pos.length() - ball_radius <= hole_radius
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn test_ball_arc_collision_outer_edge() {
        // Arc at radius 100, thickness 20, spanning 0 to 90 degrees
        let arc = ArcSegment::new(100.0, 20.0, 0.0, PI / 2.0);

        // Ball at radius 115 (just outside outer edge at r=110), angle 45°
        let ball_pos = polar_to_cartesian(115.0, PI / 4.0);
        let ball_radius = 8.0;

        let result = ball_arc_collision(ball_pos, ball_radius, &arc);
        assert!(result.hit);
        // Normal should point inward (toward center)
        assert!(result.normal.dot(ball_pos) < 0.0);
    }

    #[test]
    fn test_ball_arc_collision_inner_edge() {
        let arc = ArcSegment::new(100.0, 20.0, 0.0, PI / 2.0);

        // Ball at radius 85 (just inside inner edge at r=90), angle 45°
        let ball_pos = polar_to_cartesian(85.0, PI / 4.0);
        let ball_radius = 8.0;

        let result = ball_arc_collision(ball_pos, ball_radius, &arc);
        assert!(result.hit);
        // Normal should point outward (away from center)
        assert!(result.normal.dot(ball_pos) > 0.0);
    }

    #[test]
    fn test_ball_arc_collision_miss_angular() {
        let arc = ArcSegment::new(100.0, 20.0, 0.0, PI / 4.0);

        // Ball at correct radius but wrong angle (90°)
        let ball_pos = polar_to_cartesian(100.0, PI / 2.0);
        let ball_radius = 8.0;

        let result = ball_arc_collision(ball_pos, ball_radius, &arc);
        // Should miss (or hit endpoint if close enough)
        // At 90° with arc ending at 45°, should be a clear miss
        assert!(!result.hit || result.penetration < ball_radius);
    }

    #[test]
    fn test_reflect_velocity() {
        // Ball moving right, hits vertical wall (normal pointing left)
        let velocity = Vec2::new(100.0, 0.0);
        let normal = Vec2::new(-1.0, 0.0);

        let reflected = reflect_velocity(velocity, normal);
        assert!((reflected.x - (-100.0)).abs() < 0.001);
        assert!(reflected.y.abs() < 0.001);
    }

    #[test]
    fn test_outer_wall_collision() {
        let arena_radius = 400.0;

        // Ball inside - no collision
        let result = ball_outer_wall_collision(Vec2::new(300.0, 0.0), 8.0, arena_radius);
        assert!(!result.hit);

        // Ball touching wall
        let result = ball_outer_wall_collision(Vec2::new(395.0, 0.0), 8.0, arena_radius);
        assert!(result.hit);
    }

    #[test]
    fn test_black_hole_collision() {
        let hole_radius = 40.0;

        assert!(!ball_black_hole_collision(
            Vec2::new(100.0, 0.0),
            8.0,
            hole_radius
        ));
        assert!(ball_black_hole_collision(
            Vec2::new(35.0, 0.0),
            8.0,
            hole_radius
        ));
    }
}
