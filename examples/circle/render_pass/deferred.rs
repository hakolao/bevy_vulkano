use std::sync::Arc;

use anyhow::*;
use vulkano::{
    command_buffer::{
        AutoCommandBufferBuilder, CommandBufferUsage, PrimaryAutoCommandBuffer,
        SecondaryCommandBuffer, SubpassContents,
    },
    device::{Device, Queue},
    format::Format,
    image::ImageViewAbstract,
    render_pass::{Framebuffer, FramebufferCreateInfo, RenderPass, Subpass},
    sync::GpuFuture,
};

use crate::pipelines::CircleDrawPipeline;

pub struct Pipelines {
    circle: CircleDrawPipeline,
}

/// System that contains the necessary facilities for rendering a single frame.
/// This is a stripped down version of https://github.com/vulkano-rs/vulkano/blob/master/examples/src/bin/deferred/main.rs
pub struct RenderPassDeferred {
    gfx_queue: Arc<Queue>,
    render_pass: Arc<RenderPass>,
    pipelines: Pipelines,
}

impl RenderPassDeferred {
    pub fn new(gfx_queue: Arc<Queue>, final_output_format: Format) -> Result<RenderPassDeferred> {
        let render_pass = vulkano::ordered_passes_renderpass!(gfx_queue.device().clone(),
            attachments: {
                final_color: {
                    load: Clear,
                    store: Store,
                    format: final_output_format,
                    samples: 1,
                }
            },
            // Add more passes when needed
            passes: [
                {
                    color: [final_color],
                    depth_stencil: {},
                    input: []
                }
            ]
        )?;
        let deferred_subpass = Subpass::from(render_pass.clone(), 0).unwrap();

        let pipelines = Pipelines {
            circle: CircleDrawPipeline::new(gfx_queue.clone(), deferred_subpass)?,
        };

        Ok(RenderPassDeferred {
            gfx_queue,
            render_pass: render_pass as Arc<_>,
            pipelines,
        })
    }

    #[allow(unused)]
    #[inline]
    pub fn device(&self) -> &Arc<Device> {
        self.gfx_queue.device()
    }

    #[allow(unused)]
    #[inline]
    pub fn queue(&self) -> &Arc<Queue> {
        &self.gfx_queue
    }

    #[allow(unused)]
    #[inline]
    pub fn deferred_subpass(&self) -> Subpass {
        Subpass::from(self.render_pass.clone(), 0).unwrap()
    }

    pub fn frame<F>(
        &mut self,
        clear_color: [f32; 4],
        before_future: F,
        final_image: Arc<dyn ImageViewAbstract + 'static>,
        world_to_screen: bevy::math::Mat4,
    ) -> Result<Frame>
    where
        F: GpuFuture + 'static,
    {
        let framebuffer = Framebuffer::new(self.render_pass.clone(), FramebufferCreateInfo {
            attachments: vec![final_image],
            ..Default::default()
        })?;
        let mut command_buffer_builder = AutoCommandBufferBuilder::primary(
            self.gfx_queue.device().clone(),
            self.gfx_queue.family(),
            CommandBufferUsage::OneTimeSubmit,
        )?;
        command_buffer_builder.begin_render_pass(
            framebuffer.clone(),
            SubpassContents::SecondaryCommandBuffers,
            vec![clear_color.into()],
        )?;
        Ok(Frame {
            system: self,
            before_main_cb_future: Some(before_future.boxed()),
            framebuffer,
            num_pass: 0,
            command_buffer_builder: Some(command_buffer_builder),
            world_to_screen,
        })
    }
}

pub struct Frame<'a> {
    system: &'a mut RenderPassDeferred,
    num_pass: u8,
    before_main_cb_future: Option<Box<dyn GpuFuture>>,
    framebuffer: Arc<Framebuffer>,
    command_buffer_builder: Option<AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>>,
    world_to_screen: bevy::math::Mat4,
}

impl<'a> Frame<'a> {
    pub fn next_pass<'f>(&'f mut self) -> Result<Option<Pass<'f, 'a>>> {
        Ok(
            match {
                let current_pass = self.num_pass;
                self.num_pass += 1;
                current_pass
            } {
                0 => Some(Pass::Deferred(DrawPass {
                    frame: self,
                })),
                1 => {
                    self.command_buffer_builder
                        .as_mut()
                        .unwrap()
                        .end_render_pass()?;
                    let command_buffer = self.command_buffer_builder.take().unwrap().build()?;

                    let after_main_cb = self
                        .before_main_cb_future
                        .take()
                        .unwrap()
                        .then_execute(self.system.gfx_queue.clone(), command_buffer)?;
                    Some(Pass::Finished(after_main_cb.boxed()))
                }
                _ => None,
            },
        )
    }

    /// Appends a command that executes a secondary command buffer that performs drawing.
    #[allow(unused)]
    #[inline]
    pub fn execute<C>(&mut self, command_buffer: C) -> Result<()>
    where
        C: SecondaryCommandBuffer + Send + Sync + 'static,
    {
        self.command_buffer_builder
            .as_mut()
            .unwrap()
            .execute_commands(command_buffer)?;
        Ok(())
    }
}

/// Struct provided to the user that allows them to customize or handle the pass.
pub enum Pass<'f, 's: 'f> {
    Deferred(DrawPass<'f, 's>),
    Finished(Box<dyn GpuFuture>),
}

/// Allows the user to draw objects on the scene.
pub struct DrawPass<'f, 's: 'f> {
    frame: &'f mut Frame<'s>,
}

impl<'f, 's: 'f> DrawPass<'f, 's> {
    /// Appends a command that executes a secondary command buffer that performs drawing.
    #[inline]
    pub fn execute<C>(&mut self, command_buffer: C) -> Result<()>
    where
        C: SecondaryCommandBuffer + Send + Sync + 'static,
    {
        self.frame
            .command_buffer_builder
            .as_mut()
            .unwrap()
            .execute_commands(command_buffer)?;
        Ok(())
    }

    #[allow(unused)]
    #[inline]
    pub fn device(&self) -> &Arc<Device> {
        self.frame.system.gfx_queue.device()
    }

    #[allow(unused)]
    #[inline]
    pub fn queue(&self) -> &Arc<Queue> {
        &self.frame.system.gfx_queue
    }

    /// Returns the dimensions in pixels of the viewport.
    #[allow(unused)]
    #[inline]
    pub fn viewport_dimensions(&self) -> [u32; 2] {
        self.frame.framebuffer.extent()
    }

    #[inline]
    pub fn world_to_screen(&self) -> bevy::math::Mat4 {
        self.frame.world_to_screen
    }

    pub fn draw_circle(
        &mut self,
        pos: bevy::math::Vec2,
        radius: f32,
        color: [f32; 4],
    ) -> Result<()> {
        let dims = self.frame.framebuffer.extent();
        let cb = self.frame.system.pipelines.circle.draw(
            dims,
            self.world_to_screen(),
            pos,
            radius,
            color,
        )?;
        self.execute(cb)
    }

    // Add more drawing functionality here (create pipelines first...)
}
