//! Sweep — a fast, safe, cross-platform system cleaner.
//!
//! This crate is the library behind the `sweep` binary. It is organised as:
//!
//! * [`core`] — errors, path expansion, the keep/protection gate, the report;
//! * [`fsutil`] — traversal, sizing, deletion, free-space wiping;
//! * [`shred`] — secure deletion;
//! * [`platform`] — OS abstraction (dirs, trim, registry, ...);
//! * [`definition`] — cleaner parsing (TOML / CleanerML / winapp2) and lookup;
//! * [`action`] — the action providers and the execution [`action::RunContext`];
//! * [`engine`] — selection, scheduling, deep scan, progress;
//! * [`config`] — persisted configuration.

// Crate içi `sweep::` yolları için öz-başvuru (cli modülü kullanır).
extern crate self as sweep;

pub mod action;
pub mod cli;
pub mod config;
pub mod core;
pub mod deep;
pub mod definition;
pub mod engine;
pub mod fsutil;
pub mod i18n;
pub mod platform;
pub mod shred;

pub use definition::CleanerRegistry;

/// Current version string (from Cargo).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
