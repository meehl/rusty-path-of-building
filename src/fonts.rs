use crate::{
    color::Srgba,
    dpi::{LogicalPoint, LogicalVector},
    fonts::{
        atlas::FontAtlas, glyph_key::SubpixelBin, layout::LayoutRow, rasterizer::GlyphRasterizer,
    },
    renderer::image::ImageDelta,
    util::calculate_hash,
};
use ahash::HashMap;
use ordered_float::OrderedFloat;
use parley::{
    FontContext, FontFamily, FontStack, FontWeight, GenericFamily, LayoutContext, StyleProperty,
    TextStyle, fontique::Blob,
};
use std::sync::Arc;

pub use atlas::FontAtlasSize;
pub use layout::{Alignment, Layout, LayoutJob};

mod atlas;
mod glyph_key;
mod layout;
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
            atlas: FontAtlas::new(1024),
            glyph_rasterizer: GlyphRasterizer::new(),
            layout_cache: LayoutCache::default(),
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
        // recreate atlas when it becomes too full or overflowed
        if self.atlas.capacity() > 0.9 {
            self.clear_atlas();
        }
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
            FontFamily::Generic(GenericFamily::Monospace),
            None,
        );
        self.preload_text(
            &common_chars,
            font_size,
            FontFamily::Generic(GenericFamily::SansSerif),
            None,
        );
        self.preload_text(
            &common_chars,
            font_size,
            FontFamily::Generic(GenericFamily::SansSerif),
            Some(FontWeight::BOLD),
        );
    }

    fn preload_text(
        &mut self,
        text: &str,
        font_size: f32,
        font_family: FontFamily,
        font_weight: Option<FontWeight>,
    ) {
        profiling::scope!("preload_text");

        let style = TextStyle {
            font_stack: FontStack::Single(font_family),
            font_weight: font_weight.unwrap_or(FontWeight::NORMAL),
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

    pub fn font_atlas(&self) -> &FontAtlas {
        &self.atlas
    }

    pub fn layout(&mut self, job: LayoutJob, pixels_per_point: f32) -> Arc<Layout> {
        let hash = calculate_hash(&(&job, OrderedFloat(pixels_per_point)));

        if let Some(cached_layout) = self.layout_cache.get(hash) {
            return cached_layout;
        }

        let default_style = TextStyle::default();
        let style = TextStyle {
            font_stack: parley::FontStack::Single(job.font_family),
            font_size: job.font_size.into(),
            line_height: parley::LineHeight::Absolute(job.line_height.into()),
            font_weight: job
                .font_weight
                .map(|w| parley::FontWeight::new(w.into()))
                .unwrap_or(default_style.font_weight),
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
            parley_layout.align(None, alignment, parley::AlignmentOptions::default());
        }

        let mut layout_rows = Vec::new();
        let mut num_of_vertices = 0;
        let mut num_of_indices = 0;

        for line in parley_layout.lines() {
            let mut layout_row = LayoutRow::default();

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

                    layout_row.glyphs.push(glyph);
                    num_of_vertices += 4;
                    num_of_indices += 6;
                }
            }

            if !layout_row.glyphs.is_empty() {
                layout_rows.push(layout_row);
            }
        }

        let layout = Arc::new(Layout {
            job_hash: hash,
            parley_layout,
            rows: layout_rows,
            num_of_vertices,
            num_of_indices,
        });

        self.layout_cache.insert(hash, Arc::clone(&layout));

        layout
    }

    /// Clear atlas and invalidate caches depend on atlas state
    fn clear_atlas(&mut self) {
        self.atlas.clear();
        self.glyph_rasterizer.clear();
        self.layout_cache.clear();
    }

    /// Width of laid out text
    pub fn get_text_width(&mut self, job: LayoutJob, pixels_per_point: f32) -> i32 {
        let layout = self.layout(job, pixels_per_point);
        layout.width() as i32
    }

    /// Text index at cursor location
    pub fn get_text_index_at_cursor(
        &mut self,
        job: LayoutJob,
        cursor: LogicalPoint<f32>,
        pixels_per_point: f32,
    ) -> usize {
        let layout = self.layout(job, pixels_per_point);
        layout.cursor_index(cursor)
    }
}

struct CachedLayout {
    generation: u32,
    layout: Arc<Layout>,
}

#[derive(Default)]
struct LayoutCache {
    current_generation: u32,
    cache: nohash_hasher::IntMap<u64, CachedLayout>,
}

impl LayoutCache {
    fn get(&mut self, hash: u64) -> Option<Arc<Layout>> {
        match self.cache.entry(hash) {
            std::collections::hash_map::Entry::Occupied(entry) => {
                let cached = entry.into_mut();
                cached.generation = self.current_generation;
                Some(Arc::clone(&cached.layout))
            }
            std::collections::hash_map::Entry::Vacant(_) => None,
        }
    }

    fn insert(&mut self, hash: u64, layout: Arc<Layout>) {
        self.cache.insert(
            hash,
            CachedLayout {
                generation: self.current_generation,
                layout,
            },
        );
    }

    /// Removes unused layouts
    pub fn flush(&mut self) {
        self.cache
            .retain(|_key, cached| cached.generation == self.current_generation);
        self.current_generation = self.current_generation.wrapping_add(1);
    }

    pub fn clear(&mut self) {
        self.cache.clear();
    }
}
