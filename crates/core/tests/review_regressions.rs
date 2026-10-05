use std::{collections::HashMap, fs, os::unix::fs::{symlink, MetadataExt}, path::Path, sync::{Arc, Mutex}};
use ef_core::ops::{self, Kind, Progress, Resolution, Undo};
fn transfer(kind: Kind, src: &Path, dst: &Path) -> ops::Outcome {
    let progress = Progress::default();
    let plan = ops::plan(kind, &[src.into()], dst, &progress).unwrap();
    ops::execute(&plan, &HashMap::new(), Resolution::Replace, false, &progress)
}
// This integration executable owns its disposable Trash; no real user data is touched.
#[test]
fn file_safety_review_regressions() {
    let root = tempfile::tempdir().unwrap();
    unsafe { std::env::set_var("XDG_DATA_HOME", root.path().join("data")); }
    for (label, kind) in [("copy", Kind::Copy), ("move", Kind::Move)] {
        let src = root.path().join(label).join("src/folder");
        let dst = root.path().join(label).join("dst/folder");
        fs::create_dir_all(&src).unwrap(); fs::create_dir_all(&dst).unwrap();
        fs::write(src.join("new"), b"new").unwrap(); fs::write(src.join("replace"), b"new version").unwrap();
        fs::write(dst.join("old"), b"unrelated").unwrap(); fs::write(dst.join("replace"), b"original").unwrap();
        let out = transfer(kind, &src, dst.parent().unwrap());
        assert!(out.errors.is_empty(), "{:?}", out.errors);
        if kind == Kind::Move { assert!(!src.exists(), "a merged move must remove the emptied source"); }
        Undo::Journal(Arc::new(Mutex::new(out.journal))).apply().unwrap();
        assert_eq!(fs::read(dst.join("old")).unwrap(), b"unrelated");
        assert_eq!(fs::read(dst.join("replace")).unwrap(), b"original");
        assert_eq!(fs::read(src.join("new")).unwrap(), b"new");
        assert!(!dst.join("new").exists());
    }
    let src = root.path().join("source"); let dst = root.path().join("destination");
    fs::create_dir_all(&src).unwrap(); fs::create_dir_all(&dst).unwrap();
    fs::write(src.join("x"), b"new").unwrap(); fs::write(dst.join("x"), b"old").unwrap();
    let progress = Progress::default();
    let plan = ops::plan(Kind::Copy, &[src.join("x")], &dst, &progress).unwrap();
    fs::remove_file(src.join("x")).unwrap();
    let out = ops::execute(&plan, &HashMap::new(), Resolution::Replace, false, &progress);
    assert!(!out.errors.is_empty()); assert_eq!(fs::read(dst.join("x")).unwrap(), b"old");
    fs::write(src.join("x"), b"only copy").unwrap();
    let alias = root.path().join("alias"); symlink(&src, &alias).unwrap();
    let out = transfer(Kind::Copy, &src.join("x"), &alias);
    assert!(out.errors.is_empty()); assert_eq!(fs::read(src.join("x")).unwrap(), b"only copy");
    fs::create_dir(src.join("inner")).unwrap(); let inner_alias = root.path().join("inner-alias"); symlink(src.join("inner"), &inner_alias).unwrap();
    assert!(ops::plan(Kind::Copy, &[src.clone()], &inner_alias, &Progress::default()).is_err());
    let outside = root.path().join("outside"); fs::create_dir(&outside).unwrap(); fs::write(outside.join("x"), b"outside").unwrap();
    symlink(&outside, dst.join("source")).unwrap();
    let out = transfer(Kind::Copy, &src, &dst); assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert_eq!(fs::read(outside.join("x")).unwrap(), b"outside");
    assert!(!fs::symlink_metadata(dst.join("source")).unwrap().file_type().is_symlink());
    // Unsupported entries must fail without destroying the original across filesystems.
    let other = tempfile::tempdir_in("/dev/shm").unwrap();
    if fs::metadata(other.path()).unwrap().dev() != fs::metadata(&src).unwrap().dev() {
        let fifo = src.join("pipe");
        use std::os::unix::ffi::OsStrExt;
        let name = std::ffi::CString::new(fifo.as_os_str().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
        let out = transfer(Kind::Move, &fifo, other.path());
        assert!(!out.errors.is_empty()); assert!(fifo.exists()); assert!(!other.path().join("pipe").exists());
    }
    assert!(!ef_core::trash::is_trash_files(Path::new("/work/123/files")));
    assert!(!ef_core::trash::is_trash_files(Path::new("/work/Trash/files")));
}

#[test]
fn preserves_xattrs_and_hardlinks() {
    let root = tempfile::tempdir().unwrap(); let src = root.path().join("source"); let dst = root.path().join("dest");
    fs::create_dir(&src).unwrap(); fs::create_dir(&dst).unwrap(); fs::write(src.join("a"), b"data").unwrap(); fs::hard_link(src.join("a"), src.join("b")).unwrap();
    rustix::fs::lsetxattr(src.join("a"), "user.audit", b"value", rustix::fs::XattrFlags::empty()).unwrap();
    let out = transfer(Kind::Copy, &src, &dst); assert!(out.errors.is_empty(), "{:?}", out.errors);
    let mut value = [0; 16]; let n = rustix::fs::lgetxattr(dst.join("source/a"), "user.audit", &mut value).unwrap(); assert_eq!(&value[..n], b"value");
    assert_eq!(fs::metadata(dst.join("source/a")).unwrap().ino(), fs::metadata(dst.join("source/b")).unwrap().ino());
}

#[test]
fn conflict_changes_and_partial_undo_preserve_both_copies() {
    let root = tempfile::tempdir().unwrap();
    let src = root.path().join("src"); let dst = root.path().join("dst");
    fs::create_dir(&src).unwrap(); fs::create_dir(&dst).unwrap();
    fs::write(src.join("file"), b"source").unwrap(); fs::write(dst.join("file"), b"old").unwrap();
    let p = Progress::default();
    let plan = ops::plan(Kind::Move, &[src.join("file")], &dst, &p).unwrap();
    fs::write(dst.join("file"), b"changed after confirmation").unwrap();
    let outcome = ops::execute(&plan, &HashMap::new(), Resolution::Replace, false, &p);
    assert!(!outcome.errors.is_empty());
    assert_eq!(fs::read(src.join("file")).unwrap(), b"source");
    assert_eq!(fs::read(dst.join("file")).unwrap(), b"changed after confirmation");

    fs::write(dst.join("one"), b"one").unwrap(); fs::write(dst.join("two"), b"two").unwrap();
    fs::write(src.join("one"), b"unrelated").unwrap();
    let undo = Undo::Move(vec![(src.join("one"), dst.join("one")), (src.join("two"), dst.join("two"))]).retryable();
    assert!(undo.apply().is_err());
    assert_eq!(fs::read(src.join("two")).unwrap(), b"two");
    assert_eq!(fs::read(src.join("one")).unwrap(), b"unrelated");
    fs::rename(src.join("one"), src.join("unrelated")).unwrap();
    undo.apply().unwrap();
    assert_eq!(fs::read(src.join("one")).unwrap(), b"one");
    assert_eq!(fs::read(src.join("unrelated")).unwrap(), b"unrelated");
}

#[test]
fn progress_counts_each_leaf_once_for_copies_and_merged_moves() {
    use std::sync::atomic::Ordering;
    let root = tempfile::tempdir().unwrap();
    for (name, kind) in [("copy", Kind::Copy), ("move", Kind::Move)] {
        let src = root.path().join(name).join("source"); let dst = root.path().join(name).join("dest");
        fs::create_dir_all(src.join("nested")).unwrap(); fs::create_dir_all(dst.join("source")).unwrap();
        fs::write(src.join("a"), b"a").unwrap(); fs::write(src.join("nested/b"), b"b").unwrap();
        let progress = Progress::default();
        let plan = ops::plan(kind, &[src], &dst, &progress).unwrap();
        let out = ops::execute(&plan, &HashMap::new(), Resolution::Replace, false, &progress);
        assert!(out.errors.is_empty(), "{:?}", out.errors);
        assert_eq!(progress.files_total.load(Ordering::Relaxed), 2);
        assert_eq!(progress.files_done.load(Ordering::Relaxed), 2);
    }
}
