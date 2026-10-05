#![allow(dead_code, unused_imports)]
#[path="../src/style.rs"] mod style;
#[path="../src/kinds.rs"] mod kinds;
#[path="../src/search.rs"] mod search;
#[path="../src/system.rs"] mod system;
#[path="../src/gpu.rs"] mod gpu;
#[path="../src/actions.rs"] mod actions;
#[path="../src/app.rs"] mod app;
#[path="../src/drives.rs"] mod drives;
#[path="../src/file_list.rs"] mod file_list;
#[path="../src/indexer.rs"] mod indexer;
#[path="../src/pane.rs"] mod pane;
#[path="../src/preview.rs"] mod preview;
#[path="../src/thumbs.rs"] mod thumbs;
#[path="../src/widgets.rs"] mod widgets;
#[path="../src/overlay.rs"] mod overlay;
#[path="../src/view.rs"] mod view;
#[path="../src/network.rs"] mod network;
#[path="../src/phone.rs"] mod phone;
#[path="../src/settings.rs"] mod settings;
#[path="../src/phone_view.rs"] mod phone_view;
pub fn since_start_ms() -> f64 {0.0}

#[test]
fn audit_hidden_file_targets() {
 use std::{fs,sync::Arc};
 let root=std::env::temp_dir().join(format!("ef-ui-review-{}",std::process::id()));fs::create_dir_all(&root).unwrap();fs::write(root.join("important.txt"),"keep").unwrap();
 let fd=ef_core::listing::open_dir(&root).unwrap();let listing=ef_core::listing::read_names(&root,&fd).unwrap();let keys=Arc::new(ef_core::sort::NameKeys::build(&listing));
 let mut p=pane::Pane::new(root,ef_config::Scope::Folder);
 p.apply_loaded(Arc::new(pane::Loaded{listing,keys,metadata_ready:true,hidden:0,names_ms:0.0,meta_ms:0.0,sort_ms:0.0}),Arc::new(vec![0]));p.select_only(0);
 p.phone=Some(phone::PhonePage::Hub);println!("phone page still targets: {:?}",p.targets());assert_eq!(p.targets().len(),1);
 p.phone=None;p.scope=ef_config::Scope::Everywhere;p.query="other.txt".into();p.results=Some(search::Results{query:"other.txt".into(),hits:vec![],total:0,from_index:true,index_updated:None});
 println!("empty everywhere results still target: {:?}",p.targets());assert_eq!(p.targets().len(),1);
 p.search_generation=42;p.clear_search();println!("generation after clearing search: {}",p.search_generation);assert_eq!(p.search_generation,42);
}
