use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::{Error, Result};

const HEADER: &[u8; 6] = b"RGSSAD";
const V1_MAGIC: u32 = 0xDEAD_CAFE;
/// Shared by the reader and the packer. Longer names cannot be opened.
const MAX_ARCHIVE_NAME_LEN: usize = 1024;

/// One file stored in an RGSS archive.
#[derive(Clone, Debug)]
struct Entry {
    offset: u64,
    size: u64,
    start_magic: u32,
    /// Filename bytes before display decoding. Two different sequences that
    /// decode to one string must not overwrite each other.
    raw_name: Vec<u8>,
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
            if name_len > MAX_ARCHIVE_NAME_LEN as u32 {
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
            let raw_name = name;
            let name = normalize_name(&decode_archive_name(&raw_name));
            insert_entry(
                &mut entries,
                &mut dirs,
                &raw_name,
                name,
                Entry {
                    offset: cursor as u64,
                    size: size as u64,
                    start_magic: magic,
                    raw_name: Vec::new(),
                },
            )?;
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
            if name_len > MAX_ARCHIVE_NAME_LEN as u32 || cursor + name_len as usize > bytes.len() {
                return Err(Error::new("truncated RGSS3 archive name"));
            }
            let mut name = bytes[cursor..cursor + name_len as usize].to_vec();
            cursor += name_len as usize;
            for (i, byte) in name.iter_mut().enumerate() {
                *byte ^= ((base_magic >> (8 * (i % 4))) & 0xFF) as u8;
            }
            let raw_name = name;
            let name = normalize_name(&decode_archive_name(&raw_name));
            insert_entry(
                &mut entries,
                &mut dirs,
                &raw_name,
                name,
                Entry {
                    offset: offset as u64,
                    size: size as u64,
                    start_magic: magic,
                    raw_name: Vec::new(),
                },
            )?;
        }

        Ok(Self {
            version: 3,
            bytes,
            entries,
            dirs,
        })
    }

    /// Build a version 1 or 3 archive. `version` must be 1, 2, or 3.
    /// Offsets and sizes that do not fit the format's 32-bit fields are an error.
    pub fn pack(version: u8, files: &[(&str, &[u8])]) -> Result<Vec<u8>> {
        match version {
            1 | 2 => pack_v1(version, files),
            3 => pack_v3(files),
            other => Err(Error::new(format!("cannot pack RGSS version {other}"))),
        }
    }

    /// Write every entry under `root`. Existing symlinks in the destination
    /// are rejected so an entry cannot be redirected outside `root`.
    pub fn extract_to(&self, root: &Path) -> Result<()> {
        if let Ok(meta) = std::fs::symlink_metadata(root) {
            if meta.file_type().is_symlink() {
                return Err(Error::new(
                    "refusing to extract through a symlink destination",
                ));
            }
        }
        std::fs::create_dir_all(root).map_err(|err| Error::new(err.to_string()))?;
        for name in self.files() {
            write_extracted(root, name, &self.read(name)?)?;
        }
        Ok(())
    }
}

