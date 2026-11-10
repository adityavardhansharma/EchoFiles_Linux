//! The search index as the app drives it: when to crawl, what to crawl, and what to do
//! with a finished crawl or a change in a watched folder. The crawling itself lives in
//! `indexer.rs`.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use iced::Task;

use crate::app::{App, Message, Mode, background};
use crate::indexer::{self, IndexState, RootIndex};

impl App {
    /// Crawl every indexed folder (first run, settings changed).
    pub(crate) fn build_index(&mut self) -> Task<Message> {
        self.build_index_in(indexer::Scope::all())
    }

    /// Re-crawl only the watched folders that changed, in the roots holding them.
    pub(crate) fn refresh_index(&mut self) -> Task<Message> {
        let folders = std::mem::take(&mut self.index_changed);
        if folders.is_empty() {
            self.index_stale = None;
            return Task::none();
        }
        let scope = indexer::Scope::changed(folders, &self.settings.search.roots());
        // Changes outside every indexed root (an open folder elsewhere) need no crawl.
        if scope.roots.as_ref().is_some_and(Vec::is_empty) {
            self.index_stale = None;
            return Task::none();
        }
        self.build_index_in(scope)
    }

    /// The periodic safety net for changes no watch saw: the configured roots, plus the
    /// mounted drives not crawled for 6 hours (so an idle disk is left to spin down).
    pub(crate) fn safety_net_index(&mut self) -> Task<Message> {
        let configured: Vec<PathBuf> = self.settings.search.roots.iter().map(|r| ef_config::expand(r)).collect();
        let old = |root: &PathBuf| {
            self.roots
                .iter()
                .find(|r| &r.root == root)
                .and_then(|r| r.updated)
                .and_then(|t| t.elapsed().ok())
                .is_none_or(|age| age >= Duration::from_secs(6 * 3600))
        };
        let roots = self.settings.search.roots().into_iter().filter(|r| configured.contains(r) || old(r)).collect();
        self.build_index_in(indexer::Scope::roots(roots))
    }

    pub(crate) fn build_index_in(&mut self, scope: indexer::Scope) -> Task<Message> {
        if !self.settings.search.index || self.index_state == IndexState::Building {
            return Task::none();
        }
        self.index_state = IndexState::Building;
        self.index_dirty = None;
        self.index_stale = None;
        if scope.roots.is_none() {
            self.index_changed.clear();
        }
        self.last_index_build = Some(Instant::now());
        let generation = self.index_generation;
        let cfg = self.settings.search.clone();
        background(move || indexer::build_all(&cfg, generation, &scope), move |roots| Message::IndexBuilt(generation, roots))
    }

    /// Settings that change what's indexed: rebuild soon (debounced).
    pub(crate) fn index_settings_changed(&mut self) {
        self.index_generation = indexer::invalidate();
        if self.settings.search.index {
            self.index_dirty = Some(Instant::now());
        }
    }

    pub(crate) fn set_index_enabled(&mut self, on: bool) -> Task<Message> {
        self.index_generation = indexer::invalidate();
        self.settings.search.index = on;
        if on {
            self.index_state = IndexState::Opening;
            let generation = self.index_generation;
            let cfg = self.settings.search.clone();
            background(move || indexer::open_existing(&cfg), move |roots| Message::IndexOpened(generation, roots))
        } else {
            self.index_state = IndexState::Off;
            self.roots.clear();
            self.index_error = None;
            let cfg = self.settings.search.clone();
            let generation = self.index_generation;
            std::thread::spawn(move || indexer::remove_files(&cfg, generation));
            Task::none()
        }
    }

    /// The once-a-second (once every 30 s in the background) check: rebuild after a
    /// settings change has settled, refresh changed folders at most once a minute.
    pub(crate) fn index_tick(&mut self) -> Task<Message> {
        if indexer::settled(self.index_dirty, 1500) && self.index_state != IndexState::Building {
            return self.build_index();
        }
        if indexer::settled(self.index_stale, 10_000)
            && self.index_state != IndexState::Building
            && self.last_index_build.is_none_or(|t| t.elapsed() >= Duration::from_secs(60))
        {
            return self.refresh_index();
        }
        Task::none()
    }

    pub(crate) fn on_index_opened(&mut self, generation: u64, roots: Vec<RootIndex>) -> Task<Message> {
        if generation != self.index_generation {
            return Task::none();
        }
        if !self.settings.search.index {
            return Task::none();
        }
        let stale =
            roots.iter().any(|r| r.map.is_none() || r.updated.is_none_or(|t| t.elapsed().map_or(true, |d| d > Duration::from_secs(60))));
        self.roots = roots;
        self.index_state = if self.roots.iter().any(|r| r.map.is_some()) { IndexState::Ready } else { IndexState::Building };
        if stale {
            self.index_state = IndexState::Ready;
            return self.build_index();
        }
        Task::none()
    }

    pub(crate) fn on_index_built(&mut self, generation: u64, mut roots: Vec<RootIndex>) -> Task<Message> {
        if generation != self.index_generation {
            if !self.settings.search.index {
                return Task::none();
            }
            self.index_state = IndexState::Ready;
            return self.build_index();
        }
        for r in &mut roots {
            if r.map.is_none()
                && let Some(old) = self.roots.iter().find(|old| old.root == r.root)
            {
                r.map = old.map.clone();
                r.updated = old.updated;
            }
        }
        if !self.settings.search.index {
            return Task::none();
        }
        self.index_error = roots.iter().find_map(|r| r.error.clone());
        self.roots = roots;
        self.index_state = IndexState::Ready;
        if self.everywhere() {
            return self.run_search();
        }
        Task::none()
    }

    pub(crate) fn on_fs_changed(&mut self, change: indexer::Change) -> Task<Message> {
        if self.preview_want.as_ref().is_some_and(|p| p.parent() == Some(change.folder.as_path())) {
            self.preview_want = None;
            self.preview_generation += 1;
        }
        if change.names_changed && self.settings.search.index {
            // The settings didn't change: don't invalidate the index, just refresh the
            // folder that changed. An empty folder means the watch overflowed.
            if change.folder.as_os_str().is_empty() {
                self.index_dirty.get_or_insert_with(Instant::now);
            } else {
                if !self.index_changed.contains(&change.folder) {
                    self.index_changed.push(change.folder.clone());
                }
                self.index_stale.get_or_insert_with(Instant::now);
            }
        }
        if !(change.visible || self.show_hidden) || self.mode != Mode::Files {
            return Task::none();
        }
        let ids: Vec<u64> =
            self.tabs[self.tab].panes.iter().filter(|p| p.location == change.folder && !p.special()).map(|p| p.id).collect();
        Task::batch(ids.into_iter().map(|id| self.reload_pane(id)))
    }
}
