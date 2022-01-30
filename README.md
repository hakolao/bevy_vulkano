# bevy_vulkano

![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

This plugin replaces core loop & rendering in bevy with Vulkano backend.

The plugin contains functionality for resizing, multiple windows & utility for beginning and ending the frame.
However, you'll need to do everything in between yourself (rendering). A good way to get started is to look at the examples.

1. Add `VulkanoWinitPlugin`. It also adds `WindowPlugin` and anything that's needed.
2. Then create your own rendering systems using vulkano's pipelines (See example.). You'll need to know how to use Vulkano.
3. If you want to use `egui` library with this, add `egui_winit_vulkano` & `egui` to your dependencies and `bevy_vulkano` with feature `gui`.

## Usage

```rust
pub struct PluginBundle;

impl PluginGroup for PluginBundle {
    fn build(&mut self, group: &mut PluginGroupBuilder) {
        group.add(bevy::input::InputPlugin::default());
        group.add(VulkanoWinitPlugin::default());
    }
}

fn main() {
    App::new()
        // Vulkano configs (Modify this if you want to add features to vulkano (vulkan backend).
        // You can also disable primary window opening here
        .insert_resource(VulkanoWinitConfig::default())
        // Window configs for primary window
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
        .run();
}
```

## Dependencies

Add following to your `Cargo.toml`:
```toml
[dependencies.bevy]
version = "0.6"
default-features = false
# Add features you need, but don't add "render". This might disable a lot of features you wanted... e.g SpritePlugin
features = []

[dependencies.bevy_vulkano]
version = "*"
default-features = false
# gui or no gui...
features = ["gui"]

# For gui
egui = "0.16.0"
egui_winit_vulkano = "0.15"
# For render pipelines etc.
vulkano-shaders = "0.28"
vulkano = "0.28"
vulkano-win = "0.28"
vulkano-win = "0.28"

```

## Examples:
```bash
cargo run --example circle --features example_has_gui
cargo run --example circle
cargo run --example multi_window_gui --features example_has_gui
cargo run --example windowless_compute
```

### Contributing

ToDo:
- [ ] Update Cargo.toml to new crates once Vulkano 0.28 is out...
- [ ] Publish as a crate
- [ ] Github action workflow