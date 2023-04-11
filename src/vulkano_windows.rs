#![allow(clippy::field_reassign_with_default)]

use bevy::{
    log::warn,
    prelude::Entity,
    utils::HashMap,
    window::{PresentMode, Window, WindowMode, WindowPosition, WindowResolution},
};
#[cfg(feature = "gui")]
use egui_winit_vulkano::{Gui, GuiConfig};
use vulkano_util::{
    context::VulkanoContext,
    renderer::VulkanoWindowRenderer,
    window::{
        WindowDescriptor as VulkanoWindowDescriptor,
        WindowResizeConstraints as VulkanoWindowResizeConstraints,
    },
};
use winit::{
    dpi::{LogicalSize, PhysicalPosition},
    monitor::MonitorHandle,
};

use crate::{config::BevyVulkanoSettings, converters::convert_window_level};

pub struct VulkanoWindow {
    pub renderer: VulkanoWindowRenderer,
    #[cfg(feature = "gui")]
    pub gui: Gui,
}

impl VulkanoWindow {
    pub fn window(&self) -> &winit::window::Window {
        self.renderer.window()
    }
}

#[derive(Default)]
pub struct BevyVulkanoWindows {
    pub(crate) windows: HashMap<winit::window::WindowId, VulkanoWindow>,
    /// Maps entities to `winit` window identifiers.
    pub(crate) entity_to_winit: HashMap<Entity, winit::window::WindowId>,
    /// Maps `winit` window identifiers to entities.
    pub(crate) winit_to_entity: HashMap<winit::window::WindowId, Entity>,
    // Some winit functions, such as `set_window_icon` can only be used from the main thread. If
    // they are used in another thread, the app will hang. This marker ensures `WinitWindows` is
    // only ever accessed with bevy's non-send functions and in NonSend systems.
    _not_send_sync: core::marker::PhantomData<*const ()>,
}

impl BevyVulkanoWindows {
    pub fn create_window(
        &mut self,
        event_loop: &winit::event_loop::EventLoopWindowTarget<()>,
        entity: Entity,
        window: &Window,
        vulkano_context: &VulkanoContext,
        _settings: &BevyVulkanoSettings,
    ) -> &VulkanoWindow {
        let mut winit_window_builder = winit::window::WindowBuilder::new();

        winit_window_builder = match window.mode {
            WindowMode::BorderlessFullscreen => winit_window_builder.with_fullscreen(Some(
                winit::window::Fullscreen::Borderless(event_loop.primary_monitor()),
            )),
            WindowMode::Fullscreen => {
                winit_window_builder.with_fullscreen(Some(winit::window::Fullscreen::Exclusive(
                    get_best_videomode(&event_loop.primary_monitor().unwrap()),
                )))
            }
            WindowMode::SizedFullscreen => winit_window_builder.with_fullscreen(Some(
                winit::window::Fullscreen::Exclusive(get_fitting_videomode(
                    &event_loop.primary_monitor().unwrap(),
                    window.width() as u32,
                    window.height() as u32,
                )),
            )),
            WindowMode::Windowed => {
                if let Some(position) = winit_window_position(
                    &window.position,
                    &window.resolution,
                    event_loop.available_monitors(),
                    event_loop.primary_monitor(),
                    None,
                ) {
                    winit_window_builder = winit_window_builder.with_position(position);
                }

                let logical_size = LogicalSize::new(window.width(), window.height());
                if let Some(sf) = window.resolution.scale_factor_override() {
                    winit_window_builder.with_inner_size(logical_size.to_physical::<f64>(sf))
                } else {
                    winit_window_builder.with_inner_size(logical_size)
                }
            }
        };

        winit_window_builder = winit_window_builder
            .with_window_level(convert_window_level(window.window_level))
            .with_resizable(window.resizable)
            .with_decorations(window.decorations)
            .with_transparent(window.transparent);

        let constraints = window.resize_constraints.check_constraints();
        let min_inner_size = LogicalSize {
            width: constraints.min_width,
            height: constraints.min_height,
        };
        let max_inner_size = LogicalSize {
            width: constraints.max_width,
            height: constraints.max_height,
        };

        let winit_window_builder =
            if constraints.max_width.is_finite() && constraints.max_height.is_finite() {
                winit_window_builder
                    .with_min_inner_size(min_inner_size)
                    .with_max_inner_size(max_inner_size)
            } else {
                winit_window_builder.with_min_inner_size(min_inner_size)
            };

        let winit_window = winit_window_builder
            .with_title(window.title.as_str())
            .build(event_loop)
            .unwrap();

        // Do not set the grab mode on window creation if it's none, this can fail on mobile
        if window.cursor.grab_mode != bevy::window::CursorGrabMode::None {
            attempt_grab(&winit_window, window.cursor.grab_mode);
        }

        winit_window.set_cursor_visible(window.cursor.visible);

        // Do not set the cursor hittest on window creation if it's false, as it will always fail on some
        // platforms and log an unfixable warning.
        if !window.cursor.hit_test {
            if let Err(err) = winit_window.set_cursor_hittest(window.cursor.hit_test) {
                warn!(
                    "Could not set cursor hit test for window {:?}: {:?}",
                    window.title, err
                );
            }
        }

        let vulkano_window = {
            let pos = winit_window
                .inner_position()
                .ok()
                .map(|p| [p.x as f32, p.y as f32]);
            let window_renderer = VulkanoWindowRenderer::new(
                vulkano_context,
                winit_window,
                &window_descriptor_to_vulkano_window_descriptor(window, pos),
                move |ci| {
                    ci.image_format = Some(vulkano::format::Format::B8G8R8A8_SRGB);
                },
            );

            #[cfg(feature = "gui")]
            {
                let gui = Gui::new(
                    event_loop,
                    window_renderer.surface(),
                    window_renderer.graphics_queue(),
                    GuiConfig {
                        is_overlay: _settings.is_gui_overlay,
                        preferred_format: Some(window_renderer.swapchain_format()),
                        ..Default::default()
                    },
                );
                VulkanoWindow {
                    renderer: window_renderer,
                    gui,
                }
            }
            #[cfg(not(feature = "gui"))]
            {
                VulkanoWindow {
                    renderer: window_renderer,
                }
            }
        };

        self.entity_to_winit
            .insert(entity, vulkano_window.renderer.window().id());
        self.winit_to_entity
            .insert(vulkano_window.renderer.window().id(), entity);

        self.windows
            .entry(vulkano_window.renderer.window().id())
            .insert(vulkano_window)
            .into_mut()
    }

