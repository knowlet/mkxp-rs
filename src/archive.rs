use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

const HEADER: &[u8; 6] = b"RGSSAD";
const V1_MAGIC: u32 = 0xDEAD_CAFE;

/// One file stored in an RGSS archive.
#[derive(Clone, Debug)]
struct Entry {
    offset: u64,
    size: u64,
    start_magic: u32,
}

/// RGSSAD version 1 (`Game.rgssad`) or version 3 (`Game.rgss3a`).
///
/// Version 2 uses the version 1 layout with a different header byte, which is
/// how mkxp's `RGSS2_Archiver` is implemented.
#[derive(Clone, Debug)]
pub struct Archive {
    version: u8,
    bytes: Vec<u8>,
    entries: BTreeMap<String, Entry>,
    dirs: BTreeMap<String, Vec<String>>,
}

impl Archive {
    pub fn open(bytes: Vec<u8>) -> Result<Self> {
        if bytes.len() < 8 || &bytes[0..6] != HEADER || bytes[6] != 0 {
            return Err(Error::new("not an RGSS archive"));
        }
        match bytes[7] {
            1 | 2 => Self::open_v1(bytes),
            3 => Self::open_v3(bytes),
            other => Err(Error::new(format!(
                "unsupported RGSS archive version {other}"
            ))),
        }
    }

    pub fn version(&self) -> u8 {
        self.version
    }

    /// Path of `name` under `root`. Drive prefixes, NUL bytes, and other
    /// non-relative components are rejected so extraction cannot replace the
    /// destination (on Windows, `root.join("C:/outside")` is absolute).
    pub fn destination_path(root: &Path, name: &str) -> Result<PathBuf> {
        let normalized = normalize_name(name);
        if normalized.is_empty() {
            return Err(Error::new(format!(
                "archive entry escapes the destination: {name}"
            )));
        }
        let mut file = root.to_path_buf();
        for part in normalized.split('/') {
            if !is_relative_component(part) {
                return Err(Error::new(format!(
                    "archive entry escapes the destination: {name}"
                )));
            }
            file.push(part);
        }
        if !file.starts_with(root) {
            return Err(Error::new(format!(
                "archive entry escapes the destination: {name}"
            )));
        }
        Ok(file)
    }

    pub fn files(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }

    pub fn contains(&self, path: &str) -> bool {
        self.entries.contains_key(&normalize_name(path))
    }

    pub fn size_of(&self, path: &str) -> Option<u64> {
        self.entries.get(&normalize_name(path)).map(|e| e.size)
    }

    /// Immediate children of a directory. `""` is the archive root.
    pub fn list_dir(&self, dir: &str) -> Vec<String> {
        self.dirs
            .get(&normalize_name(dir))
            .cloned()
            .unwrap_or_default()
    }

    pub fn read(&self, path: &str) -> Result<Vec<u8>> {
        let name = normalize_name(path);
        let entry = self
            .entries
            .get(&name)
            .ok_or_else(|| Error::new(format!("archive entry not found: {name}")))?;
        self.read_entry(entry, 0, entry.size)
    }

    pub fn read_at(&self, path: &str, offset: u64, len: u64) -> Result<Vec<u8>> {
        let name = normalize_name(path);
        let entry = self
            .entries
            .get(&name)
            .ok_or_else(|| Error::new(format!("archive entry not found: {name}")))?;
        if offset > entry.size {
            return Err(Error::new("archive read starts past the end of the entry"));
        }
        self.read_entry(entry, offset, len.min(entry.size - offset))
    }

    fn read_entry(&self, entry: &Entry, offset: u64, len: u64) -> Result<Vec<u8>> {
        let start = entry.offset as usize;
        let end = start
            .checked_add(entry.size as usize)
            .ok_or_else(|| Error::new("archive entry is out of range"))?;
        if end > self.bytes.len() {
            return Err(Error::new("archive entry extends past the end of the file"));
        }
        let file = &self.bytes[start..end];
        let slice_end = (offset as usize)
            .checked_add(len as usize)
            .ok_or_else(|| Error::new("archive read is out of range"))?;
        if slice_end > file.len() {
            return Err(Error::new("archive read is out of range"));
        }
        Ok(decrypt(
            &file[offset as usize..slice_end],
            entry.start_magic,
            offset,
        ))
    }

