#!/bin/bash
cargo run --example circle --features example_has_gui
cargo run --example circle
cargo run --example multi_window_gui --features example_has_gui
cargo run --example multi_window_gui
cargo run --example windowless_compute
cargo run --example game_of_life