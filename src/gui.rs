use bevy::prelude::*;
use egui::CtxRef;
use egui_winit_vulkano::Gui;

use crate::VulkanoContext;

#[derive(Default)]
pub struct GuiPlgin;

impl Plugin for GuiPlgin {
    fn build(&self, app: &mut App) {
        let renderer = app.world.get_resource::<VulkanoContext>().unwrap();
        let gui = Gui::new(renderer.surface(), renderer.graphics_queue(), true);
        app.insert_resource(gui.context());
        app.insert_non_send_resource(gui);
        app.add_system_to_stage(CoreStage::PreUpdate, begin_frame_system);
    }
}

/// Begins gui frame pre update every frame
fn begin_frame_system(mut vulkano_win: ResMut, mut ctx: ResMut<CtxRef>) {
    gui.begin_frame();
    // Update ctx ref :)
    *ctx = gui.context();
}
