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
    buffer::{Buffer, BufferContents, BufferCreateInfo, BufferUsage, Subbuffer},
    command_buffer::{
        allocator::StandardCommandBufferAllocator, AutoCommandBufferBuilder,
        CommandBufferInheritanceInfo, CommandBufferUsage, SecondaryAutoCommandBuffer,
    },
    descriptor_set::{
        allocator::StandardDescriptorSetAllocator, PersistentDescriptorSet, WriteDescriptorSet,
    },
    device::{DeviceOwned, Queue},
    image::ImageViewAbstract,
    memory::allocator::{AllocationCreateInfo, MemoryUsage, StandardMemoryAllocator},
    pipeline::{
        graphics::{
            input_assembly::InputAssemblyState,
            vertex_input::Vertex,
            viewport::{Viewport, ViewportState},
        },
        GraphicsPipeline, Pipeline, PipelineBindPoint,
    },
    render_pass::Subpass,
    sampler::{Filter, Sampler, SamplerAddressMode, SamplerCreateInfo, SamplerMipmapMode},
};

/// Vertex for textured quads
#[repr(C)]
#[derive(BufferContents, Vertex)]
pub struct PosVertex {
    #[format(R32G32_SFLOAT)]
    pub position: [f32; 2],
    #[format(R32G32_SFLOAT)]
    pub tex_coords: [f32; 2],
}

pub fn pos_quad(width: f32, height: f32) -> (Vec<PosVertex>, Vec<u32>) {
    (
        vec![
            PosVertex {
                position: [-(width / 2.0), -(height / 2.0)],
                tex_coords: [0.0, 1.0],
            },
            PosVertex {
                position: [-(width / 2.0), height / 2.0],
                tex_coords: [0.0, 0.0],
            },
            PosVertex {
                position: [width / 2.0, height / 2.0],
                tex_coords: [1.0, 0.0],
            },
            PosVertex {
                position: [width / 2.0, -(height / 2.0)],
                tex_coords: [1.0, 1.0],
            },
        ],
        vec![0, 2, 1, 0, 3, 2],
    )
}

/// A subpass pipeline that fills a quad over frame
pub struct PixelsDrawPipeline {
    gfx_queue: Arc<Queue>,
    command_buffer_allocator: StandardCommandBufferAllocator,
    descriptor_set_allocator: StandardDescriptorSetAllocator,
    pipeline: Arc<GraphicsPipeline>,
    subpass: Subpass,
    vertices: Subbuffer<[PosVertex]>,
    indices: Subbuffer<[u32]>,
}

impl PixelsDrawPipeline {
    pub fn new(
        allocator: Arc<StandardMemoryAllocator>,
        gfx_queue: Arc<Queue>,
        subpass: Subpass,
    ) -> PixelsDrawPipeline {
        let (vertices, indices) = pos_quad(2.0, 2.0);
        let vertex_buffer = Buffer::from_iter(
            &allocator,
            BufferCreateInfo {
                usage: BufferUsage::VERTEX_BUFFER,
                ..Default::default()
            },
            AllocationCreateInfo {
                usage: MemoryUsage::Upload,
                ..Default::default()
            },
            vertices.into_iter(),
        )
        .unwrap();

        let index_buffer = Buffer::from_iter(
            &allocator,
            BufferCreateInfo {
                usage: BufferUsage::INDEX_BUFFER,
                ..Default::default()
            },
            AllocationCreateInfo {
                usage: MemoryUsage::Upload,
                ..Default::default()
            },
            indices.into_iter(),
        )
        .unwrap();

        let pipeline = {
            let vs = vs::load(gfx_queue.device().clone()).expect("failed to create shader module");
            let fs = fs::load(gfx_queue.device().clone()).expect("failed to create shader module");
            GraphicsPipeline::start()
                .vertex_input_state(PosVertex::per_vertex())
                .vertex_shader(vs.entry_point("main").unwrap(), ())
                .input_assembly_state(InputAssemblyState::new())
                .fragment_shader(fs.entry_point("main").unwrap(), ())
                .viewport_state(ViewportState::viewport_dynamic_scissor_irrelevant())
                .render_pass(subpass.clone())
                .build(gfx_queue.device().clone())
                .unwrap()
        };
        PixelsDrawPipeline {
            gfx_queue,
            command_buffer_allocator: StandardCommandBufferAllocator::new(
                allocator.device().clone(),
                Default::default(),
            ),
            descriptor_set_allocator: StandardDescriptorSetAllocator::new(
                allocator.device().clone(),
            ),
            pipeline,
            subpass,
            vertices: vertex_buffer,
            indices: index_buffer,
        }
    }

    fn create_image_sampler_nearest(
        &self,
        image: Arc<dyn ImageViewAbstract>,
    ) -> Arc<PersistentDescriptorSet> {
        let layout = self.pipeline.layout().set_layouts().get(0).unwrap();
        let sampler = Sampler::new(self.gfx_queue.device().clone(), SamplerCreateInfo {
            mag_filter: Filter::Nearest,
            min_filter: Filter::Nearest,
            address_mode: [SamplerAddressMode::Repeat; 3],
            mipmap_mode: SamplerMipmapMode::Nearest,
            ..Default::default()
        })
        .unwrap();
        PersistentDescriptorSet::new(&self.descriptor_set_allocator, layout.clone(), [
            WriteDescriptorSet::image_view_sampler(0, image, sampler),
        ])
        .unwrap()
    }

    /// Draw input `image` over a quad of size -1.0 to 1.0
    pub fn draw(
        &mut self,
        viewport_dimensions: [u32; 2],
        image: Arc<dyn ImageViewAbstract>,
    ) -> SecondaryAutoCommandBuffer {
        let mut builder = AutoCommandBufferBuilder::secondary(
            &self.command_buffer_allocator,
            self.gfx_queue.queue_family_index(),
            CommandBufferUsage::MultipleSubmit,
            CommandBufferInheritanceInfo {
                render_pass: Some(self.subpass.clone().into()),
                ..Default::default()
            },
        )
        .unwrap();
        let desc_set = self.create_image_sampler_nearest(image);
        builder
            .set_viewport(0, [Viewport {
                origin: [0.0, 0.0],
                dimensions: [viewport_dimensions[0] as f32, viewport_dimensions[1] as f32],
                depth_range: 0.0..1.0,
            }])
            .bind_pipeline_graphics(self.pipeline.clone())
            .bind_descriptor_sets(
                PipelineBindPoint::Graphics,
                self.pipeline.layout().clone(),
                0,
                desc_set,
            )
            .bind_vertex_buffers(0, self.vertices.clone())
            .bind_index_buffer(self.indices.clone())
            .draw_indexed(self.indices.len() as u32, 1, 0, 0, 0)
            .unwrap();
        builder.build().unwrap()
    }
}

mod vs {
    vulkano_shaders::shader! {
        ty: "vertex",
        src: "
#version 450
layout(location=0) in vec2 position;
layout(location=1) in vec2 tex_coords;

layout(location = 0) out vec2 f_tex_coords;

void main() {
    gl_Position =  vec4(position, 0.0, 1.0);
    f_tex_coords = tex_coords;
}
        "
    }
}

mod fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        src: "
#version 450
layout(location = 0) in vec2 v_tex_coords;

layout(location = 0) out vec4 f_color;

layout(set = 0, binding = 0) uniform sampler2D tex;

void main() {
    f_color = texture(tex, v_tex_coords);
}
"
    }
}
