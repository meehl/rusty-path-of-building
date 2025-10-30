use crate::{
    color::Srgba,
    dpi::{
        ConvertToLogical, LogicalRect, LogicalVector, PhysicalPoint, PhysicalRect, PhysicalVector,
    },
    fonts::{
        atlas::{FontAtlas, FontAtlasRect},
        glyph_key::GlyphKey,
    },
    math::{Point, Size},
};
use ahash::HashMap;
use image::GenericImage;
use ordered_float::OrderedFloat;
use parley::{FontData, GlyphRun};
use std::borrow::Cow;
use swash::zeno;

type FontBlobId = u64;
type FontIndex = u32;
type SwashFontOffset = u32;
pub type StyleId = u32;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct StyleKey<'a> {
    font_blob_id: FontBlobId,
    font_size: OrderedFloat<f32>,
    normalized_coords: Cow<'a, [i16]>,
    skew: i8,
}

impl<'a> StyleKey<'a> {
    fn new(
        font_blob_id: FontBlobId,
        font_size: f32,
        normalized_coords: &'a [i16],
        skew: i8,
    ) -> Self {
        Self {
            font_blob_id,
            font_size: font_size.into(),
            normalized_coords: Cow::Borrowed(normalized_coords),
            skew,
        }
    }

    fn to_static(&self) -> StyleKey<'static> {
        StyleKey {
            normalized_coords: self.normalized_coords.clone().into_owned().into(),
            ..*self
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CachedGlyph {
    // absolute uv rect within font atlas
    pub uv: FontAtlasRect,
    // offset from top/left to baseline
    pub baseline_offset: PhysicalVector<i32>,
}

pub struct RasterizedGlyph {
    // NOTE: this is relative to the layout origin
    pub rect: LogicalRect<f32>,
    pub uv: FontAtlasRect,
    pub color: Srgba,
}

impl RasterizedGlyph {
    fn from_cached(
        cached: CachedGlyph,
        position: PhysicalPoint<i32>,
        color: Srgba,
        pixels_per_point: f32,
    ) -> Self {
        let glyph_rect = PhysicalRect::from_origin_and_size(
            position + cached.baseline_offset,
            cached.uv.cast_unit().cast().size(),
        );

        RasterizedGlyph {
            rect: glyph_rect.to_logical(pixels_per_point),
            uv: cached.uv,
            color,
        }
    }
}

pub struct GlyphRasterizer {
    scale_context: swash::scale::ScaleContext,
    swash_keys: HashMap<(FontBlobId, FontIndex), (SwashFontOffset, swash::CacheKey)>,
    // Style properties (font, size, etc.) are the same for each glyph run and don't
    // need to be part of each glyph key. Instead, associate each style with its own
    // ID and include that in the glyph key.
    style_ids: HashMap<StyleKey<'static>, StyleId>,
    next_style_id: StyleId,
    cached_glyphs: HashMap<GlyphKey, Option<CachedGlyph>>,
    // scratch image buffer used to write bitmap data into
    scratch: swash::scale::image::Image,
}

impl GlyphRasterizer {
    pub fn new() -> Self {
        Self {
            scale_context: swash::scale::ScaleContext::new(),
            swash_keys: Default::default(),
            style_ids: Default::default(),
            next_style_id: 0,
            cached_glyphs: Default::default(),
            scratch: Default::default(),
        }
    }

    pub fn clear(&mut self) {
        self.swash_keys.clear();
        self.style_ids.clear();
        self.next_style_id = 0;
        self.cached_glyphs.clear();
    }

    /// Gets a swash::FontRef from FontData
    fn get_font_ref<'a>(&mut self, font: &'a FontData) -> swash::FontRef<'a> {
        let font_blob_id = font.data.id();

        let (swash_offset, swash_key) = *self
            .swash_keys
            .entry((font_blob_id, font.index))
            .or_insert_with(|| {
                let font_ref =
                    swash::FontRef::from_index(font.data.data(), font.index as usize).unwrap();
                (font_ref.offset, font_ref.key)
            });

        swash::FontRef {
            data: font.data.data(),
            offset: swash_offset,
            key: swash_key,
        }
    }

