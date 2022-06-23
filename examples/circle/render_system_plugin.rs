#[cfg(feature = "example_has_gui")]
use bevy::diagnostic::{Diagnostics, FrameTimeDiagnosticsPlugin};
use bevy::{prelude::*, window::WindowId};
#[cfg(feature = "example_has_gui")]
use bevy_vulkano::egui_winit_vulkano::egui;
use bevy_vulkano::{BevyVulkanoWindows, PipelineSyncData};
use vulkano::{image::ImageAccess, sync::GpuFuture};

use crate::render_pass::{Pass, RenderPassDeferred};

/// Render stages intended to be set to run after `CoreStage::PostUpdate`
#[derive(Debug, Hash, PartialEq, Eq, Clone, StageLabel)]
pub enum RenderStage {
    GuiStart,
    GuiRender,
    RenderStart,
    Render,
    RenderFinish,
}

#[derive(Default)]
pub struct MainRenderPlugin;

impl Plugin for MainRenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_startup_system(insert_render_pass_system)
            .add_stage_after(
                CoreStage::PostUpdate,
                RenderStage::GuiStart,
                SystemStage::single_threaded(),
            )
            .add_stage_after(
                RenderStage::GuiStart,
                RenderStage::GuiRender,
                SystemStage::parallel(),
            )
            .add_stage_after(
                RenderStage::GuiRender,
                RenderStage::RenderStart,
                SystemStage::single_threaded(),
            )
            .add_stage_after(
                RenderStage::RenderStart,
                RenderStage::Render,
                SystemStage::single_threaded(),
            )
            .add_stage_after(
                RenderStage::Render,
                RenderStage::RenderFinish,
                SystemStage::single_threaded(),
            )
            // Render systems
            .add_system_set_to_stage(
                RenderStage::RenderStart,
                SystemSet::new().with_system(pre_render_setup_system),
            )
            .add_system_set_to_stage(
                RenderStage::Render,
                SystemSet::new().with_system(main_render_system),
            )
            .add_system_set_to_stage(
                RenderStage::RenderFinish,
                SystemSet::new().with_system(post_render_system),
            );
        // Gui systems
        #[cfg(feature = "example_has_gui")]
        app.add_system_set_to_stage(
            RenderStage::GuiStart,
            SystemSet::new().with_system(set_gui_styles_system),
        )
        .add_system_set_to_stage(
            RenderStage::GuiRender,
            SystemSet::new().with_system(main_gui_system),
        );
    }
}

/// Insert our render pass at startup
fn insert_render_pass_system(mut commands: Commands, vulkano_windows: Res<BevyVulkanoWindows>) {
    #[cfg(feature = "example_has_gui")]
    let (window_renderer, _) = vulkano_windows.get_primary_window_renderer().unwrap();
    #[cfg(not(feature = "example_has_gui"))]
    let window_renderer = vulkano_windows.get_primary_window_renderer().unwrap();
    let queue = window_renderer.graphics_queue();
    let format = window_renderer.swapchain_format();
    let deferred_pass = RenderPassDeferred::new(queue, format).unwrap();
    commands.insert_resource(deferred_pass);
}

/// Starts frame, updates before pipeline future & final image view
fn pre_render_setup_system(
    mut vulkano_windows: ResMut<BevyVulkanoWindows>,
    mut pipeline_frame_data: ResMut<PipelineSyncData>,
) {
    for (window_id, mut frame_data) in pipeline_frame_data.data_per_window.iter_mut() {
        #[cfg(feature = "example_has_gui")]
        let window_renderer = if let Some((window_renderer, _gui)) =
            vulkano_windows.get_window_renderer_mut(*window_id)
        {
            window_renderer
        } else {
            return;
        };
        #[cfg(not(feature = "example_has_gui"))]
        let window_renderer =
            if let Some(window_renderer) = vulkano_windows.get_window_renderer_mut(*window_id) {
                window_renderer
            } else {
                return;
            };
        let before = match window_renderer.acquire() {
            Err(e) => {
                bevy::log::error!("Failed to start frame: {}", e);
                None
            }
            Ok(f) => Some(f),
        };
        frame_data.before = before;
    }
}

