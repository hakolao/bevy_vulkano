#[allow(clippy::needless_question_mark)]
mod game_of_life;
#[allow(clippy::needless_question_mark)]
mod pixels_draw_pipeline;
mod place_over_frame;

use bevy::{
    app::PluginGroupBuilder,
    prelude::*,
    time::FixedTimestep,
    window::{close_on_esc, WindowId, WindowMode},
};
use bevy_vulkano::{BevyVulkanoWindows, VulkanoWinitConfig, VulkanoWinitPlugin};
use vulkano::image::ImageAccess;

use crate::{game_of_life::GameOfLifeComputePipeline, place_over_frame::RenderPassPlaceOverFrame};

pub struct PluginBundle;

impl PluginGroup for PluginBundle {
    fn build(&mut self, group: &mut PluginGroupBuilder) {
        // Minimum plugins for the demo
        // Core needed for fixed time steps
        group.add(bevy::core::CorePlugin);
        group.add(bevy::input::InputPlugin);
        group.add(bevy::time::TimePlugin);
        // Don't add default bevy plugins or WinitPlugin. This owns "core loop" (runner).
        // Bevy winit and render should be excluded
        group.add(VulkanoWinitPlugin);
    }
}

fn main() {
    App::new()
        .insert_non_send_resource(VulkanoWinitConfig::default())
        .insert_resource(WindowDescriptor {
            width: 1024.0,
            height: 1024.0,
            title: "Bevy Vulkano Game Of Life".to_string(),
            present_mode: bevy::window::PresentMode::Immediate,
            resizable: true,
            mode: WindowMode::Windowed,
            position: WindowPosition::Centered(MonitorSelection::Primary),
            ..WindowDescriptor::default()
        })
        .add_plugins(PluginBundle)
        .add_startup_system(create_pipelines)
        .add_system(close_on_esc)
        .add_system(draw_life_system)
        .add_system(update_window_title_system)
        .add_system_set_to_stage(
            // Note that this is `PostUpdate` to ensure we render only after update
            CoreStage::PostUpdate,
            SystemSet::new()
                .with_run_criteria(FixedTimestep::steps_per_second(60.0))
                .with_system(game_of_life_pipeline_system),
        )
        .run();
}

fn update_window_title_system(vulkano_windows: NonSend<BevyVulkanoWindows>, time: ResMut<Time>) {
    let primary = vulkano_windows
        .get_winit_window(WindowId::primary())
        .unwrap();
    let fps = 1.0 / time.delta_seconds();
    primary.set_title(&format!("Bevy Vulkano Game Of Life {:.2}", fps));
}

/// Creates our simulation pipeline & render pipeline
fn create_pipelines(mut commands: Commands, vulkano_windows: NonSend<BevyVulkanoWindows>) {
    let primary_window = vulkano_windows.get_primary_window_renderer().unwrap();
    // Create compute pipeline to simulate game of life
    let game_of_life_pipeline =
        GameOfLifeComputePipeline::new(primary_window.graphics_queue(), [512, 512]);
    // Create our render pass
    let place_over_frame = RenderPassPlaceOverFrame::new(
        primary_window.graphics_queue(),
        primary_window.swapchain_format(),
    );
    // Insert resources
    commands.insert_resource(game_of_life_pipeline);
    commands.insert_resource(place_over_frame);
}

/// Draw life at mouse position on the game of life canvas
fn draw_life_system(
    mut game_of_life: ResMut<GameOfLifeComputePipeline>,
    windows: ResMut<Windows>,
    mouse_input: Res<Input<MouseButton>>,
) {
    if mouse_input.pressed(MouseButton::Left) {
        let primary = windows.get_primary().unwrap();
        if let Some(pos) = primary.cursor_position() {
            let width = primary.width();
            let height = primary.height();
            let normalized = Vec2::new(
                (pos.x / width).clamp(0.0, 1.0),
                (pos.y / height).clamp(0.0, 1.0),
            );
            let image_size = game_of_life
                .color_image()
                .image()
                .dimensions()
                .width_height();
            let draw_pos = IVec2::new(
                (image_size[0] as f32 * normalized.x) as i32,
                (image_size[1] as f32 * normalized.y) as i32,
            );
            game_of_life.draw_life(draw_pos);
        }
    }
}

/// All render occurs here in one system. If you want to split systems to separate, use
/// `PipelineSyncData` to update futures. You could have `pre_render_system` and `post_render_system` to start and finish frames
fn game_of_life_pipeline_system(
    mut vulkano_windows: NonSendMut<BevyVulkanoWindows>,
    mut game_of_life: ResMut<GameOfLifeComputePipeline>,
    mut place_over_frame: ResMut<RenderPassPlaceOverFrame>,
) {
    let primary_window = vulkano_windows.get_primary_window_renderer_mut().unwrap();

    // Start frame
    let before = match primary_window.acquire() {
        Err(e) => {
            bevy::log::error!("Failed to start frame: {}", e);
            return;
        }
        Ok(f) => f,
    };

    let after_compute = game_of_life.compute(before, [1.0, 0.0, 0.0, 1.0], [0.0; 4]);
    let color_image = game_of_life.color_image();
    let final_image = primary_window.swapchain_image_view();
    let after_render = place_over_frame.render(after_compute, color_image, final_image);

    // Finish Frame
    primary_window.present(after_render, true);
}
