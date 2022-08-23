#[cfg(feature = "example_has_gui")]
use bevy::window::close_on_esc;
#[cfg(feature = "example_has_gui")]
use bevy::{
    app::PluginGroupBuilder,
    prelude::*,
    window::{CreateWindow, WindowId, WindowMode},
};
#[cfg(feature = "example_has_gui")]
use bevy_vulkano::egui_winit_vulkano::egui;
#[cfg(feature = "example_has_gui")]
use bevy_vulkano::{BevyVulkanoWindows, VulkanoWinitConfig, VulkanoWinitPlugin};

#[cfg(feature = "example_has_gui")]
pub struct PluginBundle;

#[cfg(feature = "example_has_gui")]
impl PluginGroup for PluginBundle {
    fn build(&mut self, group: &mut PluginGroupBuilder) {
        // Minimum plugins for the demo
        group.add(bevy::input::InputPlugin);
        // Don't add default bevy plugins or WinitPlugin. This owns "core loop" (runner).
        // Bevy winit and render should be excluded
        group.add(VulkanoWinitPlugin);
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
        .insert_non_send_resource(VulkanoWinitConfig {
            // Since we're only drawing gui, let's clear each frame
            is_gui_overlay: true,
            ..VulkanoWinitConfig::default()
        })
        .insert_resource(WindowDescriptor {
            width: 1920.0,
            height: 1080.0,
            title: "Bevy Vulkano Primary Window".to_string(),
            present_mode: bevy::window::PresentMode::Fifo,
            resizable: true,
            mode: WindowMode::Windowed,
            ..WindowDescriptor::default()
        })
        .add_plugins(PluginBundle)
        .add_system(close_on_esc)
        .add_startup_system(create_new_window_system)
        .add_system(create_new_window_on_space_system)
        .add_system_set_to_stage(
            // Add render system after PostUpdate
            CoreStage::PostUpdate,
            SystemSet::new()
                .with_system(main_render_system_primary_window)
                .with_system(main_render_system_secondary_window),
        )
        .run();
}

#[cfg(feature = "example_has_gui")]
fn create_new_window_system(mut create_window_events: EventWriter<CreateWindow>) {
    let window_id = WindowId::new();
    create_window_events.send(CreateWindow {
        id: window_id,
        descriptor: WindowDescriptor {
            width: 512.,
            height: 512.,
            present_mode: bevy::window::PresentMode::Fifo,
            title: "Secondary window".to_string(),
            ..Default::default()
        },
    });
}

/// Adds new window when space is pressed
#[cfg(feature = "example_has_gui")]
fn create_new_window_on_space_system(
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
                present_mode: bevy::window::PresentMode::Fifo,
                title: "Secondary window".to_string(),
                ..Default::default()
            },
        });
    }
}

#[cfg(feature = "example_has_gui")]
pub fn main_render_system_primary_window(mut vulkano_windows: NonSendMut<BevyVulkanoWindows>) {
    let (window_renderer, gui) = vulkano_windows.get_primary_window_renderer_mut().unwrap();
    // Start Frame
    let before = match window_renderer.acquire() {
        Err(e) => {
            bevy::log::error!("Failed to start frame: {}", e);
            return;
        }
        Ok(f) => f,
    };
    // Egui calls
    let ctx = gui.context();
    egui::Area::new("Primary Window Gui")
        .fixed_pos(egui::pos2(10.0, 10.0))
        .show(&ctx, |ui| {
            ui.label("Primary Window");
        });
    let final_image = window_renderer.swapchain_image_view();
    // Render egui
    let after = gui.draw_on_image(before, final_image);
    // Finish frame
    window_renderer.present(after, true);
}

#[cfg(feature = "example_has_gui")]
pub fn main_render_system_secondary_window(mut vulkano_windows: NonSendMut<BevyVulkanoWindows>) {
    let primary_window_id = vulkano_windows.get_primary_winit_window().unwrap().id();
    for (window_id, (window_renderer, gui)) in vulkano_windows.iter_mut() {
        // Skip primary window
        if *window_id == primary_window_id {
            continue;
        }
        // Render on secondary window
        // Start Frame
        let before = match window_renderer.acquire() {
            Err(e) => {
                bevy::log::error!("Failed to start frame: {}", e);
                return;
            }
            Ok(f) => f,
        };

        // Egui calls
        let ctx = gui.context();
        egui::Area::new("Secondary Window Gui")
            .fixed_pos(egui::pos2(10.0, 10.0))
            .show(&ctx, |ui| {
                ui.label(format!("Secondary Window id {:?}", window_id));
                ui.button("Hello")
                    .clicked()
                    .then(|| println!("Clicked me!"));
            });
        // Render egui
        let final_image = window_renderer.swapchain_image_view();
        let after = gui.draw_on_image(before, final_image);
        // Finish frame
        window_renderer.present(after, false);
    }
}