/// If rendering was successful, draw gui & finish frame
fn post_render_system(
    mut vulkano_windows: ResMut<BevyVulkanoWindows>,
    mut pipeline_frame_data: ResMut<PipelineSyncData>,
) {
    for (window_id, frame_data) in pipeline_frame_data.data_per_window.iter_mut() {
        #[cfg(feature = "example_has_gui")]
        let (window_renderer, gui) = if let Some((window_renderer, gui)) =
            vulkano_windows.get_window_renderer_mut(*window_id)
        {
            (window_renderer, gui)
        } else {
            return;
        };
        #[cfg(not(feature = "example_has_gui"))]
        let window_renderer =
            if let Some(window_renderer) = vulkano_windows.get_window_renderer_mut(*window_id) {
                window_renderer
            } else {
                return;
            };
        #[cfg(feature = "example_has_gui")]
        if let Some(after) = frame_data.after.take() {
            let final_image_view = window_renderer.swapchain_image_view();
            let at_end_future = gui.draw_on_image(after, final_image_view);
            window_renderer.present(at_end_future, true);
        }
        #[cfg(not(feature = "example_has_gui"))]
        if let Some(after) = frame_data.after.take() {
            window_renderer.present(after, false);
        }
    }
}

// Only draw primary now...
// You could render different windows in their own systems...
pub fn main_render_system(
    mut vulkano_windows: ResMut<BevyVulkanoWindows>,
    mut pipeline_frame_data: ResMut<PipelineSyncData>,
    mut render_pass_deferred: ResMut<RenderPassDeferred>,
) {
    let mut frame_data = pipeline_frame_data.get_mut(WindowId::primary()).unwrap();
    #[cfg(feature = "example_has_gui")]
    let window_renderer =
        if let Some((window_renderer, _gui)) = vulkano_windows.get_primary_window_renderer_mut() {
            window_renderer
        } else {
            return;
        };
    #[cfg(not(feature = "example_has_gui"))]
    let window_renderer =
        if let Some(window_renderer) = vulkano_windows.get_primary_window_renderer_mut() {
            window_renderer
        } else {
            return;
        };

    // We take the before pipeline future leaving None in its place
    if let Some(before_future) = frame_data.before.take() {
        let final_image_view = window_renderer.swapchain_image_view();
        let dims = final_image_view.image().dimensions().width_height();
        let ar = dims[0] as f32 / dims[1] as f32;
        // Camera would be better :)
        let world_to_screen = bevy::math::Mat4::orthographic_rh(-ar, ar, -1.0, 1.0, 0.0, 999.0);
        let mut frame = render_pass_deferred
            .frame([0.0; 4], before_future, final_image_view, world_to_screen)
            .unwrap();
        let mut after_future = None;
        while let Some(pass) = frame.next_pass().unwrap() {
            after_future = match pass {
                Pass::Deferred(mut dp) => {
                    dp.draw_circle(bevy::math::Vec2::new(0.0, 0.0), 0.2, [1.0, 0.0, 0.0, 1.0])
                        .unwrap();
                    None
                }
                Pass::Finished(af) => Some(af),
            };
        }
        let after_drawing = after_future
            .unwrap()
            .then_signal_fence_and_flush()
            .unwrap()
            .boxed();
        // Update after pipeline future (so post render will know to present frame)
        frame_data.after = Some(after_drawing);
    }
}

#[cfg(feature = "example_has_gui")]
fn set_gui_styles_system(vulkano_windows: Res<BevyVulkanoWindows>) {
    let (_primary_window_renderer, gui) = vulkano_windows.get_primary_window_renderer().unwrap();
    let _ctx = gui.context();
    // Set styles here... for primary window
}

#[cfg(feature = "example_has_gui")]
fn main_gui_system(vulkano_windows: Res<BevyVulkanoWindows>, diagnostics: Res<Diagnostics>) {
    let (_primary_window_renderer, gui) = vulkano_windows.get_primary_window_renderer().unwrap();
    let ctx = gui.context();
    egui::Area::new("fps")
        .fixed_pos(egui::pos2(10.0, 10.0))
        .show(&ctx, |ui| {
            if let Some(diag) = diagnostics.get(FrameTimeDiagnosticsPlugin::FPS) {
                if let Some(avg) = diag.average() {
                    ui.label(format!(" FPS: {:.2}", avg));
                }
            }
        });
}
