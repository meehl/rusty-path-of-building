use std::num::NonZeroU32;

use crate::{
    color::Srgba,
    dpi::PhysicalSize,
    math::{Point, Rect, Scale, Size},
    renderer::{
        image::{DataOrder, ImageData, ImageDelta, MipStrategy, PartialDeltaOrigin},
        textures::TextureOptions,
    },
    uv::{UvRect, UvSpace},
};
use image::{GenericImage, RgbaImage, SubImage};

pub struct FontAtlasSpace;
pub type FontAtlasPoint = Point<u32, FontAtlasSpace>;
pub type FontAtlasSize = Size<u32, FontAtlasSpace>;
pub type FontAtlasRect = Rect<u32, FontAtlasSpace>;

pub struct AllocatedGlyph<'a> {
    pub sub_image: SubImage<&'a mut RgbaImage>,
    pub layer_idx: u32,
    pub uv: UvRect,
}

struct Layer {
    image: RgbaImage,
    /// Position of next allocation
    cursor: FontAtlasPoint,
    current_row_height: u32,
    /// Dirty region
    dirty: FontAtlasRect,
}

pub struct FontAtlas {
    layer_size: FontAtlasSize,
    // maximum amount of layers
    max_layers: u32,
    layers: Vec<Layer>,
    // used to convert to normalized UV coordinates
    to_uv: Scale<f32, FontAtlasSpace, UvSpace>,
    /// Set when a full texture upload is required
    needs_full_update: bool,
}

impl FontAtlas {
    pub fn new(layer_size: u32, max_layers: u32) -> Self {
        let mut atlas = Self {
            layer_size: Size::new(layer_size, layer_size),
            max_layers,
            layers: Vec::new(),
            to_uv: Scale::new(1.0 / layer_size as f32),
            needs_full_update: false,
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
            dirty: FontAtlasRect::zero(),
        });

        // adding a new layer requires a full texture re-allocation
        self.needs_full_update = true;
    }

    /// Initializes the atlas.
    ///
    /// Puts a white pixel at (0, 0) of layer 0 for solid color texturing.
    fn initialize(&mut self) {
        let mut allocation = self.allocate(FontAtlasSize::new(1, 1));
        allocation.sub_image.put_pixel(0, 0, Srgba::WHITE.into());
    }

    /// Allocates a new glyph
    fn allocate(&mut self, requested_size: FontAtlasSize) -> AllocatedGlyph<'_> {
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

        let glyph_rect = FontAtlasRect::from_origin_and_size(pos, requested_size);
        let uv = self.to_uv.transform_box2d(&glyph_rect.cast());

        // extend dirty region to include new glyph allocation
        layer.dirty = layer.dirty.union(&glyph_rect);

        AllocatedGlyph {
            sub_image: layer.image.sub_image(
                pos.x,
                pos.y,
                requested_size.width,
                requested_size.height,
            ),
            layer_idx: idx as u32,
            uv,
        }
    }

    pub fn take_delta(&mut self) -> Vec<ImageDelta> {
        // first check if full update is required
        if std::mem::replace(&mut self.needs_full_update, false) {
            let mut bytes = Vec::with_capacity(
                self.layers.len() * (self.layer_size.width * self.layer_size.height * 4) as usize,
            );

            for layer in &mut self.layers {
                bytes.extend_from_slice(layer.image.as_raw());
                // reset layer's dirty region since we're doing a full update anyway
                layer.dirty = FontAtlasRect::zero();
            }

            return vec![ImageDelta::full(
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
                MipStrategy::None,
            )];
        }

        // add partial update for each dirty layer
        let mut partial_updates = Vec::new();
        for (i, layer) in self.layers.iter_mut().enumerate() {
            let dirty = std::mem::replace(&mut layer.dirty, FontAtlasRect::zero());
            if dirty.is_empty() {
                continue;
            }

            let bytes = layer
                .image
                .sub_image(dirty.min.x, dirty.min.y, dirty.width(), dirty.height())
                .to_image()
                .into_vec();

            partial_updates.push(ImageDelta::partial(
                ImageData {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    width: dirty.width(),
                    height: dirty.height(),
                    array_layers: 1,
                    mipmap_count: NonZeroU32::new(1).expect("1 is non-zero"),
                    data_order: DataOrder::default(),
                    bytes,
                },
                PartialDeltaOrigin {
                    x: dirty.min.x,
                    y: dirty.min.y,
                    layer: i as u32,
                },
            ));
        }

        partial_updates
    }

    pub fn write_mask(
        &mut self,
        image: &swash::scale::image::Image,
    ) -> (PhysicalSize<u32>, UvRect, u32) {
        debug_assert!(matches!(image.content, swash::scale::image::Content::Mask));

        let size = Size::new(image.placement.width, image.placement.height);
        let mut allocation = self.allocate(size);

        let mut i = 0;
        for y in 0..size.height {
            for x in 0..size.width {
                let a = image.data[i];
                // SAFETY: allocated atlas region and swash image have the same size
                unsafe {
                    allocation.sub_image.unsafe_put_pixel(
                        x,
                        y,
                        Srgba::new(255, 255, 255, a).into(),
                    );
                }
                i += 1;
            }
        }

        (size.cast_unit(), allocation.uv, allocation.layer_idx)
    }
}
