//! Network places: Windows shares (SMB), SSH servers (SFTP) and FTP servers.
//!
//! [`address`] parses what people type, [`gvfs`] connects through GVfs (the same engine
//! Nautilus uses) and exposes each connection as a folder, [`discover`] finds servers
//! announcing themselves on the local network.

pub mod address;
pub mod discover;
pub mod gvfs;

pub use address::{Address, Protocol};
pub use discover::Nearby;
pub use gvfs::{Ask, Error, Login, LoginAsk, Mount, QuestionAsk, Remember};
