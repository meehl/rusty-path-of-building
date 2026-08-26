use ahash::HashMap;
use ordered_float::OrderedFloat;
use std::borrow::Cow;

pub type StyleId = u32;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct StyleKey<'a> {
    font_blob_id: u64,
    font_size: OrderedFloat<f32>,
    normalized_coords: Cow<'a, [i16]>,
    skew: i8,
}

impl<'a> StyleKey<'a> {
    fn new(font_blob_id: u64, font_size: f32, normalized_coords: &'a [i16], skew: i8) -> Self {
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

/// Associates font style combinations (font, size, norm coords, skew) with a small `StyleId`, so
/// `GlyphKey` never has to compare full style data per glyph, just once per run.
#[derive(Default)]
pub struct StyleCache {
    ids: HashMap<StyleKey<'static>, StyleId>,
    next_id: StyleId,
}

impl StyleCache {
    pub fn get_or_insert(
        &mut self,
        font_blob_id: u64,
        font_size: f32,
        normalized_coords: &[i16],
        skew: i8,
    ) -> StyleId {
        let key = StyleKey::new(font_blob_id, font_size, normalized_coords, skew);

        if let Some(id) = self.ids.get(&key) {
            return *id;
        }

        let id = self.next_id;
        self.next_id += 1;
        self.ids.insert(key.to_static(), id);
        id
    }
}
