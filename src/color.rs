use crate::error::{Error, Result};

/// RGSS `Color`. Components are stored unclamped, matching mkxp. The normalized
/// form used by shaders clamps each channel into `0.0..=1.0` (or the tone range).
#[derive(Clone, Copy, Debug)]
pub struct Color {
    pub red: f64,
    pub green: f64,
    pub blue: f64,
    pub alpha: f64,
}

impl Default for Color {
    fn default() -> Self {
        Self {
            red: 0.0,
            green: 0.0,
            blue: 0.0,
            alpha: 0.0,
        }
    }
}

impl PartialEq for Color {
    fn eq(&self, other: &Self) -> bool {
        self.red == other.red
            && self.green == other.green
            && self.blue == other.blue
            && self.alpha == other.alpha
    }
}

impl Color {
    pub fn new(red: f64, green: f64, blue: f64, alpha: f64) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }

    pub fn rgb(red: f64, green: f64, blue: f64) -> Self {
        Self::new(red, green, blue, 255.0)
    }

    pub fn has_effect(self) -> bool {
        self.alpha != 0.0
    }

    pub fn set(&mut self, red: f64, green: f64, blue: f64, alpha: f64) {
        self.red = red;
        self.green = green;
        self.blue = blue;
        self.alpha = alpha;
    }

    /// Truncating conversion used when a color is written into an 8-bit pixel.
    pub fn channel_u8(value: f64) -> u8 {
        value.clamp(0.0, 255.0) as u8
    }

    pub fn to_rgba8(self) -> [u8; 4] {
        [
            Self::channel_u8(self.red),
            Self::channel_u8(self.green),
            Self::channel_u8(self.blue),
            Self::channel_u8(self.alpha),
        ]
    }

    pub fn from_rgba8(px: [u8; 4]) -> Self {
        Self::new(px[0] as f64, px[1] as f64, px[2] as f64, px[3] as f64)
    }

    pub fn norm(self) -> [f32; 4] {
        [
            (self.red / 255.0) as f32,
            (self.green / 255.0) as f32,
            (self.blue / 255.0) as f32,
            (self.alpha / 255.0) as f32,
        ]
    }

    pub fn serial_size() -> usize {
        32
    }

    pub fn serialize(self) -> [u8; 32] {
        let mut out = [0u8; 32];
        out[0..8].copy_from_slice(&self.red.to_le_bytes());
        out[8..16].copy_from_slice(&self.green.to_le_bytes());
        out[16..24].copy_from_slice(&self.blue.to_le_bytes());
        out[24..32].copy_from_slice(&self.alpha.to_le_bytes());
        out
    }

    pub fn deserialize(data: &[u8]) -> Result<Self> {
        if data.len() != 32 {
            return Err(Error::new("Color: Serialized data invalid"));
        }
        let red = f64::from_le_bytes(data[0..8].try_into().unwrap());
        let green = f64::from_le_bytes(data[8..16].try_into().unwrap());
        let blue = f64::from_le_bytes(data[16..24].try_into().unwrap());
        let alpha = f64::from_le_bytes(data[24..32].try_into().unwrap());
        Ok(Self::new(red, green, blue, alpha))
    }
}

/// RGSS `Tone`. Gray is `0..=255`; the color channels are `-255..=255`.
#[derive(Clone, Copy, Debug)]
pub struct Tone {
    pub red: f64,
    pub green: f64,
    pub blue: f64,
    pub gray: f64,
}

impl Default for Tone {
    fn default() -> Self {
        Self {
            red: 0.0,
            green: 0.0,
            blue: 0.0,
            gray: 0.0,
        }
    }
}

impl PartialEq for Tone {
    fn eq(&self, other: &Self) -> bool {
        self.red == other.red
            && self.green == other.green
            && self.blue == other.blue
            && self.gray == other.gray
    }
}

impl Tone {
    pub fn new(red: f64, green: f64, blue: f64, gray: f64) -> Self {
        Self {
            red,
            green,
            blue,
            gray,
        }
    }

    pub fn has_effect(self) -> bool {
        self.red as i32 != 0
            || self.green as i32 != 0
            || self.blue as i32 != 0
            || self.gray as i32 != 0
    }

    pub fn set(&mut self, red: f64, green: f64, blue: f64, gray: f64) {
        self.red = red;
        self.green = green;
        self.blue = blue;
        self.gray = gray;
    }

    /// Shader-space tone: rgb in `-1..=1`, gray in `0..=1`.
    pub fn norm(self) -> [f32; 4] {
        [
            (self.red.clamp(-255.0, 255.0) / 255.0) as f32,
            (self.green.clamp(-255.0, 255.0) / 255.0) as f32,
            (self.blue.clamp(-255.0, 255.0) / 255.0) as f32,
            (self.gray.clamp(0.0, 255.0) / 255.0) as f32,
        ]
    }

    pub fn serialize(self) -> [u8; 32] {
        let mut out = [0u8; 32];
        out[0..8].copy_from_slice(&self.red.to_le_bytes());
        out[8..16].copy_from_slice(&self.green.to_le_bytes());
        out[16..24].copy_from_slice(&self.blue.to_le_bytes());
        out[24..32].copy_from_slice(&self.gray.to_le_bytes());
        out
    }

    pub fn deserialize(data: &[u8]) -> Result<Self> {
        if data.len() != 32 {
            return Err(Error::new("Tone: Serialized data invalid"));
        }
        Ok(Self::new(
            f64::from_le_bytes(data[0..8].try_into().unwrap()),
            f64::from_le_bytes(data[8..16].try_into().unwrap()),
            f64::from_le_bytes(data[16..24].try_into().unwrap()),
            f64::from_le_bytes(data[24..32].try_into().unwrap()),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_roundtrip_keeps_unclamped_channels() {
        let color = Color::new(300.0, -2.0, 10.5, 128.0);
        let back = Color::deserialize(&color.serialize()).unwrap();
        assert_eq!(color, back);
        assert_eq!(Color::channel_u8(300.0), 255);
        assert_eq!(Color::channel_u8(10.9), 10);
    }

    #[test]
    fn tone_norm_clamps() {
        let tone = Tone::new(510.0, -510.0, 0.0, 510.0);
        let n = tone.norm();
        assert_eq!(n, [1.0, -1.0, 0.0, 1.0]);
        assert!(tone.has_effect());
        assert!(!Tone::default().has_effect());
    }
}
