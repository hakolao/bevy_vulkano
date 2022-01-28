mod pipelines;
mod render_pass;
mod render_system_plugin;

use bevy::{
    app::PluginGroupBuilder, input::system::exit_on_esc_system, prelude::*, window::WindowMode,
};
use bevy_vulkano::{VulkanoWinitConfig, VulkanoWinitPlugin};
use vulkano::{device::Features, swapchain::PresentMode};

use crate::render_system_plugin::MainRenderPlugin;

pub struct PluginBundle;

impl PluginGroup for PluginBundle {
    fn build(&mut self, group: &mut PluginGroupBuilder) {
        group.add(bevy::log::LogPlugin::default());
        group.add(bevy::core::CorePlugin::default());
        group.add(bevy::transform::TransformPlugin::default());
        group.add(bevy::diagnostic::DiagnosticsPlugin::default());
        group.add(bevy::diagnostic::FrameTimeDiagnosticsPlugin::default());
        group.add(bevy::asset::AssetPlugin::default());
        group.add(bevy::scene::ScenePlugin::default());
        group.add(bevy::input::InputPlugin::default());
        group.add(bevy::window::WindowPlugin::default());
        // Don't add default bevy plugins or WinitPlugin. This owns "core loop" (runner)
        group.add(VulkanoWinitPlugin::default());
        // See here how rendering is orchestrated
        group.add(MainRenderPlugin::default());
    }
}

fn main() {
    App::new()
        .insert_resource(VulkanoWinitConfig {
            features: Features {
                fill_mode_non_solid: true,
                ..Features::none()
            },
            present_mode: PresentMode::Immediate,
            ..VulkanoWinitConfig::default()
        })
        .insert_resource(WindowDescriptor {
            width: 1920.0,
            height: 1080.0,
            title: "Bevy Vulkano".to_string(),
            vsync: false,
            resizable: true,
            mode: WindowMode::Windowed,
            ..WindowDescriptor::default()
        })
        .add_plugins(PluginBundle)
        .add_system(exit_on_esc_system)
        .run();
}
