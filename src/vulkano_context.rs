use std::sync::Arc;

#[cfg(target_os = "macos")]
use vulkano::instance::InstanceCreationError;
use vulkano::{
    device::{
        physical::{PhysicalDevice, PhysicalDeviceType},
        Device, DeviceExtensions, Features, Queue,
    },
    image::{view::ImageView, ImageUsage},
    instance::{
        debug::{DebugCallback, MessageSeverity, MessageType},
        Instance, InstanceExtensions,
    },
    swapchain::{
        ColorSpace, FullscreenExclusive, PresentMode, Surface, SurfaceTransform, Swapchain,
    },
    Version,
};
use winit::window::Window;

use crate::{FinalImageView, VulkanoWinitConfig};

/// Renderer that handles all gpu side rendering
pub struct VulkanoContext {
    instance: Arc<Instance>,
    _debug_callback: DebugCallback,
    device: Arc<Device>,
    graphics_queue: Arc<Queue>,
    compute_queue: Arc<Queue>,
    device_name: String,
    device_type: PhysicalDeviceType,
    max_mem_gb: f32,
}

unsafe impl Sync for VulkanoContext {}

unsafe impl Send for VulkanoContext {}

impl VulkanoContext {
    /// Creates a new GPU renderer for window with given parameters
    pub fn new(config: &VulkanoWinitConfig) -> Self {
        bevy::log::info!("Creating RenderContext");
        let instance = create_vk_instance(config.instance_extensions, &config.layers);
        let debug_callback = create_vk_debug_callback(&instance);
        // Get desired device
        let physical_device = PhysicalDevice::enumerate(&instance)
            .min_by_key(|p| match p.properties().device_type {
                PhysicalDeviceType::DiscreteGpu => 1,
                PhysicalDeviceType::IntegratedGpu => 2,
                PhysicalDeviceType::VirtualGpu => 3,
                PhysicalDeviceType::Cpu => 4,
                PhysicalDeviceType::Other => 5,
            })
            .unwrap();
        let device_name = physical_device.properties().device_name.to_string();
        #[cfg(target_os = "windows")]
        let max_mem_gb = physical_device.properties().max_memory_allocation_count as f32 * 9.31e-4;
        #[cfg(not(target_os = "windows"))]
        let max_mem_gb = physical_device.properties().max_memory_allocation_count as f32 * 9.31e-10;
        bevy::log::info!(
            "Using device {}, type: {:?}, mem: {:.2} gb",
            physical_device.properties().device_name,
            physical_device.properties().device_type,
            max_mem_gb,
        );
        let device_type = physical_device.properties().device_type;

        // Create device
        let (device, graphics_queue, compute_queue) = Self::create_device(
            physical_device,
            config.device_extensions,
            config.features.clone(),
        );

        Self {
            instance,
            _debug_callback: debug_callback,
            device,
            graphics_queue,
            compute_queue,
            device_name,
            device_type,
            max_mem_gb,
        }
    }

    /// Creates vulkan device with required queue families and required extensions
    fn create_device(
        physical: PhysicalDevice,
        device_extensions: DeviceExtensions,
        features: Features,
    ) -> (Arc<Device>, Arc<Queue>, Arc<Queue>) {
        let (gfx_index, queue_family_graphics) = physical
            .queue_families()
            .enumerate()
            .find(|&(_i, q)| q.supports_graphics())
            .unwrap();
        let compute_family_data = physical
            .queue_families()
            .enumerate()
            .find(|&(i, q)| i != gfx_index && q.supports_compute());

        if let Some((_compute_index, queue_family_compute)) = compute_family_data {
            let (device, mut queues) = {
                Device::new(
                    physical,
                    &features,
                    &physical.required_extensions().union(&device_extensions),
                    [(queue_family_graphics, 1.0), (queue_family_compute, 0.5)]
                        .iter()
                        .cloned(),
                )
                .unwrap()
            };
            let gfx_queue = queues.next().unwrap();
            let compute_queue = queues.next().unwrap();
            (device, gfx_queue, compute_queue)
        } else {
            let (device, mut queues) = {
                Device::new(
                    physical,
                    &features,
                    &physical.required_extensions().union(&device_extensions),
                    [(queue_family_graphics, 1.0)].iter().cloned(),
                )
                .unwrap()
            };
            let gfx_queue = queues.next().unwrap();
            let compute_queue = gfx_queue.clone();
            (device, gfx_queue, compute_queue)
        }
    }

