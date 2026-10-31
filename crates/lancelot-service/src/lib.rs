//! What the desktop app and the development bridge share: the commands on one open event, the
//! app settings and the types that cross into the UI. No money logic lives here; everything
//! goes through `lancelot_core`. Independent of Tauri, so it is tested without it.

pub mod api;
pub mod service;
pub mod settings;
pub mod sources;

pub use service::Service;
pub use settings::{AppSettings, Appearance};
pub use sources::{DataSource, DataSourceView, MemoryTokens, PullRequest, Pulled, TokenStore};