    fn open_v1(bytes: Vec<u8>) -> Result<Self> {
        let version = bytes[7];
        let mut cursor = 8usize;
        let mut magic = V1_MAGIC;
        let mut entries = BTreeMap::new();
        let mut dirs: BTreeMap<String, Vec<String>> = BTreeMap::new();
        dirs.insert(String::new(), Vec::new());

        while cursor + 4 <= bytes.len() {
            let name_len = read_u32(&bytes[cursor..]) ^ advance(&mut magic);
            cursor += 4;
            if name_len > 1024 {
                return Err(Error::new("RGSS archive name is unreasonably long"));
            }
            if cursor + name_len as usize + 4 > bytes.len() {
                return Err(Error::new("truncated RGSS archive directory"));
            }
            let mut name = Vec::with_capacity(name_len as usize);
            for _ in 0..name_len {
                let byte = bytes[cursor] ^ (advance(&mut magic) as u8);
                name.push(byte);
                cursor += 1;
            }
            let size = read_u32(&bytes[cursor..]) ^ advance(&mut magic);
            cursor += 4;
            let name = String::from_utf8_lossy(&name);
            let name = normalize_name(&name);
            let entry = Entry {
                offset: cursor as u64,
                size: size as u64,
                start_magic: magic,
            };
            insert_entry(&mut entries, &mut dirs, name, entry);
            cursor = cursor
                .checked_add(size as usize)
                .ok_or_else(|| Error::new("RGSS archive entry size overflows"))?;
            if cursor > bytes.len() {
                return Err(Error::new("truncated RGSS archive payload"));
            }
        }

        Ok(Self {
            version,
            bytes,
            entries,
            dirs,
        })
    }

    fn open_v3(bytes: Vec<u8>) -> Result<Self> {
        if bytes.len() < 12 {
            return Err(Error::new("truncated RGSS3 archive"));
        }
        let mut cursor = 8usize;
        let key = read_u32(&bytes[cursor..]);
        cursor += 4;
        let base_magic = key.wrapping_mul(9).wrapping_add(3);
        let mut entries = BTreeMap::new();
        let mut dirs: BTreeMap<String, Vec<String>> = BTreeMap::new();
        dirs.insert(String::new(), Vec::new());

        loop {
            if cursor + 4 > bytes.len() {
                return Err(Error::new("truncated RGSS3 archive directory"));
            }
            let offset = read_u32(&bytes[cursor..]) ^ base_magic;
            cursor += 4;
            if offset == 0 {
                break;
            }
            if cursor + 12 > bytes.len() {
                return Err(Error::new("truncated RGSS3 archive directory"));
            }
            let size = read_u32(&bytes[cursor..]) ^ base_magic;
            cursor += 4;
            let magic = read_u32(&bytes[cursor..]) ^ base_magic;
            cursor += 4;
            let name_len = read_u32(&bytes[cursor..]) ^ base_magic;
            cursor += 4;
            if name_len > 1024 || cursor + name_len as usize > bytes.len() {
                return Err(Error::new("truncated RGSS3 archive name"));
            }
            let mut name = bytes[cursor..cursor + name_len as usize].to_vec();
            cursor += name_len as usize;
            for (i, byte) in name.iter_mut().enumerate() {
                *byte ^= ((base_magic >> (8 * (i % 4))) & 0xFF) as u8;
            }
            let name = normalize_name(&String::from_utf8_lossy(&name));
            insert_entry(
                &mut entries,
                &mut dirs,
                name,
                Entry {
                    offset: offset as u64,
                    size: size as u64,
                    start_magic: magic,
                },
            );
        }

        Ok(Self {
            version: 3,
            bytes,
            entries,
            dirs,
        })
    }

    /// Build a version 1 or 3 archive. `version` must be 1, 2, or 3.
    pub fn pack(version: u8, files: &[(&str, &[u8])]) -> Result<Vec<u8>> {
        match version {
            1 | 2 => Ok(pack_v1(version, files)),
            3 => Ok(pack_v3(files)),
            other => Err(Error::new(format!("cannot pack RGSS version {other}"))),
        }
    }
}

fn insert_entry(
    entries: &mut BTreeMap<String, Entry>,
    dirs: &mut BTreeMap<String, Vec<String>>,
    name: String,
    entry: Entry,
) {
    if name.is_empty() {
        return;
    }
    if let Some((dir, file)) = name.rsplit_once('/') {
        push_unique(dirs.entry(dir.to_string()).or_default(), file.to_string());
        let mut parent = dir;
        while let Some((upper, leaf)) = parent.rsplit_once('/') {
            push_unique(dirs.entry(upper.to_string()).or_default(), leaf.to_string());
            parent = upper;
        }
        if !parent.is_empty() {
            push_unique(dirs.entry(String::new()).or_default(), parent.to_string());
        }
    } else {
        push_unique(dirs.entry(String::new()).or_default(), name.clone());
    }
    entries.insert(name, entry);
}

