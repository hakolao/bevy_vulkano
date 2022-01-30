#![allow(
    clippy::needless_question_mark,
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::module_inception
)]

/*
Pretty much the same as bevy_winit, but organized to use vulkano renderer backend.
This allows you to create your own pipelines for rendering.
 */
mod converters;
mod pipeline_frame_data;
mod utils;
mod vulkano_context;
mod vulkano_window;
mod winit_config;
mod winit_window_renderer;

use bevy::{
    app::{App, AppExit, CoreStage, EventReader, Events, ManualEventReader, Plugin},
    input::{
        keyboard::KeyboardInput,
        mouse::{MouseButtonInput, MouseMotion, MouseScrollUnit, MouseWheel},
        touch::TouchInput,
    },
    math::{ivec2, DVec2, Vec2},
    prelude::*,
    window::{
        CreateWindow, CursorEntered, CursorLeft, CursorMoved, FileDragAndDrop, ReceivedCharacter,
        WindowBackendScaleFactorChanged, WindowCloseRequested, WindowCreated, WindowFocused,
        WindowId, WindowMoved, WindowResized, WindowScaleFactorChanged, Windows,
    },
};
pub use pipeline_frame_data::*;
pub use utils::*;
use vulkano::{
    device::{DeviceExtensions, Features},
    instance::InstanceExtensions,
};
pub use vulkano_context::*;
pub use vulkano_window::*;
use winit::{
    dpi::{LogicalSize, PhysicalPosition},
    event::{self, DeviceEvent, Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop, EventLoopWindowTarget},
};
pub use winit_config::*;
pub use winit_window_renderer::*;

/// Vulkano related configurations
pub struct VulkanoWinitConfig {
    pub add_primary_window: bool,
    pub instance_extensions: InstanceExtensions,
    pub device_extensions: DeviceExtensions,
    pub features: Features,
    pub layers: Vec<&'static str>,
}

impl Default for VulkanoWinitConfig {
    fn default() -> Self {
        VulkanoWinitConfig {
            add_primary_window: true,
            instance_extensions: InstanceExtensions {
                ext_debug_utils: true,
                ..required_extensions()
            },
            device_extensions: DeviceExtensions {
                khr_swapchain: true,
                ..DeviceExtensions::none()
            },
            features: Features::none(),
            layers: vec![],
        }
    }
}

/// Plugin that allows replacing Bevy's render backend with Vulkano. See examples for usage.
#[derive(Default)]
pub struct VulkanoWinitPlugin;

impl Plugin for VulkanoWinitPlugin {
    fn build(&self, app: &mut App) {
        // Create event loop, window and renderer (tied together...)
        let event_loop = EventLoop::new();

        // Insert config if none
        if app.world.get_resource::<VulkanoWinitConfig>().is_none() {
            app.insert_resource(VulkanoWinitConfig::default());
        }
        let config = app.world.get_resource::<VulkanoWinitConfig>().unwrap();

        // Add WindowPlugin
        let add_primary_window = config.add_primary_window;

        // Create vulkano context
        let vulkano_context = VulkanoContext::new(config);

        // Insert window plugin, vulkano context, windows resource & pipeline data
        app.add_plugin(bevy::window::WindowPlugin {
            add_primary_window,
            // We don't want to run exit_on_close_system from WindowPlugin, because it closes the entire app on each window close
            exit_on_close: false,
        })
        .init_resource::<VulkanoWinitWindows>()
        .init_resource::<PipelineData>()
        .insert_resource(vulkano_context);

        // Create initial window
        handle_initial_window_events(&mut app.world, &event_loop);

        app.insert_non_send_resource(event_loop)
            .set_runner(winit_runner)
            .add_system_to_stage(CoreStage::PreUpdate, update_on_resize_system)
            .add_system_to_stage(CoreStage::PreUpdate, exit_on_window_close_system)
            .add_system_to_stage(CoreStage::PostUpdate, change_window.exclusive_system());
        // Add gui begin frame system
        #[cfg(feature = "gui")]
        app.add_system_to_stage(CoreStage::PreUpdate, begin_egui_frame_system);
    }
}

