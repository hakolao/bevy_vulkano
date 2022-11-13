# bevy_vulkano

[![Crates.io](https://img.shields.io/crates/v/bevy_vulkano.svg)](https://crates.io/crates/bevy_vulkano)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)
![CI](https://github.com/hakolao/bevy_vulkano/workflows/CI/badge.svg)

This plugin replaces core loop & rendering in [Bevy](https://github.com/bevyengine/bevy) with [Vulkano](https://github.com/vulkano-rs/vulkano) backend.
Basically this allows you to be fully in control of your render pipelines with Vulkano without having to bother yourself with engine
architecture much. Just roll your pipelines and have fun.

This makes it extremely easy to do following with Vulkano:
- Windowless Apps
- Multiple Windows
- Event handling

From Vulkano's perspective, this plugin contains functionality for resizing, multiple windows & utility for beginning and ending the frame.
However, you'll need to do everything in between yourself. A good way to get started is to look at the examples.

This should be especially useful for learning graphics pipelines from scratch using Vulkano.

1. Add `VulkanoWinitPlugin`. It also adds `WindowPlugin` and anything that's needed.
2. Then create your own rendering systems using vulkano's pipelines (See example.). You'll need to know how to use [Vulkano](https://github.com/vulkano-rs/vulkano).
3. If you want to use [egui](https://github.com/emilk/egui) library with this, add `egui` and `bevy_vulkano` with feature `gui`.

## Usage

```rust
fn main() {
    App::new()
        // Vulkano configs (Modify this if you want to add features to vulkano (vulkan backend).
        // You can also disable primary window opening here
        .insert_non_send_resource(VulkanoWinitConfig::default())
        .add_plugin(bevy::input::InputPlugin::default())
        // Window settings for primary window (if you want no window, modify config above)
        .add_plugin(VulkanoWinitPlugin {
            window_descriptor: WindowDescriptor {
                width: 1920.0,
                height: 1080.0,
                title: "Bevy Vulkano".to_string(),
                present_mode: bevy::window::PresentMode::Immediate,
                resizable: true,
                mode: WindowMode::Windowed,
                ..WindowDescriptor::default()
            }
        })
        .run();
}
```

### Creating a pipeline

```rust
/// Creates a render pipeline. Add this system with app.add_startup_system(create_pipelines).
fn create_pipelines_system(mut commands: Commands, vulkano_windows: NonSend<VulkanoWindows>) {
    let primary_window = vulkano_windows.get_primary_window_renderer().unwrap();
    // Create your render pass & pipelines (MyRenderPass could contain your pipelines, e.g. draw_circle)
    let my_pipeline = YourPipeline::new(
        primary_window.graphics_queue(),
        primary_window.swapchain_format(),
    );
    // Insert as a resource
    commands.insert_resource(my_pipeline);
}
```

### Rendering system

```rust

/// This system should be added either at `CoreStage::PostUpdate` or `CoreStage::Last`. You could also create your own
/// render stage and place it after `CoreStage::Update`.
fn my_pipeline_render_system(
    mut vulkano_windows: NonSendMut<VulkanoWindows>,
    mut pipeline: ResMut<YourPipeline>,
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

    // Access the swapchain image directly
    let final_image = primary_window.swapchain_image_view();
    // Draw your pipeline
    let after_your_pipeline = pipeline.draw(final_image);
    
    // Finish Frame by passing your last future. Wait on the future if needed.
    primary_window.present(after_your_pipeline, true);
}
```

## Dependencies

This library re-exports `egui_winit_vulkano`.

## Examples:
```bash
cargo run --example multi_window_gui --features example_has_gui
cargo run --example windowless_compute
cargo run --example game_of_life
```

### Disclaimer
While you can use `bevy_vulkano` with bevy `0.9`,
the windowing features are not quite up to date with latest `bevy_window`.
Feel free to make contributions if some feature is missing.

### Contributing

Feel free to open a PR to improve or fix anything that you see would be useful.