    /// Get the entity associated with the winit window id.
    ///
    /// This is mostly just an intermediary step between us and winit.
    pub fn get_window_entity(&self, winit_id: winit::window::WindowId) -> Option<Entity> {
        self.winit_to_entity.get(&winit_id).cloned()
    }

    /// Get the window that is associated with our entity.
    pub fn get_vulkano_window(&self, entity: Entity) -> Option<&VulkanoWindow> {
        self.entity_to_winit
            .get(&entity)
            .and_then(|winit_id| self.windows.get(winit_id))
    }

    /// Get the window that is associated with our entity.
    pub fn get_vulkano_window_mut(&mut self, entity: Entity) -> Option<&mut VulkanoWindow> {
        self.entity_to_winit
            .get(&entity)
            .and_then(|winit_id| self.windows.get_mut(winit_id))
    }

    /// Remove a window from winit.
    ///
    /// This should mostly just be called when the window is closing.
    pub fn remove_window(&mut self, entity: Entity) -> Option<VulkanoWindow> {
        let winit_id = self.entity_to_winit.remove(&entity)?;
        // Don't remove from winit_to_window_id, to track that we used to know about this winit window
        self.windows.remove(&winit_id)
    }
}

/// Gets the "best" video mode which fits the given dimensions.
///
/// The heuristic for "best" prioritizes width, height, and refresh rate in that order.
pub fn get_fitting_videomode(
    monitor: &winit::monitor::MonitorHandle,
    width: u32,
    height: u32,
) -> winit::monitor::VideoMode {
    let mut modes = monitor.video_modes().collect::<Vec<_>>();

    fn abs_diff(a: u32, b: u32) -> u32 {
        if a > b {
            return a - b;
        }
        b - a
    }

    modes.sort_by(|a, b| {
        use std::cmp::Ordering::*;
        match abs_diff(a.size().width, width).cmp(&abs_diff(b.size().width, width)) {
            Equal => {
                match abs_diff(a.size().height, height).cmp(&abs_diff(b.size().height, height)) {
                    Equal => b
                        .refresh_rate_millihertz()
                        .cmp(&a.refresh_rate_millihertz()),
                    default => default,
                }
            }
            default => default,
        }
    });

    modes.first().unwrap().clone()
}

/// Gets the "best" videomode from a monitor.
///
/// The heuristic for "best" prioritizes width, height, and refresh rate in that order.
pub fn get_best_videomode(monitor: &winit::monitor::MonitorHandle) -> winit::monitor::VideoMode {
    let mut modes = monitor.video_modes().collect::<Vec<_>>();
    modes.sort_by(|a, b| {
        use std::cmp::Ordering::*;
        match b.size().width.cmp(&a.size().width) {
            Equal => match b.size().height.cmp(&a.size().height) {
                Equal => b
                    .refresh_rate_millihertz()
                    .cmp(&a.refresh_rate_millihertz()),
                default => default,
            },
            default => default,
        }
    });

    modes.first().unwrap().clone()
}

