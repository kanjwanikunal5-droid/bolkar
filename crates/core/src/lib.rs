//! Shared core used by every surface (desktop, Android, web).
//!
//! Everything in here is deterministic and offline: no network, no audio, no UI.

/// Product name and id, generated from `/brand.json` at build time.
pub mod brand {
    include!(concat!(env!("OUT_DIR"), "/brand.rs"));
}

pub mod command;
pub mod dictionary;
pub mod hardware;
pub mod hinglish;
pub mod learn;
pub mod llm;
pub mod metrics;
pub mod pipeline;
pub mod snippets;
pub mod summary;
