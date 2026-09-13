//! Core value types shared by every layer: errors, paths, safety, reporting.

pub mod error;
pub mod keep;
pub mod path;
pub mod report;
pub mod report_html;
pub mod report_md;

pub use error::{Error, Result};
pub use keep::{builtin_keep_list, is_link, Guard, KeepList};
pub use report::{Entry, EntryKind, Failure, Report, Stopwatch};
