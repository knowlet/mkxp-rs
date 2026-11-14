use crate::error::{Error, Result};

/// RGSS `Rect`. Empty when either side is zero, matching `Rect::isEmpty`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn empty() -> Self {
        Self::default()
    }

    pub fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }

    pub fn set(&mut self, x: i32, y: i32, width: i32, height: i32) {
        self.x = x;
        self.y = y;
        self.width = width;
        self.height = height;
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn right(self) -> i32 {
        self.x.saturating_add(self.width)
    }

    pub fn bottom(self) -> i32 {
        self.y.saturating_add(self.height)
    }

    pub fn contains(self, x: i32, y: i32) -> bool {
        x >= self.x && y >= self.y && x < self.right() && y < self.bottom()
    }

    /// Flip a negative width or height so the origin is the top-left corner.
    pub fn normalized(self) -> Self {
        let mut rect = self;
        if rect.width < 0 {
            rect.x = rect.x.saturating_add(rect.width);
            rect.width = rect.width.saturating_neg();
        }
        if rect.height < 0 {
            rect.y = rect.y.saturating_add(rect.height);
            rect.height = rect.height.saturating_neg();
        }
        rect
    }

    pub fn intersect(self, other: Self) -> Self {
        let self_rect = self.normalized();
        let other = other.normalized();
        let x0 = self_rect.x.max(other.x);
        let y0 = self_rect.y.max(other.y);
        let x1 = self_rect.right().min(other.right());
        let y1 = self_rect.bottom().min(other.bottom());
        if x1 <= x0 || y1 <= y0 {
            Self::default()
        } else {
            Self::new(x0, y0, x1 - x0, y1 - y0)
        }
    }

    pub fn serialize(self) -> [u8; 16] {
        let mut out = [0u8; 16];
        out[0..4].copy_from_slice(&self.x.to_le_bytes());
        out[4..8].copy_from_slice(&self.y.to_le_bytes());
        out[8..12].copy_from_slice(&self.width.to_le_bytes());
        out[12..16].copy_from_slice(&self.height.to_le_bytes());
        out
    }

    pub fn deserialize(data: &[u8]) -> Result<Self> {
        if data.len() != 16 {
            return Err(Error::new("Rect: Serialized data invalid"));
        }
        Ok(Self::new(
            i32::from_le_bytes(data[0..4].try_into().unwrap()),
            i32::from_le_bytes(data[4..8].try_into().unwrap()),
            i32::from_le_bytes(data[8..12].try_into().unwrap()),
            i32::from_le_bytes(data[12..16].try_into().unwrap()),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_and_intersection() {
        assert!(Rect::new(0, 0, 0, 5).is_empty());
        let a = Rect::new(0, 0, 10, 10);
        let b = Rect::new(8, 8, 10, 10);
        assert_eq!(a.intersect(b), Rect::new(8, 8, 2, 2));
        assert!(a.intersect(Rect::new(20, 20, 1, 1)).is_empty());
    }
}
