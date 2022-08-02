#![allow(clippy::field_reassign_with_default)]

use bevy::{
    math::IVec2,
    utils::{
        hashbrown::hash_map::{Iter, IterMut},
        HashMap,
    },
    window::{PresentMode, Window, WindowDescriptor, WindowId, WindowMode},
};
#[cfg(feature = "gui")]
use egui_winit_vulkano::Gui;
use raw_window_handle::HasRawWindowHandle;
use vulkano_util::{
    context::VulkanoContext,
    renderer::VulkanoWindowRenderer,
    window::{
        WindowDescriptor as VulkanoWindowDescriptor,
        WindowResizeConstraints as VulkanoWindowResizeConstraints,
    },
};
use winit::dpi::{LogicalPosition, LogicalSize, PhysicalPosition};

use crate::VulkanoWinitConfig;

fn window_descriptor_to_vulkano_window_descriptor(
    wd: &WindowDescriptor,
    position: Option<[f32; 2]>,
) -> VulkanoWindowDescriptor {
    let mut window_descriptor = VulkanoWindowDescriptor::default();
    window_descriptor.width = wd.width;
    window_descriptor.height = wd.height;
    window_descriptor.position = position;
    window_descriptor.resize_constraints = VulkanoWindowResizeConstraints {
        min_width: wd.resize_constraints.min_width,
        min_height: wd.resize_constraints.min_height,
        max_width: wd.resize_constraints.max_width,
        max_height: wd.resize_constraints.max_height,
    };
    window_descriptor.scale_factor_override = wd.scale_factor_override;
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
    window_descriptor.cursor_visible = wd.cursor_locked;
    window_descriptor.mode = match wd.mode {
        WindowMode::Windowed => vulkano_util::window::WindowMode::Windowed,
        WindowMode::Fullscreen => vulkano_util::window::WindowMode::Fullscreen,
        WindowMode::BorderlessFullscreen => vulkano_util::window::WindowMode::BorderlessFullscreen,
        WindowMode::SizedFullscreen => vulkano_util::window::WindowMode::SizedFullscreen,
    };
    window_descriptor.transparent = wd.transparent;
    window_descriptor
}

#[derive(Default)]
pub struct BevyVulkanoWindows {
    #[cfg(not(feature = "gui"))]
    pub(crate) windows: HashMap<winit::window::WindowId, VulkanoWindowRenderer>,
    #[cfg(feature = "gui")]
    pub(crate) windows: HashMap<winit::window::WindowId, (VulkanoWindowRenderer, Gui)>,
    pub(crate) window_id_to_winit: HashMap<WindowId, winit::window::WindowId>,
    pub(crate) winit_to_window_id: HashMap<winit::window::WindowId, WindowId>,
}

