//! Shape generation for 2D primitives

use glam::Vec2;
use std::f32::consts::PI;

use super::vertex::Vertex;
use crate::consts::{BALL_MAX_SPEED, BALL_MIN_SPEED};
use crate::sim::ArcSegment;
use crate::sim::state::TrailPoint;

/// Interpolate color based on velocity (slow=blue, medium=green, fast=red/orange)
fn velocity_color(speed: f32, alpha: f32) -> [f32; 4] {
    // Normalize speed to 0-1 range
    let t = ((speed - BALL_MIN_SPEED) / (BALL_MAX_SPEED - BALL_MIN_SPEED)).clamp(0.0, 1.0);

    // Color gradient: blue (slow) -> cyan -> green -> yellow -> orange -> red (fast)
    let (r, g, b) = if t < 0.25 {
        // Blue to cyan
        let u = t / 0.25;
        (0.2, 0.4 + 0.4 * u, 1.0)
    } else if t < 0.5 {
        // Cyan to green
        let u = (t - 0.25) / 0.25;
        (0.2, 0.8, 1.0 - 0.6 * u)
    } else if t < 0.75 {
        // Green to yellow
        let u = (t - 0.5) / 0.25;
        (0.2 + 0.8 * u, 0.8, 0.4 - 0.2 * u)
    } else {
        // Yellow to red/orange
        let u = (t - 0.75) / 0.25;
        (1.0, 0.8 - 0.5 * u, 0.2)
    };

    [r, g, b, alpha]
}

/// Generate vertices for a ball trail with velocity-based colors
pub fn ball_trail(trail: &[TrailPoint], ball_radius: f32) -> Vec<Vertex> {
    if trail.len() < 2 {
        return Vec::new();
    }

    let mut vertices = Vec::with_capacity(trail.len() * 6);
    let trail_len = trail.len() as f32;

    for i in 0..trail.len() - 1 {
        let p1 = &trail[i];
        let p2 = &trail[i + 1];

        // Fade alpha and size along trail
        let t1 = i as f32 / trail_len;
        let t2 = (i + 1) as f32 / trail_len;

        let alpha1 = (1.0 - t1) * 0.8;
        let alpha2 = (1.0 - t2) * 0.8;

        let width1 = ball_radius * (1.0 - t1 * 0.7);
        let width2 = ball_radius * (1.0 - t2 * 0.7);

        let color1 = velocity_color(p1.speed, alpha1);
        let color2 = velocity_color(p2.speed, alpha2);

        // Direction from p1 to p2
        let dir = (p2.pos - p1.pos).normalize_or_zero();
        // Perpendicular for width
        let perp = Vec2::new(-dir.y, dir.x);

        // Quad corners
        let v1a = p1.pos + perp * width1;
        let v1b = p1.pos - perp * width1;
        let v2a = p2.pos + perp * width2;
        let v2b = p2.pos - perp * width2;

        // Two triangles
        vertices.push(Vertex::new(v1a.x, v1a.y, color1));
        vertices.push(Vertex::new(v1b.x, v1b.y, color1));
        vertices.push(Vertex::new(v2a.x, v2a.y, color2));

        vertices.push(Vertex::new(v2a.x, v2a.y, color2));
        vertices.push(Vertex::new(v1b.x, v1b.y, color1));
        vertices.push(Vertex::new(v2b.x, v2b.y, color2));
    }

    vertices
}

/// Generate vertices for a filled circle
pub fn circle(center: Vec2, radius: f32, color: [f32; 4], segments: u32) -> Vec<Vertex> {
    let mut vertices = Vec::with_capacity((segments * 3) as usize);

    for i in 0..segments {
        let theta1 = (i as f32 / segments as f32) * 2.0 * PI;
        let theta2 = ((i + 1) as f32 / segments as f32) * 2.0 * PI;

        // Triangle from center to edge
        vertices.push(Vertex::new(center.x, center.y, color));
        vertices.push(Vertex::new(
            center.x + radius * theta1.cos(),
            center.y + radius * theta1.sin(),
            color,
        ));
        vertices.push(Vertex::new(
            center.x + radius * theta2.cos(),
            center.y + radius * theta2.sin(),
            color,
        ));
    }

    vertices
}

/// Generate vertices for a ring (hollow circle)
pub fn ring(
    center: Vec2,
    inner_radius: f32,
    outer_radius: f32,
    color: [f32; 4],
    segments: u32,
) -> Vec<Vertex> {
    let mut vertices = Vec::with_capacity((segments * 6) as usize);

    for i in 0..segments {
        let theta1 = (i as f32 / segments as f32) * 2.0 * PI;
        let theta2 = ((i + 1) as f32 / segments as f32) * 2.0 * PI;

        let inner1 = Vec2::new(
            center.x + inner_radius * theta1.cos(),
            center.y + inner_radius * theta1.sin(),
        );
        let outer1 = Vec2::new(
            center.x + outer_radius * theta1.cos(),
            center.y + outer_radius * theta1.sin(),
        );
        let inner2 = Vec2::new(
            center.x + inner_radius * theta2.cos(),
            center.y + inner_radius * theta2.sin(),
        );
        let outer2 = Vec2::new(
            center.x + outer_radius * theta2.cos(),
            center.y + outer_radius * theta2.sin(),
        );

        // Two triangles per segment
        vertices.push(Vertex::new(inner1.x, inner1.y, color));
        vertices.push(Vertex::new(outer1.x, outer1.y, color));
        vertices.push(Vertex::new(inner2.x, inner2.y, color));

        vertices.push(Vertex::new(inner2.x, inner2.y, color));
        vertices.push(Vertex::new(outer1.x, outer1.y, color));
        vertices.push(Vertex::new(outer2.x, outer2.y, color));
    }

    vertices
}

/// Generate vertices for an arc segment (thick arc band)
pub fn arc_segment(arc: &ArcSegment, color: [f32; 4], segments_per_radian: f32) -> Vec<Vertex> {
    let span = arc.angular_span();
    let num_segments = ((span * segments_per_radian) as u32).max(4);
    let inner_r = arc.inner_radius();
    let outer_r = arc.outer_radius();

    let mut vertices = Vec::with_capacity((num_segments * 6) as usize);

    for i in 0..num_segments {
        let t1 = i as f32 / num_segments as f32;
        let t2 = (i + 1) as f32 / num_segments as f32;

        let theta1 = arc.theta_start + t1 * span;
        let theta2 = arc.theta_start + t2 * span;

        let inner1 = Vec2::new(inner_r * theta1.cos(), inner_r * theta1.sin());
        let outer1 = Vec2::new(outer_r * theta1.cos(), outer_r * theta1.sin());
        let inner2 = Vec2::new(inner_r * theta2.cos(), inner_r * theta2.sin());
        let outer2 = Vec2::new(outer_r * theta2.cos(), outer_r * theta2.sin());

        // Two triangles per segment
        vertices.push(Vertex::new(inner1.x, inner1.y, color));
        vertices.push(Vertex::new(outer1.x, outer1.y, color));
        vertices.push(Vertex::new(inner2.x, inner2.y, color));

        vertices.push(Vertex::new(inner2.x, inner2.y, color));
        vertices.push(Vertex::new(outer1.x, outer1.y, color));
        vertices.push(Vertex::new(outer2.x, outer2.y, color));
    }

    vertices
}
