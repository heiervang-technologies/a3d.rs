use wgpu::{Adapter, Device, Instance, Queue};

/// Holds the wgpu device, queue, and adapter.
pub struct GpuContext {
    pub device: Device,
    pub queue: Queue,
    pub adapter: Adapter,
}

impl GpuContext {
    pub async fn new() -> Option<Self> {
        let instance = Instance::default();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("a3d device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            )
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
