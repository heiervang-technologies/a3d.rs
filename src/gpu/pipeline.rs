use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use super::GpuContext;
use crate::model::Mesh;
use crate::render::Framebuffer;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct GpuUniforms {
    pub width: u32,
    pub height: u32,
    pub cos_az: f32,
    pub sin_az: f32,
    pub cos_al: f32,
    pub sin_al: f32,
    pub zoom: f32,
    pub logical_w: f32,
    pub logical_h: f32,
    pub dx: f32,
    pub dy: f32,
    pub has_fg_override: u32,
    pub light_dir: [f32; 3],
    pub _pad0: f32,
    pub fg_override: [f32; 3],
    pub _pad1: f32,
}

/// GPU compute pipeline for rasterization.
pub struct RasterPipeline {
    vertex_transform_pipeline: wgpu::ComputePipeline,
    rasterize_depth_pipeline: wgpu::ComputePipeline,
    rasterize_shade_pipeline: wgpu::ComputePipeline,

    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    num_vertices: u32,
    num_triangles: u32,

    transformed_buffer: wgpu::Buffer,
    depth_buffer: wgpu::Buffer,
    output_char_buffer: wgpu::Buffer,
    output_luminance_buffer: wgpu::Buffer,
    output_color_buffer: wgpu::Buffer,

    readback_char_buffer: wgpu::Buffer,
    readback_luminance_buffer: wgpu::Buffer,
    readback_color_buffer: wgpu::Buffer,

    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,

    fb_width: u32,
    fb_height: u32,
}

