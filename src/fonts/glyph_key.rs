use crate::{dpi::PhysicalPoint, fonts::style_cache::StyleId};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GlyphKey {
    glyph_id: swash::GlyphId,
    style_id: StyleId,
    x_bin: SubpixelBin<4>,
}

impl GlyphKey {
    pub fn from_position(
        position: PhysicalPoint<f32>,
        glyph_id: u32,
        style_id: StyleId,
    ) -> (Self, PhysicalPoint<i32>) {
        // x: use subpixel binning so bitmap encodes sub-pixel position
        let (x, x_bin) = SubpixelBin::<4>::new(position.x);
        // y: just rounded. not much benefit from sub-pixel vertical positioning
        let y = position.y.round() as i32;

        (
            Self {
                glyph_id: glyph_id as u16,
                style_id,
                x_bin,
            },
            PhysicalPoint::new(x, y),
        )
    }

    pub fn get_fractional_offset(&self) -> swash::zeno::Vector {
        swash::zeno::Vector::new(self.x_bin.as_float(), 0.0)
    }
}

/// Grouping of fractional offsets into discrete bins for cache optimization
/// Bins have size 1/N and are centered around (1/N) * i with i in {0, 1, 2, ...}
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SubpixelBin<const NUM_OF_BINS: usize>(usize);

impl<const NUM_OF_BINS: usize> SubpixelBin<NUM_OF_BINS> {
    pub const BIN_OFFSETS: [f32; NUM_OF_BINS] = {
        let mut out = [0.0; NUM_OF_BINS];
        let mut i = 0;
        while i < NUM_OF_BINS {
            out[i] = i as f32 * 1.0 / NUM_OF_BINS as f32;
            i += 1;
        }
        out
    };

    pub fn new(position: f32) -> (i32, Self) {
        let half_bin_width = 1.0 / (NUM_OF_BINS as f32 * 2.0);
        let new_position = (position + half_bin_width).floor() as i32;

        let scaled_pos = ((position - position.floor()) * NUM_OF_BINS as f32).round() as usize;
        let bin = scaled_pos.rem_euclid(NUM_OF_BINS);

        (new_position, Self(bin))
    }

    pub const fn as_float(&self) -> f32 {
        self.0 as f32 / NUM_OF_BINS as f32
    }
}

#[test]
fn test_subpixel_bins() {
    assert_eq!(SubpixelBin::<4>::new(3.14), (3, SubpixelBin(1)));
    assert_eq!(SubpixelBin::<4>::new(0.11), (0, SubpixelBin(0)));
    assert_eq!(SubpixelBin::<4>::new(-0.8), (-1, SubpixelBin(1)));
    assert_eq!(SubpixelBin::<2>::new(0.24), (0, SubpixelBin(0)));
    assert_eq!(SubpixelBin::<2>::new(0.26), (0, SubpixelBin(1)));
    assert_eq!(SubpixelBin::<2>::new(0.76), (1, SubpixelBin(0)));
    assert_eq!(SubpixelBin::<2>::new(-0.76), (-1, SubpixelBin(0)));
    assert_eq!(SubpixelBin::<2>::new(-0.25), (0, SubpixelBin(0)));
}
