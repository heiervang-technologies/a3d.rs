use super::GpuContext;

/// GPU compute pipeline for rasterization and lighting.
#[allow(dead_code)]
pub(crate) struct RasterPipeline {
    // TODO: compute pipeline, bind groups, buffers
}

impl RasterPipeline {
    #[allow(dead_code)]
    pub fn new(_ctx: &GpuContext) -> Self {
        // TODO: create compute shader pipeline
        Self {}
    }
}
