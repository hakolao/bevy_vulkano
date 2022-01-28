use std::sync::Arc;

use anyhow::*;
pub use circle_draw_pipeline::*;
use vulkano::{
    command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage, SecondaryAutoCommandBuffer},
    descriptor_set::{layout::DescriptorSetLayout, PersistentDescriptorSet, WriteDescriptorSet},
    device::Queue,
    image::ImageViewAbstract,
    render_pass::Subpass,
    sampler::{Filter, Sampler, SamplerAddressMode, SamplerMipmapMode},
};

mod circle_draw_pipeline;

#[repr(C)]
#[derive(Default, Debug, Clone, Copy)]
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
    let builder = AutoCommandBufferBuilder::secondary_graphics(
        gfx_queue.device().clone(),
        gfx_queue.family(),
        CommandBufferUsage::MultipleSubmit,
        subpass,
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
    let sampler_builder = Sampler::start(gfx_queue.device().clone())
        .filter(Filter::Nearest)
        .address_mode(sampler_mode)
        .mipmap_mode(SamplerMipmapMode::Nearest)
        .mip_lod_bias(0.0)
        .lod(0.0..=0.0);
    let sampler = sampler_builder.build()?;
    Ok(PersistentDescriptorSet::new(layout.clone(), [
        WriteDescriptorSet::image_view_sampler(0, image.clone(), sampler),
    ])?)
}
