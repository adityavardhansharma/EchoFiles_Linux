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

// Cross-filesystem moves remove their sources only after the batch's single sync, and a
// merged move still retires the emptied source folders and undoes completely.
#[test]
fn cross_filesystem_batch_and_merged_moves() {
    let home = tempfile::tempdir().unwrap();
    let Ok(other) = tempfile::tempdir_in("/dev/shm") else { return };
    if fs::metadata(other.path()).unwrap().dev() == fs::metadata(home.path()).unwrap().dev() { return; }
    unsafe { std::env::set_var("XDG_DATA_HOME", home.path().join("data")); }
    let src = home.path().join("src"); fs::create_dir(&src).unwrap();
    let files: Vec<_> = (0..20).map(|i| { let p = src.join(format!("f{i}")); fs::write(&p, format!("{i}")).unwrap(); p }).collect();
    let p = Progress::default();
    let plan = ops::plan(Kind::Move, &files, other.path(), &p).unwrap();
    let out = ops::execute(&plan, &HashMap::new(), Resolution::Skip, false, &p);
    assert!(out.errors.is_empty() && out.synced, "{:?}", out.errors);
    assert!(files.iter().all(|f| !f.exists()));
    assert_eq!(fs::read(other.path().join("f7")).unwrap(), b"7");
    assert!(out.journal.iter().all(|u| u.label() == "move"));

    let folder = src.join("folder"); fs::create_dir_all(folder.join("deep")).unwrap();
    fs::write(folder.join("new"), b"new").unwrap(); fs::write(folder.join("deep/x"), b"x").unwrap();
    let target = other.path().join("folder"); fs::create_dir_all(target.join("deep")).unwrap(); fs::write(target.join("old"), b"old").unwrap();
    let out = transfer(Kind::Move, &folder, other.path());
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert!(!folder.exists(), "the emptied source folder is retired");
    assert_eq!(fs::read(target.join("deep/x")).unwrap(), b"x");
    Undo::Journal(Arc::new(Mutex::new(out.journal))).apply().unwrap();
    assert_eq!(fs::read(folder.join("deep/x")).unwrap(), b"x");
    assert_eq!(fs::read(folder.join("new")).unwrap(), b"new");
    assert_eq!(fs::read(target.join("old")).unwrap(), b"old");
    assert!(!target.join("new").exists());
}

#[test]
fn rename_changes_only_letter_case() {
    let d = tempfile::tempdir().unwrap();
    fs::write(d.path().join("photo.jpg"), b"x").unwrap();
    assert_eq!(ops::rename(&d.path().join("photo.jpg"), "Photo.jpg").unwrap(), d.path().join("Photo.jpg"));
    assert!(d.path().join("Photo.jpg").exists());
}

fn recovery_dirs(p: &Path) -> usize {
    fs::read_dir(p).unwrap().flatten().map(|e| if e.file_name().to_string_lossy().starts_with(".echofiles-") { 1 } else if e.file_type().unwrap().is_dir() { recovery_dirs(&e.path()) } else { 0 }).sum()
}

