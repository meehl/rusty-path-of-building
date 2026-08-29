use crate::color::Srgba;

#[inline]
fn hex_byte(a: u8, b: u8) -> u8 {
    (hex_digit(a) << 4) | hex_digit(b)
}

#[inline]
fn hex_digit(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        b'A'..=b'F' => c - b'A' + 10,
        _ => unreachable!("invalid hex digit"),
    }
}

impl Srgba {
    pub fn from_escape_code(escape_str: &str) -> Self {
        let bytes = escape_str.as_bytes();

        match bytes {
            [b'^', b'0'] => Self::from_rgb(0, 0, 0),
            [b'^', b'1'] => Self::from_rgb(255, 0, 0),
            [b'^', b'2'] => Self::from_rgb(0, 255, 0),
            [b'^', b'3'] => Self::from_rgb(0, 0, 255),
            [b'^', b'4'] => Self::from_rgb(255, 255, 0),
            [b'^', b'5'] => Self::from_rgb(255, 0, 255),
            [b'^', b'6'] => Self::from_rgb(0, 255, 255),
            [b'^', b'7'] => Self::from_rgb(255, 255, 255),
            [b'^', b'8'] => Self::from_rgb(178, 178, 178),
            [b'^', b'9'] => Self::from_rgb(102, 102, 102),

            [b'^', b'x' | b'X', r1, r2, g1, g2, b1, b2] => {
                Self::from_rgb(hex_byte(*r1, *r2), hex_byte(*g1, *g2), hex_byte(*b1, *b2))
            }

            _ => Self::WHITE,
        }
    }
}
