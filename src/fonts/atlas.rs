use std::num::NonZeroU32;

use crate::{
    color::Srgba,
    dpi::{Normalize, NormalizedRect},
    math::{Point, Size},
    renderer::{
        image::{DataOrder, ImageData, ImageDelta},
        textures::TextureOptions,
    },
};
use image::{GenericImage, RgbaImage, SubImage};

pub struct FontAtlasSpace;
pub type FontAtlasPoint = Point<u32, FontAtlasSpace>;
pub type FontAtlasSize = Size<u32, FontAtlasSpace>;

pub struct AllocatedGlyph<'a> {
    pub sub_image: SubImage<&'a mut RgbaImage>,
    pub layer_idx: u32,
    pub uv: NormalizedRect,
}

struct Layer {
    image: RgbaImage,
    // position of next allocation
    cursor: FontAtlasPoint,
    current_row_height: u32,
}

pub struct FontAtlas {
    layer_size: FontAtlasSize,
    // maximum amount of layers
    max_layers: u32,
    layers: Vec<Layer>,
    // atlas has been altered and needs to be reuploaded to the GPU
    // TODO: only mark changed regions as dirty and perform partial texture update
    dirty: bool,
}

impl FontAtlas {
    pub fn new(layer_size: u32, max_layers: u32) -> Self {
        let mut atlas = Self {
            layer_size: Size::new(layer_size, layer_size),
            max_layers,
            layers: Vec::new(),
            dirty: false,
        };

        atlas.push_layer();
        atlas.initialize();
        atlas
    }

    /// Adds a new empty layer
    fn push_layer(&mut self) {
        self.layers.push(Layer {
            image: RgbaImage::new(self.layer_size.width, self.layer_size.height),
            cursor: FontAtlasPoint::zero(),
            current_row_height: 0,
        });
    }

    /// Initializes the atlas.
    ///
    /// Puts a white pixel at (0, 0) of layer 0 for solid color texturing.
    fn initialize(&mut self) {
        let mut allocation = self.allocate(FontAtlasSize::new(1, 1));
        allocation.sub_image.put_pixel(0, 0, Srgba::WHITE.into());
    }

    /// Allocates a new glyph
    pub fn allocate(&mut self, requested_size: FontAtlasSize) -> AllocatedGlyph<'_> {
        const PADDING: u32 = 1;

        let mut idx = self.layers.len() - 1;

        {
            let layer = &mut self.layers[idx];

            // start new row if new allocation doesn't fit on current row
            if layer.cursor.x + requested_size.width > self.layer_size.width {
                layer.cursor.x = 0;
                layer.cursor.y += layer.current_row_height + PADDING;
                layer.current_row_height = 0;
            }

            // add new layer if new allocation doesn't fit on current layer
            if layer.cursor.y + requested_size.height > self.layer_size.height {
                if (self.layers.len() as u32) < self.max_layers {
                    self.push_layer();
                    idx = self.layers.len() - 1;
                } else {
                    // just choose a sufficiently large layer size and count to
                    // avoid panic. a more sophisticated approach can be implemented
                    // later if needed
                    panic!("font atlas reached maximum size!");
                }
            }
        }

        let layer = &mut self.layers[idx];
        layer.current_row_height = layer.current_row_height.max(requested_size.height);
        let pos = layer.cursor;
        layer.cursor.x += requested_size.width + PADDING;
        self.dirty = true;

        AllocatedGlyph {
            sub_image: layer.image.sub_image(
                pos.x,
                pos.y,
                requested_size.width,
                requested_size.height,
            ),
            layer_idx: idx as u32,
            uv: NormalizedRect::new(
                pos.normalize(self.layer_size),
                (pos + requested_size.to_vector()).normalize(self.layer_size),
            ),
        }
    }

    pub fn take_delta(&mut self) -> Option<ImageDelta> {
        if !std::mem::replace(&mut self.dirty, false) {
            return None;
        }

        let mut bytes = Vec::with_capacity(
            self.layers.len() * (self.layer_size.width * self.layer_size.height * 4) as usize,
        );

        for layer in &self.layers {
            bytes.extend_from_slice(layer.image.as_raw());
        }

        Some(ImageDelta::new(
            ImageData {
                format: wgpu::TextureFormat::Rgba8Unorm,
                width: self.layer_size.width,
                height: self.layer_size.height,
                array_layers: self.layers.len() as u32,
                mipmap_count: NonZeroU32::new(1).expect("1 is non-zero"),
                data_order: DataOrder::default(),
                bytes,
            },
            TextureOptions::LINEAR,
        ))
    }
}