fn insert_entry(
    entries: &mut BTreeMap<String, Entry>,
    dirs: &mut BTreeMap<String, Vec<String>>,
    raw_name: &[u8],
    mut name: String,
    mut entry: Entry,
) -> Result<()> {
    if name.is_empty() {
        return Ok(());
    }
    // A Latin-1 byte and its UTF-8 encoding both display as the same string.
    // Keep the first display key, and store the other bytes under a reversible
    // identity. Never replace one payload with the other.
    if entries
        .get(&name)
        .is_some_and(|previous| previous.raw_name.as_slice() != raw_name)
    {
        name = disambiguated_archive_name(raw_name);
    }
    if entries
        .get(&name)
        .is_some_and(|previous| previous.raw_name.as_slice() != raw_name)
    {
        return Err(Error::new(format!(
            "RGSS archive entry name collides: {name}"
        )));
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
    entry.raw_name = raw_name.to_vec();
    entries.insert(name, entry);
    Ok(())
}

/// Reversible key for filename bytes whose display form is already taken.
/// `U+FFFE` cannot appear in a successful UTF-8 decode of a different sequence
/// that we then normalize into this same hex spelling without itself being
/// stored under that display key first; a remaining clash is an error.
fn disambiguated_archive_name(bytes: &[u8]) -> String {
    let mut name = String::from("\u{FFFE}");
    for byte in bytes {
        name.push(char::from_digit(u32::from(*byte >> 4), 16).unwrap_or('0'));
        name.push(char::from_digit(u32::from(*byte & 0x0f), 16).unwrap_or('0'));
    }
    name
}

fn push_unique(list: &mut Vec<String>, value: String) {
    if !list.iter().any(|item| item == &value) {
        list.push(value);
    }
}

fn decode_archive_name(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_string(),
        Err(_) => bytes.iter().map(|byte| char::from(*byte)).collect(),
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

/// Jump `steps` dwords ahead in the RGSS LCG.
fn advance_n(magic: &mut u32, mut steps: u32) {
    // The LCG repeats every 2^30 steps. Keep the jump-table index in range,
    // including for a caller that supplies a complete period or more (#381).
    steps &= 0x3fff_ffff;
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
        if pos.is_multiple_of(4) {
            advance(&mut magic);
        }
    }
    out
}

fn encrypt_body(data: &[u8], magic: u32) -> Vec<u8> {
    decrypt(data, magic, 0)
}

fn checked_archive_name(name: &str) -> Result<String> {
    let name = normalize_name(name);
    if name.len() > MAX_ARCHIVE_NAME_LEN {
        return Err(Error::new("RGSS archive name is unreasonably long"));
    }
    Ok(name)
}

fn pack_v1(version: u8, files: &[(&str, &[u8])]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    out.extend_from_slice(HEADER);
    out.push(0);
    out.push(version);
    let mut magic = V1_MAGIC;
    for (name, data) in files {
        let name = checked_archive_name(name)?;
        let name_bytes = name.as_bytes();
        let enc_len = as_u32(name_bytes.len(), "archive name")? ^ advance(&mut magic);
        out.extend_from_slice(&enc_len.to_le_bytes());
        for &byte in name_bytes {
            let key = advance(&mut magic) as u8;
            out.push(byte ^ key);
        }
        let enc_size = as_u32(data.len(), "archive file")? ^ advance(&mut magic);
        out.extend_from_slice(&enc_size.to_le_bytes());
        out.extend_from_slice(&encrypt_body(data, magic));
    }
    Ok(out)
}

fn pack_v3(files: &[(&str, &[u8])]) -> Result<Vec<u8>> {
    let base_key: u32 = 0x00C0_FFEE;
    let base_magic = base_key.wrapping_mul(9).wrapping_add(3);
    let mut dir = Vec::new();
    dir.extend_from_slice(HEADER);
    dir.push(0);
    dir.push(3);
    dir.extend_from_slice(&base_key.to_le_bytes());

    let mut bodies = Vec::new();
    let mut cursor = 0usize;
    // 8 byte header + 4 byte key + one 16-byte record per file + names + 4 byte terminator.
    let mut dir_len = 16usize;
    for (name, _) in files {
        let name_len = as_u32(checked_archive_name(name)?.len(), "archive name")? as usize;
        dir_len = dir_len
            .checked_add(16)
            .and_then(|sum| sum.checked_add(name_len))
            .ok_or_else(|| Error::new("archive directory is too large"))?;
    }
    for (index, (name, data)) in files.iter().enumerate() {
        let name = checked_archive_name(name)?;
        let offset = checked_sum(dir_len, cursor)?;
        let file_magic = 0x1000_0000u32.wrapping_add(as_u32(index, "archive file count")?);
        let size = as_u32(data.len(), "archive file")?;
        let name_len = as_u32(name.len(), "archive name")?;
        dir.extend_from_slice(&(offset ^ base_magic).to_le_bytes());
        dir.extend_from_slice(&(size ^ base_magic).to_le_bytes());
        dir.extend_from_slice(&(file_magic ^ base_magic).to_le_bytes());
        dir.extend_from_slice(&(name_len ^ base_magic).to_le_bytes());
        for (i, byte) in name.bytes().enumerate() {
            dir.push(byte ^ ((base_magic >> (8 * (i % 4))) & 0xFF) as u8);
        }
        bodies.extend_from_slice(&encrypt_body(data, file_magic));
        cursor = cursor
            .checked_add(data.len())
            .ok_or_else(|| Error::new("archive offset overflow"))?;
    }
    // A zero offset, xored with the key, is the directory terminator.
    dir.extend_from_slice(&base_magic.to_le_bytes());
    debug_assert_eq!(dir.len(), dir_len);
    dir.extend_from_slice(&bodies);
    Ok(dir)
}

fn as_u32(value: usize, what: &str) -> Result<u32> {
    u32::try_from(value).map_err(|_| Error::new(format!("{what} does not fit in an RGSS archive")))
}

fn checked_sum(left: usize, right: usize) -> Result<u32> {
    as_u32(
        left.checked_add(right)
            .ok_or_else(|| Error::new("archive offset overflow"))?,
        "archive offset",
    )
}

fn write_extracted(root: &Path, name: &str, bytes: &[u8]) -> Result<()> {
    let normalized = normalize_name(name);
    Archive::destination_path(root, name)?;
    let mut current = root.to_path_buf();
    let parts: Vec<&str> = normalized
        .split('/')
        .filter(|part| !part.is_empty())
        .collect();
    for (index, part) in parts.iter().enumerate() {
        current.push(part);
        let last = index + 1 == parts.len();
        match std::fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(Error::new(format!(
                    "refusing to extract through a symlink: {name}"
                )));
            }
            Ok(meta) if last && meta.is_dir() => {
                return Err(Error::new(format!("archive entry is not a file: {name}")));
            }
            Ok(meta) if !last && !meta.is_dir() => {
                return Err(Error::new(format!(
                    "archive entry parent is not a directory: {name}"
                )));
            }
            Ok(_) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                if !last {
                    std::fs::create_dir(&current).map_err(|err| Error::new(err.to_string()))?;
                }
            }
            Err(err) => return Err(Error::new(err.to_string())),
        }
    }
    // A new inode is renamed into place. `fs::write` would truncate an existing
    // hard link and change the file outside `root` that shares that inode.
    install_extracted(&current, bytes)
}

