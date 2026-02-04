//! WebGPU render pipeline setup

use wgpu::util::DeviceExt;

use super::vertex::Vertex;

/// Main render state
pub struct RenderState {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub pipeline: wgpu::RenderPipeline,
    pub vertex_buffer: wgpu::Buffer,
    pub vertex_count: u32,
    /// Viewport size in pixels
    pub size: (u32, u32),
    /// Arena radius for coordinate mapping
    pub arena_radius: f32,
}

impl RenderState {
    pub async fn new(
        surface: wgpu::Surface<'static>,
        adapter: &wgpu::Adapter,
        width: u32,
        height: u32,
        arena_radius: f32,
    ) -> Self {
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("roto-pong-device"),
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

        // Create shader module
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        // Create pipeline
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pipeline_layout"),
            bind_group_layouts: &[],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        // Create initial vertex buffer (will be updated each frame)
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("vertex_buffer"),
            contents: bytemuck::cast_slice(&[Vertex::new(0.0, 0.0, [1.0; 4])]),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        Self {
            surface,
            device,
            queue,
            config,
            pipeline,
            vertex_buffer,
            vertex_count: 0,
            size: (width, height),
            arena_radius,
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

    /// Convert game coordinates to normalized device coordinates
    /// Game uses arena-centered coords; NDC is -1 to 1
    pub fn game_to_ndc(&self, x: f32, y: f32) -> (f32, f32) {
        let (w, h) = self.size;
        let aspect = w as f32 / h as f32;
        let scale = 1.0 / (self.arena_radius * 1.1); // Add 10% margin

        if aspect > 1.0 {
            // Wider than tall
            (x * scale / aspect, y * scale)
        } else {
            // Taller than wide
            (x * scale, y * scale * aspect)
        }
    }

    /// Upload vertices and render
    pub fn render(&mut self, vertices: &[Vertex]) -> Result<(), wgpu::SurfaceError> {
        // Convert vertices to NDC
        let ndc_vertices: Vec<Vertex> = vertices
            .iter()
            .map(|v| {
                let (x, y) = self.game_to_ndc(v.position[0], v.position[1]);
                Vertex::new(x, y, v.color)
            })
            .collect();

        // Recreate buffer if needed (simple approach; could optimize)
        self.vertex_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("vertex_buffer"),
                contents: bytemuck::cast_slice(&ndc_vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
        self.vertex_count = ndc_vertices.len() as u32;

        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render_encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.02,
                            g: 0.02,
                            b: 0.05,
                            a: 1.0,
                        }),
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
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.draw(0..self.vertex_count, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}
