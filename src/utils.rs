use std::cell::UnsafeCell;

use vulkano::sync::GpuFuture;

/// With this we can extend `Send` and `Sync` on `dyn GpuFuture`
pub struct UnsafeGpuFuture(UnsafeCell<Box<dyn GpuFuture>>);

impl UnsafeGpuFuture {
    pub fn new(future: Box<dyn GpuFuture>) -> UnsafeGpuFuture {
        UnsafeGpuFuture(UnsafeCell::new(future))
    }

    pub fn into_inner(self) -> Box<dyn GpuFuture> {
        self.0.into_inner()
    }
}

unsafe impl Send for UnsafeGpuFuture {}
unsafe impl Sync for UnsafeGpuFuture {}

/// Wrapper for before pipeline future (You can add your own if you have many pipelines)
pub struct BeforePipelineFuture(pub Option<UnsafeGpuFuture>);
/// Wrapper for after pipeline future (You can add your own if you have many pipelines)
pub struct AfterPipelineFuture(pub Option<UnsafeGpuFuture>);
