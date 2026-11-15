use std::fs;
use std::path::{Path, PathBuf};

use crate::archive::Archive;
use crate::error::{Error, Result};

/// A mounted directory or RGSS archive. Earlier mounts win, matching PhysFS
/// with `append = 1`.
enum Mount {
    Dir { root: PathBuf },
    Archive(Archive),
}

/// Case-insensitive game filesystem with RGSS archive support and the
/// extension-supplementing lookup used by `FileSystem::openRead`.
pub struct FileSystem {
    mounts: Vec<Mount>,
    allow_symlinks: bool,
}

impl FileSystem {
    pub fn new(allow_symlinks: bool) -> Self {
        Self {
            mounts: Vec::new(),
            allow_symlinks,
        }
    }

    pub fn mount_dir(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let root = path.as_ref();
        if !self.allow_symlinks
            && fs::symlink_metadata(root).is_ok_and(|meta| meta.file_type().is_symlink())
        {
            return Err(Error::new(format!(
                "Failed to mount {}: symlink roots are disabled",
                root.display()
            )));
        }
        if !root.is_dir() {
            return Err(Error::new(format!(
                "Failed to mount {}: not a directory",
                root.display()
            )));
        }
        self.mounts.push(Mount::Dir {
            root: root.to_path_buf(),
        });
        Ok(())
    }

    pub fn mount_archive_file(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let bytes = fs::read(path.as_ref()).map_err(|err| {
            Error::new(format!(
                "Failed to mount {}: {err}",
                path.as_ref().display()
            ))
        })?;
        self.mount_archive_bytes(bytes)
    }

    pub fn mount_archive_bytes(&mut self, bytes: Vec<u8>) -> Result<()> {
        self.mounts.push(Mount::Archive(Archive::open(bytes)?));
        Ok(())
    }

    pub fn exists(&self, path: &str) -> bool {
        let logical = normalize_logical(path);
        for mount in &self.mounts {
            let present = match mount {
                Mount::Dir { root } => self.dir_has_file(root, &logical),
                Mount::Archive(archive) => archive
                    .files()
                    .any(|name| name.eq_ignore_ascii_case(&logical)),
            };
            if present {
                return true;
            }
        }
        false
    }

    pub fn read(&self, path: &str) -> Result<Vec<u8>> {
        self.lookup(path, false)?
            .ok_or_else(|| Error::new(format!("file not found: {path}")))
    }

    /// Like `read`, but also accepts `name.ext` when the caller asked for `name`.
    pub fn read_with_extension(&self, path: &str) -> Result<(String, Vec<u8>)> {
        self.lookup_named(path, true)?
            .ok_or_else(|| Error::new(format!("file not found: {path}")))
    }

    fn lookup(&self, path: &str, supplement: bool) -> Result<Option<Vec<u8>>> {
        Ok(self.lookup_named(path, supplement)?.map(|(_, bytes)| bytes))
    }

    fn lookup_named(&self, path: &str, supplement: bool) -> Result<Option<(String, Vec<u8>)>> {
        let logical = normalize_logical(path);
        for mount in &self.mounts {
            match mount {
                Mount::Dir { root } => match self.lookup_dir(root, &logical, supplement) {
                    Some(Some(found)) => return Ok(Some(found)),
                    Some(None) => return Ok(None),
                    None => {}
                },
                Mount::Archive(archive) => {
                    if let Some(found) = lookup_archive(archive, &logical, supplement)? {
                        return Ok(Some(found));
                    }
                }
            }
        }
        Ok(None)
    }

    fn lookup_dir(
        &self,
        root: &Path,
        logical: &str,
        supplement: bool,
    ) -> Option<Option<(String, Vec<u8>)>> {
        let parent_rel = parent_logical(logical);
        let file_rel = file_logical(logical);
        let parent = self.resolve_existing(root, &parent_rel)?;
        let entries = fs::read_dir(&parent).ok()?;
        let mut names = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !self.allow_entry(&entry.path()) || !entry.path().is_file() {
                continue;
            }
            if name.eq_ignore_ascii_case(&file_rel) {
                names.push((true, name));
            } else if supplement && extension_match(&name, &file_rel) {
                names.push((false, name));
            }
        }
        names.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        let name = names.into_iter().next()?.1;
        let full = parent.join(&name);
        let display_parent = self.display_logical(root, &parent);
        let logical_name = if display_parent.is_empty() {
            name
        } else {
            format!("{display_parent}/{name}")
        };
        Some(fs::read(full).ok().map(|bytes| (logical_name, bytes)))
    }

    fn dir_has_file(&self, root: &Path, logical: &str) -> bool {
        let parent_rel = parent_logical(logical);
        let file_rel = file_logical(logical);
        let Some(parent) = self.resolve_existing(root, &parent_rel) else {
            return false;
        };
        let Ok(entries) = fs::read_dir(&parent) else {
            return false;
        };
        entries.flatten().any(|entry| {
            self.allow_entry(&entry.path())
                && entry.path().is_file()
                && entry
                    .file_name()
                    .to_string_lossy()
                    .eq_ignore_ascii_case(&file_rel)
        })
    }

    /// Walk `logical` under `root`, matching each component without case.
    fn resolve_existing(&self, root: &Path, logical: &str) -> Option<PathBuf> {
        let mut current = root.to_path_buf();
        if logical.is_empty() {
            return Some(current);
        }
        for part in logical.split('/') {
            if !current.is_dir() {
                return None;
            }
            let mut found = None;
            for entry in fs::read_dir(&current).ok()?.flatten() {
                if !self.allow_entry(&entry.path()) {
                    continue;
                }
                if entry
                    .file_name()
                    .to_string_lossy()
                    .eq_ignore_ascii_case(part)
                {
                    found = Some(entry.path());
                    break;
                }
            }
            current = found?;
        }
        Some(current)
    }

    fn display_logical(&self, root: &Path, path: &Path) -> String {
        path.strip_prefix(root)
            .ok()
            .map(|rel| rel.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default()
    }

    fn allow_entry(&self, path: &Path) -> bool {
        if self.allow_symlinks {
            return true;
        }
        fs::symlink_metadata(path)
            .map(|meta| !meta.file_type().is_symlink())
            .unwrap_or(false)
    }
}

