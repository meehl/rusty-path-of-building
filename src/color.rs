/// `sRGB` color with _unmultiplied alpha_.
#[repr(C)]
#[repr(align(4))]
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Srgba(pub [u8; 4]);

impl Srgba {
    pub const WHITE: Self = Self::from_rgb(255, 255, 255);
    pub const TRANSPARENT: Self = Self::new(0, 0, 0, 0);

    #[inline]
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self([r, g, b, a])
    }

    #[inline]
    pub const fn new_f32(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self([f32_to_u8(r), f32_to_u8(g), f32_to_u8(b), f32_to_u8(a)])
    }

    #[inline]
    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self([r, g, b, 255])
    }

    #[inline]
    pub fn from_hex<T: AsRef<str>>(hex: T) -> anyhow::Result<Self> {
        let hex = hex.as_ref();
        let hex = hex.strip_prefix("#").unwrap_or(hex);

        match hex.len() {
            // RGB shorthand form, e.g. #FC0 -> #FFCC00
            3 => {
                let [m, l] = u16::from_str_radix(hex, 16)?.to_be_bytes();
                let (r, g, b) = (m & 0x0F, (l & 0xF0) >> 4, l & 0x0F);
                Ok(Self::from_rgb((r << 4) | r, (g << 4) | g, (b << 4) | b))
            }
            // RGBA shorthand form, e.g. #FC0F -> #FFCC00FF
            4 => {
                let [m, l] = u16::from_str_radix(hex, 16)?.to_be_bytes();
                let (r, g, b, a) = ((m & 0xF0) >> 4, m & 0xF, (l & 0xF0) >> 4, l & 0x0F);
                Ok(Self::new(
                    (r << 4) | r,
                    (g << 4) | g,
                    (b << 4) | b,
                    (a << 4) | a,
                ))
            }
            // RRGGBB
            6 => {
                let [_, r, g, b] = u32::from_str_radix(hex, 16)?.to_be_bytes();
                Ok(Self::from_rgb(r, g, b))
            }
            // RRGGBBAA
            8 => {
                let [r, g, b, a] = u32::from_str_radix(hex, 16)?.to_be_bytes();
                Ok(Self::new(r, g, b, a))
            }
            _ => Err(anyhow::anyhow!("Unexpected length of hex string")),
        }
    }
}

impl From<Srgba> for image::Rgba<u8> {
    fn from(value: Srgba) -> Self {
        Self(value.0)
    }
}

impl From<Srgba> for [f32; 4] {
    fn from(value: Srgba) -> Self {
        value.0.map(u8_to_f32)
    }
}

/// f32 [0.0, 1.0] -> u8 [0, 255]
#[inline(always)]
const fn f32_to_u8(c: f32) -> u8 {
    ((c * 255.0) + 0.5) as u8
}

/// u8 [0, 255] -> f32 [0.0, 1.0]
#[inline(always)]
const fn u8_to_f32(c: u8) -> f32 {
    c as f32 / 255.0
}
