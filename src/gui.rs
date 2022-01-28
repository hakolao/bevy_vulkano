use bevy::prelude::*;
use egui::CtxRef;
use egui_winit_vulkano::Gui;
use crate::Renderer;


#[derive(Default)]
pub struct GuiPlgin;

impl Plugin for GuiPlgin {
    fn build(&self, app: &mut App) {
        app.add_startup_system(create_gui_system.exclusive_system().at_start())
            .add_system_to_stage(CoreStage::PreUpdate, begin_frame_system);
    }
}

/// Greates gui system using renderer. Inserts it as a resource
fn create_gui_system(world: &mut World) {
    let renderer = world.get_resource::<Renderer>().unwrap();
    let gui = Gui::new(renderer.surface(), renderer.graphics_queue(), true);
    world.insert_resource(gui.context());
    world.insert_non_send(gui);
}

/// Begins gui frame pre update every frame
fn begin_frame_system(mut gui: NonSendMut<Gui>, mut ctx: ResMut<CtxRef>) {
    gui.begin_frame();
    // Update ctx ref :)
    *ctx = gui.context();
}
