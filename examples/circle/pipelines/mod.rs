#![allow(clippy::needless_question_mark)]

use std::sync::Arc;

use anyhow::*;
use bytemuck::{Pod, Zeroable};
pub use circle_draw_pipeline::*;
use vulkano::{
    command_buffer::{
        AutoCommandBufferBuilder, CommandBufferInheritanceInfo, CommandBufferUsage,
        SecondaryAutoCommandBuffer,
    },
    descriptor_set::{layout::DescriptorSetLayout, PersistentDescriptorSet, WriteDescriptorSet},
    device::Queue,
    image::ImageViewAbstract,
    render_pass::Subpass,
    sampler::{Filter, Sampler, SamplerAddressMode, SamplerCreateInfo, SamplerMipmapMode},
};

mod circle_draw_pipeline;

#[repr(C)]
#[derive(Default, Debug, Copy, Clone, Zeroable, Pod)]
pub struct TextVertex {
    pub position: [f32; 2],
    pub normal: [f32; 2],
    pub tex_coords: [f32; 2],
    pub color: [f32; 4],
}
vulkano::impl_vertex!(TextVertex, position, normal, tex_coords, color);

pub fn textured_quad(color: [f32; 4], width: f32, height: f32) -> (Vec<TextVertex>, Vec<u32>) {
    (
        vec![
            TextVertex {
                position: [-(width / 2.0), -(height / 2.0)],
                normal: [0.0, 0.0],
                tex_coords: [0.0, 1.0],
                color,
            },
            TextVertex {
                position: [-(width / 2.0), height / 2.0],
                normal: [0.0, 0.0],
                tex_coords: [0.0, 0.0],
                color,
            },
            TextVertex {
                position: [width / 2.0, height / 2.0],
                normal: [0.0, 0.0],
                tex_coords: [1.0, 0.0],
                color,
            },
            TextVertex {
                position: [width / 2.0, -(height / 2.0)],
                normal: [0.0, 0.0],
                tex_coords: [1.0, 1.0],
                color,
            },
        ],
        vec![0, 2, 1, 0, 3, 2],
    )
}

pub fn command_buffer_builder(
    gfx_queue: Arc<Queue>,
    subpass: Subpass,
) -> Result<AutoCommandBufferBuilder<SecondaryAutoCommandBuffer>> {
    let builder = AutoCommandBufferBuilder::secondary(
        gfx_queue.device().clone(),
        gfx_queue.family(),
        CommandBufferUsage::MultipleSubmit,
        CommandBufferInheritanceInfo {
            render_pass: Some(subpass.clone().into()),
            ..Default::default()
        },
    )?;
    Ok(builder)
}

/// Creates a descriptor set for images
#[allow(unused)]
pub fn sampled_image_desc_set(
    gfx_queue: Arc<Queue>,
    layout: &Arc<DescriptorSetLayout>,
    image: Arc<dyn ImageViewAbstract + 'static>,
    sampler_mode: SamplerAddressMode,
) -> Result<Arc<PersistentDescriptorSet>> {
    let sampler = Sampler::new(gfx_queue.device().clone(), SamplerCreateInfo {
        mag_filter: Filter::Nearest,
        min_filter: Filter::Nearest,
        address_mode: [sampler_mode; 3],
        mipmap_mode: SamplerMipmapMode::Nearest,
        ..Default::default()
    })
    .unwrap();

    Ok(PersistentDescriptorSet::new(layout.clone(), [
        WriteDescriptorSet::image_view_sampler(0, image.clone(), sampler),
    ])?)
}
