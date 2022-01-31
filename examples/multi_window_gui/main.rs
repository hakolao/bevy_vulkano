#[cfg(feature = "example_has_gui")]
use bevy::{
    app::PluginGroupBuilder,
    input::system::exit_on_esc_system,
    prelude::*,
    window::{CreateWindow, WindowId, WindowMode},
};
#[cfg(feature = "example_has_gui")]
use bevy_vulkano::{VulkanoWindows, VulkanoWinitConfig, VulkanoWinitPlugin};

#[cfg(feature = "example_has_gui")]
pub struct PluginBundle;

#[cfg(feature = "example_has_gui")]
impl PluginGroup for PluginBundle {
    fn build(&mut self, group: &mut PluginGroupBuilder) {
        // Minimum plugins for the demo
        group.add(bevy::input::InputPlugin::default());
        // Don't add default bevy plugins or WinitPlugin. This owns "core loop" (runner).
        // Bevy winit and render should be excluded
        group.add(VulkanoWinitPlugin::default());
    }
}

#[cfg(not(feature = "example_has_gui"))]
fn main() {
    println!("Multi window Gui example needs to be run with --features example_has_gui")
}

/*
* This example adds windows when clicking space button. Windows can be closed as well
 */

#[cfg(feature = "example_has_gui")]
fn main() {
    App::new()
        .insert_resource(VulkanoWinitConfig::default())
        .insert_resource(WindowDescriptor {
            width: 1920.0,
            height: 1080.0,
            title: "Bevy Vulkano Primary Window".to_string(),
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

/// Adds new window when space is pressed
#[cfg(feature = "example_has_gui")]
fn create_new_window_system(
    keys: Res<Input<KeyCode>>,
    mut create_window_events: EventWriter<CreateWindow>,
) {
    if keys.just_pressed(KeyCode::Space) {
        let window_id = WindowId::new();
        create_window_events.send(CreateWindow {
            id: window_id,
            descriptor: WindowDescriptor {
                width: 512.,
                height: 512.,
                vsync: true,
                title: "Secondary window".to_string(),
                ..Default::default()
            },
        });
    }
}

#[cfg(feature = "example_has_gui")]
pub fn main_render_system_primary_window(mut vulkano_windows: ResMut<VulkanoWindows>) {
    let vulkano_window = vulkano_windows
        .get_window_renderer_mut(WindowId::primary())
        .unwrap();
    // Start Frame
    let before = match vulkano_window.start_frame() {
        Err(e) => {
            bevy::log::error!("Failed to start frame: {}", e);
            return;
        }
        Ok(f) => f,
    };
    // Egui calls
    let ctx = vulkano_window.gui_context();
    egui::Area::new("Primary Window Gui")
        .fixed_pos(egui::pos2(10.0, 10.0))
        .show(&ctx, |ui| {
            ui.label("Primary Window");
        });
    // Render egui
    let final_image = vulkano_window.final_image();
    let after = vulkano_window.gui().draw_on_image(before, final_image);
    // Finish frame
    vulkano_window.finish_frame(after);
}

#[cfg(feature = "example_has_gui")]
pub fn main_render_system_secondary_window(mut vulkano_windows: ResMut<VulkanoWindows>) {
    let primary_window_id = vulkano_windows
        .get_winit_window(WindowId::primary())
        .unwrap()
        .id();
    for (window_id, vulkano_window) in vulkano_windows.windows.iter_mut() {
        // Skip primary window
        if *window_id == primary_window_id {
            continue;
        }
        // Render on secondary window
        // Start Frame
        let before = match vulkano_window.start_frame() {
            Err(e) => {
                bevy::log::error!("Failed to start frame: {}", e);
                return;
            }
            Ok(f) => f,
        };
        // Egui calls
        let ctx = vulkano_window.gui_context();
        egui::Area::new("Secondary Window Gui")
            .fixed_pos(egui::pos2(10.0, 10.0))
            .show(&ctx, |ui| {
                ui.label(format!("Secondary Window id {:?}", window_id));
            });
        // Render egui
        let final_image = vulkano_window.final_image();
        let after = vulkano_window.gui().draw_on_image(before, final_image);
        // Finish frame
        vulkano_window.finish_frame(after);
    }
}
