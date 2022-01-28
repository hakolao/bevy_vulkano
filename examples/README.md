# bevy_vulkano

This plugin replaces core loop & rendering in bevy with Vulkano backend. Currenlty you can only use it for a single window.

Provides a `Renderer` to organize target images and exposes an api to `start_frame` and `end_frame`.
However, you'll have handle rendering yourself in between. Resizing is handled as well. Provide your own camera though.

See example `circle` for how to use this.

1. Add `VulkanoWinitPlugin`. You'll need bevy's `WindowPlugin`
2. Then create your own rendering systems using vulkano's pipelines (See example.). You'll need to know how to use Vulkano.
3. If you want to use `egui` library with this, add `egui_winit_vulkano` & `egui` to your dependencies and `bevy_vulkano` with feature `gui`

```rust
pub struct PluginBundle;

impl PluginGroup for PluginBundle {
    fn build(&mut self, group: &mut PluginGroupBuilder) {
        group.add(bevy::log::LogPlugin::default());
        group.add(bevy::core::CorePlugin::default());
        group.add(bevy::transform::TransformPlugin::default());
        group.add(bevy::diagnostic::DiagnosticsPlugin::default());
        group.add(bevy::diagnostic::FrameTimeDiagnosticsPlugin::default());
        group.add(bevy::asset::AssetPlugin::default());
        group.add(bevy::scene::ScenePlugin::default());
        group.add(bevy::input::InputPlugin::default());
        group.add(bevy::window::WindowPlugin::default());
        // Don't add default bevy plugins or WinitPlugin. This owns "core loop" (runner)
        group.add(VulkanoWinitPlugin::default());
        // See here how rendering is orchestrated
        group.add(MainRenderPlugin::default());
    }
}

fn main() {
    App::new()
        // Vulkano configs
        .insert_resource(VulkanoWinitConfig {
            features: Features {
                fill_mode_non_solid: true,
                ..Features::none()
            },
            present_mode: PresentMode::Immediate,
            ..VulkanoWinitConfig::default()
        })
        // Window configs
        .insert_resource(WindowDescriptor {
            width: 1920.0,
            height: 1080.0,
            title: "Bevy Vulkano".to_string(),
            vsync: false,
            resizable: true,
            mode: WindowMode::Windowed,
            ..WindowDescriptor::default()
        })
        .add_plugins(PluginBundle)
        .add_system(exit_on_esc_system)
        .run();
}
```

## Examples:
```bash
cargo run --example circle --features example_has_gui
cargo run --example circle
```

ToDo:
- [ ] Multi-window