    fn get_style_id(
        &mut self,
        font_data: &FontData,
        font_size: f32,
        norm_coords: &[i16],
        skew: i8,
    ) -> StyleId {
        let style_key = StyleKey::new(font_data.data.id(), font_size, norm_coords, skew);
        match self.style_ids.get(&style_key) {
            Some(key) => *key,
            None => *self
                .style_ids
                .entry(style_key.to_static())
                .or_insert_with(|| {
                    let id = self.next_style_id;
                    self.next_style_id += 1;
                    id
                }),
        }
    }

    /// Rasterizes glyph run and returns the placement and UV for each glyph.
    /// Can return `None` if glyph doesn't take up any space (e.g. whitespace).
    pub fn rasterize_glyph_run<'slf: 'run, 'run, 'atlas>(
        &'slf mut self,
        atlas: &'atlas mut FontAtlas,
        glyph_run: &'run GlyphRun<'_, Srgba>,
        // additional offset relative to layout origin
        glyph_offset: LogicalVector<f32>,
        pixels_per_point: f32,
    ) -> impl Iterator<Item = Option<RasterizedGlyph>> + use<'slf, 'run, 'atlas> {
        let run = glyph_run.run();
        let color = glyph_run.style().brush;
        let font_size = run.font_size() * pixels_per_point;
        let normalized_coords = run.normalized_coords();
        let skew = run.synthesis().skew(); // skew angle for faux italic

        let font_ref = self.get_font_ref(run.font());
        let style_id = self.get_style_id(
            run.font(),
            font_size,
            normalized_coords,
            // parley stores skew as i8 internally so this conversion is ok
            skew.unwrap_or_default() as i8,
        );

        let mut scaler = self
            .scale_context
            .builder(font_ref)
            .size(font_size)
            .normalized_coords(normalized_coords)
            .hint(true)
            .build();

        let image = &mut self.scratch;
        let cached_glyphs = &mut self.cached_glyphs;
        glyph_run.positioned_glyphs().map(move |mut glyph| {
            glyph.x += glyph_offset.x;
            glyph.y += glyph_offset.y;

            let (glyph_key, glyph_pos) = GlyphKey::from_glyph(&glyph, style_id, pixels_per_point);

            if let Some(cached_glyph) = cached_glyphs.get(&glyph_key) {
                return cached_glyph.map(|cached| {
                    RasterizedGlyph::from_cached(cached, glyph_pos, color, pixels_per_point)
                });
            }

            let fract_offset = glyph_key.get_fractional_offset();

            image.clear();
            let did_render = swash::scale::Render::new(&[
                swash::scale::Source::ColorOutline(0),
                swash::scale::Source::ColorBitmap(swash::scale::StrikeWith::BestFit),
                swash::scale::Source::Outline,
            ])
            .format(zeno::Format::Alpha)
            .transform(skew.map(|skew| {
                zeno::Transform::skew(zeno::Angle::from_degrees(skew), zeno::Angle::ZERO)
            }))
            .offset(fract_offset)
            .render_into(&mut scaler, glyph.id as u16, image);

            if !did_render || image.placement.width == 0 || image.placement.height == 0 {
                cached_glyphs.insert(glyph_key, None);
                return None;
            };

            let atlas_region = write_to_atlas(image, atlas);

            let cached_glyph = CachedGlyph {
                uv: atlas_region,
                baseline_offset: PhysicalVector::new(image.placement.left, -image.placement.top),
            };
            cached_glyphs.insert(glyph_key, Some(cached_glyph));

            Some(RasterizedGlyph::from_cached(
                cached_glyph,
                glyph_pos,
                color,
                pixels_per_point,
            ))
        })
    }
}

/// Writes rasterized glyph to atlas and returns region it wrote into
fn write_to_atlas(image: &swash::scale::image::Image, atlas: &mut FontAtlas) -> FontAtlasRect {
    let mut atlas_region = atlas.allocate(Size::new(image.placement.width, image.placement.height));

    match image.content {
        swash::scale::image::Content::Mask => {
            let mut i = 0;
            for y in 0..image.placement.height {
                for x in 0..image.placement.width {
                    let a = image.data[i];
                    // SAFETY: allocated atlas region and swash image have the same size
                    unsafe {
                        atlas_region.unsafe_put_pixel(x, y, Srgba::new(255, 255, 255, a).into())
                    };
                    i += 1;
                }
            }
        }
        _ => unreachable!(),
    };

    FontAtlasRect::from_origin_and_size(
        Point::new(atlas_region.offsets().0, atlas_region.offsets().1),
        Size::new(image.placement.width, image.placement.height),
    )
}