// Replacements share one staging folder per destination folder, undo restores the tree
// exactly and leaves nothing behind, and discarding the undo frees what was retained.
#[test]
fn merge_replace_undo_and_discard_leave_no_recovery_folders() {
    let root = tempfile::tempdir().unwrap();
    unsafe { std::env::set_var("XDG_DATA_HOME", root.path().join("data")); }
    for discard in [false, true] {
        let base = root.path().join(if discard { "discard" } else { "undo" });
        let src = base.join("src/tree"); let dst = base.join("dst");
        for d in ["a", "b"] {
            fs::create_dir_all(src.join(d)).unwrap(); fs::create_dir_all(dst.join("tree").join(d)).unwrap();
            for i in 0..50 { fs::write(src.join(d).join(format!("f{i}")), b"new").unwrap(); }
            for i in 0..25 { fs::write(dst.join("tree").join(d).join(format!("f{i}")), b"old").unwrap(); }
        }
        let out = transfer(Kind::Copy, &src, &dst);
        assert!(out.errors.is_empty(), "{:?}", out.errors);
        assert_eq!(recovery_dirs(&dst), 2, "one staging folder per destination folder, not one per file");
        let journal = Undo::Journal(Arc::new(Mutex::new(out.journal)));
        if discard {
            journal.discard();
            assert_eq!(fs::read(dst.join("tree/a/f0")).unwrap(), b"new");
        } else {
            journal.apply().unwrap();
            for d in ["a", "b"] {
                assert_eq!(fs::read(dst.join("tree").join(d).join("f0")).unwrap(), b"old");
                assert!(!dst.join("tree").join(d).join("f30").exists());
            }
        }
        assert_eq!(recovery_dirs(&dst), 0);
    }
}

#[test]
fn many_items_with_one_name_trash_and_restore() {
    let root = tempfile::tempdir().unwrap();
    unsafe { std::env::set_var("XDG_DATA_HOME", root.path().join("data")); }
    let paths: Vec<_> = (0..300).map(|i| { let d = root.path().join(format!("d{i}")); fs::create_dir(&d).unwrap(); let p = d.join("README.md"); fs::write(&p, format!("{i}")).unwrap(); p }).collect();
    let (ok, failed) = ops::trash_all(&paths);
    assert!(failed.is_empty() && ok.len() == 300);
    let names: std::collections::HashSet<_> = ok.iter().map(|t| t.file.clone()).collect();
    assert_eq!(names.len(), 300, "every trashed item has its own name");
    Undo::Trash(ok).retryable().apply().unwrap();
    for (i, p) in paths.iter().enumerate() { assert_eq!(fs::read_to_string(p).unwrap(), i.to_string()); }
}

#[test]
fn duplicate_names_are_found_with_gaps_and_without_gaps() {
    let d = tempfile::tempdir().unwrap();
    let f = d.path().join("photo.jpg"); fs::write(&f, b"x").unwrap();
    let made: Vec<_> = (0..40).map(|_| { let p = ops::unique_path(&f, ops::Naming::Copy); assert!(!p.exists()); fs::write(&p, b"x").unwrap(); p }).collect();
    let unique: std::collections::HashSet<_> = made.iter().collect();
    assert_eq!(unique.len(), 40);
    fs::remove_file(&made[5]).unwrap();
    let p = ops::unique_path(&f, ops::Naming::Copy);
    assert!(!p.exists());
}

#[test]
fn a_few_files_copy_and_move_with_targeted_fsync() {
    let root = tempfile::tempdir().unwrap();
    let src = root.path().join("s"); let dst = root.path().join("d"); fs::create_dir(&src).unwrap(); fs::create_dir(&dst).unwrap();
    let files: Vec<_> = (0..3).map(|i| { let p = src.join(format!("f{i}")); fs::write(&p, format!("{i}")).unwrap(); p }).collect();
    let p = Progress::default();
    let plan = ops::plan(Kind::Copy, &files, &dst, &p).unwrap();
    let out = ops::execute(&plan, &HashMap::new(), Resolution::Skip, false, &p);
    assert!(out.errors.is_empty() && out.synced);
    assert_eq!(fs::read(dst.join("f2")).unwrap(), b"2");
    if let Ok(other) = tempfile::tempdir_in("/dev/shm") {
        let plan = ops::plan(Kind::Move, &files, other.path(), &p).unwrap();
        let out = ops::execute(&plan, &HashMap::new(), Resolution::Skip, false, &p);
        assert!(out.errors.is_empty() && out.synced, "{:?}", out.errors);
        assert!(files.iter().all(|f| !f.exists()));
        Undo::Journal(Arc::new(Mutex::new(out.journal))).apply().unwrap();
        assert!(files.iter().all(|f| f.exists()));
    }
}