impl RasterPipeline {
    pub fn new(ctx: &GpuContext, mesh: &Mesh, width: u32, height: u32) -> Self {
        let device = &ctx.device;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("raster shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("raster.wgsl").into()),
        });

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("raster bind group layout"),
            entries: &[
                // 0: uniforms
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // 1: vertices (read-only storage)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // 2: indices (read-only storage)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // 3: transformed (read-write storage)
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // 4: depth_buf (read-write storage)
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // 5: output_char (read-write storage)
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // 6: output_luminance (read-write storage)
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // 7: output_color (read-write storage)
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("raster pipeline layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let create_pipeline = |entry: &str, label: &str| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: Some(entry),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache: None,
            })
        };

        let vertex_transform_pipeline = create_pipeline("vertex_transform", "vertex transform");
        let rasterize_depth_pipeline = create_pipeline("rasterize_depth", "rasterize depth");
        let rasterize_shade_pipeline = create_pipeline("rasterize_shade", "rasterize shade");

        // Upload mesh data. wgpu rejects zero-sized storage buffers, so an empty
        // mesh (e.g. a model file with no geometry) would otherwise panic the
        // pipeline. The dispatch counts below are 0 for an empty mesh, so no work
        // runs regardless; a 4-byte stub just keeps the bindings valid and yields
        // a blank frame instead of a crash.
        const EMPTY_STORAGE_STUB: &[u8] = &[0u8; 4];
        let vertex_bytes = bytemuck::cast_slice(&mesh.vertices);
        let index_bytes = bytemuck::cast_slice(&mesh.indices);
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("vertex buffer"),
            contents: if vertex_bytes.is_empty() {
                EMPTY_STORAGE_STUB
            } else {
                vertex_bytes
            },
            usage: wgpu::BufferUsages::STORAGE,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("index buffer"),
            contents: if index_bytes.is_empty() {
                EMPTY_STORAGE_STUB
            } else {
                index_bytes
            },
            usage: wgpu::BufferUsages::STORAGE,
        });

        let num_vertices = mesh.vertices.len() as u32;
        let num_triangles = (mesh.indices.len() / 3) as u32;

        // Uniform buffer
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uniform buffer"),
            size: std::mem::size_of::<GpuUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create framebuffer-sized buffers
        let pixel_count = (width * height) as u64;

        let transformed_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("transformed buffer"),
            size: (num_vertices as u64 * 16).max(16), // vec4<f32> per vertex (min 1 for empty meshes)
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        let depth_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("depth buffer"),
            size: pixel_count * 8, // atomic<u64> per pixel: depth<<32 | tri index
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let output_char_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("output char buffer"),
            size: pixel_count * 4, // u32 per pixel
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let output_luminance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("output luminance buffer"),
            size: pixel_count * 4, // f32 per pixel
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let output_color_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("output color buffer"),
            size: pixel_count * 12, // 3x f32 per pixel
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Readback staging buffers
        let readback_char_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback char"),
            size: pixel_count * 4,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let readback_luminance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback luminance"),
            size: pixel_count * 4,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let readback_color_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback color"),
            size: pixel_count * 12,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Bind group
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("raster bind group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: vertex_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: index_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: transformed_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: depth_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: output_char_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: output_luminance_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: output_color_buffer.as_entire_binding(),
                },
            ],
        });

        Self {
            vertex_transform_pipeline,
            rasterize_depth_pipeline,
            rasterize_shade_pipeline,
            vertex_buffer,
            index_buffer,
            num_vertices,
            num_triangles,
            transformed_buffer,
            depth_buffer,
            output_char_buffer,
            output_luminance_buffer,
            output_color_buffer,
            readback_char_buffer,
            readback_luminance_buffer,
            readback_color_buffer,
            uniform_buffer,
            bind_group,
            fb_width: width,
            fb_height: height,
        }
    }

    /// Resize framebuffer-dependent buffers. Must be called when terminal size changes.
    pub fn resize(&mut self, ctx: &GpuContext, width: u32, height: u32) {
        if width == self.fb_width && height == self.fb_height {
            return;
        }
        let device = &ctx.device;
        let pixel_count = (width * height) as u64;

        self.depth_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("depth buffer"),
            size: pixel_count * 8, // atomic<u64> per pixel
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.output_char_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("output char buffer"),
            size: pixel_count * 4,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.output_luminance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("output luminance buffer"),
            size: pixel_count * 4,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.output_color_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("output color buffer"),
            size: pixel_count * 12,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.readback_char_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback char"),
            size: pixel_count * 4,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.readback_luminance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback luminance"),
            size: pixel_count * 4,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.readback_color_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback color"),
            size: pixel_count * 12,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Rebuild bind group with new buffers
        let bind_group_layout = self.vertex_transform_pipeline.get_bind_group_layout(0);
        self.bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("raster bind group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.vertex_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.index_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.transformed_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.depth_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: self.output_char_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: self.output_luminance_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: self.output_color_buffer.as_entire_binding(),
                },
            ],
        });

        self.fb_width = width;
        self.fb_height = height;
    }

    /// Render a frame on the GPU and read results back into the framebuffer.
    pub fn render(&self, ctx: &GpuContext, fb: &mut Framebuffer, uniforms: &GpuUniforms) {
        let device = &ctx.device;
        let queue = &ctx.queue;
        let pixel_count = (self.fb_width * self.fb_height) as usize;

        // Write uniforms
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(uniforms));

        // Clear depth buffer to u64::MAX (no triangle has index 0xFFFFFFFF, so
        // unwritten pixels never match a real triangle in the shade pass).
        let clear_depth: Vec<u64> = vec![u64::MAX; pixel_count];
        queue.write_buffer(&self.depth_buffer, 0, bytemuck::cast_slice(&clear_depth));

        // Clear output char to space (32)
        let clear_char: Vec<u32> = vec![32u32; pixel_count];
        queue.write_buffer(
            &self.output_char_buffer,
            0,
            bytemuck::cast_slice(&clear_char),
        );

        // Clear luminance to 0
        let clear_lum: Vec<f32> = vec![0.0f32; pixel_count];
        queue.write_buffer(
            &self.output_luminance_buffer,
            0,
            bytemuck::cast_slice(&clear_lum),
        );

        // Clear color to 0
        let clear_color: Vec<f32> = vec![0.0f32; pixel_count * 3];
        queue.write_buffer(
            &self.output_color_buffer,
            0,
            bytemuck::cast_slice(&clear_color),
        );

        // Encode compute dispatches
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("raster encoder"),
        });

        // Pass 1: vertex transform
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("vertex transform"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.vertex_transform_pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.dispatch_workgroups(self.num_vertices.div_ceil(256), 1, 1);
        }

        // Pass 2a: rasterize depth
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("rasterize depth"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.rasterize_depth_pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.dispatch_workgroups(self.num_triangles.div_ceil(64), 1, 1);
        }

        // Pass 2b: rasterize shade
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("rasterize shade"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.rasterize_shade_pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.dispatch_workgroups(self.num_triangles.div_ceil(64), 1, 1);
        }

        // Copy output to readback buffers
        let char_size = (pixel_count * 4) as u64;
        let lum_size = (pixel_count * 4) as u64;
        let color_size = (pixel_count * 12) as u64;
        encoder.copy_buffer_to_buffer(
            &self.output_char_buffer,
            0,
            &self.readback_char_buffer,
            0,
            char_size,
        );
        encoder.copy_buffer_to_buffer(
            &self.output_luminance_buffer,
            0,
            &self.readback_luminance_buffer,
            0,
            lum_size,
        );
        encoder.copy_buffer_to_buffer(
            &self.output_color_buffer,
            0,
            &self.readback_color_buffer,
            0,
            color_size,
        );

        queue.submit(std::iter::once(encoder.finish()));

        // Read back all three buffers with a SINGLE device sync. Mapping each
        // buffer and polling separately (the old read_buffer helper) blocked on
        // three GPU round-trips per frame; here we kick off all three maps, then
        // poll(Wait) once to resolve them together.
        let char_slice = self.readback_char_buffer.slice(0..char_size);
        let lum_slice = self.readback_luminance_buffer.slice(0..lum_size);
        let color_slice = self.readback_color_buffer.slice(0..color_size);

        let (tx_c, rx_c) = std::sync::mpsc::channel();
        let (tx_l, rx_l) = std::sync::mpsc::channel();
        let (tx_k, rx_k) = std::sync::mpsc::channel();
        char_slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx_c.send(r);
        });
        lum_slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx_l.send(r);
        });
        color_slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx_k.send(r);
        });
        device.poll(wgpu::PollType::Wait).unwrap();
        rx_c.recv().unwrap().unwrap();
        rx_l.recv().unwrap().unwrap();
        rx_k.recv().unwrap().unwrap();

        let char_data = char_slice.get_mapped_range().to_vec();
        let lum_data = lum_slice.get_mapped_range().to_vec();
        let color_data = color_slice.get_mapped_range().to_vec();
        self.readback_char_buffer.unmap();
        self.readback_luminance_buffer.unmap();
        self.readback_color_buffer.unmap();

        // Populate framebuffer
        let chars: &[u32] = bytemuck::cast_slice(&char_data);
        let luminances: &[f32] = bytemuck::cast_slice(&lum_data);
        let colors: &[f32] = bytemuck::cast_slice(&color_data);

        fb.clear();
        for i in 0..pixel_count {
            fb.chars[i] = char::from(chars[i] as u8);
            fb.luminances[i] = luminances[i];
            if i * 3 + 2 < colors.len() {
                fb.colors[i] = [colors[i * 3], colors[i * 3 + 1], colors[i * 3 + 2]];
            }
        }
    }
}
