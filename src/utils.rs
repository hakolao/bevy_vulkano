use std::{
    cell::UnsafeCell,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use image::RgbaImage;
use vulkano::{
    device::Queue,
    format::Format,
    image::{
        view::ImageView, ImageCreateFlags, ImageCreationError, ImageDimensions, ImageUsage,
        ImageViewAbstract, ImmutableImage, MipmapsCount, StorageImage, SwapchainImage,
    },
    instance::InstanceExtensions,
    sync::GpuFuture,
};
use winit::window::Window;

/// With this we can extend `Send` and `Sync` on `dyn GpuFuture`
pub struct UnsafeGpuFuture(UnsafeCell<Box<dyn GpuFuture>>);

impl UnsafeGpuFuture {
    pub fn new(future: Box<dyn GpuFuture>) -> UnsafeGpuFuture {
        UnsafeGpuFuture(UnsafeCell::new(future))
    }

    pub fn into_inner(self) -> Box<dyn GpuFuture> {
        self.0.into_inner()
    }
}

unsafe impl Send for UnsafeGpuFuture {}
unsafe impl Sync for UnsafeGpuFuture {}

/// Final render target onto which whole app is rendered
pub type FinalImageView = Arc<ImageView<SwapchainImage<Window>>>;
/// Multipurpose image view
pub type DeviceImageView = Arc<ImageView<StorageImage>>;

fn create_image_texture_id() -> ImageTextureId {
    static COUNTER: AtomicUsize = AtomicUsize::new(1);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);

    ImageTextureId(id as u32)
}

#[derive(Debug, Hash, Eq, PartialEq, Copy, Clone)]
pub struct ImageTextureId(pub u32);

impl ImageTextureId {
    pub fn new() -> ImageTextureId {
        create_image_texture_id()
    }
}

impl Default for ImageTextureId {
    fn default() -> Self {
        Self::new()
    }
}

pub const DEFAULT_IMAGE_FORMAT: Format = Format::R8G8B8A8_UNORM;

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

/// Copied from vulkano winit (one less winit dep...)
pub fn required_extensions() -> InstanceExtensions {
    let ideal = InstanceExtensions {
        khr_surface: true,
        khr_xlib_surface: true,
        khr_xcb_surface: true,
        khr_wayland_surface: true,
        khr_android_surface: true,
        khr_win32_surface: true,
        mvk_ios_surface: true,
        mvk_macos_surface: true,
        khr_get_physical_device_properties2: true,
        khr_get_surface_capabilities2: true,
        ..InstanceExtensions::none()
    };

    match InstanceExtensions::supported_by_core() {
        Ok(supported) => supported.intersection(&ideal),
        Err(_) => InstanceExtensions::none(),
    }
}
