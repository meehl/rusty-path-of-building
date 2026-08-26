use crate::{
    color::Srgba,
    dpi::{PhysicalRect, PhysicalSize, PhysicalVector, ScaleFactor},
    fonts::{
        atlas::FontAtlas,
        glyph_key::GlyphKey,
        layout::{LayoutPoint, LayoutRect, LayoutVector},
        style_cache::StyleCache,
    },
    uv::UvRect,
};
use ahash::HashMap;
use parley::{FontData, GlyphRun};
use swash::zeno;

type FontBlobId = u64;
type FontIndex = u32;
type SwashFontOffset = u32;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CachedGlyph {
    /// Size of the trimmed bitmap. Swash crops away transparant rows/columns so this is smaller
    /// than the glyph's full advance box.
    pub size: PhysicalSize<u32>,
    pub uv: UvRect,
    pub texture_layer_idx: u32,
    /// Offset from the glyph's origin (top-left) to the trimmed bitmap's top-left corner.
    pub baseline_offset: PhysicalVector<i32>,
}

pub struct RasterizedGlyph {
    pub rect: LayoutRect<f32>,
    pub uv: UvRect,
    pub texture_layer_idx: u32,
    pub color: Srgba,
}

pub struct GlyphRasterizer {
    scale_context: swash::scale::ScaleContext,
    swash_keys: HashMap<(FontBlobId, FontIndex), (SwashFontOffset, swash::CacheKey)>,
    style_cache: StyleCache,
    cached_glyphs: HashMap<GlyphKey, Option<CachedGlyph>>,
    /// Scratch image buffer used to temporarily write bitmap data into
    scratch: swash::scale::image::Image,
}

impl GlyphRasterizer {
    pub fn new() -> Self {
        Self {
            scale_context: swash::scale::ScaleContext::new(),
            swash_keys: Default::default(),
            style_cache: Default::default(),
            cached_glyphs: Default::default(),
            scratch: Default::default(),
        }
    }

    /// Gets a [`swash::FontRef`] from [`FontData`]
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

    /// Rasterizes a glyph run and returns the placement and UV for each glyph.
    /// Returns `None` for glyphs that doesn't take up visible space (e.g. whitespace).
    pub fn rasterize_glyph_run<'slf: 'run, 'run, 'atlas>(
        &'slf mut self,
        atlas: &'atlas mut FontAtlas,
        glyph_run: &'run GlyphRun<'_, Srgba>,
        // additional offset relative to layout origin
        layout_offset: LayoutVector<f32>,
        scale_factor: ScaleFactor<f32>,
    ) -> impl Iterator<Item = Option<RasterizedGlyph>> + use<'slf, 'run, 'atlas> {
        let run = glyph_run.run();
        let color = glyph_run.style().brush;
        let font_size = run.font_size() * scale_factor.get();
        let normalized_coords = run.normalized_coords();
        let skew = run.synthesis().skew(); // skew angle for faux italic

        let font_ref = self.get_font_ref(run.font());
        let style_id = self.style_cache.get_or_insert(
            run.font().data.id(),
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

        glyph_run.positioned_glyphs().map(move |glyph| {
            let layout_position = LayoutPoint::new(glyph.x, glyph.y) + layout_offset;

            let (glyph_key, pixel_position) =
                GlyphKey::from_position(layout_position, glyph.id, style_id, scale_factor.get());

            let cached = *cached_glyphs.entry(glyph_key).or_insert_with(|| {
                rasterize_glyph(&mut scaler, image, atlas, glyph.id, &glyph_key, skew)
            });

            cached.map(|cached| {
                // apply the baseline offset to get the actual bitmap position
                let bitmap_top_left = pixel_position + cached.baseline_offset;
                let physical_rect =
                    PhysicalRect::from_origin_and_size(bitmap_top_left, cached.size.cast());

                RasterizedGlyph {
                    rect: (physical_rect.cast() / scale_factor).cast_unit(),
                    uv: cached.uv,
                    texture_layer_idx: cached.texture_layer_idx,
                    color,
                }
            })
        })
    }
}

/// Renders glyph's bitmap into the atlas
fn rasterize_glyph(
    scaler: &mut swash::scale::Scaler,
    image: &mut swash::scale::image::Image,
    atlas: &mut FontAtlas,
    glyph_id: u32,
    glyph_key: &GlyphKey,
    skew: Option<f32>,
) -> Option<CachedGlyph> {
    image.clear();

    let did_render = swash::scale::Render::new(&[
        swash::scale::Source::ColorOutline(0),
        swash::scale::Source::ColorBitmap(swash::scale::StrikeWith::BestFit),
        swash::scale::Source::Outline,
    ])
    .format(zeno::Format::Alpha)
    .transform(
        skew.map(|skew| zeno::Transform::skew(zeno::Angle::from_degrees(skew), zeno::Angle::ZERO)),
    )
    .offset(glyph_key.get_fractional_offset())
    .render_into(scaler, glyph_id as u16, image);

    if !did_render || image.placement.width == 0 || image.placement.height == 0 {
        return None;
    };

    let (size, uv, layer_idx) = atlas.write_mask(image);

    Some(CachedGlyph {
        size,
        uv,
        texture_layer_idx: layer_idx,
        // swash uses `Origin::BottomLeft` so we need to negate the vertical component
        baseline_offset: PhysicalVector::new(image.placement.left, -image.placement.top),
    })
}
