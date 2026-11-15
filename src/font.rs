use crate::color::Color;

/// Bitmap font used by `Bitmap#draw_text`. mkxp draws with FreeType; this
/// runtime uses a 3x5 glyph set so text layout, alignment, outline, and shadow
/// can run without a system font. ASCII letters are case-insensitive.
/// `bold` and `italic` are stored for script compatibility and do not change
/// these glyphs.
#[derive(Clone, Debug)]
pub struct Font {
    pub name: String,
    pub size: i32,
    pub bold: bool,
    pub italic: bool,
    pub outline: bool,
    pub shadow: bool,
    pub color: Color,
    pub out_color: Color,
}

impl Font {
    pub fn for_version(rgss_version: i32) -> Self {
        let (name, size, outline, shadow) = match rgss_version {
            1 => ("Arial", 22, false, false),
            2 => ("UmePlus Gothic", 20, false, true),
            _ => ("VL Gothic", 24, true, false),
        };
        Self {
            name: name.to_string(),
            size,
            bold: false,
            italic: false,
            outline,
            shadow,
            color: Color::rgb(255.0, 255.0, 255.0),
            out_color: Color::new(0.0, 0.0, 0.0, 128.0),
        }
    }

    pub fn scale(&self) -> i32 {
        (self.size.max(1) as f32 / 5.0).round().max(1.0) as i32
    }
}

impl Default for Font {
    fn default() -> Self {
        Self::for_version(1)
    }
}

/// Five rows of three bits, top row first.
pub fn glyph_rows(ch: char) -> [u8; 5] {
    match ch.to_ascii_uppercase() {
        'A' => [0b010, 0b101, 0b111, 0b101, 0b101],
        'B' => [0b110, 0b101, 0b110, 0b101, 0b110],
        'C' => [0b011, 0b100, 0b100, 0b100, 0b011],
        'D' => [0b110, 0b101, 0b101, 0b101, 0b110],
        'E' => [0b111, 0b100, 0b110, 0b100, 0b111],
        'F' => [0b111, 0b100, 0b110, 0b100, 0b100],
        'G' => [0b011, 0b100, 0b101, 0b101, 0b011],
        'H' => [0b101, 0b101, 0b111, 0b101, 0b101],
        'I' => [0b111, 0b010, 0b010, 0b010, 0b111],
        'J' => [0b001, 0b001, 0b001, 0b101, 0b010],
        'K' => [0b101, 0b101, 0b110, 0b101, 0b101],
        'L' => [0b100, 0b100, 0b100, 0b100, 0b111],
        'M' => [0b101, 0b111, 0b111, 0b101, 0b101],
        'N' => [0b110, 0b101, 0b101, 0b101, 0b101],
        'O' => [0b010, 0b101, 0b101, 0b101, 0b010],
        'P' => [0b110, 0b101, 0b110, 0b100, 0b100],
        'Q' => [0b010, 0b101, 0b101, 0b011, 0b001],
        'R' => [0b110, 0b101, 0b110, 0b101, 0b101],
        'S' => [0b011, 0b100, 0b010, 0b001, 0b110],
        'T' => [0b111, 0b010, 0b010, 0b010, 0b010],
        'U' => [0b101, 0b101, 0b101, 0b101, 0b010],
        'V' => [0b101, 0b101, 0b101, 0b010, 0b010],
        'W' => [0b101, 0b101, 0b111, 0b111, 0b101],
        'X' => [0b101, 0b101, 0b010, 0b101, 0b101],
        'Y' => [0b101, 0b101, 0b010, 0b010, 0b010],
        'Z' => [0b111, 0b001, 0b010, 0b100, 0b111],
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b111, 0b001, 0b111, 0b100, 0b111],
        '3' => [0b111, 0b001, 0b111, 0b001, 0b111],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b111, 0b001, 0b111],
        '6' => [0b111, 0b100, 0b111, 0b101, 0b111],
        '7' => [0b111, 0b001, 0b010, 0b010, 0b010],
        '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
        '9' => [0b111, 0b101, 0b111, 0b001, 0b111],
        ' ' => [0, 0, 0, 0, 0],
        '.' => [0, 0, 0, 0b010, 0],
        ',' => [0, 0, 0, 0b010, 0b100],
        '!' => [0b010, 0b010, 0b010, 0, 0b010],
        '?' => [0b111, 0b001, 0b010, 0, 0b010],
        '-' => [0, 0, 0b111, 0, 0],
        '\'' => [0b010, 0b010, 0, 0, 0],
        ':' => [0, 0b010, 0, 0b010, 0],
        _ if ch.is_whitespace() => [0, 0, 0, 0, 0],
        _ => [0b111, 0b101, 0b101, 0b101, 0b111],
    }
}

pub const GLYPH_WIDTH: i32 = 3;
pub const GLYPH_HEIGHT: i32 = 5;
