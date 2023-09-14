use bevy::{
    app::PluginGroupBuilder,
    prelude::*,
    window::{close_on_esc, PrimaryWindow, WindowMode},
};
use bevy_vulkano::{
    egui_winit_vulkano::egui, BevyVulkanoSettings, BevyVulkanoWindows, VulkanoWinitPlugin,
};

pub struct PluginBundle;

impl PluginGroup for PluginBundle {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<PluginBundle>()
            .add(bevy::input::InputPlugin)
            .add(bevy::window::WindowPlugin::default())
            .add(VulkanoWinitPlugin)
    }
}

/*
* This example adds windows when clicking space button. Windows can be closed as well
 */

fn main() {
    App::new()
        .insert_non_send_resource(BevyVulkanoSettings {
            // Since we're only drawing gui, let's clear each frame
            is_gui_overlay: true,
            ..BevyVulkanoSettings::default()
        })
        .add_plugins(PluginBundle.set(WindowPlugin {
            primary_window: Some(Window {
                resolution: (1920.0, 1080.0).into(),
                title: "Bevy Vulkano Primary Window".to_string(),
                present_mode: bevy::window::PresentMode::Fifo,
                resizable: true,
                mode: WindowMode::Windowed,
                ..default()
            }),
            ..default()
        }))
        .add_systems(Update, close_on_esc)
        .add_systems(Update, create_new_window_system)
        .add_systems(PostUpdate, main_render_system_primary_window)
        .run();
}

fn create_new_window_system(mut commands: Commands, keys: Res<Input<KeyCode>>) {
    if keys.just_released(KeyCode::Space) {
        commands.spawn(Window {
            resolution: (512.0, 512.0).into(),
            present_mode: bevy::window::PresentMode::Fifo,
            title: "Secondary window".to_string(),
            ..default()
        });
    }
}

pub fn main_render_system_primary_window(
    window_query: Query<(Entity, Option<&PrimaryWindow>), With<Window>>,
    mut vulkano_windows: NonSendMut<BevyVulkanoWindows>,
) {
    for (window, maybe_primary) in window_query.iter() {
        if let Some(vulkano_window) = vulkano_windows.get_vulkano_window_mut(window) {
            // Start Frame
            let before = match vulkano_window.renderer.acquire() {
                Err(e) => {
                    bevy::log::error!("Failed to start frame: {}", e);
                    return;
                }
                Ok(f) => f,
            };
            // Egui calls
            let ctx = vulkano_window.gui.context();
            egui::Area::new("Window Gui")
                .fixed_pos(egui::pos2(10.0, 10.0))
                .show(&ctx, |ui| {
                    if maybe_primary.is_some() {
                        ui.label("Primary Window");
                    } else {
                        ui.label("Secondary Window");
                    }
                });
            let final_image = vulkano_window.renderer.swapchain_image_view();
            // Render egui
            let after = vulkano_window.gui.draw_on_image(before, final_image);
            // Finish frame
            vulkano_window.renderer.present(after, true);
        }
    }
}
