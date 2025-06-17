use super::*;
use image::*;

/// Utility for creating `Texture` from `DynamicImage`
#[inline(always)]
pub fn image2texture(device_handler: &DeviceHandler, image: &DynamicImage) -> Texture {
    let buffer = image.to_rgba8();
    imagebuffer2texture(device_handler, &buffer, TextureFormat::Rgba8Unorm)
}

fn imagebuffer2texture<P, Container>(
    device_handler: &DeviceHandler,
    image_buffer: &ImageBuffer<P, Container>,
    format: TextureFormat,
) -> Texture
where
    P: Pixel + 'static,
    P::Subpixel: Pod + Zeroable + 'static,
    Container: std::ops::Deref<Target = [P::Subpixel]>,
{
    let (device, queue) = (device_handler.device(), device_handler.queue());
    let size = Extent3d {
        width: image_buffer.width(),
        height: image_buffer.height(),
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&TextureDescriptor {
        label: None,
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format,
        view_formats: &[],
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
    });
    queue.write_texture(
        TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: Origin3d::ZERO,
            aspect: TextureAspect::All,
        },
        bytemuck::cast_slice(image_buffer),
        TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(size.width * size_of::<P>() as u32),
            rows_per_image: Some(size.height),
        },
        size,
    );
    texture
}
