//! termem — terminal memory: index and resume Claude Code, Codex, and shell
//! sessions by the directory they were started in.
//!
//! Zero token cost: titles are read from data the tools already stored
//! (Claude `ai-title`, Codex `thread_name`) or derived from the first user
//! prompt. termem never calls an LLM.

pub mod index;
pub mod model;
pub mod query;
pub mod resume;
pub mod scan;
pub mod shellhook;
pub mod tui;
