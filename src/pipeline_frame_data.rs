#[allow(unused)]
use bevy::{utils::HashMap, window::WindowId};

use crate::UnsafeGpuFuture;

#[derive(Default)]
pub struct PipelineData {
    pub frame_data: HashMap<WindowId, PipelineFrameData>,
}

impl PipelineData {
    pub fn add(&mut self, data: PipelineFrameData) {
        self.frame_data.insert(data.window_id, data);
    }

    pub fn get(&self, id: WindowId) -> Option<&PipelineFrameData> {
        self.frame_data.get(&id)
    }

    pub fn get_mut(&mut self, id: WindowId) -> Option<&mut PipelineFrameData> {
        self.frame_data.get_mut(&id)
    }

    pub fn get_primary(&self) -> Option<&PipelineFrameData> {
        self.get(WindowId::primary())
    }

    pub fn get_primary_mut(&mut self) -> Option<&mut PipelineFrameData> {
        self.get_mut(WindowId::primary())
    }

    pub fn iter(&self) -> impl Iterator<Item = &PipelineFrameData> {
        self.frame_data.values()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut PipelineFrameData> {
        self.frame_data.values_mut()
    }
}

/// Wrapper for useful data for rendering during pipeline
pub struct PipelineFrameData {
    pub window_id: WindowId,
    pub before: Option<UnsafeGpuFuture>,
    pub after: Option<UnsafeGpuFuture>,
}

#[derive(Debug, Copy, Clone)]
pub struct ShouldUpdatePipelineFrameData {
    pub id: WindowId,
}