static EXTRACT_TEMP: AtomicU64 = AtomicU64::new(0);

fn install_extracted(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    if path.file_name().is_none() {
        return Err(Error::new("archive entry is not a file"));
    }
    let seq = EXTRACT_TEMP.fetch_add(1, Ordering::Relaxed);
    // Keep the temporary name short. A destination component may already be
    // 255 bytes, which is the maximum on common Linux filesystems.
    let tmp = parent.join(format!(".mkxpz-{}-{seq}", std::process::id()));
    // A collision belongs to somebody else: do not enter cleanup until we
    // have successfully created and own this temporary file.
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .map_err(|err| Error::new(err.to_string()))?;
    let write_result = (|| -> Result<()> {
        file.write_all(bytes)
            .map_err(|err| Error::new(err.to_string()))?;
        // Replace the directory entry, but keep the mode of a file already
        // at that name so a private file does not become world-readable.
        if let Ok(meta) = std::fs::symlink_metadata(path) {
            if meta.file_type().is_file() {
                std::fs::set_permissions(&tmp, meta.permissions())
                    .map_err(|err| Error::new(err.to_string()))?;
            }
        }
        Ok(())
    })();
    drop(file);
    if let Err(err) = write_result {
        let _ = std::fs::remove_file(&tmp);
        return Err(err);
    }
    match std::fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
            // Windows refuses to replace an existing name. Unlink that
            // directory entry only; other hard links keep the old bytes.
            let replaced = std::fs::remove_file(path).and_then(|_| std::fs::rename(&tmp, path));
            if replaced.is_err() {
                let _ = std::fs::remove_file(&tmp);
            }
            replaced.map_err(|err| Error::new(err.to_string()))
        }
        Err(err) => {
            let _ = std::fs::remove_file(&tmp);
            Err(Error::new(err.to_string()))
        }
    }
}

