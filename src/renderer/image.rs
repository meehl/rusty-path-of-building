use crate::{color::Srgba, renderer::textures::TextureOptions};
use image::{DynamicImage, RgbaImage};
use std::{io::Read, num::NonZeroU32, path::Path};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageDelta {
    pub image: ImageData,
    pub options: TextureOptions,
}

impl ImageDelta {
    pub fn new<I: Into<ImageData>>(image: I, options: TextureOptions) -> Self {
        Self {
            image: image.into(),
            options,
        }
    }
}

/// Order in which data is laid out.
/// Doesn't matter for data with a single layer and no mipmaps.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Hash)]
pub enum DataOrder {
    /// Data is laid out as: `[Layer0Mip0, Layer0Mip1, Layer0Mip2, Layer1Mip0, Layer1Mip1, ...]`
    /// Used by dds files.
    #[default]
    LayerMajor,
    /// Data is laid out as: `[Layer0Mip0, Layer1Mip0, Layer2Mip0, Layer0Mip1, Layer1Mip1, ...]`
    /// Used by ktx and ktx2 files.
    #[allow(dead_code)]
    MipMajor,
}

impl From<DataOrder> for wgpu::wgt::TextureDataOrder {
    fn from(value: DataOrder) -> Self {
        match value {
            DataOrder::LayerMajor => Self::LayerMajor,
            DataOrder::MipMajor => Self::MipMajor,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ImageData {
    pub format: wgpu::TextureFormat,
    pub width: u32,
    pub height: u32,
    pub array_layers: u32,
    pub mipmap_count: NonZeroU32,
    pub data_order: DataOrder,
    pub bytes: Vec<u8>,
}

impl ImageData {
    pub fn from_solid_color(dimensions: [usize; 2], color: Srgba) -> Self {
        let width = dimensions[0] as u32;
        let height = dimensions[1] as u32;
        Self {
            format: wgpu::TextureFormat::Rgba8Unorm,
            width,
            height,
            array_layers: 1,
            mipmap_count: NonZeroU32::new(1).expect("1 is non-zero"),
            data_order: Default::default(),
            bytes: RgbaImage::from_pixel(width, height, color.0.into()).into_raw(),
        }
    }
}

impl From<DynamicImage> for ImageData {
    fn from(image: DynamicImage) -> Self {
        Self {
            format: wgpu::TextureFormat::Rgba8Unorm,
            width: image.width(),
            height: image.height(),
            array_layers: 1,
            mipmap_count: NonZeroU32::new(1).expect("1 is non-zero"),
            data_order: Default::default(),
            bytes: image.to_rgba8().into_raw(),
        }
    }
}

impl From<RgbaImage> for ImageData {
    fn from(image: RgbaImage) -> Self {
        Self {
            format: wgpu::TextureFormat::Rgba8Unorm,
            width: image.width(),
            height: image.height(),
            array_layers: 1,
            mipmap_count: NonZeroU32::new(1).expect("1 is non-zero"),
            data_order: Default::default(),
            bytes: image.into_raw(),
        }
    }
}

impl std::fmt::Debug for ImageData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageData")
            .field("format", &self.format)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("array_layers", &self.array_layers)
            .field("mipmap_count", &self.mipmap_count)
            .finish()
    }
}

// TODO: clean this up
pub fn load_image_file<P: AsRef<Path>>(path: P) -> anyhow::Result<ImageData> {
    // PoB2 assumes a case insensitive filesystem is used. This can lead to some
    // files not being found on case sensitive systems.
    // As a workaround, try using a lower-cased path if the original path doesn't
    // exist.
    let path = if path.as_ref().exists() {
        path.as_ref().to_owned()
    } else {
        let mut path = path.as_ref().to_owned();
        let lowercase_filename = path.file_name().unwrap().to_ascii_lowercase();
        path.set_file_name(lowercase_filename);
        path
    };

    // special handling for compressed dds files
    if path.extension().and_then(|s| s.to_str()) == Some("zst")
        && path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.ends_with(".dds"))
            .unwrap_or(false)
    {
        let f = std::fs::File::open(&path)?;
        let file_len = f.metadata().ok().map(|m| m.len());
        let mut zstd_decoder = zstd::Decoder::new(f)?;

        let parse_option = dds::header::ParseOptions::new_permissive(file_len);
        let header = dds::header::Header::read(&mut zstd_decoder, &parse_option)?;
        let dxgi_format = dds::Format::from_header(&header)?;
        // zstd_decoder will now be at start of pixel data
        let non_data_len = dds::header::Header::MAGIC.len() + header.byte_len();
        let data_len = file_len.and_then(|l| (l as usize).checked_sub(non_data_len));
        let mut pixel_data = Vec::with_capacity(data_len.unwrap_or(0));
        zstd_decoder.read_to_end(&mut pixel_data)?;

        let format = match dxgi_format {
            dds::Format::BC1_UNORM => wgpu::TextureFormat::Bc1RgbaUnorm,
            dds::Format::BC2_UNORM => wgpu::TextureFormat::Bc2RgbaUnorm,
            dds::Format::BC3_UNORM => wgpu::TextureFormat::Bc3RgbaUnorm,
            dds::Format::BC7_UNORM => wgpu::TextureFormat::Bc7RgbaUnorm,
            dds::Format::R8G8B8A8_UNORM => wgpu::TextureFormat::Rgba8Unorm,
            _ => anyhow::bail!("Unsupported dxgi format"),
        };

        let image_data = ImageData {
            format,
            width: header.width(),
            height: header.height(),
            array_layers: header.array_size(),
            mipmap_count: header.mipmap_count(),
            data_order: DataOrder::LayerMajor,
            bytes: pixel_data,
        };

        return Ok(image_data);
    }

    // let image crate deal with other file types
    let image = image::ImageReader::open(&path)?.decode()?;
    Ok(image.into())
}
