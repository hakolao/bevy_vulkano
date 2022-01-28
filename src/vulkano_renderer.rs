use core::result::Result::Ok;
use std::{
    collections::HashMap,
    hash::Hash,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use image::RgbaImage;
#[cfg(target_os = "macos")]
use vulkano::instance::InstanceCreationError;
use vulkano::{
    device::{
        physical::{PhysicalDevice, PhysicalDeviceType},
        Device, DeviceExtensions, Features, Queue,
    },
    format::Format,
    image::{
        view::ImageView, ImageAccess, ImageCreateFlags, ImageCreationError, ImageDimensions,
        ImageUsage, ImageViewAbstract, ImmutableImage, MipmapsCount, StorageImage, SwapchainImage,
    },
    instance::{
        debug::{DebugCallback, MessageSeverity, MessageType},
        Instance, InstanceExtensions,
    },
    swapchain,
    swapchain::{
        AcquireError, ColorSpace, FullscreenExclusive, PresentMode, Surface, SurfaceTransform,
        Swapchain, SwapchainCreationError,
    },
    sync,
    sync::{FlushError, GpuFuture},
    Version,
};
use vulkano_win::create_vk_surface_from_handle;
use winit::window::Window;

use crate::VulkanoWinitConfig;

#[derive(Debug, Hash, Eq, PartialEq, Copy, Clone)]
pub struct ImageTextureId(pub u32);

/// Final render target onto which whole app is rendered
pub type FinalImageView = Arc<ImageView<SwapchainImage<Window>>>;
/// Multipurpose image view
pub type DeviceImageView = Arc<ImageView<StorageImage>>;

/// Renderer that handles all gpu side rendering
pub struct Renderer {
    _instance: Arc<Instance>,
    _debug_callback: DebugCallback,
    device: Arc<Device>,
    surface: Arc<Surface<Window>>,
    graphics_queue: Arc<Queue>,
    compute_queue: Arc<Queue>,
    swap_chain: Arc<Swapchain<Window>>,
    image_index: usize,
    final_views: Vec<FinalImageView>,
    /// Image view that is to be rendered with our pipeline.
    /// (bool refers to whether it should get resized with swapchain resize)
    interim_image_views: HashMap<usize, (DeviceImageView, bool)>,
    // Texture cache for textures and their descriptor sets
    image_textures: HashMap<ImageTextureId, Arc<dyn ImageViewAbstract + 'static>>,
    recreate_swapchain: bool,
    previous_frame_end: Option<Box<dyn GpuFuture>>,
    device_name: String,
    device_type: PhysicalDeviceType,
    max_mem_gb: f32,
}

unsafe impl Sync for Renderer {}

unsafe impl Send for Renderer {}

impl Renderer {
    /// Creates a new GPU renderer for window with given parameters
    pub fn new(window: Window, config: &VulkanoWinitConfig) -> Self {
        bevy::log::info!("Creating renderer");
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
        // Create rendering surface from window
        let surface = create_vk_surface_from_handle(window, instance.clone()).unwrap();

        // Create device
        let (device, graphics_queue, compute_queue) = Self::create_device(
            physical_device,
            surface.clone(),
            config.device_extensions,
            config.features.clone(),
        );
        // Create swap chain & frame(s) to which we'll render
        let (swap_chain, final_images) = Self::create_swap_chain(
            surface.clone(),
            physical_device,
            device.clone(),
            graphics_queue.clone(),
            config.present_mode,
        );
        let previous_frame_end = Some(sync::now(device.clone()).boxed());
        let image_format = final_images.first().unwrap().format();
        bevy::log::info!("Swapchain format {:?}", image_format);

        Self {
            _instance: instance,
            _debug_callback: debug_callback,
            device,
            surface,
            graphics_queue,
            compute_queue,
            swap_chain,
            image_index: 0,
            final_views: final_images,
            interim_image_views: HashMap::new(),
            image_textures: HashMap::new(),
            previous_frame_end,
            recreate_swapchain: false,
            device_name,
            device_type,
            max_mem_gb,
        }
    }

    /*================
    STATIC FUNCTIONS
    =================*/

    /// Creates vulkan device with required queue families and required extensions
    fn create_device(
        physical: PhysicalDevice,
        surface: Arc<Surface<Window>>,
        device_extensions: DeviceExtensions,
        features: Features,
    ) -> (Arc<Device>, Arc<Queue>, Arc<Queue>) {
        let (gfx_index, queue_family_graphics) = physical
            .queue_families()
            .enumerate()
            .find(|&(_i, q)| q.supports_graphics() && surface.is_supported(q).unwrap_or(false))
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
    fn create_swap_chain(
        surface: Arc<Surface<Window>>,
        physical: PhysicalDevice,
        device: Arc<Device>,
        queue: Arc<Queue>,
        present_mode: PresentMode,
    ) -> (Arc<Swapchain<Window>>, Vec<FinalImageView>) {
        let caps = surface.capabilities(physical).unwrap();
        let alpha = caps.supported_composite_alpha.iter().next().unwrap();
        let format = caps.supported_formats[0].0;
        let dimensions: [u32; 2] = surface.window().inner_size().into();
        let (swap_chain, images) = Swapchain::start(device, surface)
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

    fn create_image_texture_id() -> ImageTextureId {
        static COUNTER: AtomicUsize = AtomicUsize::new(1);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);

        ImageTextureId(id as u32)
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

    /// Adds texture to image_textures for later use, returns ImageTextureId
    pub fn add_texture_from_file_bytes(&mut self, image_file_as_bytes: &[u8]) -> ImageTextureId {
        let image_view = self.create_image_from_file_bytes(image_file_as_bytes);
        let new_id = Self::create_image_texture_id();
        self.add_image_texture(new_id, image_view);
        new_id
    }

    /// Adds texture to image_textures for later use, returns ImageTextureId
    pub fn add_texture_from_image_view(
        &mut self,
        image_view: Arc<dyn ImageViewAbstract + 'static>,
    ) -> ImageTextureId {
        let new_id = Self::create_image_texture_id();
        self.add_image_texture(new_id, image_view);
        new_id
    }

    /// Adds texture to image_textures for later use, returns ImageTextureId
    pub fn update_texture_from_image_view(
        &mut self,
        image_view: Arc<dyn ImageViewAbstract + 'static>,
        texture_id: ImageTextureId,
    ) {
        self.add_image_texture(texture_id, image_view);
    }

    fn add_image_texture(
        &mut self,
        key: ImageTextureId,
        texture: Arc<dyn ImageViewAbstract + 'static>,
    ) {
        self.image_textures.insert(key, texture);
    }

    /// Get image texture (if exists, else panic)
    pub fn get_image_texture(&self, key: &ImageTextureId) -> Arc<dyn ImageViewAbstract + 'static> {
        self.image_textures.get(key).unwrap().clone()
    }

    /// Creates image view from image file bytes
    fn create_image_from_file_bytes(
        &self,
        file_bytes: &[u8],
    ) -> Arc<dyn ImageViewAbstract + 'static> {
        let image_view =
            texture_from_file(self.graphics_queue(), file_bytes, self.image_format()).unwrap();
        image_view
    }

    /// Return default image format for images (swapchain format may differ)
    pub fn image_format(&self) -> Format {
        Format::R8G8B8A8_UNORM
    }

    /// Return default image format for images (swapchain format may differ)
    pub fn swapchain_format(&self) -> Format {
        self.final_views[self.image_index].format()
    }

    /// Returns the index of last swapchain image that is the next render target
    /// All camera views will render onto their image at the same index
    pub fn image_index(&self) -> usize {
        self.image_index
    }

    /// Access device
    pub fn device(&self) -> Arc<Device> {
        self.device.clone()
    }

    /// Access rendering queue
    pub fn graphics_queue(&self) -> Arc<Queue> {
        self.graphics_queue.clone()
    }

    /// Access rendering queue
    pub fn compute_queue(&self) -> Arc<Queue> {
        self.compute_queue.clone()
    }

    /// Render target surface
    pub fn surface(&self) -> Arc<Surface<Window>> {
        self.surface.clone()
    }

    /// Winit window
    pub fn window(&self) -> &Window {
        self.surface.window()
    }

    pub fn window_size(&self) -> [u32; 2] {
        let size = self.window().inner_size();
        [size.width, size.height]
    }

    /// Size of the final swapchain image (surface)
    pub fn final_image_size(&self) -> [u32; 2] {
        self.final_views[0].image().dimensions().width_height()
    }

    /// Return final image which can be used as a render pipeline target
    pub fn final_image(&self) -> FinalImageView {
        self.final_views[self.image_index].clone()
    }

    /*================
    View related functions
    =================*/

    /// Return scale factor accounted window size
    pub fn resolution(&self) -> [u32; 2] {
        let size = self.window().inner_size();
        let scale_factor = self.window().scale_factor();
        [
            (size.width as f64 / scale_factor) as u32,
            (size.height as f64 / scale_factor) as u32,
        ]
    }

    pub fn aspect_ratio(&self) -> f32 {
        let dims = self.window_size();
        dims[0] as f32 / dims[1] as f32
    }

    /// Add interim image view that can be used to render e.g. camera views or other views using
    /// the render pipeline. Not giving a view size ensures the image view follows swapchain (window).
    pub fn add_image_target(&mut self, key: usize, view_size: Option<[u32; 2]>, format: Format) {
        let size = if let Some(s) = view_size {
            s
        } else {
            self.final_image_size()
        };
        let image = create_device_image(self.graphics_queue.clone(), size, format);
        self.interim_image_views
            .insert(key, (image, view_size.is_none()));
    }

    /// Get interim image view by key (for render calls or for registering as texture for egui)
    pub fn get_image_target(&mut self, key: usize) -> DeviceImageView {
        self.interim_image_views.get(&key).unwrap().clone().0
    }

    /// Get interim image view by key (for render calls or for registering as texture for egui)
    pub fn has_image_target(&mut self, key: usize) -> bool {
        self.interim_image_views.get(&key).is_some()
    }

    pub fn remove_image_target(&mut self, key: usize) {
        self.interim_image_views.remove(&key);
    }

    /*================
    Updates
    =================*/

    /// Resize swapchain and camera view images
    pub fn resize(&mut self) {
        self.recreate_swapchain = true;
    }

    /*================
    RENDERING
    =================*/

    /// Acquires next swapchain image and increments image index
    /// This is the first to call in render orchestration.
    /// Returns a gpu future representing the time after which the swapchain image has been acquired
    /// and previous frame ended.
    /// After this, execute command buffers and return future from them to `finish_frame`.
    pub fn start_frame(&mut self) -> std::result::Result<Box<dyn GpuFuture>, AcquireError> {
        // Recreate swap chain if needed (when resizing of window occurs or swapchain is outdated)
        // Also resize render views if needed
        if self.recreate_swapchain {
            self.recreate_swapchain_and_views();
        }

        // Acquire next image in the swapchain
        let (image_num, suboptimal, acquire_future) =
            match swapchain::acquire_next_image(self.swap_chain.clone(), None) {
                Ok(r) => r,
                Err(AcquireError::OutOfDate) => {
                    self.recreate_swapchain = true;
                    return Err(AcquireError::OutOfDate);
                }
                Err(e) => panic!("Failed to acquire next image: {:?}", e),
            };
        if suboptimal {
            self.recreate_swapchain = true;
        }
        // Update our image index
        self.image_index = image_num;

        let future = self.previous_frame_end.take().unwrap().join(acquire_future);

        Ok(future.boxed())
    }

    /// Finishes render by presenting the swapchain
    pub fn finish_frame(&mut self, after_future: Box<dyn GpuFuture>) {
        let future = after_future
            .then_swapchain_present(
                self.graphics_queue.clone(),
                self.swap_chain.clone(),
                self.image_index,
            )
            .then_signal_fence_and_flush();
        match future {
            Ok(future) => {
                // A hack to prevent OutOfMemory error on Nvidia :(
                // https://github.com/vulkano-rs/vulkano/issues/627
                match future.wait(None) {
                    Ok(x) => x,
                    Err(err) => bevy::log::error!("{:?}", err),
                }
                self.previous_frame_end = Some(future.boxed());
            }
            Err(FlushError::OutOfDate) => {
                self.recreate_swapchain = true;
                self.previous_frame_end = Some(sync::now(self.device.clone()).boxed());
            }
            Err(e) => {
                bevy::log::error!("Failed to flush future: {:?}", e);
                self.previous_frame_end = Some(sync::now(self.device.clone()).boxed());
            }
        }
    }

    /// Swapchain is recreated when resized
    /// Swapchain images also get recreated
    fn recreate_swapchain_and_views(&mut self) {
        let dimensions: [u32; 2] = self.window().inner_size().into();
        let (new_swapchain, new_images) =
            match self.swap_chain.recreate().dimensions(dimensions).build() {
                Ok(r) => r,
                Err(SwapchainCreationError::UnsupportedDimensions) => {
                    bevy::log::error!(
                        "{}",
                        SwapchainCreationError::UnsupportedDimensions.to_string()
                    );
                    return;
                }
                Err(e) => panic!("Failed to recreate swapchain: {:?}", e),
            };

        self.swap_chain = new_swapchain;
        let new_images = new_images
            .into_iter()
            .map(|image| ImageView::new(image).unwrap())
            .collect::<Vec<_>>();
        self.final_views = new_images;
        // Resize images that follow swapchain size
        let resizable_views = self
            .interim_image_views
            .iter()
            .filter(|(_, (_img, follow_swapchain))| *follow_swapchain)
            .map(|c| *c.0)
            .collect::<Vec<usize>>();
        for i in resizable_views {
            let format = self.get_image_target(i).format();
            self.remove_image_target(i);
            self.add_image_target(i, None, format);
        }
        self.recreate_swapchain = false;
    }
}

/// Creates a storage image on device
#[allow(unused)]
pub fn create_device_image(queue: Arc<Queue>, size: [u32; 2], format: Format) -> DeviceImageView {
    let dims = ImageDimensions::Dim2d {
        width: size[0],
        height: size[1],
        array_layers: 1,
    };
    let flags = ImageCreateFlags::none();
    ImageView::new(
        StorageImage::with_usage(
            queue.device().clone(),
            dims,
            format,
            ImageUsage {
                sampled: true,
                storage: true,
                color_attachment: true,
                transfer_destination: true,
                ..ImageUsage::none()
            },
            flags,
            Some(queue.family()),
        )
        .unwrap(),
    )
    .unwrap()
}

#[allow(unused)]
pub fn create_device_image_with_usage(
    queue: Arc<Queue>,
    size: [u32; 2],
    format: Format,
    usage: ImageUsage,
) -> DeviceImageView {
    let dims = ImageDimensions::Dim2d {
        width: size[0],
        height: size[1],
        array_layers: 1,
    };
    let flags = ImageCreateFlags::none();
    ImageView::new(
        StorageImage::with_usage(
            queue.device().clone(),
            dims,
            format,
            usage,
            flags,
            Some(queue.family()),
        )
        .unwrap(),
    )
    .unwrap()
}

pub fn texture_from_file(
    queue: Arc<Queue>,
    file_bytes: &[u8],
    format: vulkano::format::Format,
) -> Result<Arc<dyn ImageViewAbstract + Send + Sync + 'static>, ImageCreationError> {
    use image::GenericImageView;

    let img = image::load_from_memory(file_bytes).expect("Failed to load image from bytes");
    let rgba = if let Some(rgba) = img.as_rgba8() {
        rgba.to_owned().to_vec()
    } else {
        // Convert rgb to rgba
        let rgb = img.as_rgb8().unwrap().to_owned();
        let mut raw_data = vec![];
        for val in rgb.chunks(3) {
            raw_data.push(val[0]);
            raw_data.push(val[1]);
            raw_data.push(val[2]);
            raw_data.push(255);
        }
        let new_rgba = RgbaImage::from_raw(rgb.width(), rgb.height(), raw_data).unwrap();
        new_rgba.to_vec()
    };
    let dimensions = img.dimensions();
    let vko_dims = ImageDimensions::Dim2d {
        width: dimensions.0,
        height: dimensions.1,
        array_layers: 1,
    };
    let (texture, _tex_fut) =
        ImmutableImage::from_iter(rgba.into_iter(), vko_dims, MipmapsCount::One, format, queue)?;
    Ok(ImageView::new(texture).unwrap())
}

// Create vk instance
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