#[cfg(test)]
fn pack_v1_raw(files: &[(&[u8], &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(HEADER);
    out.push(0);
    out.push(1);
    let mut magic = V1_MAGIC;
    for (name, data) in files {
        let enc_len = (name.len() as u32) ^ advance(&mut magic);
        out.extend_from_slice(&enc_len.to_le_bytes());
        for &byte in *name {
            out.push(byte ^ (advance(&mut magic) as u8));
        }
        let enc_size = (data.len() as u32) ^ advance(&mut magic);
        out.extend_from_slice(&enc_size.to_le_bytes());
        out.extend_from_slice(&encrypt_body(data, magic));
    }
    out
}

mod tests {
    #[cfg(test)]
    use super::*;

    #[test]
    fn extraction_temp_collision_preserves_existing_file() {
        const CHILD: &str = "MKXPZ_TEMP_COLLISION_TEST";
        if std::env::var_os(CHILD).is_none() {
            // Isolate the temp counter from concurrently running extraction tests.
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "archive::tests::extraction_temp_collision_preserves_existing_file",
                ])
                .env(CHILD, "1")
                .status()
                .unwrap();
            assert!(status.success());
            return;
        }
        let root = std::env::temp_dir().join(format!("mkxpz-collision-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let tmp = root.join(format!(".mkxpz-{}-0", std::process::id()));
        std::fs::write(&tmp, b"existing data").unwrap();
        let result = install_extracted(&root.join("output"), b"archive data");
        let preserved = std::fs::read(&tmp);
        std::fs::remove_dir_all(&root).unwrap();
        assert!(result.is_err());
        assert_eq!(preserved.unwrap(), b"existing data");
    }

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

    #[test]
    fn advance_n_handles_large_seek_counts() {
        // Upstream #381 overflowed a 30-entry jump table at large seeks.
        // Independent exponentiation of x -> 7*x + 3 modulo 2^32.
        for count in [1 << 28, 1 << 30, 1 << 31, u32::MAX] {
            let mut remaining = count;
            let (mut mul, mut add) = (7u32, 3u32);
            let mut expected = 0x1234_5678u32;
            while remaining != 0 {
                if remaining & 1 != 0 {
                    expected = expected.wrapping_mul(mul).wrapping_add(add);
                }
                add = add.wrapping_mul(mul.wrapping_add(1));
                mul = mul.wrapping_mul(mul);
                remaining >>= 1;
            }
            let mut actual = 0x1234_5678;
            advance_n(&mut actual, count);
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn archive_fields_reject_values_past_u32() {
        // `u32::MAX + 1` overflows the archive field on every pointer width.
        // `saturating_add` on `usize` would hide that on 32-bit targets.
        assert!(checked_sum(u32::MAX as usize, 1).is_err());
        assert!(checked_sum(usize::MAX, 1).is_err());
    }

    #[test]
    fn extract_writes_entries_on_every_platform() {
        let dir = std::env::temp_dir().join(format!("mkxpz-extract-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        let ok = dir.join("plain");
        let packed = Archive::pack(1, &[("Data/settings.ini", b"secret")]).unwrap();
        Archive::open(packed).unwrap().extract_to(&ok).unwrap();
        assert_eq!(
            std::fs::read(ok.join("Data").join("settings.ini")).unwrap(),
            b"secret"
        );

        let linked = dir.join("linked");
        let outside_file = dir.join("outside.txt");
        std::fs::create_dir_all(linked.join("Data")).unwrap();
        std::fs::write(&outside_file, b"keep").unwrap();
        std::fs::hard_link(&outside_file, linked.join("Data").join("settings.ini")).unwrap();
        let packed = Archive::pack(1, &[("Data/settings.ini", b"secret")]).unwrap();
        Archive::open(packed).unwrap().extract_to(&linked).unwrap();
        assert_eq!(std::fs::read(&outside_file).unwrap(), b"keep");
        assert_eq!(
            std::fs::read(linked.join("Data").join("settings.ini")).unwrap(),
            b"secret"
        );

        let long_name = "a".repeat(255);
        let long_root = dir.join("long");
        let packed = Archive::pack(1, &[(&long_name, b"ok")]).unwrap();
        Archive::open(packed)
            .unwrap()
            .extract_to(&long_root)
            .unwrap();
        assert_eq!(std::fs::read(long_root.join(&long_name)).unwrap(), b"ok");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn extract_refuses_to_follow_a_destination_symlink() {
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir().join(format!("mkxpz-extract-unix-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let dest = dir.join("out");
        let outside = dir.join("outside");
        std::fs::create_dir_all(&dest).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, dest.join("Data")).unwrap();
        let packed = Archive::pack(1, &[("Data/settings.ini", b"secret")]).unwrap();
        let archive = Archive::open(packed).unwrap();
        assert!(archive.extract_to(&dest).is_err());
        assert!(!outside.join("settings.ini").exists());

        let private_root = dir.join("private");
        std::fs::create_dir_all(&private_root).unwrap();
        let private_file = private_root.join("secret.txt");
        std::fs::write(&private_file, b"old").unwrap();
        let mut perms = std::fs::metadata(&private_file).unwrap().permissions();
        perms.set_mode(0o600);
        std::fs::set_permissions(&private_file, perms).unwrap();
        let packed = Archive::pack(1, &[("secret.txt", b"new")]).unwrap();
        Archive::open(packed)
            .unwrap()
            .extract_to(&private_root)
            .unwrap();
        assert_eq!(std::fs::read(&private_file).unwrap(), b"new");
        let mode = std::fs::metadata(&private_file)
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn decoded_names_with_different_bytes_are_both_kept() {
        let packed = pack_v1_raw(&[
            (b"\xe9.dat".as_slice(), b"latin"),
            (b"\xc3\xa9.dat".as_slice(), b"utf8"),
        ]);
        let archive = Archive::open(packed).unwrap();
        let mut payloads: Vec<Vec<u8>> = archive
            .files()
            .map(|name| archive.read(name).unwrap())
            .collect();
        payloads.sort();
        assert_eq!(payloads, vec![b"latin".to_vec(), b"utf8".to_vec()]);
        assert_eq!(archive.read("é.dat").unwrap(), b"latin");
        let other = archive.files().find(|name| *name != "é.dat").unwrap();
        assert_eq!(archive.read(other).unwrap(), b"utf8");
    }

    #[test]
    fn pack_rejects_names_the_reader_cannot_open() {
        let name = format!("{}x", "a/".repeat(512));
        assert_eq!(name.len(), 1025);
        for version in [1, 2, 3] {
            let err = Archive::pack(version, &[(&name, b"data")]).unwrap_err();
            assert!(
                err.to_string().contains("unreasonably long"),
                "version {version}: {err}"
            );
        }
        let limit = "a".repeat(MAX_ARCHIVE_NAME_LEN);
        let packed = Archive::pack(1, &[(&limit, b"ok")]).unwrap();
        assert_eq!(Archive::open(packed).unwrap().read(&limit).unwrap(), b"ok");
    }
}
