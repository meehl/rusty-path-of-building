use crate::{
    color::Srgba,
    dpi::{LogicalVector, PhysicalPoint, PhysicalRect, PhysicalSize, PhysicalVector, ScaleFactor},
    fonts::{atlas::FontAtlas, glyph_key::GlyphKey, layout::LayoutRect, style_cache::StyleCache},
    math::Size,
    uv::UvRect,
};
use ahash::HashMap;
use image::GenericImage;
use parley::{FontData, GlyphRun};
use swash::zeno;

type FontBlobId = u64;
type FontIndex = u32;
type SwashFontOffset = u32;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CachedGlyph {
    pub size: PhysicalSize<u32>,
    pub uv: UvRect,
    pub texture_layer_idx: u32,
    // offset from top/left to baseline
    pub baseline_offset: PhysicalVector<i32>,
}

pub struct RasterizedGlyph {
    pub rect: LayoutRect<f32>,
    pub uv: UvRect,
    pub texture_layer_idx: u32,
    pub color: Srgba,
}

impl RasterizedGlyph {
    fn from_cached(
        cached: CachedGlyph,
        position: PhysicalPoint<i32>,
        color: Srgba,
        scale_factor: ScaleFactor<f32>,
    ) -> Self {
        let glyph_rect = PhysicalRect::from_origin_and_size(
            position + cached.baseline_offset,
            cached.size.cast(),
        );

        RasterizedGlyph {
            rect: (glyph_rect.cast() / scale_factor).cast_unit(),
            uv: cached.uv,
            texture_layer_idx: cached.texture_layer_idx,
            color,
        }
    }
}

pub struct GlyphRasterizer {
    scale_context: swash::scale::ScaleContext,
    swash_keys: HashMap<(FontBlobId, FontIndex), (SwashFontOffset, swash::CacheKey)>,
    style_cache: StyleCache,
    cached_glyphs: HashMap<GlyphKey, Option<CachedGlyph>>,
    // scratch image buffer used to write bitmap data into
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

    /// Rasterizes glyph run and returns the placement and UV for each glyph.
    /// Can return `None` if glyph doesn't take up any space (e.g. whitespace).
    pub fn rasterize_glyph_run<'slf: 'run, 'run, 'atlas>(
        &'slf mut self,
        atlas: &'atlas mut FontAtlas,
        glyph_run: &'run GlyphRun<'_, Srgba>,
        // additional offset relative to layout origin
        glyph_offset: LogicalVector<f32>,
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
        glyph_run.positioned_glyphs().map(move |mut glyph| {
            glyph.x += glyph_offset.x;
            glyph.y += glyph_offset.y;

            let (glyph_key, glyph_pos) = GlyphKey::from_glyph(&glyph, style_id, scale_factor.get());

            if let Some(cached_glyph) = cached_glyphs.get(&glyph_key) {
                return cached_glyph.map(|cached| {
                    RasterizedGlyph::from_cached(cached, glyph_pos, color, scale_factor)
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

            let (glyph_size, atlas_uv, atlas_layer_idx) = write_to_atlas(image, atlas);

            let cached_glyph = CachedGlyph {
                size: glyph_size,
                uv: atlas_uv,
                texture_layer_idx: atlas_layer_idx,
                baseline_offset: PhysicalVector::new(image.placement.left, -image.placement.top),
            };
            cached_glyphs.insert(glyph_key, Some(cached_glyph));

            Some(RasterizedGlyph::from_cached(
                cached_glyph,
                glyph_pos,
                color,
                scale_factor,
            ))
        })
    }
}

/// Writes rasterized glyph to atlas and returns the UV coordinates of it
fn write_to_atlas(
    image: &swash::scale::image::Image,
    atlas: &mut FontAtlas,
) -> (PhysicalSize<u32>, UvRect, u32) {
    let size = Size::new(image.placement.width, image.placement.height);
    let mut allocated_glyph = atlas.allocate(size);

    match image.content {
        swash::scale::image::Content::Mask => {
            let mut i = 0;
            for y in 0..image.placement.height {
                for x in 0..image.placement.width {
                    let a = image.data[i];
                    // SAFETY: allocated atlas region and swash image have the same size
                    unsafe {
                        allocated_glyph.sub_image.unsafe_put_pixel(
                            x,
                            y,
                            Srgba::new(255, 255, 255, a).into(),
                        )
                    };
                    i += 1;
                }
            }
        }
        _ => unreachable!(),
    };

    (
        size.cast_unit(),
        allocated_glyph.uv,
        allocated_glyph.layer_idx,
    )
}
