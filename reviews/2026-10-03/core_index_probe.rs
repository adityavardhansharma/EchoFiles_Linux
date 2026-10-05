// Read-only audit harness except for disposable directories supplied by the runner.
use std::{collections::HashMap, fs, path::PathBuf};
use std::os::unix::fs::{symlink, MetadataExt};
use ef_core::ops::{self, Kind, Progress, Resolution, Undo};
fn run(kind:Kind, src:PathBuf, dst:PathBuf)->ops::Outcome {
 let p=Progress::default(); let plan=ops::plan(kind,&[src],&dst,&p).unwrap();
 ops::execute(&plan,&HashMap::new(),Resolution::Replace,false,&p)
}
fn main(){
 let r=PathBuf::from(std::env::args().nth(1).unwrap());fs::create_dir_all(&r).unwrap();
 for (name,kind) in [("copy",Kind::Copy),("move",Kind::Move)] {
  let a=r.join(name).join("src/folder"); let b=r.join(name).join("dst/folder");fs::create_dir_all(&a).unwrap();fs::create_dir_all(&b).unwrap();
  fs::write(a.join("new"),"new").unwrap();fs::write(b.join("old"),"old").unwrap();
  let out=run(kind,a.clone(),b.parent().unwrap().into());assert!(out.errors.is_empty());
  match kind {Kind::Copy=>Undo::Copy(out.done.into_iter().map(|(_,d)|d).collect()),Kind::Move=>Undo::Move(out.done)}.apply().unwrap();
  println!("merge_undo_{name}: existing file remains in destination = {}",b.join("old").exists());
  assert!(!b.join("old").exists());
 }
 let a=r.join("replace/a"); let b=r.join("replace/b");fs::create_dir_all(&a).unwrap();fs::create_dir_all(&b).unwrap();
 fs::write(a.join("x"),"new").unwrap();fs::write(b.join("x"),"old").unwrap();
 let p=Progress::default(); let plan=ops::plan(Kind::Copy,&[a.join("x")],&b,&p).unwrap();fs::remove_file(a.join("x")).unwrap();
 let out=ops::execute(&plan,&HashMap::new(),Resolution::Replace,false,&p);
 println!("failed_replace: old destination exists = {}; errors = {:?}",b.join("x").exists(),out.errors);assert!(!b.join("x").exists());
 let a=r.join("alias/source");fs::create_dir_all(&a).unwrap();fs::write(a.join("x"),"only copy").unwrap();
 let alias=r.join("alias/link");symlink(&a,&alias).unwrap();let out=run(Kind::Copy,a.join("x"),alias);
 println!("copy_via_alias: source still exists = {}; errors = {:?}",a.join("x").exists(),out.errors);assert!(!a.join("x").exists());
 let a=r.join("selfcopy/source");fs::create_dir_all(a.join("inside")).unwrap();let alias=r.join("selfcopy/link");symlink(a.join("inside"),&alias).unwrap();
 println!("selfcopy_alias: plan accepted = {}",ops::plan(Kind::Copy,&[a],&alias,&Progress::default()).is_ok());
 let a=r.join("symlinkmerge/src/folder");let b=r.join("symlinkmerge/dst");let outside=r.join("symlinkmerge/outside");
 fs::create_dir_all(&a).unwrap();fs::create_dir_all(&b).unwrap();fs::create_dir_all(&outside).unwrap();
 fs::write(a.join("x"),"new").unwrap();fs::write(outside.join("x"),"outside original").unwrap();symlink(&outside,b.join("folder")).unwrap();
 let out=run(Kind::Copy,a,b);println!("symlink_merge: outside file = {:?}; errors = {:?}",fs::read_to_string(outside.join("x")).unwrap(),out.errors);assert_eq!(fs::read(outside.join("x")).unwrap(),b"new");
 let a=r.join("fifo/src");let b=PathBuf::from(std::env::args().nth(2).unwrap());fs::create_dir_all(&a).unwrap();fs::create_dir_all(&b).unwrap();
 assert_ne!(fs::metadata(&a).unwrap().dev(),fs::metadata(&b).unwrap().dev());
 let fifo=a.join("pipe");assert!(std::process::Command::new("mkfifo").arg(&fifo).status().unwrap().success());
 let out=run(Kind::Move,fifo.clone(),b.clone());println!("cross_device_fifo: source = {}; destination = {}; done = {}; errors = {:?}",fifo.exists(),b.join("pipe").exists(),out.done.len(),out.errors);assert!(!fifo.exists()&&!b.join("pipe").exists());
 let a=r.join("metadata/src");let b=r.join("metadata/dst");fs::create_dir_all(&a).unwrap();fs::create_dir_all(&b).unwrap();fs::write(a.join("x"),"data").unwrap();
 let set=std::process::Command::new("python").args(["-c","import os,sys;os.setxattr(sys.argv[1],b'user.audit',b'value')"]).arg(a.join("x")).status().unwrap();assert!(set.success());
 let out=run(Kind::Copy,a.join("x"),b.clone());assert!(out.errors.is_empty());
 std::process::Command::new("python").args(["-c","import os,sys;print('copied_xattrs:',os.listxattr(sys.argv[1]))"]).arg(b.join("x")).status().unwrap();
 let root=r.join("indexroot");fs::create_dir_all(&root).unwrap();fs::write(root.join("x"),"x").unwrap();let file=r.join("index.efidx");
 ef_index::Index::build(&root).unwrap().save(&file).unwrap();let mut bytes=fs::read(&file).unwrap();bytes[32..40].copy_from_slice(&u64::MAX.to_le_bytes());bytes[40..48].copy_from_slice(&0u64.to_le_bytes());fs::write(&file,bytes).unwrap();
 let panic=std::panic::catch_unwind(|| ef_index::MappedIndex::open(&file).is_ok()).is_err();println!("malformed_index: panicked instead of returning error = {panic}");assert!(panic);
}
