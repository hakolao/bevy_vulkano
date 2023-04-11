use bevy::{
    log::{error, info, warn},
    prelude::{
        Changed, Commands, Component, Entity, EventWriter, Mut, NonSend, NonSendMut, Query,
        RemovedComponents, Resource, Window,
    },
    utils::HashMap,
    window::{RawHandleWrapper, WindowClosed, WindowCreated},
};
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use winit::{
    dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize},
    event_loop::EventLoopWindowTarget,
};

use crate::{
    config::BevyVulkanoSettings, converters, converters::convert_window_level, get_best_videomode,
    get_fitting_videomode, vulkano_windows::attempt_grab, BevyVulkanoContext, BevyVulkanoWindows,
};

/// System responsible for creating new windows whenever a `Window` component is added
/// to an entity.
///
/// This will default any necessary components if they are not already added.
#[allow(clippy::too_many_arguments)]
pub(crate) fn create_window<'a>(
    mut commands: Commands,
    event_loop: &EventLoopWindowTarget<()>,
    created_windows: impl Iterator<Item = (Entity, Mut<'a, Window>)>,
    mut event_writer: EventWriter<WindowCreated>,
    mut vulkano_windows: NonSendMut<BevyVulkanoWindows>,
    context: NonSend<BevyVulkanoContext>,
    settings: NonSend<BevyVulkanoSettings>,
) {
    for (entity, mut window) in created_windows {
        if vulkano_windows.get_vulkano_window(entity).is_some() {
            continue;
        }

        info!(
            "Creating new window {:?} ({:?})",
            window.title.as_str(),
            entity
        );

        let vulkano_window =
            vulkano_windows.create_window(event_loop, entity, &window, &context.context, &settings);
        window
            .resolution
            .set_scale_factor(vulkano_window.window().scale_factor());
        commands
            .entity(entity)
            .insert(RawHandleWrapper {
                window_handle: vulkano_window.window().raw_window_handle(),
                display_handle: vulkano_window.window().raw_display_handle(),
            })
            .insert(CachedWindow {
                window: window.clone(),
            });

        event_writer.send(WindowCreated {
            window: entity,
        });
    }
}

/// Cache for closing windows so we can get better debug information.
#[derive(Debug, Clone, Resource)]
pub struct WindowTitleCache(HashMap<Entity, String>);

pub(crate) fn despawn_window(
    mut closed: RemovedComponents<Window>,
    window_entities: Query<&Window>,
    mut close_events: EventWriter<WindowClosed>,
    mut vulkano_windows: NonSendMut<BevyVulkanoWindows>,
) {
    for window in closed.iter() {
        info!("Closing window {:?}", window);
        // Guard to verify that the window is in fact actually gone,
        // rather than having the component added and removed in the same frame.
        if !window_entities.contains(window) {
            vulkano_windows.remove_window(window);
            close_events.send(WindowClosed {
                window,
            });
        }
    }
}

/// The cached state of the window so we can check which properties were changed from within the app.
#[derive(Debug, Clone, Component)]
pub struct CachedWindow {
    pub window: Window,
}

