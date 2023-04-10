use bevy::prelude::Entity;
#[allow(unused)]
use bevy::{ecs::system::Resource, utils::HashMap};
use vulkano::sync::GpuFuture;

/// Contains gpu future data per window to be used in Vulkano pipeline synchronization
#[derive(Default, Resource)]
pub struct PipelineSyncData {
    pub data_per_window: HashMap<Entity, SyncData>,
}

impl PipelineSyncData {
    pub fn add(&mut self, data: SyncData) {
        self.data_per_window.insert(data.window_entity, data);
    }

    pub fn remove(&mut self, id: Entity) {
        self.data_per_window.remove(&id);
    }

    pub fn get(&self, id: Entity) -> Option<&SyncData> {
        self.data_per_window.get(&id)
    }

    pub fn get_mut(&mut self, id: Entity) -> Option<&mut SyncData> {
        self.data_per_window.get_mut(&id)
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
    pub window_entity: Entity,
    pub before: Option<Box<dyn GpuFuture>>,
    pub after: Option<Box<dyn GpuFuture>>,
}

unsafe impl Send for SyncData {}
unsafe impl Sync for SyncData {}
