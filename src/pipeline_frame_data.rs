#[allow(unused)]
use bevy::{utils::HashMap, window::WindowId};

use crate::UnsafeGpuFuture;

#[derive(Default)]
pub struct WindowSyncData {
    pub frame_data: HashMap<WindowId, SyncData>,
}

impl WindowSyncData {
    pub fn add(&mut self, data: SyncData) {
        self.frame_data.insert(data.window_id, data);
    }

    pub fn remove(&mut self, id: WindowId) {
        self.frame_data.remove(&id);
    }

    pub fn get(&self, id: WindowId) -> Option<&SyncData> {
        self.frame_data.get(&id)
    }

    pub fn get_mut(&mut self, id: WindowId) -> Option<&mut SyncData> {
        self.frame_data.get_mut(&id)
    }

    pub fn get_primary(&self) -> Option<&SyncData> {
        self.get(WindowId::primary())
    }

    pub fn get_primary_mut(&mut self) -> Option<&mut SyncData> {
        self.get_mut(WindowId::primary())
    }

    pub fn iter(&self) -> impl Iterator<Item = &SyncData> {
        self.frame_data.values()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut SyncData> {
        self.frame_data.values_mut()
    }
}

/// Wrapper for useful data for rendering during pipeline
pub struct SyncData {
    pub window_id: WindowId,
    pub before: Option<UnsafeGpuFuture>,
    pub after: Option<UnsafeGpuFuture>,
}
