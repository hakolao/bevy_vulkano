// Copyright (c) 2022 The vulkano developers
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>,
// at your option. All files in the project carrying such
// notice may not be copied, modified, or distributed except
// according to those terms.

use std::sync::Arc;

use vulkano::{
    command_buffer::{
        allocator::StandardCommandBufferAllocator, AutoCommandBufferBuilder, CommandBufferUsage,
        RenderPassBeginInfo, SubpassBeginInfo, SubpassContents,
    },
    device::{DeviceOwned, Queue},
    format::Format,
    image::view::ImageView,
    memory::allocator::StandardMemoryAllocator,
    render_pass::{Framebuffer, FramebufferCreateInfo, RenderPass, Subpass},
    sync::GpuFuture,
};

use crate::{pixels_draw_pipeline::PixelsDrawPipeline, Resource};

/// A render pass which places an incoming image over frame filling it
#[derive(Resource)]
pub struct RenderPassPlaceOverFrame {
    gfx_queue: Arc<Queue>,
    command_buffer_allocator: StandardCommandBufferAllocator,
    render_pass: Arc<RenderPass>,
    pixels_draw_pipeline: PixelsDrawPipeline,
}

impl RenderPassPlaceOverFrame {
    pub fn new(
        allocator: Arc<StandardMemoryAllocator>,
        gfx_queue: Arc<Queue>,
        output_format: Format,
    ) -> RenderPassPlaceOverFrame {
        let render_pass = vulkano::single_pass_renderpass!(gfx_queue.device().clone(),
            attachments: {
                color: {
                    format: output_format,
                    samples: 1,
                    load_op: Clear,
                    store_op: Store,
                }
            },
            pass: {
                    color: [color],
                    depth_stencil: {}
            }
        )
        .unwrap();
        let subpass = Subpass::from(render_pass.clone(), 0).unwrap();
        let pixels_draw_pipeline =
            PixelsDrawPipeline::new(allocator.clone(), gfx_queue.clone(), subpass);
        RenderPassPlaceOverFrame {
            gfx_queue,
            command_buffer_allocator: StandardCommandBufferAllocator::new(
                allocator.device().clone(),
                Default::default(),
            ),
            render_pass,
            pixels_draw_pipeline,
        }
    }

    /// Place view exactly over swapchain image target.
    /// Texture draw pipeline uses a quad onto which it places the view.
    pub fn render<F>(
        &mut self,
        before_future: F,
        view: Arc<ImageView>,
        target: Arc<ImageView>,
    ) -> Box<dyn GpuFuture>
    where
        F: GpuFuture + 'static,
    {
        // Get dimensions
        let img_dims = target.image().extent();
        // Create framebuffer (must be in same order as render pass description in `new`
        let framebuffer = Framebuffer::new(self.render_pass.clone(), FramebufferCreateInfo {
            attachments: vec![target],
            ..Default::default()
        })
        .unwrap();
        // Create primary command buffer builder
        let mut command_buffer_builder = AutoCommandBufferBuilder::primary(
            &self.command_buffer_allocator,
            self.gfx_queue.queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();
        // Begin render pass
        command_buffer_builder
            .begin_render_pass(
                RenderPassBeginInfo {
                    clear_values: vec![Some([0.0; 4].into())],
                    ..RenderPassBeginInfo::framebuffer(framebuffer)
                },
                SubpassBeginInfo {
                    contents: SubpassContents::SecondaryCommandBuffers,
                    ..Default::default()
                },
            )
            .unwrap();
        // Create secondary command buffer from texture pipeline & send draw commands
        let cb = self
            .pixels_draw_pipeline
            .draw([img_dims[0], img_dims[1]], view);
        // Execute above commands (subpass)
        command_buffer_builder.execute_commands(cb).unwrap();
        // End render pass
        command_buffer_builder
            .end_render_pass(Default::default())
            .unwrap();
        // Build command buffer
        let command_buffer = command_buffer_builder.build().unwrap();
        // Execute primary command buffer
        let after_future = before_future
            .then_execute(self.gfx_queue.clone(), command_buffer)
            .unwrap();

        after_future.boxed()
    }
}
