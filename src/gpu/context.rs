use wgpu::{Adapter, Device, Instance, Queue};

/// Holds the wgpu device, queue, and adapter.
pub struct GpuContext {
    /// The logical GPU device used to create pipelines and buffers.
    pub device: Device,
    /// The command queue for submitting work to the device.
    pub queue: Queue,
    /// The physical adapter the device was created from.
    pub adapter: Adapter,
}

impl GpuContext {
    /// Initialize a GPU context, or return `None` if no suitable adapter is
    /// available. Requires 64-bit atomic min/max (`SHADER_INT64_ATOMIC_MIN_MAX`);
    /// adapters lacking it return `None` so the caller can fall back to the CPU.
    pub async fn new() -> Option<Self> {
        let instance = Instance::default();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await
            .ok()?;

        // The rasterizer resolves depth with a 64-bit atomicMin that packs
        // (depth << 32 | triangle_index), so the per-pixel winner is unique and
        // the shade pass never has to recompute/compare depth. That needs
        // 64-bit integer atomics; adapters without them fall back to the CPU
        // renderer (which is always available).
        let needed = wgpu::Features::SHADER_INT64 | wgpu::Features::SHADER_INT64_ATOMIC_MIN_MAX;
        if !adapter.features().contains(needed) {
            log::info!(
                "GPU {} lacks 64-bit atomic min/max; using CPU renderer",
                adapter.get_info().name
            );
            return None;
        }

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("a3d device"),
                required_features: needed,
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            })
            .await
            .ok()?;

        log::info!("GPU: {}", adapter.get_info().name);

        Some(Self {
            device,
            queue,
            adapter,
        })
    }
}
