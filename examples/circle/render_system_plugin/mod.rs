#[cfg(feature = "example_has_gui")]
use bevy::diagnostic::{Diagnostics, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy_vulkano::{
    AfterPipelineFuture, BeforePipelineFuture, FinalImageView, Renderer, UnsafeGpuFuture,
};
#[cfg(feature = "example_has_gui")]
use egui::CtxRef;
#[cfg(feature = "example_has_gui")]
use egui_winit_vulkano::Gui;
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
        let renderer = app.world.get_resource::<Renderer>().unwrap();
        let deferred_pass =
            RenderPassDeferred::new(renderer.graphics_queue(), renderer.swapchain_format())
                .unwrap();

        // Insert our render target as a resource, insert our render pass
        let final_image_view = renderer.final_image();
        app.insert_resource(final_image_view)
            .insert_resource(deferred_pass)
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

/// Starts frame, updates before pipeline future & final image view
fn pre_render_setup_system(
    mut renderer: ResMut<Renderer>,
    mut before_pipeline_future: ResMut<BeforePipelineFuture>,
    mut final_image_view: ResMut<FinalImageView>,
) {
    let before = match renderer.start_frame() {
        Err(e) => {
            bevy::log::error!("Failed to start frame: {}", e);
            None
        }
        Ok(f) => Some(UnsafeGpuFuture::new(f)),
    };
    *before_pipeline_future = BeforePipelineFuture(before);
    // Final image may have changed in `start_frame()` if e.g. resize occurred
    *final_image_view = renderer.final_image();
}

/// If rendering was successful, draw gui & finish frame
fn post_render_system(
    mut renderer: ResMut<Renderer>,
    mut after_pipeline_future: ResMut<AfterPipelineFuture>,
    #[cfg(feature = "example_has_gui")] final_image_view: Res<FinalImageView>,
    #[cfg(feature = "example_has_gui")] mut gui: NonSendMut<Gui>,
) {
    #[cfg(feature = "example_has_gui")]
    if let Some(after) = after_pipeline_future.0.take() {
        let at_end_future = gui.draw_on_image(after.into_inner(), final_image_view.clone());
        renderer.finish_frame(at_end_future);
    }
    #[cfg(not(feature = "example_has_gui"))]
    if let Some(after) = after_pipeline_future.0.take() {
        renderer.finish_frame(after.into_inner());
    }
}

pub fn main_render_system(
    mut before_pipeline_future: ResMut<BeforePipelineFuture>,
    mut after_pipeline_future: ResMut<AfterPipelineFuture>,
    mut render_pass_deferred: ResMut<RenderPassDeferred>,
    final_image_view: Res<FinalImageView>,
) {
    // We take the before pipeline future leaving None in its place
    if let Some(before_future) = before_pipeline_future.0.take() {
        let dims = final_image_view.image().dimensions().width_height();
        let ar = dims[0] as f32 / dims[1] as f32;
        // Camera would be better :)
        let world_to_screen = bevy::math::Mat4::orthographic_rh(-ar, ar, -1.0, 1.0, 0.0, 999.0);
        let mut frame = render_pass_deferred
            .frame(
                [0.0; 4],
                before_future.into_inner(),
                final_image_view.clone(),
                world_to_screen,
            )
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
        // Update after pipeline future
        *after_pipeline_future = AfterPipelineFuture(Some(UnsafeGpuFuture::new(after_drawing)));
    }
}

#[cfg(feature = "example_has_gui")]
fn set_gui_styles_system(_ctx: Res<CtxRef>) {
    // Set styles here if needed
}

#[cfg(feature = "example_has_gui")]
fn main_gui_system(ctx: Res<CtxRef>, diagnostics: Res<Diagnostics>) {
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
