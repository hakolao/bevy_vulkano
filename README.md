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

1. Add `VulkanoWinitPlugin`. (Don't forget to add `WindowPlugin`, and some basic bevy plugins). Don't add default plugins.
2. Then create your own rendering systems using vulkano's pipelines (See example.). You'll need to know how to use [Vulkano](https://github.com/vulkano-rs/vulkano).
3. If you want to use [egui](https://github.com/emilk/egui) library with this, add `egui` and `bevy_vulkano` with feature `gui`.

## Usage

See examples.

## Dependencies

This library re-exports `egui_winit_vulkano`.

## Examples:
```bash
cargo run --example multi_window_gui --features "gui links clipboard"
cargo run --example windowless_compute
cargo run --example game_of_life
```

### Contributing

Feel free to open a PR to improve or fix anything that you see would be useful.