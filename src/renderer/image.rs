use crate::{color::Srgba, renderer::textures::TextureOptions};
use image::{DynamicImage, RgbaImage};
use std::{io::Read, num::NonZeroU32, path::Path};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MipStrategy {
    /// Single mip level.
    None,
    /// Generate mip chain from mip 0 on upload.
    GenerateOnUpload,
    /// Upload as is. Used when `ImageData` already contains a full mip chain (e.g. when loaded from DDS).
    Provided,
}

impl MipStrategy {
    /// Resolves to a `MipStrategy` based on the actual `ImageData` and the callers request to
    /// generate a mip chain if possible.
    pub fn resolve(image: &ImageData, generate_if_possible: bool) -> Self {
        if image.mipmap_count.get() > 1 {
            Self::Provided
        } else if generate_if_possible && !image.format.is_compressed() {
            Self::GenerateOnUpload
        } else {
            Self::None
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PartialDeltaOrigin {
    pub x: u32,
    pub y: u32,
    pub layer: u32,
}

/// Represents the change to a GPU texture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImageDelta {
    /// Creates a whole new texture.
    Full {
        image: ImageData,
        options: TextureOptions,
        mip_strategy: MipStrategy,
    },
    /// Updates a sub-region starting at `origin` of an existing texture.
    Partial {
        image: ImageData,
        origin: PartialDeltaOrigin,
    },
}

impl ImageDelta {
    pub fn full<I: Into<ImageData>>(
        image: I,
        options: TextureOptions,
        mip_strategy: MipStrategy,
    ) -> Self {
        let image = image.into();

        debug_assert!(
            match mip_strategy {
                MipStrategy::None => true,
                MipStrategy::GenerateOnUpload => {
                    image.mipmap_count.get() == 1 && !image.format.is_compressed()
                }
                MipStrategy::Provided => image.mipmap_count.get() > 1,
            },
            "MipStrategy inconsistent with image data"
        );

        Self::Full {
            image,
            options,
            mip_strategy,
        }
    }

    pub fn partial<I: Into<ImageData>>(image: I, origin: PartialDeltaOrigin) -> Self {
        Self::Partial {
            image: image.into(),
            origin,
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
            data_order: DataOrder::default(),
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
            data_order: DataOrder::default(),
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
            data_order: DataOrder::default(),
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
            .finish_non_exhaustive()
    }
}

pub fn load_image_file<P: AsRef<Path>>(path: P) -> anyhow::Result<ImageData> {
    let path = resolve_path(path);

    if is_compressed_dds(&path) {
        load_compressed_dds(&path)
    } else {
        // let image crate deal with other file types
        let image = image::ImageReader::open(&path)?.decode()?;
        Ok(image.into())
    }
}

/// Attempts to find the file, trying lowercase filename if it doesn't exist.
///
/// NOTE: PoB2 assumes a case insensitive filesystem, so checking the lowercase name
/// helps on case sensitive systems in some cases (no pun intended).
fn resolve_path<P: AsRef<Path>>(path: P) -> std::path::PathBuf {
    let path_ref = path.as_ref();
    if path_ref.exists() {
        return path_ref.to_owned();
    }

    let mut lowercase_path = path_ref.to_owned();
    if let Some(filename) = path_ref.file_name() {
        lowercase_path.set_file_name(filename.to_ascii_lowercase());
    }
    lowercase_path
}

/// Checks if file is a compressed DDS file (.dds.zst)
fn is_compressed_dds<P: AsRef<Path>>(path: P) -> bool {
    path.as_ref()
        .to_string_lossy()
        .to_ascii_lowercase()
        .ends_with(".dds.zst")
}

fn dds_format_to_wgpu(format: dds::Format) -> anyhow::Result<wgpu::TextureFormat> {
    Ok(match format {
        dds::Format::BC1_UNORM => wgpu::TextureFormat::Bc1RgbaUnorm,
        dds::Format::BC2_UNORM => wgpu::TextureFormat::Bc2RgbaUnorm,
        dds::Format::BC3_UNORM => wgpu::TextureFormat::Bc3RgbaUnorm,
        dds::Format::BC7_UNORM => wgpu::TextureFormat::Bc7RgbaUnorm,
        dds::Format::R8G8B8A8_UNORM => wgpu::TextureFormat::Rgba8Unorm,
        _ => anyhow::bail!("Unsupported DDS format: {format:?}"),
    })
}

/// Loads a compressed DDS file (.dds.zst)
fn load_compressed_dds<P: AsRef<Path>>(path: P) -> anyhow::Result<ImageData> {
    let file = std::fs::File::open(path.as_ref())?;
    let file_len = file.metadata().ok().map(|m| m.len());

    let mut decoder = zstd::Decoder::new(file)?;

    let parse_options = dds::header::ParseOptions::new_permissive(file_len);
    let header = dds::header::Header::read(&mut decoder, &parse_options)?;
    let dxgi_format = dds::Format::from_header(&header)?;
    // zstd_decoder will now be at start of pixel data

    // Read pixel data
    let non_data_len = dds::header::Header::MAGIC.len() + header.byte_len();
    let expected_data_len = file_len
        .and_then(|len| (len as usize).checked_sub(non_data_len))
        .unwrap_or(0);

    let mut pixel_data = Vec::with_capacity(expected_data_len);
    decoder.read_to_end(&mut pixel_data)?;

    Ok(ImageData {
        format: dds_format_to_wgpu(dxgi_format)?,
        width: header.width(),
        height: header.height(),
        array_layers: header.array_size(),
        mipmap_count: header.mipmap_count(),
        data_order: DataOrder::LayerMajor,
        bytes: pixel_data,
    })
}
