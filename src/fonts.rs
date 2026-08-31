use crate::{
    color::Srgba,
    dpi::{PhysicalVector, ScaleFactor},
    fonts::{
        atlas::FontAtlas, glyph_key::SubpixelBin, layout::PositionedGlyph,
        layout_cache::LayoutCache, rasterizer::GlyphRasterizer,
    },
    renderer::image::ImageDelta,
    util::calculate_hash,
};
use ahash::HashMap;
use ordered_float::OrderedFloat;
use parley::{
    FontContext, FontFamily, FontFamilyName, FontWeight, GenericFamily, LayoutContext,
    StyleProperty, TextStyle, fontique::Blob,
};
use std::{rc::Rc, sync::Arc};

pub use layout::{Alignment, FontStyle, Layout, LayoutJob, LayoutToLogical};

mod atlas;
mod glyph_key;
mod layout;
mod layout_cache;
mod rasterizer;
mod style_cache;

/// Data of a .ttf or .otf file
#[derive(Clone, Debug)]
pub struct FontData {
    data: std::borrow::Cow<'static, [u8]>,
}

impl FontData {
    pub fn from_static(font_data: &'static [u8]) -> Self {
        Self {
            data: std::borrow::Cow::Borrowed(font_data),
        }
    }
}

impl AsRef<[u8]> for FontData {
    fn as_ref(&self) -> &[u8] {
        self.data.as_ref()
    }
}

#[derive(Clone, Debug, Default)]
pub struct FontDefinitions {
    pub font_data: HashMap<String, Arc<FontData>>,
    pub generic_families: HashMap<GenericFamily, Vec<String>>,
}

pub struct Fonts {
    definitions: FontDefinitions,
    font_context: FontContext,
    layout_context: LayoutContext<Srgba>,
    atlas: FontAtlas,
    glyph_rasterizer: GlyphRasterizer,
    layout_cache: LayoutCache,
}

impl Fonts {
    pub fn new(definitions: FontDefinitions) -> Self {
        let mut fonts = Self {
            definitions,
            font_context: FontContext::new(),
            layout_context: LayoutContext::new(),
            atlas: FontAtlas::new(1024, 1024),
            glyph_rasterizer: GlyphRasterizer::new(),
            layout_cache: Default::default(),
        };

        fonts.register_fonts();

        fonts
    }

    fn register_fonts(&mut self) {
        for data in self.definitions.font_data.values() {
            let blob = Blob::new(Arc::new(data.data.clone()));
            self.font_context.collection.register_fonts(blob, None);
        }

        for (generic_family, family_fonts) in &self.definitions.generic_families {
            let family_ids: Vec<_> = family_fonts
                .iter()
                .filter_map(|family_name| self.font_context.collection.family_id(family_name))
                .collect();

            self.font_context
                .collection
                .set_generic_families(*generic_family, family_ids.into_iter());
        }
    }

    /// Needs to be called at beginning of each frame.
    pub fn begin_frame(&mut self) {
        self.layout_cache.flush();
    }

    /// Gets changes to the font atlas texture since last call.
    pub fn font_atlas_delta(&mut self) -> Vec<ImageDelta> {
        self.atlas.take_delta()
    }

    /// Warms the glyph cache for common ACSII characters
    pub fn preload_common_characters(&mut self, font_size: f32, scale_factor: ScaleFactor<f32>) {
        const ASCII_PRINTABLE_START: u8 = 32;
        const ASCII_PRINTABLE_END: u8 = 126;

        let mut common_chars =
            String::with_capacity((ASCII_PRINTABLE_END - ASCII_PRINTABLE_START + 1) as usize);

        for c in ASCII_PRINTABLE_START..=ASCII_PRINTABLE_END {
            common_chars.push(c as char);
        }

        self.preload_text(
            &common_chars,
            font_size,
            FontFamily::Single(FontFamilyName::Generic(GenericFamily::Monospace)),
            None,
            parley::FontStyle::Normal,
            scale_factor,
        );
        self.preload_text(
            &common_chars,
            font_size,
            FontFamily::Single(FontFamilyName::Generic(GenericFamily::SansSerif)),
            None,
            parley::FontStyle::Normal,
            scale_factor,
        );
        self.preload_text(
            &common_chars,
            font_size,
            FontFamily::Single(FontFamilyName::Generic(GenericFamily::SansSerif)),
            Some(FontWeight::BOLD),
            parley::FontStyle::Normal,
            scale_factor,
        );
    }

