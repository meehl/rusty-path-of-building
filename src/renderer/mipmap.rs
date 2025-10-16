/// Generate entire mipmap chain for all layers starting from mip level 0 image data.
/// Assumes data to contain one mip level 0 image for each layer in RGBA format.
pub fn generate_mipmap_chain(queue: &wgpu::Queue, texture: &wgpu::Texture, data: &[u8]) {
    assert!(!texture.format().is_compressed());
    assert!(texture.format().components() == 4);

    let size = texture.size();
    let block_size = texture.format().block_copy_size(None).unwrap_or(4);
    let data_size = (size.width * size.height * block_size) as usize;

    let srgb_mapper = fast_image_resize::create_srgb_mapper();

    let mut data_offset = 0;
    for layer in 0..texture.depth_or_array_layers() {
        let bytes = data[data_offset..data_size].to_owned();
        let mut src_image = fast_image_resize::images::Image::from_vec_u8(
            size.width,
            size.height,
            bytes,
            fast_image_resize::PixelType::U8x4,
        )
        .unwrap();

        for mip in 1..texture.mip_level_count() {
            let mut target_size = size.mip_level_size(mip, wgpu::TextureDimension::D2);
            target_size.depth_or_array_layers = 1; // copying layers separately

            // intermediate buffer that stores linear source image data in a higher
            // bit depth to avoid loss of information
            let mut linear_src_image = fast_image_resize::images::Image::new(
                src_image.width(),
                src_image.height(),
                fast_image_resize::PixelType::U16x4,
            );

            // convert from sRGB space into linear space
            // resizing needs to be performed in linear space to get the correct visual result
            srgb_mapper
                .forward_map(&src_image, &mut linear_src_image)
                .unwrap();

            let mut dst_image = fast_image_resize::images::Image::new(
                target_size.width,
                target_size.height,
                fast_image_resize::PixelType::U8x4,
            );

            let mut linear_dst_image = fast_image_resize::images::Image::new(
                dst_image.width(),
                dst_image.height(),
                fast_image_resize::PixelType::U16x4,
            );

            let mut resizer = fast_image_resize::Resizer::new();
            resizer
                .resize(
                    &linear_src_image,
                    &mut linear_dst_image,
                    Some(&fast_image_resize::ResizeOptions {
                        algorithm: fast_image_resize::ResizeAlg::Convolution(
                            fast_image_resize::FilterType::Bilinear,
                        ),
                        cropping: fast_image_resize::SrcCropping::None,
                        mul_div_alpha: true,
                    }),
                )
                .unwrap();

            // texture needs to be in sRGB, so map back from resized linear image
            srgb_mapper
                .backward_map(&linear_dst_image, &mut dst_image)
                .unwrap();

            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture,
                    mip_level: mip,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: layer,
                    },
                    aspect: wgpu::wgt::TextureAspect::All,
                },
                dst_image.buffer(),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(block_size * target_size.width),
                    rows_per_image: None,
                },
                target_size,
            );

            src_image = dst_image;
        }

        data_offset += data_size;
    }
}