fn update_on_resize_system(
    mut pipeline_data: ResMut<PipelineData>,
    mut windows: ResMut<VulkanoWinitWindows>,
    mut window_resized_events: EventReader<WindowResized>,
    mut window_created_events: EventReader<WindowCreated>,
) {
    let mut changed_window_ids = Vec::new();
    for event in window_resized_events.iter().rev() {
        if changed_window_ids.contains(&event.id) {
            continue;
        }
        changed_window_ids.push(event.id);
    }
    for event in window_created_events.iter().rev() {
        if changed_window_ids.contains(&event.id) {
            continue;
        }
        changed_window_ids.push(event.id);
    }
    for id in changed_window_ids {
        if let Some(vulkano_window) = windows.get_vulkano_window_mut(id) {
            // Swap chain will be resized at the beginning of next frame. But user should update pipeline frame data
            vulkano_window.resize();
            // Insert or update pipeline frame data
            pipeline_data.add(PipelineFrameData {
                window_id: id,
                before: None,
                after: None,
            });
        }
    }
}

fn change_window(world: &mut World) {
    let world = world.cell();
    let vulkano_winit_windows = world.get_resource::<VulkanoWinitWindows>().unwrap();
    let mut windows = world.get_resource_mut::<Windows>().unwrap();

    for bevy_window in windows.iter_mut() {
        let id = bevy_window.id();
        for command in bevy_window.drain_commands() {
            match command {
                bevy::window::WindowCommand::SetWindowMode {
                    mode,
                    resolution: (width, height),
                } => {
                    let window = vulkano_winit_windows.get_winit_window(id).unwrap();
                    match mode {
                        bevy::window::WindowMode::BorderlessFullscreen => {
                            window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)))
                        }
                        bevy::window::WindowMode::Fullscreen => {
                            window.set_fullscreen(Some(winit::window::Fullscreen::Exclusive(
                                get_best_videomode(&window.current_monitor().unwrap()),
                            )))
                        }
                        bevy::window::WindowMode::SizedFullscreen => window.set_fullscreen(Some(
                            winit::window::Fullscreen::Exclusive(get_fitting_videomode(
                                &window.current_monitor().unwrap(),
                                width,
                                height,
                            )),
                        )),
                        bevy::window::WindowMode::Windowed => window.set_fullscreen(None),
                    }
                }
                bevy::window::WindowCommand::SetTitle {
                    title,
                } => {
                    let window = vulkano_winit_windows.get_winit_window(id).unwrap();
                    window.set_title(&title);
                }
                bevy::window::WindowCommand::SetScaleFactor {
                    scale_factor,
                } => {
                    let mut window_dpi_changed_events = world
                        .get_resource_mut::<Events<WindowScaleFactorChanged>>()
                        .unwrap();
                    window_dpi_changed_events.send(WindowScaleFactorChanged {
                        id,
                        scale_factor,
                    });
                }
                bevy::window::WindowCommand::SetResolution {
                    logical_resolution: (width, height),
                    scale_factor,
                } => {
                    let window = vulkano_winit_windows.get_winit_window(id).unwrap();
                    window.set_inner_size(
                        winit::dpi::LogicalSize::new(width, height)
                            .to_physical::<f64>(scale_factor),
                    );
                }
                bevy::window::WindowCommand::SetVsync {
                    ..
                } => (),
                bevy::window::WindowCommand::SetResizable {
                    resizable,
                } => {
                    let window = vulkano_winit_windows.get_winit_window(id).unwrap();
                    window.set_resizable(resizable);
                }
                bevy::window::WindowCommand::SetDecorations {
                    decorations,
                } => {
                    let window = vulkano_winit_windows.get_winit_window(id).unwrap();
                    window.set_decorations(decorations);
                }
                bevy::window::WindowCommand::SetCursorIcon {
                    icon,
                } => {
                    let window = vulkano_winit_windows.get_winit_window(id).unwrap();
                    window.set_cursor_icon(converters::convert_cursor_icon(icon));
                }
                bevy::window::WindowCommand::SetCursorLockMode {
                    locked,
                } => {
                    let window = vulkano_winit_windows.get_winit_window(id).unwrap();
                    window
                        .set_cursor_grab(locked)
                        .unwrap_or_else(|e| error!("Unable to un/grab cursor: {}", e));
                }
                bevy::window::WindowCommand::SetCursorVisibility {
                    visible,
                } => {
                    let window = vulkano_winit_windows.get_winit_window(id).unwrap();
                    window.set_cursor_visible(visible);
                }
                bevy::window::WindowCommand::SetCursorPosition {
                    position,
                } => {
                    let window = vulkano_winit_windows.get_winit_window(id).unwrap();
                    let inner_size = window.inner_size().to_logical::<f32>(window.scale_factor());
                    window
                        .set_cursor_position(winit::dpi::LogicalPosition::new(
                            position.x,
                            inner_size.height - position.y,
                        ))
                        .unwrap_or_else(|e| error!("Unable to set cursor position: {}", e));
                }
                bevy::window::WindowCommand::SetMaximized {
                    maximized,
                } => {
                    let window = vulkano_winit_windows.get_winit_window(id).unwrap();
                    window.set_maximized(maximized)
                }
                bevy::window::WindowCommand::SetMinimized {
                    minimized,
                } => {
                    let window = vulkano_winit_windows.get_winit_window(id).unwrap();
                    window.set_minimized(minimized)
                }
                bevy::window::WindowCommand::SetPosition {
                    position,
                } => {
                    let window = vulkano_winit_windows.get_winit_window(id).unwrap();
                    window.set_outer_position(PhysicalPosition {
                        x: position[0],
                        y: position[1],
                    });
                }
                bevy::window::WindowCommand::SetResizeConstraints {
                    resize_constraints,
                } => {
                    let window = vulkano_winit_windows.get_winit_window(id).unwrap();
                    let constraints = resize_constraints.check_constraints();
                    let min_inner_size = LogicalSize {
                        width: constraints.min_width,
                        height: constraints.min_height,
                    };
                    let max_inner_size = LogicalSize {
                        width: constraints.max_width,
                        height: constraints.max_height,
                    };

                    window.set_min_inner_size(Some(min_inner_size));
                    if constraints.max_width.is_finite() && constraints.max_height.is_finite() {
                        window.set_max_inner_size(Some(max_inner_size));
                    }
                }
            }
        }
    }
}

