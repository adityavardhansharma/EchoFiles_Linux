//! The search box and the path bar: typing, scope, results, and jumping to a typed path.

use std::path::Path;

use ef_config::Scope;
use iced::Task;

use crate::app::*;
use crate::system::{self, Request};

impl App {
    /// Messages from the search box, its results and the path bar.
    pub(crate) fn search_update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Search(q) => {
                let hidden = self.show_hidden;
                let pane = self.pane_mut();
                pane.query = q;
                match pane.scope {
                    Scope::Folder => pane.reorder(hidden),
                    Scope::Everywhere => self.run_search(),
                }
            }
            Message::SearchSubmit => match (self.everywhere(), self.pane().result_cursor) {
                (true, Some(i)) => self.handle(Message::ResultOpen(i)),
                _ => Task::none(),
            },
            Message::SetScope(s) => {
                let hidden = self.show_hidden;
                let pane = self.pane_mut();
                pane.scope = s;
                pane.results = None;
                pane.result_cursor = None;
                let t = pane.reorder(hidden);
                if s == Scope::Everywhere { Task::batch([t, self.run_search()]) } else { t }
            }
            Message::SearchDone { generation, results } => {
                if let Some(pane) = self.tabs.iter_mut().flat_map(|t| t.panes.iter_mut()).find(|p| p.search_generation == generation) {
                    // `None`: the search was cancelled. Errors arrive as results with a note.
                    pane.searching = false;
                    pane.results = None;
                    pane.result_cursor = None;
                    if let Some(r) = results {
                        pane.result_cursor = (!r.hits.is_empty()).then_some(0);
                        pane.results = Some(r);
                    }
                }
                Task::none()
            }
            Message::ResultClick(i) => {
                self.pane_mut().result_cursor = Some(i);
                Task::none()
            }
            Message::ResultOpen(i) => match self.pane().results.as_ref().and_then(|r| r.hits.get(i)).map(|h| h.path.clone()) {
                Some(p) if p.to_string_lossy().starts_with("phone://") => {
                    let directory = self.pane().results.as_ref().and_then(|r| r.hits.get(i)).is_some_and(|h| h.is_dir);
                    self.open_phone_result(&p, directory, false)
                }
                Some(p) => self.open_path(p),
                None => Task::none(),
            },
            Message::ResultReveal(i) => {
                let Some(h) = self.pane().results.as_ref().and_then(|r| r.hits.get(i)) else { return Task::none() };
                if h.path.to_string_lossy().starts_with("phone://") {
                    let p = h.path.clone();
                    return self.open_phone_result(&p, false, true);
                }
                let (Some(parent), Some(name)) = (h.path.parent().map(Path::to_path_buf), h.path.file_name().map(|n| n.to_os_string()))
                else {
                    return Task::none();
                };
                self.pane_mut().scope = Scope::Folder;
                let t = self.go(parent, true);
                self.pane_mut().reveal = vec![name];
                t
            }
            Message::FocusSearch => iced::widget::operation::focus(SEARCH_ID),
            // ---- path bar
            Message::EditPath(on) => {
                if on {
                    let loc = self.pane().location.clone();
                    let s = self.net_uri_for(&loc).unwrap_or_else(|| loc.to_string_lossy().into_owned());
                    self.path_edit = Some(s);
                    Task::batch([iced::widget::operation::focus(PATH_ID), iced::widget::operation::select_all(PATH_ID)])
                } else {
                    self.path_edit = None;
                    Task::none()
                }
            }
            Message::PathDraft(s) => {
                self.path_edit = Some(s);
                Task::none()
            }
            Message::PathSubmit => {
                let Some(raw) = self.path_edit.clone() else { return Task::none() };
                if let Request::Connect(uri) = system::request_for(raw.trim()) {
                    self.path_edit = None;
                    return match ef_net::Address::parse(&uri, ef_net::Protocol::Smb) {
                        Ok(a) => self.connect(a, false),
                        Err(e) => {
                            self.toast_error("Can't go there".into(), e);
                            Task::none()
                        }
                    };
                }
                let resolved = self.resolve_typed_path(raw.trim());
                background(
                    move || resolved.and_then(|p| std::fs::metadata(&p).map(|m| (p, m.is_dir())).map_err(|e| e.to_string())),
                    move |r| Message::PathChecked(raw.clone(), r),
                )
            }
            // ---- subsystems
            _ => Task::none(),
        }
    }
}