// Detect changes to the window and update the winit window accordingly.
//
// Notes:
// - [`Window::present_mode`] and [`Window::composite_alpha_mode`] updating should be handled in the bevy render crate.
// - [`Window::transparent`] currently cannot be updated after startup for winit.
// - [`Window::canvas`] currently cannot be updated after startup, not entirely sure if it would work well with the
//   event channel stuff.
pub(crate) fn changed_window(
    mut changed_windows: Query<(Entity, &mut Window, &mut CachedWindow), Changed<Window>>,
    vulkano_windows: NonSendMut<BevyVulkanoWindows>,
) {
    for (entity, mut window, mut cache) in &mut changed_windows {
        if let Some(vulkano_window) = vulkano_windows.get_vulkano_window(entity) {
            if window.title != cache.window.title {
                vulkano_window.window().set_title(window.title.as_str());
            }

            if window.mode != cache.window.mode {
                let new_mode = match window.mode {
                    bevy::window::WindowMode::BorderlessFullscreen => {
                        Some(winit::window::Fullscreen::Borderless(None))
                    }
                    bevy::window::WindowMode::Fullscreen => {
                        Some(winit::window::Fullscreen::Exclusive(get_best_videomode(
                            &vulkano_window.window().current_monitor().unwrap(),
                        )))
                    }
                    bevy::window::WindowMode::SizedFullscreen => {
                        Some(winit::window::Fullscreen::Exclusive(get_fitting_videomode(
                            &vulkano_window.window().current_monitor().unwrap(),
                            window.width() as u32,
                            window.height() as u32,
                        )))
                    }
                    bevy::window::WindowMode::Windowed => None,
                };

                if vulkano_window.window().fullscreen() != new_mode {
                    vulkano_window.window().set_fullscreen(new_mode);
                }
            }
            if window.resolution != cache.window.resolution {
                let physical_size = PhysicalSize::new(
                    window.resolution.physical_width(),
                    window.resolution.physical_height(),
                );
                vulkano_window.window().set_inner_size(physical_size);
            }

            if window.physical_cursor_position() != cache.window.physical_cursor_position() {
                if let Some(physical_position) = window.physical_cursor_position() {
                    let inner_size = vulkano_window.window().inner_size();

                    let position = PhysicalPosition::new(
                        physical_position.x,
                        // Flip the coordinate space back to winit's context.
                        inner_size.height as f32 - physical_position.y,
                    );

                    if let Err(err) = vulkano_window.window().set_cursor_position(position) {
                        error!("could not set cursor position: {:?}", err);
                    }
                }
            }

            if window.cursor.icon != cache.window.cursor.icon {
                vulkano_window
                    .window()
                    .set_cursor_icon(converters::convert_cursor_icon(window.cursor.icon));
            }

            if window.cursor.grab_mode != cache.window.cursor.grab_mode {
                attempt_grab(vulkano_window.window(), window.cursor.grab_mode);
            }

            if window.cursor.visible != cache.window.cursor.visible {
                vulkano_window
                    .window()
                    .set_cursor_visible(window.cursor.visible);
            }

            if window.cursor.hit_test != cache.window.cursor.hit_test {
                if let Err(err) = vulkano_window
                    .window()
                    .set_cursor_hittest(window.cursor.hit_test)
                {
                    window.cursor.hit_test = cache.window.cursor.hit_test;
                    warn!(
                        "Could not set cursor hit test for window {:?}: {:?}",
                        window.title, err
                    );
                }
            }

            if window.decorations != cache.window.decorations
                && window.decorations != vulkano_window.window().is_decorated()
            {
                vulkano_window.window().set_decorations(window.decorations);
            }

            if window.resizable != cache.window.resizable
                && window.resizable != vulkano_window.window().is_resizable()
            {
                vulkano_window.window().set_resizable(window.resizable);
            }

            if window.resize_constraints != cache.window.resize_constraints {
                let constraints = window.resize_constraints.check_constraints();
                let min_inner_size = LogicalSize {
                    width: constraints.min_width,
                    height: constraints.min_height,
                };
                let max_inner_size = LogicalSize {
                    width: constraints.max_width,
                    height: constraints.max_height,
                };

                vulkano_window
                    .window()
                    .set_min_inner_size(Some(min_inner_size));
                if constraints.max_width.is_finite() && constraints.max_height.is_finite() {
                    vulkano_window
                        .window()
                        .set_max_inner_size(Some(max_inner_size));
                }
            }

            if window.position != cache.window.position {
                if let Some(position) = crate::winit_window_position(
                    &window.position,
                    &window.resolution,
                    vulkano_window.window().available_monitors(),
                    vulkano_window.window().primary_monitor(),
                    vulkano_window.window().current_monitor(),
                ) {
                    let should_set = match vulkano_window.window().outer_position() {
                        Ok(current_position) => current_position != position,
                        _ => true,
                    };

                    if should_set {
                        vulkano_window.window().set_outer_position(position);
                    }
                }
            }

            if let Some(maximized) = window.internal.take_maximize_request() {
                vulkano_window.window().set_maximized(maximized);
            }

            if let Some(minimized) = window.internal.take_minimize_request() {
                vulkano_window.window().set_minimized(minimized);
            }

            if window.focused != cache.window.focused && window.focused {
                vulkano_window.window().focus_window();
            }

            if window.window_level != cache.window.window_level {
                vulkano_window
                    .window()
                    .set_window_level(convert_window_level(window.window_level));
            }

            // Currently unsupported changes
            if window.transparent != cache.window.transparent {
                window.transparent = cache.window.transparent;
                warn!(
                    "Winit does not currently support updating transparency after window creation."
                );
            }

            if window.ime_enabled != cache.window.ime_enabled {
                vulkano_window.window().set_ime_allowed(window.ime_enabled);
            }

            if window.ime_position != cache.window.ime_position {
                vulkano_window
                    .window()
                    .set_ime_position(LogicalPosition::new(
                        window.ime_position.x,
                        window.ime_position.y,
                    ));
            }

            cache.window = window.clone();
        }
    }
}
