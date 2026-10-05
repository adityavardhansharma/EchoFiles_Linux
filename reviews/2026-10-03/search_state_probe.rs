use std::{fs,path::PathBuf,sync::{Arc,atomic::AtomicBool},time::SystemTime};
mod indexer {
 use super::*;
 #[derive(Clone)] pub struct RootIndex {pub root:PathBuf,pub map:Option<Arc<ef_index::MappedIndex>>,pub updated:Option<SystemTime>}
 pub fn background_pool()-> &'static rayon::ThreadPool {static P:std::sync::OnceLock<rayon::ThreadPool>=std::sync::OnceLock::new();P.get_or_init(||rayon::ThreadPoolBuilder::new().num_threads(2).build().unwrap())}
}
#[path="../../crates/ui/src/search.rs"] mod search;
fn indexed(root:&std::path::Path,file:&std::path::Path,cfg:&ef_config::SearchConfig)->indexer::RootIndex {
 ef_index::Index::build_with(root,&ef_index::Options::from_search(cfg,root)).unwrap().save(file).unwrap();
 indexer::RootIndex{root:root.into(),map:Some(Arc::new(ef_index::MappedIndex::open(file).unwrap())),updated:Some(SystemTime::now())}
}
fn main(){
 let root=PathBuf::from(std::env::args().nth(1).unwrap());fs::create_dir_all(&root).unwrap();
 let a=root.join("a");let b=root.join("b");fs::create_dir_all(&a).unwrap();fs::create_dir_all(&b).unwrap();for n in 0..300{fs::write(a.join(format!("some-report-{n}")),"").unwrap();}fs::write(b.join("report"),"").unwrap();
 let cfg=ef_config::SearchConfig::default();let maps=vec![indexed(&a,&root.join("a.idx"),&cfg),indexed(&b,&root.join("b.idx"),&cfg)];let found=search::from_index(&maps,"report",false).unwrap();
 println!("multi_root_ranking: exact match shown = {}; total = {}",found.hits.iter().any(|h|h.name=="report"),found.total);assert!(!found.hits.iter().any(|h|h.name=="report"));
 let tagged=root.join("tagged");fs::create_dir_all(tagged.join("cache")).unwrap();fs::write(tagged.join("cache/CACHEDIR.TAG"),"Signature: 8a477f597d28d172789f06886806bc55\n").unwrap();fs::write(tagged.join("cache/secret"),"").unwrap();
 let maps=vec![indexed(&tagged,&root.join("tagged.idx"),&cfg)];let ix=search::from_index(&maps,"secret",true).unwrap();let live=search::live(&[tagged],&cfg,"secret",true,&Arc::new(AtomicBool::new(false))).unwrap();println!("cache_exclusion: indexed = {}; live = {}",ix.total,live.total);assert_eq!((ix.total,live.total),(0,1));
 let private=root.join("private");fs::create_dir_all(&private).unwrap();fs::write(private.join("secret"),"").unwrap();let cfg=ef_config::SearchConfig{roots:vec![private.display().to_string()],exclude_paths:vec![private.display().to_string()],..Default::default()};let maps=vec![indexed(&private,&root.join("private.idx"),&cfg)];let found=search::from_index(&maps,"secret",true).unwrap();println!("excluded_root: secrets still indexed = {}",found.total);assert_eq!(found.total,1);
 // Reproduce saves from the uncoordinated settings path under simultaneous callers.
 let file=root.join("settings.toml");let barrier=Arc::new(std::sync::Barrier::new(16));let mut workers=Vec::new();for n in 0..16 {let file=file.clone();let barrier=barrier.clone();workers.push(std::thread::spawn(move||{let mut s=ef_config::Settings::default();s.sidebar.width=200+n;s.network.servers=(0..300).map(|i|ef_config::Server{uri:format!("sftp://host-{n}-{i}/"),name:None}).collect();barrier.wait();(0..25).filter(|_|s.save_to(&file).is_err()).count()}));}let errors:usize=workers.into_iter().map(|w|w.join().unwrap()).sum();println!("concurrent_settings: save errors = {errors}; final TOML parses = {}",ef_config::Settings::load_from(&file).is_ok());assert!(errors>0);
}
