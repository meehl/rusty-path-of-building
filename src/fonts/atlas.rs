use crate::{
    color::Srgba,
    math::{Point, Rect, Size},
    renderer::{
        image::{ImageData, ImageDelta},
        textures::TextureOptions,
    },
};
use image::{GenericImage, RgbaImage, SubImage, imageops};

pub struct FontAtlasSpace;
pub type FontAtlasPoint = Point<u32, FontAtlasSpace>;
pub type FontAtlasSize = Size<u32, FontAtlasSpace>;
pub type FontAtlasRect = Rect<u32, FontAtlasSpace>;

pub struct FontAtlas {
    // max width/height of atlas texture
    max_texture_side: u32,
    image: RgbaImage,
    // position of next allocation
    cursor: FontAtlasPoint,
    current_row_height: u32,
    // atlas has been altered and needs to be reuploaded to the GPU
    // TODO: only mark changed region as dirty and perform partial texture update
    dirty: bool,
    // atlas has overflowed and needs to be recreated
    overflowed: bool,
}

impl FontAtlas {
    pub fn new(max_texture_side: u32) -> Self {
        // start out with maximum width and let height grow as needed
        let width = max_texture_side;
        let initial_height = 256;

        let mut atlas = Self {
            max_texture_side,
            image: RgbaImage::new(width, initial_height),
            cursor: FontAtlasPoint::zero(),
            current_row_height: 0,
            dirty: false,
            overflowed: false,
        };

        atlas.initialize();
        atlas
    }

    fn initialize(&mut self) {
        // Puts white pixel at (0, 0).
        // NOTE: Rendering a solid color shape is done by setting the texture to
        // the font atlas and sampling the white pixel at (0, 0).
        let mut sub_image = self.allocate(FontAtlasSize::new(1, 1));
        sub_image.put_pixel(0, 0, Srgba::WHITE.into());
    }

    // TODO: use an actual bin packing algorithm for tighter packing
    /// Returns a mutable view into the atlas of given size.
    pub fn allocate(&mut self, size: FontAtlasSize) -> SubImage<&mut RgbaImage> {
        const PADDING: u32 = 1;

        if self.cursor.x + size.width > self.image.width() {
            self.cursor.x = 0;
            self.cursor.y += self.current_row_height + PADDING;
            self.current_row_height = 0;
        }

        self.current_row_height = self.current_row_height.max(size.height);

        let required_atlas_height = self.cursor.y + self.current_row_height;
        if required_atlas_height > self.max_texture_side {
            log::warn!("font atlas overflowed!");
            // start overwriting old glyphs
            self.cursor = Point::new(0, self.image.height() / 3);
            // setting this flag causes atlas to be recreated next frame
            self.overflowed = true;
        } else if required_atlas_height > self.image().height() {
            // increase height
            let mut new_height = self.image.height();
            while new_height < required_atlas_height {
                new_height *= 2;
            }
            self.image = extend_image_height(&self.image, new_height, Srgba::TRANSPARENT);
        }

        let pos = self.cursor;
        self.cursor.x += size.width + PADDING;

        self.dirty = true;

        self.image.sub_image(pos.x, pos.y, size.width, size.height)
    }

    pub fn take_delta(&mut self) -> Option<ImageDelta> {
        let dirty = std::mem::replace(&mut self.dirty, false);
        if dirty {
            Some(ImageDelta::new(
                ImageData::from(self.image.clone()),
                TextureOptions::LINEAR,
            ))
        } else {
            None
        }
    }

    pub fn capacity(&self) -> f32 {
        if self.overflowed {
            1.0
        } else {
            (self.cursor.y + self.current_row_height) as f32 / self.max_texture_side as f32
        }
    }

    pub fn clear(&mut self) {
        self.image.fill(0);
        self.cursor = FontAtlasPoint::zero();
        self.current_row_height = 0;
        self.dirty = false;
        self.overflowed = false;
        self.initialize();
    }

    pub fn image(&self) -> &RgbaImage {
        &self.image
    }

    pub fn size(&self) -> FontAtlasSize {
        FontAtlasSize::new(self.image.width(), self.image.height())
    }
}

fn extend_image_height(image: &RgbaImage, new_height: u32, fill_color: Srgba) -> RgbaImage {
    let width = image.width();
    let mut extended_image = RgbaImage::from_pixel(width, new_height, fill_color.into());
    imageops::overlay(&mut extended_image, image, 0, 0);
    extended_image
}
