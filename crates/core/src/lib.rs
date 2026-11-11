//! Core of coda-writer: notes are plain markdown files in a folder.
//!
//! This crate has no GUI or Tauri dependency so it can be tested on its own
//! and reused later by the blog renderer and the sync service.

pub mod frontmatter;
pub mod store;

pub use frontmatter::Document;
pub use store::{Error, FsStore, Note, NoteSummary};
