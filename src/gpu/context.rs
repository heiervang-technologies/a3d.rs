use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use wgpu::{Adapter, Device, Instance, Queue};

/// Holds the wgpu device, queue, and adapter.
pub struct GpuContext {
    lost: Arc<AtomicBool>,
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

        let adapter = match instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await
        {
            Ok(adapter) => adapter,
            Err(error) => {
                log::info!("no suitable GPU adapter: {error}");
                return None;
            }
        };

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

        let (device, queue) = match adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("a3d device"),
                required_features: needed,
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            })
            .await
        {
            Ok(pair) => pair,
            Err(error) => {
                log::warn!("failed to create GPU device: {error}");
                return None;
            }
        };

        log::info!("GPU: {}", adapter.get_info().name);

        let lost = Arc::new(AtomicBool::new(false));
        let callback_lost = Arc::clone(&lost);
        device.set_device_lost_callback(move |reason, message| {
            callback_lost.store(true, Ordering::Release);
            log::debug!("GPU device lost ({reason:?}): {message}");
        });
        Some(Self {
            lost,
            device,
            queue,
            adapter,
        })
    }

    // Scope every fallible GPU operation so validation and allocation failures
    // reach the caller instead of wgpu's default panic handler. Always pop all
    // scopes, including when the operation itself returns an error.
    pub(crate) fn checked<T>(
        &self,
        operation: impl FnOnce() -> Result<T, String>,
    ) -> Result<T, String> {
        if self.lost.load(Ordering::Acquire) {
            return Err("GPU device is lost".into());
        }
        for filter in [
            wgpu::ErrorFilter::OutOfMemory,
            wgpu::ErrorFilter::Internal,
            wgpu::ErrorFilter::Validation,
        ] {
            self.device.push_error_scope(filter);
        }
        let mut result = operation();
        for _ in 0..3 {
            if let Some(error) = pollster::block_on(self.device.pop_error_scope()) {
                result = Err(format!("GPU operation failed: {error}"));
            }
        }
        if self.lost.load(Ordering::Acquire) {
            return Err("GPU device is lost".into());
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_errors_are_scoped_and_do_not_poison_later_operations() {
        let Some(ctx) = pollster::block_on(GpuContext::new()) else {
            eprintln!("Skipping GPU error scopes: no suitable adapter");
            return;
        };
        let result = ctx.checked(|| {
            let _buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("deliberately invalid buffer"),
                size: ctx.device.limits().max_buffer_size + 1,
                usage: wgpu::BufferUsages::STORAGE,
                mapped_at_creation: false,
            });
            Ok(())
        });
        assert!(result.is_err());
        let result: Result<(), String> = ctx.checked(|| Err("caller error".into()));
        assert_eq!(result.unwrap_err(), "caller error");
        assert_eq!(ctx.checked(|| Ok(42)).unwrap(), 42);
    }
}
