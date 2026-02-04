//! Arc segment geometry for curved blocks and paddle
//!
//! In polar coordinates, an arc segment is defined by:
//! - radius: distance from center
//! - thickness: radial extent (inner = radius - thickness/2, outer = radius + thickness/2)
//! - theta_start, theta_end: angular extent

use glam::Vec2;
use serde::{Deserialize, Serialize};

use crate::{normalize_angle, polar_to_cartesian};

/// A thickened arc segment in polar space
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArcSegment {
    /// Centerline radius from arena center
    pub radius: f32,
    /// Radial thickness (extends radius ± thickness/2)
    pub thickness: f32,
    /// Start angle (radians, normalized to [-π, π))
    pub theta_start: f32,
    /// End angle (radians, normalized to [-π, π))
    pub theta_end: f32,
}

impl ArcSegment {
    pub fn new(radius: f32, thickness: f32, theta_start: f32, theta_end: f32) -> Self {
        Self {
            radius,
            thickness,
            theta_start: normalize_angle(theta_start),
            theta_end: normalize_angle(theta_end),
        }
    }

    /// Inner radius of the arc band
    #[inline]
    pub fn inner_radius(&self) -> f32 {
        self.radius - self.thickness / 2.0
    }

    /// Outer radius of the arc band
    #[inline]
    pub fn outer_radius(&self) -> f32 {
        self.radius + self.thickness / 2.0
    }

    /// Angular span of the arc (handles wraparound)
    pub fn angular_span(&self) -> f32 {
        let mut span = self.theta_end - self.theta_start;
        if span < 0.0 {
            span += std::f32::consts::TAU;
        }
        span
    }

    /// Check if an angle is within the arc's angular extent
    pub fn contains_angle(&self, theta: f32) -> bool {
        let theta = normalize_angle(theta);
        let start = self.theta_start;
        let end = self.theta_end;

        if start <= end {
            // No wraparound
            theta >= start && theta <= end
        } else {
            // Wraparound case (e.g., start=170°, end=-170°)
            theta >= start || theta <= end
        }
    }

    /// Check if a point (in cartesian) is inside the arc segment
    pub fn contains_point(&self, point: Vec2) -> bool {
        let r = point.length();
        let theta = point.y.atan2(point.x);

        r >= self.inner_radius() && r <= self.outer_radius() && self.contains_angle(theta)
    }

    /// Get the center point of the arc (at centerline radius, mid-angle)
    pub fn center(&self) -> Vec2 {
        let mid_theta = self.theta_start + self.angular_span() / 2.0;
        polar_to_cartesian(self.radius, mid_theta)
    }

    /// Get the surface normal at a given angle (pointing outward from center)
    pub fn outward_normal_at(&self, theta: f32) -> Vec2 {
        Vec2::new(theta.cos(), theta.sin())
    }

    /// Get the surface normal pointing inward (toward arena center)
    pub fn inward_normal_at(&self, theta: f32) -> Vec2 {
        -self.outward_normal_at(theta)
    }

    /// Sample points along the outer edge (for rendering or debugging)
    pub fn sample_outer_edge(&self, num_points: usize) -> Vec<Vec2> {
        let span = self.angular_span();
        let outer_r = self.outer_radius();

        (0..num_points)
            .map(|i| {
                let t = i as f32 / (num_points - 1).max(1) as f32;
                let theta = self.theta_start + t * span;
                polar_to_cartesian(outer_r, theta)
            })
            .collect()
    }

    /// Sample points along the inner edge
    pub fn sample_inner_edge(&self, num_points: usize) -> Vec<Vec2> {
        let span = self.angular_span();
        let inner_r = self.inner_radius();

        (0..num_points)
            .map(|i| {
                let t = i as f32 / (num_points - 1).max(1) as f32;
                let theta = self.theta_start + t * span;
                polar_to_cartesian(inner_r, theta)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn test_arc_contains_angle_no_wrap() {
        let arc = ArcSegment::new(100.0, 10.0, 0.0, PI / 2.0);
        assert!(arc.contains_angle(0.1));
        assert!(arc.contains_angle(PI / 4.0));
        assert!(!arc.contains_angle(PI));
        assert!(!arc.contains_angle(-PI / 4.0));
    }

    #[test]
    fn test_arc_contains_angle_wraparound() {
        // Arc from 170° to -170° (wraps around ±180°)
        let arc = ArcSegment::new(100.0, 10.0, 170.0_f32.to_radians(), -170.0_f32.to_radians());
        assert!(arc.contains_angle(PI)); // 180°
        assert!(arc.contains_angle(-PI + 0.01)); // just past -180°
        assert!(!arc.contains_angle(0.0)); // 0° is outside
    }

    #[test]
    fn test_arc_contains_point() {
        let arc = ArcSegment::new(100.0, 20.0, 0.0, PI / 2.0);
        // Point at (95, 0) - inside radial band, at angle 0
        assert!(arc.contains_point(Vec2::new(95.0, 0.0)));
        // Point at (100, 100) normalized - angle ~45°, radius ~141 - outside radial band
        assert!(!arc.contains_point(Vec2::new(100.0, 100.0)));
        // Point at angle 45°, radius 100
        let p = polar_to_cartesian(100.0, PI / 4.0);
        assert!(arc.contains_point(p));
    }

    #[test]
    fn test_angular_span() {
        let arc = ArcSegment::new(100.0, 10.0, 0.0, PI / 2.0);
        assert!((arc.angular_span() - PI / 2.0).abs() < 0.001);

        // Wraparound case
        let arc2 = ArcSegment::new(100.0, 10.0, PI * 0.9, -PI * 0.9);
        assert!((arc2.angular_span() - 0.2 * PI).abs() < 0.001);
    }
}
