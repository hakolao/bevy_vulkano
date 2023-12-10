use bevy::{app::AppExit, prelude::*};
use bevy_vulkano::{BevyVulkanoContext, VulkanoWinitPlugin};
use vulkano::{
    buffer::{Buffer, BufferCreateInfo, BufferUsage},
    command_buffer::{
        allocator::StandardCommandBufferAllocator, AutoCommandBufferBuilder, CommandBufferUsage,
    },
    descriptor_set::{
        allocator::StandardDescriptorSetAllocator, PersistentDescriptorSet, WriteDescriptorSet,
    },
    memory::allocator::{AllocationCreateInfo, MemoryTypeFilter},
    pipeline::{
        compute::ComputePipelineCreateInfo, layout::PipelineDescriptorSetLayoutCreateInfo,
        ComputePipeline, Pipeline, PipelineBindPoint, PipelineLayout,
        PipelineShaderStageCreateInfo,
    },
    sync,
    sync::GpuFuture,
};

// https://github.com/vulkano-rs/vulkano/blob/master/examples/src/bin/basic-compute-shader.rs

fn main() {
    App::new()
        .add_plugins((
            bevy::log::LogPlugin::default(),
            bevy::core::TaskPoolPlugin::default(),
            WindowPlugin {
                primary_window: None,
                ..default()
            },
            VulkanoWinitPlugin,
        ))
        .add_systems(Startup, run_compute_shader_once_then_exit)
        .run();
}

/// Just a simple run once compute shader pipeline.
/// In a proper app you'd extract your compute shader pipeline ot an own struct and would run it on
/// our data e.g. each frame. For example, ray tracing and drawing on an image.
fn run_compute_shader_once_then_exit(
    context: Res<BevyVulkanoContext>,
    mut app_exit_events: EventWriter<AppExit>,
) {
    // Create pipeline
    #[allow(clippy::needless_question_mark)]
    let pipeline = {
        mod cs {
            vulkano_shaders::shader! {
                ty: "compute",
                src: "
                    #version 450
                    layout(local_size_x = 64, local_size_y = 1, local_size_z = 1) in;
                    layout(set = 0, binding = 0) buffer Data {
                        uint data[];
                    } data;
                    void main() {
                        uint idx = gl_GlobalInvocationID.x;
                        data.data[idx] *= 12;
                    }
                "
            }
        }
        let cs = cs::load(context.context.device().clone())
            .unwrap()
            .entry_point("main")
            .unwrap();
        let stage = PipelineShaderStageCreateInfo::new(cs);
        let layout = PipelineLayout::new(
            context.context.device().clone(),
            PipelineDescriptorSetLayoutCreateInfo::from_stages([&stage])
                .into_pipeline_layout_create_info(context.context.device().clone())
                .unwrap(),
        )
        .unwrap();

        ComputePipeline::new(
            context.context.device().clone(),
            None,
            ComputePipelineCreateInfo::stage_layout(stage, layout),
        )
        .unwrap()
    };

    // Create buffer
    let data_buffer = Buffer::from_iter(
        context.context.memory_allocator().clone(),
        BufferCreateInfo {
            usage: BufferUsage::STORAGE_BUFFER,
            ..Default::default()
        },
        AllocationCreateInfo {
            memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                | MemoryTypeFilter::HOST_RANDOM_ACCESS,
            ..Default::default()
        },
        (0..65536u32).collect::<Vec<u32>>(),
    )
    .unwrap();

    let command_buffer_allocator =
        StandardCommandBufferAllocator::new(context.context.device().clone(), Default::default());

    let descriptor_set_allocator =
        StandardDescriptorSetAllocator::new(context.context.device().clone(), Default::default());

    // Create pipeline layout & descriptor set (data inputs)
    let layout = pipeline.layout().set_layouts().get(0).unwrap();
    let set = PersistentDescriptorSet::new(
        &descriptor_set_allocator,
        layout.clone(),
        [WriteDescriptorSet::buffer(0, data_buffer.clone())],
        [],
    )
    .unwrap();

    // Build command buffer
    let mut builder = AutoCommandBufferBuilder::primary(
        &command_buffer_allocator,
        context.context.compute_queue().queue_family_index(),
        CommandBufferUsage::OneTimeSubmit,
    )
    .unwrap();
    builder
        .bind_pipeline_compute(pipeline.clone())
        .unwrap()
        .bind_descriptor_sets(
            PipelineBindPoint::Compute,
            pipeline.layout().clone(),
            0,
            set,
        )
        .unwrap()
        .dispatch([1024, 1, 1])
        .unwrap();
    let command_buffer = builder.build().unwrap();

    // Execute the command buffer & wait on it to finish
    let future = sync::now(context.context.device().clone())
        .then_execute(context.context.compute_queue().clone(), command_buffer)
        .unwrap()
        .then_signal_fence_and_flush()
        .unwrap();
    future.wait(None).unwrap();

    // Ensure our data has been updated by the computation
    let data_buffer_content = data_buffer.read().unwrap();
    for n in 0..65536u32 {
        assert_eq!(data_buffer_content[n as usize], n * 12);
    }

    // Exit
    app_exit_events.send(AppExit);

    println!("Compute shader successfully ran, exiting the example");
}
