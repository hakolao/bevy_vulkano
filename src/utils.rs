use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use image::RgbaImage;
use vulkano::{
    device::Queue,
    image::{
        immutable::ImmutableImageCreationError, view::ImageView, ImageDimensions,
        ImageViewAbstract, ImmutableImage, MipmapsCount,
    },
};

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

pub fn texture_from_file_bytes(
    queue: Arc<Queue>,
    file_bytes: &[u8],
    format: vulkano::format::Format,
) -> Result<Arc<dyn ImageViewAbstract + Send + Sync + 'static>, ImmutableImageCreationError> {
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
    Ok(ImageView::new_default(texture).unwrap())
}
