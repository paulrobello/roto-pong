//! SDF-based WebGPU render pipeline
//!
//! Renders the entire scene in fragment shader using signed distance fields.

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use crate::sim::GameState;
use crate::consts::*;

/// Maximum number of balls supported
const MAX_BALLS: usize = 8;
/// Maximum number of trail points
const MAX_TRAIL: usize = 32;
/// Maximum number of blocks
const MAX_BLOCKS: usize = 256;
/// Maximum number of particles
const MAX_PARTICLES: usize = 256;

// ============================================================================
// GPU DATA STRUCTURES (must match shader)
// ============================================================================

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Globals {
    resolution: [f32; 2],   // offset 0
    time: f32,              // offset 8
    arena_radius: f32,      // offset 12
    black_hole_radius: f32, // offset 16
    ball_count: u32,        // offset 20
    block_count: u32,       // offset 24
    trail_count: u32,       // offset 28
    particle_count: u32,    // offset 32
    _pad1: u32,             // offset 36 - align camera_pos to 8 bytes
    camera_pos: [f32; 2],   // offset 40 (8-byte aligned for WGSL vec2)
    camera_zoom: f32,       // offset 48
    screen_shake: f32,      // offset 52 - screen shake intensity (0-1)
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct PaddleUniform {
    theta: f32,
    arc_width: f32,
    radius: f32,
    thickness: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct BallData {
    pos: [f32; 2],
    radius: f32,
    speed: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct BlockData {
    theta_start: f32,
    theta_end: f32,
    radius: f32,
    thickness: f32,
    kind: u32,
    wobble: f32,
    _pad2: f32,
    _pad3: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct TrailPoint {
    pos: [f32; 2],
    speed: f32,
    alpha: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct ParticleData {
    pos: [f32; 2],
    size: f32,
    life: f32,
    color: u32,
    _pad1: u32,
    _pad2: u32,
    _pad3: u32,
}

// ============================================================================
// SDF RENDER STATE
// ============================================================================

pub struct SdfRenderState {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub pipeline: wgpu::RenderPipeline,
    
    // Uniform buffers
    globals_buffer: wgpu::Buffer,
    paddle_buffer: wgpu::Buffer,
    balls_buffer: wgpu::Buffer,
    blocks_buffer: wgpu::Buffer,
    trail_buffer: wgpu::Buffer,
    particles_buffer: wgpu::Buffer,
    
    bind_group: wgpu::BindGroup,
    
    pub size: (u32, u32),
    start_time: f64,
    
    // Camera state
    camera_pos: [f32; 2],
    camera_zoom: f32,
}

impl SdfRenderState {
    pub async fn new(
        surface: wgpu::Surface<'static>,
        adapter: &wgpu::Adapter,
        width: u32,
        height: u32,
    ) -> Self {
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("sdf-device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_webgl2_defaults(),
                memory_hints: Default::default(),
                trace: Default::default(),
                experimental_features: Default::default(),
            })
            .await
            .expect("Failed to create device");

        let surface_caps = surface.get_capabilities(adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        // Create shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sdf_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("sdf_shader.wgsl").into()),
        });

        // Create buffers
        let globals_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("globals"),
            contents: bytemuck::bytes_of(&Globals {
                resolution: [width as f32, height as f32],
                time: 0.0,
                arena_radius: ARENA_OUTER_RADIUS,
                black_hole_radius: BLACK_HOLE_RADIUS,
                ball_count: 0,
                block_count: 0,
                trail_count: 0,
                particle_count: 0,
                _pad1: 0,
                camera_pos: [0.0, 0.0],
                camera_zoom: 1.0,
                screen_shake: 0.0,
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let paddle_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("paddle"),
            contents: bytemuck::bytes_of(&PaddleUniform {
                theta: 0.0,
                arc_width: PADDLE_ARC_WIDTH,
                radius: PADDLE_RADIUS,
                thickness: PADDLE_THICKNESS,
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let balls_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("balls"),
            size: (std::mem::size_of::<BallData>() * MAX_BALLS) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let blocks_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("blocks"),
            size: (std::mem::size_of::<BlockData>() * MAX_BLOCKS) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let trail_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("trail"),
            size: (std::mem::size_of::<TrailPoint>() * MAX_TRAIL) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let particles_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("particles"),
            size: (std::mem::size_of::<ParticleData>() * MAX_PARTICLES) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sdf_bind_group_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sdf_bind_group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: globals_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: paddle_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: balls_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: blocks_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: trail_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: particles_buffer.as_entire_binding(),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sdf_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sdf_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[], // No vertex buffers - fullscreen triangle
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        Self {
            surface,
            device,
            queue,
            config,
            pipeline,
            globals_buffer,
            paddle_buffer,
            balls_buffer,
            blocks_buffer,
            trail_buffer,
            particles_buffer,
            bind_group,
            size: (width, height),
            start_time: 0.0,
            camera_pos: [0.0, 0.0],
            camera_zoom: 1.0,
        }
    }

    pub fn resize(&mut self, new_width: u32, new_height: u32) {
        if new_width > 0 && new_height > 0 {
            self.size = (new_width, new_height);
            self.config.width = new_width;
            self.config.height = new_height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    pub fn set_start_time(&mut self, time: f64) {
        self.start_time = time;
    }

    /// Update GPU buffers from game state and render
    pub fn render(&mut self, state: &GameState, time: f64) -> Result<(), wgpu::SurfaceError> {
        // time is ms since page load from requestAnimationFrame, convert to seconds
        let elapsed = (time / 1000.0) as f32;
        
        let ball_count = state.balls.len().min(MAX_BALLS) as u32;
        let block_count = state.blocks.len().min(MAX_BLOCKS) as u32;
        let trail_count = state.balls.iter().map(|b| b.trail.len()).sum::<usize>().min(MAX_TRAIL) as u32;
        let particle_count = state.particles.len().min(MAX_PARTICLES) as u32;

        // Camera follow - enabled when arena is larger than base viewport
        // Base viewport shows ~440px radius (ARENA_OUTER_RADIUS * 1.1)
        // Enable camera when arena extends beyond this
        let base_viewport = 440.0;
        let camera_enabled = ARENA_OUTER_RADIUS > base_viewport;
        
        if camera_enabled {
            let dt = 1.0 / 60.0;
            let pos_smooth = 3.0;
            let zoom_smooth = 1.0;
            
            // Find active ball position
            let target_pos = state.balls.iter()
                .find(|b| matches!(b.state, crate::sim::BallState::Free | crate::sim::BallState::Dying { .. }))
                .map(|b| [b.pos.x, b.pos.y])
                .unwrap_or([0.0, 0.0]);
            
            // Smooth interpolation toward ball
            self.camera_pos[0] += (target_pos[0] - self.camera_pos[0]) * pos_smooth * dt;
            self.camera_pos[1] += (target_pos[1] - self.camera_pos[1]) * pos_smooth * dt;
            
            // Zoom to keep both ball and black hole visible with 20% padding
            let viewport_base = base_viewport * 0.8;
            let camera_dist = (self.camera_pos[0].powi(2) + self.camera_pos[1].powi(2)).sqrt();
            
            let target_zoom = if camera_dist <= viewport_base {
                1.0
            } else {
                camera_dist / viewport_base
            };
            
            self.camera_zoom += (target_zoom - self.camera_zoom) * zoom_smooth * dt;
            self.camera_zoom = self.camera_zoom.clamp(1.0, 2.0);
        } else {
            // Arena fits on screen - keep centered
            self.camera_pos = [0.0, 0.0];
            self.camera_zoom = 1.0;
        }

        // Update globals
        let globals = Globals {
            resolution: [self.size.0 as f32, self.size.1 as f32],
            time: elapsed,
            arena_radius: ARENA_OUTER_RADIUS,
            black_hole_radius: BLACK_HOLE_RADIUS,
            ball_count,
            block_count,
            trail_count,
            particle_count,
            _pad1: 0,
            camera_pos: self.camera_pos,
            camera_zoom: self.camera_zoom,
            screen_shake: state.screen_shake,
        };
        self.queue.write_buffer(&self.globals_buffer, 0, bytemuck::bytes_of(&globals));

        // Update paddle
        let paddle = PaddleUniform {
            theta: state.paddle.theta,
            arc_width: state.paddle.arc_width,
            radius: PADDLE_RADIUS,
            thickness: PADDLE_THICKNESS,
        };
        self.queue.write_buffer(&self.paddle_buffer, 0, bytemuck::bytes_of(&paddle));

        // Update balls
        let mut balls_data = vec![BallData { pos: [0.0; 2], radius: 0.0, speed: 0.0 }; MAX_BALLS];
        for (i, ball) in state.balls.iter().take(MAX_BALLS).enumerate() {
            balls_data[i] = BallData {
                pos: [ball.pos.x, ball.pos.y],
                radius: ball.radius,
                speed: ball.vel.length(),
            };
        }
        self.queue.write_buffer(&self.balls_buffer, 0, bytemuck::cast_slice(&balls_data));

        // Update blocks
        let mut blocks_data = vec![BlockData {
            theta_start: 0.0,
            theta_end: 0.0,
            radius: 0.0,
            thickness: 0.0,
            kind: 0,
            wobble: 0.0,
            _pad2: 0.0,
            _pad3: 0.0,
        }; MAX_BLOCKS];
        for (i, block) in state.blocks.iter().take(MAX_BLOCKS).enumerate() {
            let kind = match block.kind {
                crate::sim::BlockKind::Glass => 0,
                crate::sim::BlockKind::Armored => 1,
                crate::sim::BlockKind::Explosive => 2,
                crate::sim::BlockKind::Invincible => 3,
                crate::sim::BlockKind::Portal { .. } => 4,
                crate::sim::BlockKind::Jello => 5,
                _ => 0,
            };
            blocks_data[i] = BlockData {
                theta_start: block.arc.theta_start,
                theta_end: block.arc.theta_end,
                radius: block.arc.radius,
                thickness: block.arc.thickness,
                kind,
                wobble: block.wobble,
                _pad2: 0.0,
                _pad3: 0.0,
            };
        }
        self.queue.write_buffer(&self.blocks_buffer, 0, bytemuck::cast_slice(&blocks_data));

        // Update trail
        let mut trail_data = vec![TrailPoint { pos: [0.0, 0.0], speed: 0.0, alpha: 0.0 }; MAX_TRAIL];
        let mut trail_idx = 0;
        for ball in &state.balls {
            for (i, point) in ball.trail.iter().enumerate() {
                if trail_idx >= MAX_TRAIL { break; }
                let alpha = 1.0 - (i as f32 / ball.trail.len().max(1) as f32);
                trail_data[trail_idx] = TrailPoint {
                    pos: [point.pos.x, point.pos.y],
                    speed: point.speed,
                    alpha,
                };
                trail_idx += 1;
            }
        }
        self.queue.write_buffer(&self.trail_buffer, 0, bytemuck::cast_slice(&trail_data));

        // Update particles
        let mut particles_data = vec![ParticleData { 
            pos: [0.0, 0.0], 
            size: 0.0, 
            life: 0.0, 
            color: 0,
            _pad1: 0,
            _pad2: 0,
            _pad3: 0,
        }; MAX_PARTICLES];
        for (i, particle) in state.particles.iter().take(MAX_PARTICLES).enumerate() {
            particles_data[i] = ParticleData {
                pos: [particle.pos.x, particle.pos.y],
                size: particle.size,
                life: particle.life,
                color: particle.color,
                _pad1: 0,
                _pad2: 0,
                _pad3: 0,
            };
        }
        self.queue.write_buffer(&self.particles_buffer, 0, bytemuck::cast_slice(&particles_data));

        // Render
        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("sdf_encoder"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("sdf_render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &self.bind_group, &[]);
            render_pass.draw(0..3, 0..1); // Fullscreen triangle
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}