pub(crate) fn attempt_grab(
    winit_window: &winit::window::Window,
    grab_mode: bevy::window::CursorGrabMode,
) {
    let grab_result = match grab_mode {
        bevy::window::CursorGrabMode::None => {
            winit_window.set_cursor_grab(winit::window::CursorGrabMode::None)
        }
        bevy::window::CursorGrabMode::Confined => winit_window
            .set_cursor_grab(winit::window::CursorGrabMode::Confined)
            .or_else(|_e| winit_window.set_cursor_grab(winit::window::CursorGrabMode::Locked)),
        bevy::window::CursorGrabMode::Locked => winit_window
            .set_cursor_grab(winit::window::CursorGrabMode::Locked)
            .or_else(|_e| winit_window.set_cursor_grab(winit::window::CursorGrabMode::Confined)),
    };

    if let Err(err) = grab_result {
        let err_desc = match grab_mode {
            bevy::window::CursorGrabMode::Confined | bevy::window::CursorGrabMode::Locked => "grab",
            bevy::window::CursorGrabMode::None => "ungrab",
        };

        bevy::utils::tracing::error!("Unable to {} cursor: {}", err_desc, err);
    }
}

pub fn winit_window_position(
    position: &WindowPosition,
    resolution: &WindowResolution,
    mut available_monitors: impl Iterator<Item = MonitorHandle>,
    primary_monitor: Option<MonitorHandle>,
    current_monitor: Option<MonitorHandle>,
) -> Option<PhysicalPosition<i32>> {
    match position {
        WindowPosition::Automatic => {
            /* Window manager will handle position */
            None
        }
        WindowPosition::Centered(monitor_selection) => {
            use bevy::window::MonitorSelection::*;
            let maybe_monitor = match monitor_selection {
                Current => {
                    if current_monitor.is_none() {
                        warn!(
                            "Can't select current monitor on window creation or cannot find \
                             current monitor!"
                        );
                    }
                    current_monitor
                }
                Primary => primary_monitor,
                Index(n) => available_monitors.nth(*n),
            };

            if let Some(monitor) = maybe_monitor {
                let screen_size = monitor.size();

                let scale_factor = resolution.base_scale_factor();

                // Logical to physical window size
                let (width, height): (u32, u32) =
                    LogicalSize::new(resolution.width(), resolution.height())
                        .to_physical::<u32>(scale_factor)
                        .into();

                let position = PhysicalPosition {
                    x: screen_size.width.saturating_sub(width) as f64 / 2.
                        + monitor.position().x as f64,
                    y: screen_size.height.saturating_sub(height) as f64 / 2.
                        + monitor.position().y as f64,
                };

                Some(position.cast::<i32>())
            } else {
                warn!("Couldn't get monitor selected with: {monitor_selection:?}");
                None
            }
        }
        WindowPosition::At(position) => {
            Some(PhysicalPosition::new(position[0] as f64, position[1] as f64).cast::<i32>())
        }
    }
}

fn window_descriptor_to_vulkano_window_descriptor(
    wd: &Window,
    position: Option<[f32; 2]>,
) -> VulkanoWindowDescriptor {
    let mut window_descriptor = VulkanoWindowDescriptor::default();
    window_descriptor.width = wd.width();
    window_descriptor.height = wd.height();
    window_descriptor.position = position;
    window_descriptor.resize_constraints = VulkanoWindowResizeConstraints {
        min_width: wd.resize_constraints.min_width,
        min_height: wd.resize_constraints.min_height,
        max_width: wd.resize_constraints.max_width,
        max_height: wd.resize_constraints.max_height,
    };
    window_descriptor.scale_factor_override = wd.resolution.scale_factor_override();
    window_descriptor.title = wd.title.clone();
    window_descriptor.present_mode = match wd.present_mode {
        PresentMode::Fifo => vulkano::swapchain::PresentMode::Fifo,
        PresentMode::Immediate => vulkano::swapchain::PresentMode::Immediate,
        PresentMode::Mailbox => vulkano::swapchain::PresentMode::Mailbox,
        PresentMode::AutoNoVsync => vulkano::swapchain::PresentMode::Immediate,
        PresentMode::AutoVsync => vulkano::swapchain::PresentMode::FifoRelaxed,
    };
    window_descriptor.resizable = wd.resizable;
    window_descriptor.decorations = wd.decorations;
    window_descriptor.cursor_visible = wd.cursor.visible;
    window_descriptor.cursor_locked = match wd.cursor.grab_mode {
        bevy::window::CursorGrabMode::Locked => true,
        _ => false,
    };
    window_descriptor.mode = match wd.mode {
        WindowMode::Windowed => vulkano_util::window::WindowMode::Windowed,
        WindowMode::Fullscreen => vulkano_util::window::WindowMode::Fullscreen,
        WindowMode::BorderlessFullscreen => vulkano_util::window::WindowMode::BorderlessFullscreen,
        WindowMode::SizedFullscreen => vulkano_util::window::WindowMode::SizedFullscreen,
    };
    window_descriptor.transparent = wd.transparent;
    window_descriptor
}