    /// Creates swapchain and swapchain images
    pub(crate) fn create_swap_chain(
        &self,
        surface: Arc<Surface<Window>>,
        queue: Arc<Queue>,
        present_mode: PresentMode,
    ) -> (Arc<Swapchain<Window>>, Vec<FinalImageView>) {
        let caps = surface.capabilities(self.device.physical_device()).unwrap();
        let alpha = caps.supported_composite_alpha.iter().next().unwrap();
        let format = caps.supported_formats[0].0;
        let dimensions: [u32; 2] = surface.window().inner_size().into();
        let (swap_chain, images) = Swapchain::start(self.device.clone(), surface)
            .num_images(caps.min_image_count)
            .format(format)
            .dimensions(dimensions)
            .usage(ImageUsage::color_attachment())
            .sharing_mode(&queue)
            .composite_alpha(alpha)
            .transform(SurfaceTransform::Identity)
            .present_mode(present_mode)
            .fullscreen_exclusive(FullscreenExclusive::Default)
            .clipped(true)
            .color_space(ColorSpace::SrgbNonLinear)
            .layers(1)
            .build()
            .unwrap();
        let images = images
            .into_iter()
            .map(|image| ImageView::new(image).unwrap())
            .collect::<Vec<_>>();
        (swap_chain, images)
    }

    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    pub fn device_type(&self) -> PhysicalDeviceType {
        self.device_type
    }

    pub fn max_mem_gb(&self) -> f32 {
        self.max_mem_gb
    }

    /// Access instance
    pub fn instance(&self) -> Arc<Instance> {
        self.instance.clone()
    }

    /// Access device
    pub fn device(&self) -> Arc<Device> {
        self.device.clone()
    }

    /// Access rendering queue
    pub fn graphics_queue(&self) -> Arc<Queue> {
        self.graphics_queue.clone()
    }

    /// Access compute queue
    pub fn compute_queue(&self) -> Arc<Queue> {
        self.compute_queue.clone()
    }
}

// Create vk instance with given layers
pub fn create_vk_instance(
    instance_extensions: InstanceExtensions,
    layers: &[&str],
) -> Arc<Instance> {
    // Create instance
    #[cfg(target_os = "macos")]
    {
        match Instance::new(None, Version::V1_2, &instance_extensions, layers.to_vec()) {
            Err(e) => {
                match e {
                    InstanceCreationError::LoadingError(le) => {
                        error!("{:?}, Did you install vulkanSDK from https://vulkan.lunarg.com/sdk/home?", le);
                        Err(le).expect("")
                    }
                    _ => Err(e).expect("Failed to create instance"),
                }
            }
            Ok(i) => i,
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        Instance::new(None, Version::V1_2, &instance_extensions, layers.to_vec())
            .expect("Failed to create instance")
    }
}

// Create vk debug call back (to exists outside renderer)
pub fn create_vk_debug_callback(instance: &Arc<Instance>) -> DebugCallback {
    // Create debug callback for printing vulkan errors and warnings
    let severity = if std::env::var("VULKAN_VALIDATION").is_ok() {
        MessageSeverity {
            error: true,
            warning: true,
            information: true,
            verbose: true,
        }
    } else {
        MessageSeverity::none()
    };

    let ty = MessageType::all();
    DebugCallback::new(instance, severity, ty, |msg| {
        let severity = if msg.severity.error {
            "error"
        } else if msg.severity.warning {
            "warning"
        } else if msg.severity.information {
            "information"
        } else if msg.severity.verbose {
            "verbose"
        } else {
            panic!("no-impl");
        };

        let ty = if msg.ty.general {
            "general"
        } else if msg.ty.validation {
            "validation"
        } else if msg.ty.performance {
            "performance"
        } else {
            panic!("no-impl");
        };

        bevy::log::info!(
            "{} {} {}: {}",
            msg.layer_prefix.unwrap_or("unknown"),
            ty,
            severity,
            msg.description
        );
    })
    .unwrap()
}
