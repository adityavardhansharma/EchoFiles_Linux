//! Snapshot audit: only disposable fixture files; no network or real user files.
use std::{collections::HashMap, fs, path::{Path, PathBuf}, sync::{Arc, Mutex}, time::Instant};
use ef_core::ops::{self, Kind, Progress, Resolution, Undo};
fn run(src: &Path, dst: &Path) -> ops::Outcome {
    let p = Progress::default();
    let plan = ops::plan(Kind::Copy, &[src.into()], dst, &p).unwrap();
    ops::execute(&plan, &HashMap::new(), Resolution::Replace, false, &p)
}
fn replacement(root: &Path, journal: bool) {
    let d = root.join(if journal { "journal" } else { "ui-undo" });
    fs::create_dir_all(d.join("src")).unwrap(); fs::create_dir_all(d.join("dst")).unwrap();
    fs::write(d.join("src/x"), b"new").unwrap(); fs::write(d.join("dst/x"), b"old").unwrap();
    let out = run(&d.join("src/x"), &d.join("dst"));
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    let undo = if journal { Undo::Journal(Arc::new(Mutex::new(out.journal))) } else { Undo::Copy(out.done.into_iter().map(|(_, p)| p).collect()) };
    let result = undo.apply();
    println!("replace undo journal={journal}: result={result:?}; old destination restored={}", fs::read(d.join("dst/x")).is_ok_and(|b| b==b"old"));
}
fn main() {
    let root = PathBuf::from(std::env::args().nth(1).unwrap()); fs::create_dir_all(&root).unwrap();
    replacement(&root, false); replacement(&root, true);
    let d=root.join("failed-replace"); fs::create_dir_all(d.join("src")).unwrap(); fs::create_dir_all(d.join("dst")).unwrap();
    fs::write(d.join("src/x"), b"new").unwrap(); fs::write(d.join("dst/x"), b"old").unwrap();
    let p=Progress::default(); let plan=ops::plan(Kind::Copy,&[d.join("src/x")],&d.join("dst"),&p).unwrap(); fs::remove_file(d.join("src/x")).unwrap();
    let out=ops::execute(&plan,&HashMap::new(),Resolution::Replace,false,&p);
    println!("failed replacement: old preserved={}; errors={}",fs::read(d.join("dst/x")).unwrap()==b"old",out.errors.len());
    let d=root.join("alias"); fs::create_dir_all(d.join("src")).unwrap();fs::write(d.join("src/x"),b"only copy").unwrap(); std::os::unix::fs::symlink(d.join("src"),d.join("link")).unwrap();
    let out=run(&d.join("src/x"),&d.join("link"));
    println!("copy through alias: source preserved={}; successful destinations={}",fs::read(d.join("src/x")).unwrap()==b"only copy",out.done.len());
    let d=root.join("fifo");fs::create_dir_all(d.join("src")).unwrap();fs::create_dir_all(d.join("dst")).unwrap();assert!(std::process::Command::new("mkfifo").arg(d.join("src/pipe")).status().unwrap().success());
    let fifo_progress=Progress::default();let fifo_plan=ops::plan(Kind::Copy,&[d.join("src/pipe")],&d.join("dst"),&fifo_progress).unwrap();
    let fifo_out=ops::execute(&fifo_plan,&HashMap::new(),Resolution::Replace,false,&fifo_progress);
    println!("FIFO copy: execution errors={}; source preserved={}; destination absent={}",fifo_out.errors.len(),d.join("src/pipe").exists(),!d.join("dst/pipe").exists());
    let d=root.join("progress");fs::create_dir_all(d.join("src")).unwrap();fs::create_dir_all(d.join("dst")).unwrap();fs::write(d.join("src/x"),b"x").unwrap();
    let p=Progress::default();let plan=ops::plan(Kind::Copy,&[d.join("src/x")],&d.join("dst"),&p).unwrap();let out=ops::execute(&plan,&HashMap::new(),Resolution::Replace,false,&p);assert!(out.errors.is_empty());
    println!("single file copy progress: files_done={}; files_total={}",p.files_done.load(std::sync::atomic::Ordering::Relaxed),p.files_total.load(std::sync::atomic::Ordering::Relaxed));
    let d=root.join("index");fs::create_dir_all(&d).unwrap();fs::write(d.join("x"),b"x").unwrap();let file=root.join("malformed.efidx");ef_index::Index::build(&d).unwrap().save(&file).unwrap();
    let mut bytes=fs::read(&file).unwrap();bytes[32..40].copy_from_slice(&u64::MAX.to_le_bytes());bytes[40..48].copy_from_slice(&0u64.to_le_bytes());fs::write(&file,bytes).unwrap();
    let original=std::panic::take_hook(); std::panic::set_hook(Box::new(|_|{}));let result=std::panic::catch_unwind(||ef_index::MappedIndex::open(&file)); std::panic::set_hook(original);
    println!("malformed mmap header: panicked={}",result.is_err());
    let cfg=root.join("settings.toml");let mut errors=0; let mut malformed=0;
    for _ in 0..50 {
        let barrier=Arc::new(std::sync::Barrier::new(9));let mut jobs=Vec::new();
        for n in 0..8 { let b=barrier.clone();let path=cfg.clone();jobs.push(std::thread::spawn(move||{let mut s=ef_config::Settings::default();s.sidebar.width=200+n; b.wait();s.save_to(&path).is_err()})); }
        barrier.wait();for job in jobs {errors+=usize::from(job.join().unwrap());}malformed+=usize::from(ef_config::Settings::load_from(&cfg).is_err());
    }
    println!("concurrent config saves: save errors={errors}/400; malformed final files={malformed}/50");
    let state=root.join("state");fs::create_dir_all(state.join("echofiles/phone-index")).unwrap();
    // Single-threaded environment mutation before phone search; no worker reads this variable.
    unsafe {std::env::set_var("XDG_STATE_HOME",&state);}
    let mut text=String::new();for i in 0..10_000 {text.push_str(&format!("{{\"device\":\"phone\",\"path\":\"Documents/report-{i}.txt\",\"directory\":false}}\n"));}
    fs::write(state.join("echofiles/phone-index/phone.jsonl"),text).unwrap();
    let q=ef_index::Query{limit:Some(50),..ef_index::Query::new("report")};
    let _=ef_index::phone::search(&q);let mut timings=Vec::new();for _ in 0..9 {let t=Instant::now();assert_eq!(ef_index::phone::search(&q).len(),50);timings.push(t.elapsed().as_secs_f64()*1000.0);}timings.sort_by(f64::total_cmp);
    println!("phone JSONL search 10,000 entries top50: median={:.3}ms; range={:.3}..{:.3}ms (dev build, loaded host, exploratory)",timings[4],timings[0],timings[8]);
}
