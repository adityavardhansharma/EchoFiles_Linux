//! One folder view: location, history, listing, selection and search. A tab holds one pane,
//! or two side by side (design system `DualPane`).

use std::collections::HashSet;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use ef_config::Scope;
use ef_core::listing;
use ef_core::sort::{self, NameKeys, SortBy, SortSpec};
use ef_core::volume::{self, FsInfo};
use ef_core::Listing;
use iced::futures::channel::mpsc;
use iced::Task;

use crate::app::{background, Message};
use crate::file_list;
use crate::search::Results;

/// A directory as far as it has been loaded. Shared with the loader thread through `Arc`.
pub struct Loaded {
    pub listing: Listing,
    pub keys: Arc<NameKeys>,
    pub metadata_ready: bool,
    /// Dotfiles in the listing, counted once on the loader thread (not per frame).
    pub hidden: usize,
    pub names_ms: f64,
    pub meta_ms: f64,
    pub sort_ms: f64,
}

impl std::fmt::Debug for Loaded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Loaded({} entries, metadata: {})", self.listing.len(), self.metadata_ready)
    }
}

/// Listing and search generations are unique across all panes, so a late result finds its
/// pane (or no pane) by number alone.
static NEXT: AtomicU64 = AtomicU64::new(1);

pub fn next_id() -> u64 {
    NEXT.fetch_add(1, Ordering::Relaxed)
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

/// Error text per the design system: name the thing, the reason, and the fix.
fn describe(dir: &std::path::Path, e: &std::io::Error) -> String {
    let name = ef_config::tilde(dir);
    match e.kind() {
        std::io::ErrorKind::PermissionDenied => format!("Couldn't open {name} — you don't have permission to read it."),
        std::io::ErrorKind::NotFound => format!("{name} no longer exists."),
        _ => format!("Couldn't open {name}: {e}"),
    }
}

/// Keep entries whose name contains `query`, ignoring ASCII case.
fn filter(l: &Listing, order: Vec<u32>, query: &str) -> Vec<u32> {
    if query.is_empty() {
        return order;
    }
    let q = query.as_bytes();
    order
        .into_iter()
        .filter(|&i| {
            let n = l.name_bytes(i as usize);
            n.len() >= q.len() && n.windows(q.len()).any(|w| w.eq_ignore_ascii_case(q))
        })
        .collect()
}

pub struct Pane {
    pub id: u64,
    pub location: PathBuf,
    pub back: Vec<PathBuf>,
    pub forward: Vec<PathBuf>,
    pub generation: u64,
    nav_started: Instant,
    pub first_paint_ms: f64,
    pub loaded: Option<Arc<Loaded>>,
    pub order: Arc<Vec<u32>>,
    pub selected: Vec<u64>,
    /// Entries shown dimmed because they're cut to the clipboard.
    pub cut: Vec<u64>,
    pub cursor: Option<usize>,
    pub anchor: Option<usize>,
    /// Select these names once the next listing arrives ("Show in folder", pasted items).
    pub reveal: Vec<OsString>,
    typeahead: (String, Instant),
    pub sort: SortSpec,
    pub query: String,
    pub pending: bool,
    pub skeleton: bool,
    pub fs: Option<FsInfo>,
    pub scope: Scope,
    pub results: Option<Results>,
    pub result_cursor: Option<usize>,
    pub searching: bool,
    pub search_generation: u64,
    pub search_cancel: Arc<AtomicBool>,
    /// Tiles instead of rows (design system `FileGrid`).
    pub grid: bool,
    /// Showing the Drives overview instead of a folder.
    pub drives: bool,
}

impl Pane {
    pub fn new(location: PathBuf, scope: Scope) -> Self {
        Pane {
            id: next_id(),
            fs: volume::fs_info(&location),
            location,
            back: Vec::new(),
            forward: Vec::new(),
            generation: 0,
            nav_started: Instant::now(),
            first_paint_ms: 0.0,
            loaded: None,
            order: Arc::new(Vec::new()),
            selected: Vec::new(),
            cut: Vec::new(),
            cursor: None,
            anchor: None,
            reveal: Vec::new(),
            typeahead: (String::new(), Instant::now()),
            sort: SortSpec::default(),
            query: String::new(),
            pending: true,
            skeleton: false,
            scope,
            results: None,
            result_cursor: None,
            searching: false,
            search_generation: 0,
            search_cancel: Arc::new(AtomicBool::new(false)),
            grid: false,
            drives: false,
        }
    }

    /// Start listing `dir` on the rayon pool: names first, then metadata (build plan §2.2).
    pub fn load(&mut self, dir: PathBuf, show_hidden: bool) -> Task<Message> {
        self.generation = next_id();
        self.nav_started = Instant::now();
        let generation = self.generation;
        let spec = self.sort;
        self.pending = true;
        self.fs = volume::fs_info(&dir);
        let (tx, rx) = mpsc::unbounded();
        let timer_tx = tx.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            let _ = timer_tx.unbounded_send(Message::ShowSkeleton(generation));
        });
        let windows = self.fs.as_ref().is_some_and(|f| f.fs_type.starts_with("ntfs"));
        rayon::spawn(move || {
            let send = |m: Message| {
                let _ = tx.unbounded_send(m);
            };
            let t0 = Instant::now();
            let fd = match listing::open_dir(&dir) {
                Ok(fd) => fd,
                Err(e) => return send(Message::LoadFailed { generation, error: describe(&dir, &e) }),
            };
            let mut names = match listing::read_names(&dir, &fd) {
                Ok(l) => l,
                Err(e) => return send(Message::LoadFailed { generation, error: describe(&dir, &e) }),
            };
            if windows {
                listing::mark_windows_junk(&mut names);
            }
            let names_ms = ms(t0);
            let hidden = (0..names.len()).filter(|&i| names.is_hidden(i)).count();
            let t1 = Instant::now();
            let keys = Arc::new(NameKeys::build(&names));
            let order = Arc::new(sort::order(&names, &keys, spec, show_hidden));
            let sort_ms = ms(t1);
            let mut full = names.clone();
            send(Message::Loaded {
                generation,
                loaded: Arc::new(Loaded { listing: names, keys: keys.clone(), metadata_ready: false, hidden, names_ms, meta_ms: 0.0, sort_ms }),
                order: order.clone(),
            });
            let t2 = Instant::now();
            listing::fill_metadata(&mut full, &fd);
            if windows {
                listing::fill_windows_attributes(&mut full, &fd);
            }
            let hidden = (0..full.len()).filter(|&i| full.is_hidden(i)).count();
            let meta_ms = ms(t2);
            let order = if matches!(spec.by, SortBy::Size | SortBy::Modified) || windows { Arc::new(sort::order(&full, &keys, spec, show_hidden)) } else { order };
            send(Message::Loaded { generation, loaded: Arc::new(Loaded { listing: full, keys, metadata_ready: true, hidden, names_ms, meta_ms, sort_ms }), order });
        });
        Task::run(rx, |m| m)
    }

    /// A listing arrived for this pane.
    pub fn apply_loaded(&mut self, loaded: Arc<Loaded>, order: Arc<Vec<u32>>) -> bool {
        let first = !loaded.metadata_ready;
        let same_dir = self.loaded.as_ref().is_some_and(|l| l.listing.dir == loaded.listing.dir);
        // Keep the cursor and the selection on the same names across a refresh.
        let keep_cursor = same_dir
            .then(|| {
                let pos = self.cursor?;
                let i = *self.order.get(pos)? as usize;
                Some(self.loaded.as_ref()?.listing.name(i).to_os_string())
            })
            .flatten();
        let keep_sel: HashSet<OsString> = if same_dir { self.selected_names().into_iter().collect() } else { HashSet::new() };
        if first {
            self.pending = false;
            self.skeleton = false;
            self.first_paint_ms = ms(self.nav_started);
        }
        self.selected = vec![0; loaded.listing.len().div_ceil(64)];
        self.loaded = Some(loaded);
        self.order = order;
        let reveal = std::mem::take(&mut self.reveal);
        if !reveal.is_empty() {
            self.cursor = None;
            self.anchor = None;
            if !self.select_names(&reveal) {
                // Not listed yet (names phase can miss a brand-new file): try again next time.
                self.reveal = reveal;
            }
        } else {
            let l = &self.loaded.as_ref().unwrap().listing;
            self.cursor = keep_cursor.and_then(|name| self.order.iter().position(|&i| l.name(i as usize) == name));
            if !keep_sel.is_empty() {
                for p in 0..self.order.len() {
                    let i = self.order[p] as usize;
                    if keep_sel.contains(l.name(i)) {
                        file_list::set_bit(&mut self.selected, i, true);
                    }
                }
            }
            if self.cursor.is_none() {
                self.anchor = None;
            }
        }
        first
    }

    /// Select the entries with these names; the cursor goes to the first. False if none found.
    pub fn select_names(&mut self, names: &[OsString]) -> bool {
        let Some(l) = self.loaded.clone() else { return false };
        let want: HashSet<&OsString> = names.iter().collect();
        self.clear_selection();
        let mut first = None;
        for p in 0..self.order.len() {
            let i = self.order[p] as usize;
            if want.contains(&l.listing.name(i).to_os_string()) {
                file_list::set_bit(&mut self.selected, i, true);
                first.get_or_insert(p);
            }
        }
        if let Some(p) = first {
            self.cursor = Some(p);
            self.anchor = Some(p);
        }
        first.is_some()
    }

    pub fn reorder(&self, show_hidden: bool) -> Task<Message> {
        let Some(loaded) = self.loaded.clone() else { return Task::none() };
        let generation = self.generation;
        let spec = self.sort;
        let query = if self.scope == Scope::Folder { self.query.clone() } else { String::new() };
        background(
            move || {
                let order = sort::order(&loaded.listing, &loaded.keys, spec, show_hidden);
                filter(&loaded.listing, order, &query)
            },
            move |order| Message::Reordered { generation, order: Arc::new(order) },
        )
    }

    pub fn entry_at(&self, pos: usize) -> Option<usize> {
        self.order.get(pos).map(|&i| i as usize)
    }

    pub fn set_bit(&mut self, i: usize, on: bool) {
        file_list::set_bit(&mut self.selected, i, on);
    }

    pub fn clear_selection(&mut self) {
        self.selected.iter_mut().for_each(|w| *w = 0);
    }

    pub fn select_only(&mut self, pos: usize) {
        self.clear_selection();
        if let Some(i) = self.entry_at(pos) {
            self.set_bit(i, true);
        }
        self.cursor = Some(pos);
        self.anchor = Some(pos);
    }

    pub fn select_all(&mut self) {
        for p in 0..self.order.len() {
            let i = self.order[p] as usize;
            self.set_bit(i, true);
        }
    }

    /// Selected entries in display order.
    pub fn selected_entries(&self) -> Vec<usize> {
        self.order.iter().map(|&i| i as usize).filter(|&i| file_list::is_selected(&self.selected, i)).collect()
    }

    pub fn selected_names(&self) -> Vec<OsString> {
        let Some(l) = self.loaded.as_ref() else { return Vec::new() };
        self.selected_entries().into_iter().map(|i| l.listing.name(i).to_os_string()).collect()
    }

    /// What an action applies to: the selection, else the entry under the cursor.
    pub fn targets(&self) -> Vec<PathBuf> {
        let Some(l) = self.loaded.as_ref() else { return Vec::new() };
        let sel = self.selected_entries();
        if !sel.is_empty() {
            return sel.into_iter().map(|i| l.listing.path(i)).collect();
        }
        self.cursor.and_then(|c| self.entry_at(c)).map(|i| vec![l.listing.path(i)]).unwrap_or_default()
    }

    /// (files, folders, bytes of the files) selected.
    pub fn selection_stats(&self) -> (usize, usize, u64) {
        let Some(l) = self.loaded.as_ref().map(|l| &l.listing) else { return (0, 0, 0) };
        let (mut files, mut dirs, mut bytes) = (0, 0, 0);
        for (wi, &w) in self.selected.iter().enumerate() {
            let mut bits = w;
            while bits != 0 {
                let i = wi * 64 + bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if l.is_dir(i) {
                    dirs += 1;
                } else {
                    files += 1;
                    bytes += l.size[i];
                }
            }
        }
        (files, dirs, bytes)
    }

    pub fn jump_to_prefix(&mut self, c: &str) {
        let now = Instant::now();
        if now.duration_since(self.typeahead.1) > Duration::from_millis(900) {
            self.typeahead.0.clear();
        }
        self.typeahead.0.push_str(&c.to_lowercase());
        self.typeahead.1 = now;
        let Some(l) = self.loaded.as_ref().map(|l| &l.listing) else { return };
        let prefix = self.typeahead.0.as_bytes();
        if let Some(pos) = self.order.iter().position(|&i| {
            let n = l.name_bytes(i as usize);
            n.len() >= prefix.len() && n[..prefix.len()].eq_ignore_ascii_case(prefix)
        }) {
            self.select_only(pos);
        }
    }

    pub fn everywhere(&self) -> bool {
        self.scope == Scope::Everywhere && !self.query.trim().is_empty()
    }

    pub fn clear_search(&mut self) {
        self.query.clear();
        self.results = None;
        self.result_cursor = None;
        self.searching = false;
        self.search_cancel.store(true, Ordering::Relaxed);
    }

    /// Short label for tabs and the window title.
    pub fn title(&self) -> String {
        if self.drives {
            return "Drives".into();
        }
        if self.location == ef_config::home() {
            return "Home".into();
        }
        self.location.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "Computer".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ef_core::ops::{self, Kind, Progress, Resolution};
    use std::collections::HashMap;
    use std::fs;

    #[test]
    fn multiple_selected_files_copy_move_and_delete_together() {
        let root = std::env::temp_dir().join(format!("echofiles-bulk-{}-{}", std::process::id(), next_id()));
        let source = root.join("source");
        let copied = root.join("copied");
        let moved = root.join("moved");
        for dir in [&source, &copied, &moved] {
            fs::create_dir_all(dir).unwrap();
        }
        for name in ["first.txt", "second.txt", "untouched.txt"] {
            fs::write(source.join(name), name).unwrap();
        }
        let listing = listing::list(&source).unwrap();
        let keys = Arc::new(NameKeys::build(&listing));
        let mut pane = Pane::new(source.clone(), Scope::default());
        pane.order = Arc::new(sort::order(&listing, &keys, SortSpec::default(), true));
        pane.selected = vec![0; listing.len().div_ceil(64)];
        for i in 0..listing.len() {
            if listing.name(i) != "untouched.txt" {
                pane.set_bit(i, true);
            }
        }
        pane.loaded = Some(Arc::new(Loaded {
            listing, keys, metadata_ready: true, hidden: 0,
            names_ms: 0.0, meta_ms: 0.0, sort_ms: 0.0,
        }));
        // A marquee has no keyboard cursor. Menu and shortcut actions use these targets.
        assert!(pane.cursor.is_none());
        let selected = pane.targets();
        assert_eq!(selected, vec![source.join("first.txt"), source.join("second.txt")]);
        let progress = Progress::default();
        let plan = ops::plan(Kind::Copy, &selected, &copied, &progress).unwrap();
        let outcome = ops::execute(&plan, &HashMap::new(), Resolution::Skip, false, &progress);
        assert!(outcome.errors.is_empty());
        assert_eq!(outcome.done.len(), 2);
        for name in ["first.txt", "second.txt"] {
            assert_eq!(fs::read(copied.join(name)).unwrap(), name.as_bytes());
            assert!(source.join(name).exists());
        }
        assert!(!copied.join("untouched.txt").exists());
        let progress = Progress::default();
        let plan = ops::plan(Kind::Move, &selected, &moved, &progress).unwrap();
        let outcome = ops::execute(&plan, &HashMap::new(), Resolution::Skip, false, &progress);
        assert!(outcome.errors.is_empty());
        assert_eq!(outcome.done.len(), 2);
        assert!(selected.iter().all(|p| !p.exists()));
        let destinations: Vec<_> = outcome.done.into_iter().map(|(_, to)| to).collect();
        assert!(ops::delete(&destinations, &Progress::default()).is_empty());
        assert!(destinations.iter().all(|p| !p.exists()));
        assert!(source.join("untouched.txt").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
