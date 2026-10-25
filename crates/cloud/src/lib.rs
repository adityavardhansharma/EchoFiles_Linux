//! Google Drive through rclone (design/docs/google-drive.md).
//!
//! EchoFiles never talks to Google itself: it runs the user's installed `rclone`, which
//! has its own Google-approved app (or the user's own client ID). One `rclone rcd` child,
//! owned by the app, serves every mount and every call over a private Unix socket.
//!
//! No iced here: plain functions, data and threads, like `ef-net`, so the UI decides
//! threading and `ef` can reuse what it needs.

pub mod paths;
pub mod rclone;
pub mod rc;
pub mod error;
pub mod supervisor;
pub mod uri;
pub mod drive;
pub mod signin;
pub mod catalog;
pub mod crawl;

pub use error::{Error, Kind};
pub use rc::Rc;
pub use rclone::{Availability, Rclone, Version};
pub use uri::DriveUri;