fn lookup_archive(
    archive: &Archive,
    logical: &str,
    supplement: bool,
) -> Result<Option<(String, Vec<u8>)>> {
    let wanted = logical.to_ascii_lowercase();
    let mut exact = None;
    let mut extra = None;
    for name in archive.files() {
        let lower = name.to_ascii_lowercase();
        if lower == wanted {
            exact = Some(name.to_string());
            break;
        }
        if supplement && extension_match(&lower, &wanted) && extra.is_none() {
            extra = Some(name.to_string());
        }
    }
    let Some(name) = exact.or(extra) else {
        return Ok(None);
    };
    // A matching entry that cannot be decoded is corrupt. Do not continue
    // into a lower-priority mount and return that older copy.
    let bytes = archive.read(&name)?;
    Ok(Some((name, bytes)))
}

fn extension_match(candidate: &str, requested: &str) -> bool {
    // PhysFS enumerates one directory. A nested name such as
    // `face.backup/old.png` is not an extension of `face`.
    if parent_logical(candidate) != parent_logical(requested) {
        return false;
    }
    let candidate = file_logical(candidate).into_bytes();
    let requested = file_logical(requested).into_bytes();
    if candidate.len() <= requested.len() {
        return false;
    }
    candidate
        .get(..requested.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(&requested))
        && candidate[requested.len()] == b'.'
}

fn normalize_logical(path: &str) -> String {
    let replaced = path.replace('\\', "/");
    let mut out = Vec::new();
    for part in replaced.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            out.pop();
            continue;
        }
        out.push(part);
    }
    out.join("/")
}

fn parent_logical(path: &str) -> String {
    match path.rfind('/') {
        Some(index) => path[..index].to_string(),
        None => String::new(),
    }
}

fn file_logical(path: &str) -> String {
    match path.rfind('/') {
        Some(index) => path[index + 1..].to_string(),
        None => path.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directory_lookup_is_case_insensitive_and_supplements_extensions() {
        let dir = std::env::temp_dir().join(format!("mkxpz-fs-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("Graphics/Pictures")).unwrap();
        fs::write(dir.join("Graphics/Pictures/Face.PNG"), b"pixels").unwrap();
        let mut fs = FileSystem::new(false);
        fs.mount_dir(&dir).unwrap();
        assert_eq!(fs.read("graphics\\pictures\\face.png").unwrap(), b"pixels");
        let (name, bytes) = fs.read_with_extension("Graphics/Pictures/Face").unwrap();
        assert_eq!(name, "Graphics/Pictures/Face.PNG");
        assert_eq!(bytes, b"pixels");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn archive_mount_is_visible_through_the_filesystem() {
        let packed = Archive::pack(1, &[("Data/System.rxdata", b"system")]).unwrap();
        let mut fs = FileSystem::new(true);
        fs.mount_archive_bytes(packed).unwrap();
        assert!(fs.exists("data/system.rxdata"));
        assert_eq!(fs.read("Data/System.rxdata").unwrap(), b"system");
    }

    #[test]
    fn extension_supplement_stays_in_the_requested_directory() {
        let packed = Archive::pack(
            1,
            &[
                ("Graphics/face.backup/old.png", b"nested"),
                ("Graphics/face.png", b"sibling"),
            ],
        )
        .unwrap();
        let mut fs = FileSystem::new(true);
        fs.mount_archive_bytes(packed).unwrap();
        let (name, bytes) = fs.read_with_extension("Graphics/face").unwrap();
        assert_eq!(name, "Graphics/face.png");
        assert_eq!(bytes, b"sibling");
    }

    #[test]
    fn corrupt_archive_entry_does_not_fall_through() {
        let mut broken = Archive::pack(3, &[("Data/System.rxdata", b"new")]).unwrap();
        broken.pop();
        let direct = Archive::open(broken.clone()).unwrap();
        let direct_err = direct.read("Data/System.rxdata").unwrap_err();
        assert!(direct_err
            .to_string()
            .contains("extends past the end of the file"));

        let older = Archive::pack(3, &[("Data/System.rxdata", b"old")]).unwrap();
        let mut fs = FileSystem::new(true);
        fs.mount_archive_bytes(broken).unwrap();
        fs.mount_archive_bytes(older).unwrap();
        let err = fs.read("Data/System.rxdata").unwrap_err();
        assert!(err.to_string().contains("extends past the end of the file"));
        assert!(!err.to_string().contains("old"));
    }
}