fn run<F>(event_loop: EventLoop<()>, event_handler: F) -> !
where
    F: 'static + FnMut(Event<'_, ()>, &EventLoopWindowTarget<()>, &mut ControlFlow),
{
    event_loop.run(event_handler)
}

#[cfg(any(
    target_os = "windows",
    target_os = "macos",
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]
fn run_return<F>(event_loop: &mut EventLoop<()>, event_handler: F)
where
    F: FnMut(Event<'_, ()>, &EventLoopWindowTarget<()>, &mut ControlFlow),
{
    use winit::platform::run_return::EventLoopExtRunReturn;
    event_loop.run_return(event_handler)
}

#[cfg(not(any(
    target_os = "windows",
    target_os = "macos",
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
)))]
fn run_return<F>(_event_loop: &mut EventLoop<()>, _event_handler: F)
where
    F: FnMut(Event<'_, ()>, &EventLoopWindowTarget<()>, &mut ControlFlow),
{
    panic!("Run return is not supported on this platform!")
}

pub fn winit_runner(app: App) {
    winit_runner_with(app);
}

pub fn winit_runner_with(mut app: App) {
    let mut event_loop = app.world.remove_non_send::<EventLoop<()>>().unwrap();
    let mut create_window_event_reader = ManualEventReader::<CreateWindow>::default();
    let mut app_exit_event_reader = ManualEventReader::<AppExit>::default();
    app.world.insert_non_send(event_loop.create_proxy());

    trace!("Entering winit event loop");

    let should_return_from_run = app
        .world
        .get_resource::<WinitConfig>()
        .map_or(false, |config| config.return_from_run);

    let mut active = true;

    let event_handler = move |event: Event<()>,
                              event_loop: &EventLoopWindowTarget<()>,
                              control_flow: &mut ControlFlow| {
        *control_flow = ControlFlow::Poll;

        if let Some(app_exit_events) = app.world.get_resource_mut::<Events<AppExit>>() {
            if app_exit_event_reader
                .iter(&app_exit_events)
                .next_back()
                .is_some()
            {
                *control_flow = ControlFlow::Exit;
            }
        }

        // Update gui with winit event
        #[cfg(feature = "gui")]
        {
            let event_wrapper = &event;
            match &event_wrapper {
                event::Event::WindowEvent {
                    event: _event,
                    window_id: winit_window_id,
                    ..
                } => {
                    let world = app.world.cell();
                    let mut vulkano_winit_windows =
                        world.get_resource_mut::<VulkanoWinitWindows>().unwrap();
                    let window_id = if let Some(window_id) =
                        vulkano_winit_windows.get_window_id(*winit_window_id)
                    {
                        window_id
                    } else {
                        return;
                    };
                    if let Some(vulkano_window) =
                        vulkano_winit_windows.get_vulkano_window_mut(window_id)
                    {
                        // Update egui with the window event
                        vulkano_window.gui().update(event_wrapper);
                    }
                }
                _ => (),
            }
        }

        // Main events...
        match event {
            event::Event::WindowEvent {
                event,
                window_id: winit_window_id,
                ..
            } => {
                let world = app.world.cell();
                let vulkano_winit_windows =
                    world.get_resource_mut::<VulkanoWinitWindows>().unwrap();
                let mut windows = world.get_resource_mut::<Windows>().unwrap();
                let window_id =
                    if let Some(window_id) = vulkano_winit_windows.get_window_id(winit_window_id) {
                        window_id
                    } else {
                        warn!(
                            "Skipped event for unknown winit Window Id {:?}",
                            winit_window_id
                        );
                        return;
                    };

                let window = if let Some(window) = windows.get_mut(window_id) {
                    window
                } else {
                    warn!("Skipped event for unknown Window Id {:?}", winit_window_id);
                    return;
                };

                match event {
                    WindowEvent::Resized(size) => {
                        window.update_actual_size_from_backend(size.width, size.height);
                        let mut resize_events =
                            world.get_resource_mut::<Events<WindowResized>>().unwrap();
                        resize_events.send(WindowResized {
                            id: window_id,
                            width: window.width(),
                            height: window.height(),
                        });
                    }
                    WindowEvent::CloseRequested => {
                        let mut window_close_requested_events = world
                            .get_resource_mut::<Events<WindowCloseRequested>>()
                            .unwrap();
                        window_close_requested_events.send(WindowCloseRequested {
                            id: window_id,
                        });
                    }
                    WindowEvent::KeyboardInput {
                        ref input, ..
                    } => {
                        let mut keyboard_input_events =
                            world.get_resource_mut::<Events<KeyboardInput>>().unwrap();
                        keyboard_input_events.send(converters::convert_keyboard_input(input));
                    }
                    WindowEvent::CursorMoved {
                        position, ..
                    } => {
                        let mut cursor_moved_events =
                            world.get_resource_mut::<Events<CursorMoved>>().unwrap();
                        let winit_window =
                            vulkano_winit_windows.get_winit_window(window_id).unwrap();
                        let inner_size = winit_window.inner_size();

                        // move origin to bottom left
                        let y_position = inner_size.height as f64 - position.y;

                        let physical_position = DVec2::new(position.x, y_position);
                        window
                            .update_cursor_physical_position_from_backend(Some(physical_position));

                        cursor_moved_events.send(CursorMoved {
                            id: window_id,
                            position: (physical_position / window.scale_factor()).as_vec2(),
                        });
                    }
                    WindowEvent::CursorEntered {
                        ..
                    } => {
                        let mut cursor_entered_events =
                            world.get_resource_mut::<Events<CursorEntered>>().unwrap();
                        cursor_entered_events.send(CursorEntered {
                            id: window_id,
                        });
                    }
                    WindowEvent::CursorLeft {
                        ..
                    } => {
                        let mut cursor_left_events =
                            world.get_resource_mut::<Events<CursorLeft>>().unwrap();
                        window.update_cursor_physical_position_from_backend(None);
                        cursor_left_events.send(CursorLeft {
                            id: window_id,
                        });
                    }
                    WindowEvent::MouseInput {
                        state,
                        button,
                        ..
                    } => {
                        let mut mouse_button_input_events = world
                            .get_resource_mut::<Events<MouseButtonInput>>()
                            .unwrap();
                        mouse_button_input_events.send(MouseButtonInput {
                            button: converters::convert_mouse_button(button),
                            state: converters::convert_element_state(state),
                        });
                    }
                    WindowEvent::MouseWheel {
                        delta, ..
                    } => match delta {
                        event::MouseScrollDelta::LineDelta(x, y) => {
                            let mut mouse_wheel_input_events =
                                world.get_resource_mut::<Events<MouseWheel>>().unwrap();
                            mouse_wheel_input_events.send(MouseWheel {
                                unit: MouseScrollUnit::Line,
                                x,
                                y,
                            });
                        }
                        event::MouseScrollDelta::PixelDelta(p) => {
                            let mut mouse_wheel_input_events =
                                world.get_resource_mut::<Events<MouseWheel>>().unwrap();
                            mouse_wheel_input_events.send(MouseWheel {
                                unit: MouseScrollUnit::Pixel,
                                x: p.x as f32,
                                y: p.y as f32,
                            });
                        }
                    },
                    WindowEvent::Touch(touch) => {
                        let mut touch_input_events =
                            world.get_resource_mut::<Events<TouchInput>>().unwrap();

                        let mut location = touch.location.to_logical(window.scale_factor());

                        // On a mobile window, the start is from the top while on PC/Linux/OSX from
                        // bottom
                        if cfg!(target_os = "android") || cfg!(target_os = "ios") {
                            let window_height = windows.get_primary().unwrap().height();
                            location.y = window_height - location.y;
                        }
                        touch_input_events.send(converters::convert_touch_input(touch, location));
                    }
                    WindowEvent::ReceivedCharacter(c) => {
                        let mut char_input_events = world
                            .get_resource_mut::<Events<ReceivedCharacter>>()
                            .unwrap();

                        char_input_events.send(ReceivedCharacter {
                            id: window_id,
                            char: c,
                        })
                    }
                    WindowEvent::ScaleFactorChanged {
                        scale_factor,
                        new_inner_size,
                    } => {
                        let mut backend_scale_factor_change_events = world
                            .get_resource_mut::<Events<WindowBackendScaleFactorChanged>>()
                            .unwrap();
                        backend_scale_factor_change_events.send(WindowBackendScaleFactorChanged {
                            id: window_id,
                            scale_factor,
                        });
                        let prior_factor = window.scale_factor();
                        window.update_scale_factor_from_backend(scale_factor);
                        let new_factor = window.scale_factor();
                        if let Some(forced_factor) = window.scale_factor_override() {
                            // If there is a scale factor override, then force that to be used
                            // Otherwise, use the OS suggested size
                            // We have already told the OS about our resize constraints, so
                            // the new_inner_size should take those into account
                            *new_inner_size = winit::dpi::LogicalSize::new(
                                window.requested_width(),
                                window.requested_height(),
                            )
                            .to_physical::<u32>(forced_factor);
                        } else if approx::relative_ne!(new_factor, prior_factor) {
                            let mut scale_factor_change_events = world
                                .get_resource_mut::<Events<WindowScaleFactorChanged>>()
                                .unwrap();

                            scale_factor_change_events.send(WindowScaleFactorChanged {
                                id: window_id,
                                scale_factor,
                            });
                        }

                        let new_logical_width = new_inner_size.width as f64 / new_factor;
                        let new_logical_height = new_inner_size.height as f64 / new_factor;
                        if approx::relative_ne!(window.width() as f64, new_logical_width)
                            || approx::relative_ne!(window.height() as f64, new_logical_height)
                        {
                            let mut resize_events =
                                world.get_resource_mut::<Events<WindowResized>>().unwrap();
                            resize_events.send(WindowResized {
                                id: window_id,
                                width: new_logical_width as f32,
                                height: new_logical_height as f32,
                            });
                        }
                        window.update_actual_size_from_backend(
                            new_inner_size.width,
                            new_inner_size.height,
                        );
                    }
                    WindowEvent::Focused(focused) => {
                        window.update_focused_status_from_backend(focused);
                        let mut focused_events =
                            world.get_resource_mut::<Events<WindowFocused>>().unwrap();
                        focused_events.send(WindowFocused {
                            id: window_id,
                            focused,
                        });
                    }
                    WindowEvent::DroppedFile(path_buf) => {
                        let mut events =
                            world.get_resource_mut::<Events<FileDragAndDrop>>().unwrap();
                        events.send(FileDragAndDrop::DroppedFile {
                            id: window_id,
                            path_buf,
                        });
                    }
                    WindowEvent::HoveredFile(path_buf) => {
                        let mut events =
                            world.get_resource_mut::<Events<FileDragAndDrop>>().unwrap();
                        events.send(FileDragAndDrop::HoveredFile {
                            id: window_id,
                            path_buf,
                        });
                    }
                    WindowEvent::HoveredFileCancelled => {
                        let mut events =
                            world.get_resource_mut::<Events<FileDragAndDrop>>().unwrap();
                        events.send(FileDragAndDrop::HoveredFileCancelled {
                            id: window_id,
                        });
                    }
                    WindowEvent::Moved(position) => {
                        let position = ivec2(position.x, position.y);
                        window.update_actual_position_from_backend(position);
                        let mut events = world.get_resource_mut::<Events<WindowMoved>>().unwrap();
                        events.send(WindowMoved {
                            id: window_id,
                            position,
                        });
                    }
                    _ => {}
                }
            }
            event::Event::DeviceEvent {
                event: DeviceEvent::MouseMotion {
                    delta,
                },
                ..
            } => {
                let mut mouse_motion_events =
                    app.world.get_resource_mut::<Events<MouseMotion>>().unwrap();
                mouse_motion_events.send(MouseMotion {
                    delta: Vec2::new(delta.0 as f32, delta.1 as f32),
                });
            }
            event::Event::Suspended => {
                active = false;
            }
            event::Event::Resumed => {
                active = true;
            }
            event::Event::MainEventsCleared => {
                handle_create_window_events(
                    &mut app.world,
                    event_loop,
                    &mut create_window_event_reader,
                );
                if active {
                    app.update();
                }
            }
            _ => (),
        }
    };
    if should_return_from_run {
        run_return(&mut event_loop, event_handler);
    } else {
        run(event_loop, event_handler);
    }
}

fn handle_create_window_events(
    world: &mut World,
    event_loop: &EventLoopWindowTarget<()>,
    create_window_event_reader: &mut ManualEventReader<CreateWindow>,
) {
    let world = world.cell();
    let vulkano_context = world.get_resource::<VulkanoContext>().unwrap();
    let mut vulkano_winit_windows = world.get_resource_mut::<VulkanoWinitWindows>().unwrap();
    let mut windows = world.get_resource_mut::<Windows>().unwrap();
    let create_window_events = world.get_resource::<Events<CreateWindow>>().unwrap();
    let mut window_created_events = world.get_resource_mut::<Events<WindowCreated>>().unwrap();
    for create_window_event in create_window_event_reader.iter(&create_window_events) {
        let window = vulkano_winit_windows.create_window(
            event_loop,
            create_window_event.id,
            &create_window_event.descriptor,
            &vulkano_context,
        );
        windows.add(window);
        window_created_events.send(WindowCreated {
            id: create_window_event.id,
        });
    }
}

fn handle_initial_window_events(world: &mut World, event_loop: &EventLoop<()>) {
    let world = world.cell();
    let vulkano_context = world.get_resource::<VulkanoContext>().unwrap();
    let mut vulkano_winit_windows = world.get_resource_mut::<VulkanoWinitWindows>().unwrap();
    let mut windows = world.get_resource_mut::<Windows>().unwrap();
    let mut create_window_events = world.get_resource_mut::<Events<CreateWindow>>().unwrap();
    let mut window_created_events = world.get_resource_mut::<Events<WindowCreated>>().unwrap();
    for create_window_event in create_window_events.drain() {
        let window = vulkano_winit_windows.create_window(
            event_loop,
            create_window_event.id,
            &create_window_event.descriptor,
            &vulkano_context,
        );
        windows.add(window);
        window_created_events.send(WindowCreated {
            id: create_window_event.id,
        });
    }
}

pub fn exit_on_window_close_system(
    mut app_exit_events: EventWriter<AppExit>,
    mut window_close_requested_events: EventReader<WindowCloseRequested>,
    mut windows: ResMut<VulkanoWinitWindows>,
    mut pipeline_data: ResMut<PipelineData>,
) {
    for event in window_close_requested_events.iter() {
        // Close app on primary window exit
        if event.id == WindowId::primary() {
            app_exit_events.send(AppExit);
        }
        // But don't close app on secondary window exit. Instead cleanup...
        else {
            let window_id = event.id;
            pipeline_data.remove(window_id);
            let winit_id = if let Some(winit_window) = windows.get_winit_window(window_id) {
                winit_window.id()
            } else {
                continue;
            };
            windows.windows.remove(&winit_id);
        }
    }
}

#[cfg(feature = "gui")]
pub fn begin_egui_frame_system(mut vulkano_windows: ResMut<VulkanoWinitWindows>) {
    for (_, v) in vulkano_windows.windows.iter_mut() {
        v.gui().begin_frame();
    }
}
