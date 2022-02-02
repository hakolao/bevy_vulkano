use bevy::{
    app::PluginGroupBuilder,
    input::system::exit_on_esc_system,
    prelude::*,
    window::{CreateWindow, WindowId, WindowMode},
};
use bevy_vulkano::{VulkanoWindows, VulkanoWinitConfig, VulkanoWinitPlugin};

pub struct PluginBundle;

impl PluginGroup for PluginBundle {
    fn build(&mut self, group: &mut PluginGroupBuilder) {
        // Minimum plugins for the demo
        group.add(bevy::input::InputPlugin::default());
        // Don't add default bevy plugins or WinitPlugin. This owns "core loop" (runner).
        // Bevy winit and render should be excluded
        group.add(VulkanoWinitPlugin::default());
    }
}

fn main() {
    App::new()
        .insert_resource(VulkanoWinitConfig::default())
        .insert_resource(WindowDescriptor {
            width: 1920.0,
            height: 1080.0,
            title: "Bevy Vulkano Game Of Life".to_string(),
            vsync: false,
            resizable: true,
            mode: WindowMode::Windowed,
            ..WindowDescriptor::default()
        })
        .add_plugins(PluginBundle)
        .add_system(exit_on_esc_system)
        .add_system(create_new_window_system)
        .add_system_set_to_stage(
            // Add render system after PostUpdate
            CoreStage::PostUpdate,
            SystemSet::new()
                .with_system(main_render_system_primary_window)
                .with_system(main_render_system_secondary_window),
        )
        .run();
}