    fn preload_text(
        &mut self,
        text: &str,
        font_size: f32,
        font_family: FontFamily,
        font_weight: Option<FontWeight>,
        font_style: parley::FontStyle,
        scale_factor: ScaleFactor<f32>,
    ) {
        profiling::scope!("preload_text");

        let style = TextStyle {
            font_family,
            font_weight: font_weight.unwrap_or(FontWeight::NORMAL),
            font_style,
            font_size,
            ..Default::default()
        };
        let mut builder = self.layout_context.tree_builder(
            &mut self.font_context,
            scale_factor.get(),
            false,
            &style,
        );
        builder.push_text(text);

        let (mut parley_layout, _) = builder.build();
        parley_layout.break_all_lines(None);

        for_each_glyph_run(&parley_layout, |run| {
            for horizontal_offset in SubpixelBin::<4>::BIN_OFFSETS {
                self.glyph_rasterizer
                    .rasterize_glyph_run(
                        &mut self.atlas,
                        run,
                        PhysicalVector::new(horizontal_offset, 0.0),
                        scale_factor,
                    )
                    .for_each(|_| {});
            }
        });
    }

    pub fn layout(&mut self, job: LayoutJob, scale_factor: ScaleFactor<f32>) -> Rc<Layout> {
        let hash = calculate_hash(&(&job, OrderedFloat(scale_factor.get())));

        if let Some(cached_layout) = self.layout_cache.get(hash) {
            return cached_layout;
        }

        let layout = Rc::new(self.build_layout(job, scale_factor));

        self.layout_cache.insert(hash, Rc::clone(&layout));

        layout
    }

    fn build_layout(&mut self, job: LayoutJob, scale_factor: ScaleFactor<f32>) -> Layout {
        let default_style = TextStyle::default();
        let style = TextStyle {
            font_family: job.font_family,
            font_size: job.font_size.into(),
            line_height: parley::LineHeight::Absolute(job.line_height.into()),
            font_weight: job
                .font_weight
                .map(|w| parley::FontWeight::new(w.into()))
                .unwrap_or(default_style.font_weight),
            font_style: job.font_style.into(),
            ..default_style
        };

        // scale factor is passed to builder so coordinates in resulting layout will be physical
        let mut builder = self.layout_context.tree_builder(
            &mut self.font_context,
            scale_factor.get(),
            false,
            &style,
        );

        for segment in job.segments {
            let brush_style = StyleProperty::Brush(segment.color);
            builder.push_style_modification_span(&[brush_style]);
            builder.push_text(segment.text);
            builder.pop_style_span();
        }

        let (mut parley_layout, _) = builder.build();
        parley_layout.break_all_lines(None);

        if let Some(alignment) = job.alignment {
            let parley_alignment = match alignment {
                Alignment::Min => parley::Alignment::Start,
                Alignment::Center => parley::Alignment::Center,
                Alignment::Max => parley::Alignment::End,
            };
            parley_layout.align(parley_alignment, parley::AlignmentOptions::default());
        }

        let mut glyphs = Vec::new();

        for_each_glyph_run(&parley_layout, |run| {
            for rasterized in self.glyph_rasterizer.rasterize_glyph_run(
                &mut self.atlas,
                run,
                PhysicalVector::zero(),
                scale_factor,
            ) {
                let Some(glyph) = rasterized else { continue };
                glyphs.push(PositionedGlyph {
                    rect: glyph.rect,
                    uv: glyph.uv,
                    layer_idx: glyph.texture_layer_idx,
                    color: glyph.color,
                });
            }
        });

        Layout {
            // full_width is physical so it needs to be converted back to logical
            width: parley_layout.full_width() / scale_factor.get(),
            glyphs,
            parley_layout,
        }
    }
}

/// Applies a function to each glyph run in a `parley::Layout`
fn for_each_glyph_run<'a>(
    layout: &'a parley::Layout<Srgba>,
    mut f: impl FnMut(&parley::GlyphRun<'a, Srgba>),
) {
    for line in layout.lines() {
        for item in line.items() {
            if let parley::PositionedLayoutItem::GlyphRun(run) = item {
                f(&run);
            }
        }
    }
}