fn push_unique(list: &mut Vec<String>, value: String) {
    if !list.iter().any(|item| item == &value) {
        list.push(value);
    }
}

fn is_relative_component(part: &str) -> bool {
    !part.is_empty()
        && part != "."
        && part != ".."
        && !part.contains(':')
        && !part.contains('\0')
        && !Path::new(part).is_absolute()
}

fn normalize_name(path: &str) -> String {
    let mut out = String::new();
    for part in path.replace('\\', "/").split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            if let Some(index) = out.rfind('/') {
                out.truncate(index);
            } else {
                out.clear();
            }
            continue;
        }
        if !out.is_empty() {
            out.push('/');
        }
        out.push_str(part);
    }
    out
}

fn read_u32(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

fn advance(magic: &mut u32) -> u32 {
    let old = *magic;
    *magic = magic.wrapping_mul(7).wrapping_add(3);
    old
}

/// Jump `steps` dwords ahead in the RGSS LCG. `steps` fits in `u32` because an
/// RGSS payload cannot contain more than 2^30 dwords.
fn advance_n(magic: &mut u32, mut steps: u32) {
    const LCG: [[u32; 2]; 30] = [
        [0x0000_0007, 0x0000_0003],
        [0x0000_0031, 0x0000_0018],
        [0x0000_0961, 0x0000_04b0],
        [0x0057_f6c1, 0x002b_fb60],
        [0xa505_7d81, 0xd282_bec0],
        [0x6e91_3b01, 0x3748_9d80],
        [0xc0bb_7601, 0x605d_bb00],
        [0x1bda_ec01, 0x8ded_7600],
        [0x0145_d801, 0x80a2_ec00],
        [0x28cb_b001, 0x1465_d800],
        [0xea97_6001, 0x754b_b000],
        [0x392e_c001, 0x1c97_6000],
        [0x025d_8001, 0x012e_c000],
        [0x44bb_0001, 0x225d_8000],
        [0x8976_0001, 0xc4bb_0000],
        [0x12ec_0001, 0x8976_0000],
        [0x25d8_0001, 0x12ec_0000],
        [0x4bb0_0001, 0x25d8_0000],
        [0x9760_0001, 0x4bb0_0000],
        [0x2ec0_0001, 0x9760_0000],
        [0x5d80_0001, 0x2ec0_0000],
        [0xbb00_0001, 0x5d80_0000],
        [0x7600_0001, 0xbb00_0000],
        [0xec00_0001, 0x7600_0000],
        [0xd800_0001, 0xec00_0000],
        [0xb000_0001, 0xd800_0000],
        [0x6000_0001, 0xb000_0000],
        [0xc000_0001, 0x6000_0000],
        [0x8000_0001, 0xc000_0000],
        [0x0000_0001, 0x8000_0000],
    ];
    let mut index = 0;
    while steps != 0 {
        if steps & 1 == 1 {
            *magic = magic
                .wrapping_mul(LCG[index][0])
                .wrapping_add(LCG[index][1]);
        }
        steps >>= 1;
        index += 1;
    }
}

fn decrypt(data: &[u8], start_magic: u32, offset: u64) -> Vec<u8> {
    let mut magic = start_magic;
    advance_n(&mut magic, (offset / 4) as u32);
    let mut out = data.to_vec();
    let mut pos = offset;
    for byte in &mut out {
        let shift = (pos % 4) * 8;
        *byte ^= ((magic >> shift) & 0xFF) as u8;
        pos += 1;
        if pos % 4 == 0 {
            advance(&mut magic);
        }
    }
    out
}

fn encrypt_body(data: &[u8], magic: u32) -> Vec<u8> {
    decrypt(data, magic, 0)
}

fn pack_v1(version: u8, files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(HEADER);
    out.push(0);
    out.push(version);
    let mut magic = V1_MAGIC;
    for (name, data) in files {
        let name = normalize_name(name);
        let name_bytes = name.as_bytes();
        let enc_len = (name_bytes.len() as u32) ^ advance(&mut magic);
        out.extend_from_slice(&enc_len.to_le_bytes());
        for &byte in name_bytes {
            let key = advance(&mut magic) as u8;
            out.push(byte ^ key);
        }
        let enc_size = (data.len() as u32) ^ advance(&mut magic);
        out.extend_from_slice(&enc_size.to_le_bytes());
        out.extend_from_slice(&encrypt_body(data, magic));
    }
    out
}

fn pack_v3(files: &[(&str, &[u8])]) -> Vec<u8> {
    let base_key: u32 = 0x00C0_FFEE;
    let base_magic = base_key.wrapping_mul(9).wrapping_add(3);
    let mut dir = Vec::new();
    dir.extend_from_slice(HEADER);
    dir.push(0);
    dir.push(3);
    dir.extend_from_slice(&base_key.to_le_bytes());

    let mut bodies = Vec::new();
    let mut cursor = 0usize;
    // Directory size is known once names are known: 12 byte header + records + 4 byte terminator.
    let dir_len = 12
        + files
            .iter()
            .map(|(name, _)| 16 + normalize_name(name).len())
            .sum::<usize>()
        + 4;
    for (index, (name, data)) in files.iter().enumerate() {
        let name = normalize_name(name);
        let offset = (dir_len + cursor) as u32;
        let file_magic = 0x1000_0000u32.wrapping_add(index as u32);
        dir.extend_from_slice(&(offset ^ base_magic).to_le_bytes());
        dir.extend_from_slice(&((data.len() as u32) ^ base_magic).to_le_bytes());
        dir.extend_from_slice(&(file_magic ^ base_magic).to_le_bytes());
        dir.extend_from_slice(&((name.len() as u32) ^ base_magic).to_le_bytes());
        for (i, byte) in name.bytes().enumerate() {
            dir.push(byte ^ ((base_magic >> (8 * (i % 4))) & 0xFF) as u8);
        }
        bodies.extend_from_slice(&encrypt_body(data, file_magic));
        cursor += data.len();
    }
    // A zero offset, xored with the key, is the directory terminator.
    dir.extend_from_slice(&base_magic.to_le_bytes());
    debug_assert_eq!(dir.len(), dir_len);
    dir.extend_from_slice(&bodies);
    dir
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v1_and_v3_roundtrip_including_a_mid_file_seek() {
        let files: &[(&str, &[u8])] = &[
            ("Hello.txt", b"Hello, RGSS"),
            ("Graphics\\Pictures\\Face.png", b"\x89PNG-not-really"),
            ("Data/Map001.rxdata", &[1, 2, 3, 4, 5, 6, 7]),
        ];
        for version in [1u8, 2, 3] {
            let packed = Archive::pack(version, files).unwrap();
            let archive = Archive::open(packed).unwrap();
            assert_eq!(archive.version(), version);
            assert_eq!(archive.read("Hello.txt").unwrap(), b"Hello, RGSS");
            assert_eq!(archive.read_at("Hello.txt", 7, 4).unwrap(), b"RGSS");
            assert_eq!(
                archive.read("Graphics/Pictures/Face.png").unwrap(),
                b"\x89PNG-not-really"
            );
            let root_dir = Path::new("out");
            let face = Archive::destination_path(root_dir, r"Graphics\Pictures\Face.png").unwrap();
            assert_eq!(
                face,
                root_dir.join("Graphics").join("Pictures").join("Face.png")
            );
            assert!(Archive::destination_path(root_dir, "C:/outside/file").is_err());
            assert!(Archive::destination_path(root_dir, r"C:\outside\file").is_err());
            assert!(Archive::destination_path(root_dir, "..").is_err());
            assert!(Archive::destination_path(root_dir, "a\0b").is_err());
            let nested = Archive::destination_path(root_dir, "/etc/passwd").unwrap();
            assert!(nested.starts_with(root_dir));
            let mut root = archive.list_dir("");
            root.sort();
            assert_eq!(root, ["Data", "Graphics", "Hello.txt"]);
            assert_eq!(archive.list_dir("Graphics"), ["Pictures"]);
            assert_eq!(archive.list_dir("Graphics/Pictures"), ["Face.png"]);
        }
    }

    #[test]
    fn advance_n_matches_stepping() {
        let mut stepped = 0x1234_5678;
        let mut jumped = stepped;
        for _ in 0..1000 {
            advance(&mut stepped);
        }
        advance_n(&mut jumped, 1000);
        assert_eq!(stepped, jumped);
    }
}
