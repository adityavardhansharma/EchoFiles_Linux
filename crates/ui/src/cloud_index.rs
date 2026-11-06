//! The Google Drive index (opt-in, Settings → Google Drive): filled in phase 5.

use iced::widget::{container, row, text};
use iced::{Alignment, Element, Length, Task};

use crate::app::{App, Message};
use crate::style::{self, color};
use crate::widgets as w;

impl App {
    /// The switch changed. Off deletes every saved Drive catalog at once.
    pub(crate) fn cloud_index_switched(&mut self, on: bool) -> Task<Message> {
        if !on {
            let _ = std::fs::remove_dir_all(ef_config::cloud_index_dir());
        }
        Task::none()
    }

    pub(crate) fn cloud_index_status(&self) -> Element<'_, Message> {
        let p = &self.palette;
        container(row![w::pill(p, "Off", None), text("Not built yet.").size(style::META).font(style::FONT).color(color(p.ink_muted)).width(Length::Fill)].spacing(style::SPACE_4).align_y(Alignment::Center)).padding([10, 16]).width(Length::Fill).into()
    }
}
