use crate::{
    color::Srgba,
    dpi::LogicalVector,
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

pub use layout::{Alignment, FontStyle, Layout, LayoutJob};

mod atlas;
mod glyph_key;
mod layout;
mod layout_cache;
mod rasterizer;

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
        fonts.preload_common_characters(14.0);
        fonts.preload_common_characters(16.0);

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
    pub fn font_atlas_delta(&mut self) -> Option<ImageDelta> {
        self.atlas.take_delta()
    }

    pub fn preload_common_characters(&mut self, font_size: f32) {
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
        );
        self.preload_text(
            &common_chars,
            font_size,
            FontFamily::Single(FontFamilyName::Generic(GenericFamily::SansSerif)),
            None,
            parley::FontStyle::Normal,
        );
        self.preload_text(
            &common_chars,
            font_size,
            FontFamily::Single(FontFamilyName::Generic(GenericFamily::SansSerif)),
            Some(FontWeight::BOLD),
            parley::FontStyle::Normal,
        );
    }

    fn preload_text(
        &mut self,
        text: &str,
        font_size: f32,
        font_family: FontFamily,
        font_weight: Option<FontWeight>,
        font_style: parley::FontStyle,
    ) {
        profiling::scope!("preload_text");

        let style = TextStyle {
            font_family,
            font_weight: font_weight.unwrap_or(FontWeight::NORMAL),
            font_style,
            font_size,
            ..Default::default()
        };
        let mut builder =
            self.layout_context
                .tree_builder(&mut self.font_context, 1.0, false, &style);
        builder.push_text(text);

        let (mut layout, _) = builder.build();
        layout.break_all_lines(None);

        for line in layout.lines() {
            for item in line.items() {
                let parley::PositionedLayoutItem::GlyphRun(run) = item else {
                    continue;
                };

                for horizontal_offset in SubpixelBin::<4>::BIN_OFFSETS {
                    self.glyph_rasterizer
                        .rasterize_glyph_run(
                            &mut self.atlas,
                            &run,
                            LogicalVector::new(horizontal_offset, 0.0),
                            1.0,
                        )
                        .for_each(|_| {});
                }
            }
        }
    }

    pub fn layout(&mut self, job: LayoutJob, pixels_per_point: f32) -> Rc<Layout> {
        let hash = calculate_hash(&(&job, OrderedFloat(pixels_per_point)));

        if let Some(cached_layout) = self.layout_cache.get(hash) {
            return cached_layout;
        }

        let layout = Rc::new(self.build_layout(job, pixels_per_point));

        self.layout_cache.insert(hash, Rc::clone(&layout));

        layout
    }

    fn build_layout(&mut self, job: LayoutJob, pixels_per_point: f32) -> Layout {
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

        let mut builder =
            self.layout_context
                .tree_builder(&mut self.font_context, 1.0, false, &style);

        for segment in job.segments {
            let brush_style = StyleProperty::Brush(segment.color);
            builder.push_style_modification_span(&[brush_style]);
            builder.push_text(segment.text);
            builder.pop_style_span();
        }

        let (mut parley_layout, _) = builder.build();
        parley_layout.break_all_lines(None);

        // extra offset applied to each glyph to get position relative to layout origin
        let mut glyph_offset = LogicalVector::new(0.0, 0.0);
        if let Some(alignment) = job.alignment {
            let alignment = match alignment {
                Alignment::Min => parley::Alignment::Start,
                Alignment::Center => {
                    glyph_offset.x += -parley_layout.full_width() * 0.5;
                    parley::Alignment::Center
                }
                Alignment::Max => {
                    glyph_offset.x += -parley_layout.full_width();
                    parley::Alignment::End
                }
            };
            parley_layout.align(alignment, parley::AlignmentOptions::default());
        }

        let mut glyphs = Vec::new();

        for line in parley_layout.lines() {
            for item in line.items() {
                let parley::PositionedLayoutItem::GlyphRun(run) = item else {
                    continue;
                };

                for rasterized_glyph in self.glyph_rasterizer.rasterize_glyph_run(
                    &mut self.atlas,
                    &run,
                    glyph_offset,
                    pixels_per_point,
                ) {
                    let Some(glyph) = rasterized_glyph else {
                        continue;
                    };

                    glyphs.push(PositionedGlyph {
                        rect: glyph.rect,
                        uv: glyph.uv,
                        layer_idx: glyph.texture_layer_idx,
                        color: glyph.color,
                    });
                }
            }
        }

        Layout {
            width: parley_layout.full_width(),
            glyphs,
            parley_layout,
        }
    }
}
