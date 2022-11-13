#[allow(unused)]
use bevy::{ecs::system::Resource, utils::HashMap, window::WindowId};
use vulkano::sync::GpuFuture;

/// Contains gpu future data per window to be used in Vulkano pipeline synchronization
#[derive(Default, Resource)]
pub struct PipelineSyncData {
    pub data_per_window: HashMap<WindowId, SyncData>,
}

impl PipelineSyncData {
    pub fn add(&mut self, data: SyncData) {
        self.data_per_window.insert(data.window_id, data);
    }

    pub fn remove(&mut self, id: WindowId) {
        self.data_per_window.remove(&id);
    }

    pub fn get(&self, id: WindowId) -> Option<&SyncData> {
        self.data_per_window.get(&id)
    }

    pub fn get_mut(&mut self, id: WindowId) -> Option<&mut SyncData> {
        self.data_per_window.get_mut(&id)
    }

    pub fn get_primary(&self) -> Option<&SyncData> {
        self.get(WindowId::primary())
    }

    pub fn get_primary_mut(&mut self) -> Option<&mut SyncData> {
        self.get_mut(WindowId::primary())
    }

    pub fn iter(&self) -> impl Iterator<Item = &SyncData> {
        self.data_per_window.values()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut SyncData> {
        self.data_per_window.values_mut()
    }
}

/// Wrapper for useful data for rendering during pipeline
pub struct SyncData {
    pub window_id: WindowId,
    pub before: Option<Box<dyn GpuFuture>>,
    pub after: Option<Box<dyn GpuFuture>>,
}

unsafe impl Send for SyncData {}
unsafe impl Sync for SyncData {}
