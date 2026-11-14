use crate::error::{Error, Result};

/// RGSS `Table`: a dense `i16` array of one, two, or three dimensions.
///
/// The on-disk layout matches mkxp's marshal payload: five little-endian
/// `i32`s (dimension count, x, y, z, cell count) followed by little-endian cells.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Table {
    x: i32,
    y: i32,
    z: i32,
    data: Vec<i16>,
}

impl Table {
    pub fn new(x: i32, y: i32, z: i32) -> Result<Self> {
        if x < 0 || y < 0 || z < 0 {
            return Err(Error::new("Table dimensions must be non-negative"));
        }
        let cells = (x as usize)
            .checked_mul(y as usize)
            .and_then(|n| n.checked_mul(z as usize))
            .ok_or_else(|| Error::new("Table dimensions overflow"))?;
        Ok(Self {
            x,
            y,
            z,
            data: vec![0; cells],
        })
    }

    pub fn x_size(&self) -> i32 {
        self.x
    }
    pub fn y_size(&self) -> i32 {
        self.y
    }
    pub fn z_size(&self) -> i32 {
        self.z
    }

    pub fn dimensions(&self) -> i32 {
        if self.z > 1 {
            3
        } else if self.y > 1 {
            2
        } else {
            1
        }
    }

    fn index(&self, x: i32, y: i32, z: i32) -> Option<usize> {
        if x < 0 || y < 0 || z < 0 || x >= self.x || y >= self.y || z >= self.z {
            return None;
        }
        Some(
            (self.x as usize) * (self.y as usize) * (z as usize)
                + (self.x as usize) * (y as usize)
                + (x as usize),
        )
    }

    /// Ruby `table[x, y, z]`. Out of range is `nil`.
    pub fn get(&self, x: i32, y: i32, z: i32) -> Option<i16> {
        self.index(x, y, z).map(|i| self.data[i])
    }

    /// Ruby `table[x, y, z] = value`. Out of range is ignored.
    pub fn set(&mut self, value: i16, x: i32, y: i32, z: i32) {
        if let Some(i) = self.index(x, y, z) {
            self.data[i] = value;
        }
    }

    pub fn resize(&mut self, x: i32, y: i32, z: i32) -> Result<()> {
        if x < 0 || y < 0 || z < 0 {
            return Err(Error::new("Table dimensions must be non-negative"));
        }
        if x == self.x && y == self.y && z == self.z {
            return Ok(());
        }
        let mut next = Self::new(x, y, z)?;
        let x_copy = x.min(self.x);
        let y_copy = y.min(self.y);
        let z_copy = z.min(self.z);
        if x_copy > 0 && y_copy > 0 && z_copy > 0 {
            for kz in 0..z_copy {
                for jy in 0..y_copy {
                    for ix in 0..x_copy {
                        let value = self.get(ix, jy, kz).unwrap_or(0);
                        next.set(value, ix, jy, kz);
                    }
                }
            }
        }
        *self = next;
        Ok(())
    }

    pub fn serial_size(&self) -> usize {
        20 + self.data.len() * 2
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.serial_size());
        let size = self.data.len() as i32;
        for value in [self.dimensions(), self.x, self.y, self.z, size] {
            out.extend_from_slice(&value.to_le_bytes());
        }
        for cell in &self.data {
            out.extend_from_slice(&cell.to_le_bytes());
        }
        out
    }

    pub fn deserialize(data: &[u8]) -> Result<Self> {
        if data.len() < 20 {
            return Err(Error::new("Marshal: Table: bad file format"));
        }
        let mut nums = [0i32; 5];
        for (i, slot) in nums.iter_mut().enumerate() {
            let start = i * 4;
            *slot = i32::from_le_bytes(data[start..start + 4].try_into().unwrap());
        }
        let [_dim, x, y, z, size] = nums;
        if x < 0 || y < 0 || z < 0 || size != x.saturating_mul(y).saturating_mul(z) {
            return Err(Error::new("Marshal: Table: bad file format"));
        }
        if data.len() != 20 + (size as usize) * 2 {
            return Err(Error::new("Marshal: Table: bad file format"));
        }
        let mut table = Self::new(x, y, z)?;
        for (i, chunk) in data[20..].chunks_exact(2).enumerate() {
            table.data[i] = i16::from_le_bytes([chunk[0], chunk[1]]);
        }
        Ok(table)
    }
}

/// Positive modulo, matching `wrap()` in `tilemap-common.h`.
pub fn wrap_index(value: i32, range: i32) -> i32 {
    if range <= 0 {
        return 0;
    }
    let res = value % range;
    if res < 0 {
        res + range
    } else {
        res
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resize_keeps_the_overlapping_corner() {
        let mut table = Table::new(2, 2, 1).unwrap();
        table.set(7, 1, 1, 0);
        table.set(4, 0, 1, 0);
        table.resize(3, 1, 1).unwrap();
        assert_eq!(table.get(0, 0, 0), Some(0));
        assert_eq!(table.get(1, 1, 0), None);
        assert_eq!(table.dimensions(), 1);
    }

    #[test]
    fn marshal_roundtrip_uses_mkxp_header() {
        let mut table = Table::new(2, 3, 1).unwrap();
        table.set(-12, 1, 2, 0);
        let bytes = table.serialize();
        assert_eq!(&bytes[0..4], &2i32.to_le_bytes());
        assert_eq!(&bytes[4..8], &2i32.to_le_bytes());
        assert_eq!(&bytes[8..12], &3i32.to_le_bytes());
        assert_eq!(&bytes[12..16], &1i32.to_le_bytes());
        assert_eq!(&bytes[16..20], &6i32.to_le_bytes());
        let back = Table::deserialize(&bytes).unwrap();
        assert_eq!(back.get(1, 2, 0), Some(-12));
        assert!(Table::deserialize(&bytes[..10]).is_err());
    }

    #[test]
    fn set_ignores_out_of_range() {
        let mut table = Table::new(1, 1, 1).unwrap();
        table.set(9, -1, 0, 0);
        table.set(3, 0, 0, 0);
        assert_eq!(table.get(0, 0, 0), Some(3));
        assert_eq!(table.get(1, 0, 0), None);
    }
}
