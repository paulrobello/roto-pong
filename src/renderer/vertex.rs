//! Vertex types for 2D rendering

use bytemuck::{Pod, Zeroable};

/// Simple 2D vertex with position and color
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 2],
    pub color: [f32; 4],
}

impl Vertex {
    pub const fn new(x: f32, y: f32, color: [f32; 4]) -> Self {
        Self {
            position: [x, y],
            color,
        }
    }

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

/// Colors for game elements
pub mod colors {
    pub const ARENA_WALL: [f32; 4] = [0.3, 0.3, 0.4, 1.0];
    pub const BLACK_HOLE: [f32; 4] = [0.05, 0.0, 0.1, 1.0];
    pub const BLACK_HOLE_RING: [f32; 4] = [0.6, 0.2, 0.8, 1.0];
    pub const PADDLE: [f32; 4] = [0.2, 0.8, 0.4, 1.0];
    pub const BALL: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
    pub const BLOCK_GLASS: [f32; 4] = [0.4, 0.7, 1.0, 1.0];
    pub const BLOCK_ARMORED: [f32; 4] = [0.7, 0.7, 0.8, 1.0];
    pub const BLOCK_EXPLOSIVE: [f32; 4] = [1.0, 0.4, 0.2, 1.0];
    pub const BLOCK_INVINCIBLE: [f32; 4] = [0.9, 0.85, 0.3, 1.0]; // Gold/yellow
    pub const BACKGROUND: [f32; 4] = [0.02, 0.02, 0.05, 1.0];
}
