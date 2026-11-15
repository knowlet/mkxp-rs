//! Small, deterministic regressions derived from mkxp-z's open issues/PRs.
use mkxpz::{Archive, Bitmap, FileSystem, Table};

#[test]
fn pathological_archive_names_do_not_recurse() {
    // mkxp-z/mkxp-z#417: names that formerly recursed forever in cacheEnumCB.
    for version in [1, 2, 3] {
        let bytes = Archive::pack(
            version,
            &[("", b""), (".", b""), ("..", b""), ("data/a", b"ok")],
        )
        .unwrap();
        let archive = Archive::open(bytes.clone()).unwrap();
        assert_eq!(archive.files().collect::<Vec<_>>(), ["data/a"]);
        assert_eq!(archive.list_dir(""), ["data"]);
        assert_eq!(archive.list_dir("data"), ["a"]);
        let mut fs = FileSystem::new(false);
        fs.mount_archive_bytes(bytes).unwrap();
        assert_eq!(fs.read("DATA/A").unwrap(), b"ok");
    }
}

#[test]
fn table_wire_bytes_are_little_endian() {
    // #382/#383: fixed byte fixture, not a native-endian round trip.
    let bytes = [
        1, 0, 0, 0, 2, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 0x34, 0x12, 0xfe, 0xff,
    ];
    let table = Table::deserialize(&bytes).unwrap();
    assert_eq!(table.get(0, 0, 0), Some(0x1234));
    assert_eq!(table.get(1, 0, 0), Some(-2));
    assert_eq!(table.serialize(), bytes);
}

#[test]
fn oversized_bitmaps_return_errors_without_allocating() {
    // #314/#339: this core rejects mega textures, rather than allocating GPU surfaces.
    for (w, h) in [(8193, 1), (1, 8193), (i32::MAX, i32::MAX), (0, 1), (1, 0)] {
        assert!(Bitmap::new(w, h).is_err());
    }
}

#[test]
fn directory_mounts_see_new_files_without_cache_rebuild() {
    // #315/#319 and mount-order concern #93.
    // A leftover `picture.txt` from an interrupted run would make the directory
    // mount win before the archive is observed, so the directory must be unique
    // and removed even when an assertion fails.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!(
        "mkxp-upstream-cache-{}-{nanos}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let _cleanup = RemoveOnDrop(&root);
    let mut fs = FileSystem::new(false);
    fs.mount_dir(&root).unwrap();
    fs.mount_archive_bytes(Archive::pack(3, &[("picture.txt", b"archive")]).unwrap())
        .unwrap();
    assert_eq!(fs.read("PICTURE.TXT").unwrap(), b"archive");
    std::fs::write(root.join("picture.txt"), b"directory").unwrap();
    assert_eq!(fs.read_with_extension("PICTURE").unwrap().1, b"directory");
}

struct RemoveOnDrop<'a>(&'a std::path::Path);

impl Drop for RemoveOnDrop<'_> {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(self.0);
    }
}
