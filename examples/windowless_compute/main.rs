use bevy::{app::AppExit, prelude::*};
use bevy_vulkano::{VulkanoContext, VulkanoWinitConfig, VulkanoWinitPlugin};
use vulkano::{
    buffer::{BufferUsage, CpuAccessibleBuffer},
    command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage},
    descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet},
    pipeline::{ComputePipeline, Pipeline, PipelineBindPoint},
    sync,
    sync::GpuFuture,
};

// https://github.com/vulkano-rs/vulkano/blob/master/examples/src/bin/basic-compute-shader.rs

fn main() {
    App::new()
        .insert_resource(VulkanoWinitConfig {
            // No window...
            add_primary_window: false,
            ..VulkanoWinitConfig::default()
        })
        .add_plugin(VulkanoWinitPlugin::default())
        .add_startup_system(run_compute_shader_once_then_exit)
        .run();
}

/// Just a simple run once compute shader pipeline.
/// In a proper app you'd extract your compute shader pipeline ot an own struct and would run it on
/// our data e.g. each frame. For example, ray tracing and drawing on an image.
fn run_compute_shader_once_then_exit(
    vulkano_context: Res<VulkanoContext>,
    mut app_exit_events: EventWriter<AppExit>,
) {
    // Create pipeline
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
        let shader = cs::load(vulkano_context.device()).unwrap();
        ComputePipeline::new(
            vulkano_context.device(),
            shader.entry_point("main").unwrap(),
            &(),
            None,
            |_| {},
        )
        .unwrap()
    };
    // Create buffer
    let data_buffer = {
        let data_iter = (0..65536u32).map(|n| n);
        CpuAccessibleBuffer::from_iter(
            vulkano_context.device(),
            BufferUsage {
                storage_buffer: true,
                ..BufferUsage::none()
            },
            false,
            data_iter,
        )
        .unwrap()
    };

    // Create pipeline layout & descriptor set (data inputs)
    let layout = pipeline.layout().descriptor_set_layouts().get(0).unwrap();
    let set = PersistentDescriptorSet::new(layout.clone(), [WriteDescriptorSet::buffer(
        0,
        data_buffer.clone(),
    )])
    .unwrap();

    // Build command buffer
    let mut builder = AutoCommandBufferBuilder::primary(
        vulkano_context.device(),
        vulkano_context.compute_queue().family(),
        CommandBufferUsage::OneTimeSubmit,
    )
    .unwrap();
    builder
        .bind_pipeline_compute(pipeline.clone())
        .bind_descriptor_sets(
            PipelineBindPoint::Compute,
            pipeline.layout().clone(),
            0,
            set.clone(),
        )
        .dispatch([1024, 1, 1])
        .unwrap();
    let command_buffer = builder.build().unwrap();

    // Execute the command buffer & wait on it to finish
    let future = sync::now(vulkano_context.device())
        .then_execute(vulkano_context.compute_queue(), command_buffer)
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