impl BevyVulkanoWindows {
    pub fn create_window(
        &mut self,
        event_loop: &winit::event_loop::EventLoopWindowTarget<()>,
        window_id: WindowId,
        window_descriptor: &WindowDescriptor,
        vulkano_context: &VulkanoContext,
        config: &VulkanoWinitConfig,
    ) -> Window {
        #[cfg(target_os = "windows")]
        let mut winit_window_builder = {
            use winit::platform::windows::WindowBuilderExtWindows;
            winit::window::WindowBuilder::new().with_drag_and_drop(false)
        };

        #[cfg(not(target_os = "windows"))]
        let mut winit_window_builder = winit::window::WindowBuilder::new();

        winit_window_builder = match window_descriptor.mode {
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
                    window_descriptor.width as u32,
                    window_descriptor.height as u32,
                )),
            )),
            _ => {
                let WindowDescriptor {
                    width,
                    height,
                    position,
                    scale_factor_override,
                    ..
                } = window_descriptor;

                 match position {
                    bevy::window::WindowPosition::Automatic => { /* Window manager will handle position */ }
                     bevy::window::WindowPosition::Centered(monitor_selection) => {
                        let maybe_monitor = match monitor_selection {
                            bevy::window::MonitorSelection::Current => {
                                bevy::log::warn!("Can't select current monitor on window creation!");
                                None
                            }
                            bevy::window::MonitorSelection::Primary => event_loop.primary_monitor(),
                            bevy::window::MonitorSelection::Number(n) => event_loop.available_monitors().nth(*n),
                        };

                        if let Some(monitor) = maybe_monitor {
                            let screen_size = monitor.size();

                            let scale_factor = scale_factor_override.unwrap_or(1.0);

                            // Logical to physical window size
                            let (width, height): (u32, u32) = LogicalSize::new(*width, *height)
                                .to_physical::<u32>(scale_factor)
                                .into();

                            let position = PhysicalPosition {
                                x: screen_size.width.saturating_sub(width) as f64 / 2.
                                    + monitor.position().x as f64,
                                y: screen_size.height.saturating_sub(height) as f64 / 2.
                                    + monitor.position().y as f64,
                            };

                            winit_window_builder = winit_window_builder.with_position(position);
                        } else {
                            bevy::log::warn!("Couldn't get monitor selected with: {monitor_selection:?}");
                        }
                    }
                     bevy::window::WindowPosition::At(position) => {
                        if let Some(sf) = scale_factor_override {
                            winit_window_builder = winit_window_builder.with_position(
                                LogicalPosition::new(position[0] as f64, position[1] as f64)
                                    .to_physical::<f64>(*sf),
                            );
                        } else {
                            winit_window_builder = winit_window_builder.with_position(
                                LogicalPosition::new(position[0] as f64, position[1] as f64),
                            );
                        }
                    }
                }

                if let Some(sf) = scale_factor_override {
                    winit_window_builder.with_inner_size(
                        winit::dpi::LogicalSize::new(*width, *height).to_physical::<f64>(*sf),
                    )
                } else {
                    winit_window_builder
                        .with_inner_size(winit::dpi::LogicalSize::new(*width, *height))
                }
            }
            .with_resizable(window_descriptor.resizable)
            .with_decorations(window_descriptor.decorations)
            .with_transparent(window_descriptor.transparent),
        };

        let constraints = window_descriptor.resize_constraints.check_constraints();
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

        #[allow(unused_mut)]
        let mut winit_window_builder = winit_window_builder.with_title(&window_descriptor.title);

        let winit_window = winit_window_builder.build(event_loop).unwrap();

        if window_descriptor.cursor_locked {
            match winit_window.set_cursor_grab(true) {
                Ok(_) => {}
                Err(winit::error::ExternalError::NotSupported(_)) => {}
                Err(err) => Err(err).unwrap(),
            }
        }

        winit_window.set_cursor_visible(window_descriptor.cursor_visible);

        let winit_id = winit_window.id();
        self.window_id_to_winit.insert(window_id, winit_id);
        self.winit_to_window_id.insert(winit_id, window_id);

        let position = winit_window
            .outer_position()
            .ok()
            .map(|position| IVec2::new(position.x, position.y));
        let inner_size = winit_window.inner_size();
        let scale_factor = winit_window.scale_factor();
        let raw_window_handle = winit_window.raw_window_handle();

        let window_renderer = VulkanoWindowRenderer::new(
            vulkano_context,
            winit_window,
            &window_descriptor_to_vulkano_window_descriptor(
                window_descriptor,
                position.map(|p| [p.x as f32, p.y as f32]),
            ),
            move |ci| {
                ci.image_format = Some(vulkano::format::Format::B8G8R8A8_SRGB);
            },
        );

        let _is_gui_overlay = config.is_gui_overlay;
        #[cfg(feature = "gui")]
        {
            let gui = Gui::new(
                window_renderer.surface(),
                Some(window_renderer.swapchain_format()),
                window_renderer.graphics_queue(),
                _is_gui_overlay,
            );
            self.windows.insert(winit_id, (window_renderer, gui));
        }

        #[cfg(not(feature = "gui"))]
        self.windows.insert(winit_id, window_renderer);

        Window::new(
            window_id,
            window_descriptor,
            inner_size.width,
            inner_size.height,
            scale_factor,
            position,
            raw_window_handle,
        )
    }

    #[cfg(not(feature = "gui"))]
    pub fn get_primary_window_renderer_mut(&mut self) -> Option<&mut VulkanoWindowRenderer> {
        self.get_window_renderer_mut(WindowId::primary())
    }

    #[cfg(not(feature = "gui"))]
    pub fn get_primary_window_renderer(&self) -> Option<&VulkanoWindowRenderer> {
        self.get_window_renderer(WindowId::primary())
    }

    #[cfg(not(feature = "gui"))]
    pub fn get_window_renderer_mut(&mut self, id: WindowId) -> Option<&mut VulkanoWindowRenderer> {
        self.window_id_to_winit
            .get(&id)
            .and_then(|id| self.windows.get_mut(id))
    }

    #[cfg(not(feature = "gui"))]
    pub fn get_window_renderer(&self, id: WindowId) -> Option<&VulkanoWindowRenderer> {
        self.window_id_to_winit
            .get(&id)
            .and_then(|id| self.windows.get(id))
    }

    #[cfg(feature = "gui")]
    pub fn get_primary_window_renderer_mut(&mut self) -> Option<&mut (VulkanoWindowRenderer, Gui)> {
        self.get_window_renderer_mut(WindowId::primary())
    }

    #[cfg(feature = "gui")]
    pub fn get_primary_window_renderer(&self) -> Option<&(VulkanoWindowRenderer, Gui)> {
        self.get_window_renderer(WindowId::primary())
    }

    #[cfg(feature = "gui")]
    pub fn get_window_renderer_mut(
        &mut self,
        id: WindowId,
    ) -> Option<&mut (VulkanoWindowRenderer, Gui)> {
        self.window_id_to_winit
            .get(&id)
            .and_then(|id| self.windows.get_mut(id))
    }

    #[cfg(feature = "gui")]
    pub fn get_window_renderer(&self, id: WindowId) -> Option<&(VulkanoWindowRenderer, Gui)> {
        self.window_id_to_winit
            .get(&id)
            .and_then(|id| self.windows.get(id))
    }

    pub fn get_primary_winit_window(&self) -> Option<&winit::window::Window> {
        self.get_winit_window(WindowId::primary())
    }

    #[cfg(feature = "gui")]
    pub fn get_winit_window(&self, id: WindowId) -> Option<&winit::window::Window> {
        self.window_id_to_winit
            .get(&id)
            .and_then(|id| self.windows.get(id))
            .map(|(v_window, _)| v_window.window())
    }

    #[cfg(not(feature = "gui"))]
    pub fn get_winit_window(&self, id: WindowId) -> Option<&winit::window::Window> {
        self.window_id_to_winit
            .get(&id)
            .and_then(|id| self.windows.get(id))
            .map(|r| r.window())
    }

    pub fn get_window_id(&self, id: winit::window::WindowId) -> Option<WindowId> {
        self.winit_to_window_id.get(&id).cloned()
    }

    #[cfg(not(feature = "gui"))]
    pub fn iter(&self) -> Iter<winit::window::WindowId, VulkanoWindowRenderer> {
        self.windows.iter()
    }

    #[cfg(not(feature = "gui"))]
    pub fn iter_mut(&mut self) -> IterMut<winit::window::WindowId, VulkanoWindowRenderer> {
        self.windows.iter_mut()
    }

    #[cfg(feature = "gui")]
    pub fn iter(&self) -> Iter<winit::window::WindowId, (VulkanoWindowRenderer, Gui)> {
        self.windows.iter()
    }

    #[cfg(feature = "gui")]
    pub fn iter_mut(&mut self) -> IterMut<winit::window::WindowId, (VulkanoWindowRenderer, Gui)> {
        self.windows.iter_mut()
    }
}

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
                    Equal => b.refresh_rate().cmp(&a.refresh_rate()),
                    default => default,
                }
            }
            default => default,
        }
    });

    modes.first().unwrap().clone()
}

pub fn get_best_videomode(monitor: &winit::monitor::MonitorHandle) -> winit::monitor::VideoMode {
    let mut modes = monitor.video_modes().collect::<Vec<_>>();
    modes.sort_by(|a, b| {
        use std::cmp::Ordering::*;
        match b.size().width.cmp(&a.size().width) {
            Equal => match b.size().height.cmp(&a.size().height) {
                Equal => b.refresh_rate().cmp(&a.refresh_rate()),
                default => default,
            },
            default => default,
        }
    });

    modes.first().unwrap().clone()
}
