//! Path filters shared by every filesystem-backed action.
//!
//! BleachBit supports four filters (`regex`, `nregex`, `wholeregex`,
//! `nwholeregex`) plus a `type` restriction. Sweep keeps all of those and
//! adds age and size filters, which let a cleaner express intent that
//! previously needed a second, hand-written action:
//!
//! ```toml
//! [[option.action]]
//! command = "delete"
//! search = "walk.files"
//! path = "~/.cache/thumbnails/"
//! max_age_days = 30
//! min_size = "1KB"
//! ```
//!
//! Regexes are compiled once per action instead of once per candidate path;
//! on a `~/.cache` walk with 200k files this is the difference between "fast"
//! and "unusable".

use std::path::Path;
use std::time::{Duration, SystemTime};

use regex::bytes::Regex as BytesRegex;
use regex::Regex;

use crate::core::error::{Error, Result};
use crate::definition::model::{ActionDef, ObjectType};

/// Compiled form of an action's filters.
#[derive(Debug, Clone, Default)]
pub struct ActionFilter {
    regex: Option<Regex>,
    nregex: Option<Regex>,
    wholeregex: Option<BytesRegex>,
    nwholeregex: Option<BytesRegex>,
    object_type: Option<ObjectType>,
    max_age: Option<Duration>,
    min_age: Option<Duration>,
    min_size: Option<u64>,
    max_size: Option<u64>,
}

/// Lossy byte view of a path, used by the whole-path regexes.
///
/// `Path::to_string_lossy()` allocates; for `wholeregex` we want the raw bytes
/// so that non-UTF-8 paths on Unix still match.
fn path_bytes(path: &Path) -> Vec<u8> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        path.as_os_str().as_bytes().to_vec()
    }
    #[cfg(not(unix))]
    {
        path.to_string_lossy().into_owned().into_bytes()
    }
}

fn compile(source: &Option<String>) -> Result<Option<Regex>> {
    match source {
        Some(pattern) => Regex::new(pattern)
            .map(Some)
            .map_err(|source| Error::Regex {
                pattern: pattern.clone(),
                source,
            }),
        None => Ok(None),
    }
}

fn compile_bytes(source: &Option<String>) -> Result<Option<BytesRegex>> {
    match source {
        Some(pattern) => BytesRegex::new(pattern)
            .map(Some)
            .map_err(|source| Error::Regex {
                pattern: pattern.clone(),
                source,
            }),
        None => Ok(None),
    }
}

fn days(days: u32) -> Duration {
    Duration::from_secs(u64::from(days) * 86_400)
}

impl ActionFilter {
    /// Compile the filters declared by `def`.
    pub fn compile(def: &ActionDef) -> Result<Self> {
        Ok(Self {
            regex: compile(&def.regex)?,
            nregex: compile(&def.nregex)?,
            wholeregex: compile_bytes(&def.wholeregex)?,
            nwholeregex: compile_bytes(&def.nwholeregex)?,
            object_type: def.object_type,
            max_age: def.max_age_days.map(days),
            min_age: def.min_age_days.map(days),
            min_size: def.min_size,
            max_size: def.max_size,
        })
    }

    /// A filter that lets everything through.
    pub fn none() -> Self {
        Self::default()
    }

    /// Cheap test that needs no `stat()`: name, whole-path and type filters.
    pub fn accepts_name(&self, path: &Path, is_dir: bool) -> bool {
        if let Some(wanted) = self.object_type {
            match wanted {
                ObjectType::File if is_dir => return false,
                ObjectType::Dir if !is_dir => return false,
                _ => {}
            }
        }

        if let Some(regex) = &self.regex {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            if !regex.is_match(&name) {
                return false;
            }
        }
        if let Some(regex) = &self.nregex {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            if regex.is_match(&name) {
                return false;
            }
        }

        let bytes = if self.wholeregex.is_some() || self.nwholeregex.is_some() {
            path_bytes(path)
        } else {
            Vec::new()
        };
        if let Some(regex) = &self.wholeregex {
            if !regex.is_match(&bytes) {
                return false;
            }
        }
        if let Some(regex) = &self.nwholeregex {
            if regex.is_match(&bytes) {
                return false;
            }
        }

        true
    }

    /// Does this file's age and size pass the filter?
    ///
    /// Called only for the paths that survived [`ActionFilter::accepts_name`],
    /// so the `stat()` cost is paid exactly once per real candidate.
    pub fn accepts_metadata(&self, modified: SystemTime, size: u64) -> bool {
        if let Some(min) = self.min_size {
            if size < min {
                return false;
            }
        }
        if let Some(max) = self.max_size {
            if size > max {
                return false;
            }
        }
        if self.min_age.is_some() || self.max_age.is_some() {
            let age = SystemTime::now()
                .duration_since(modified)
                .unwrap_or(Duration::ZERO);
            if let Some(max) = self.max_age {
                if age < max {
                    return false;
                }
            }
            if let Some(min) = self.min_age {
                if age > min {
                    return false;
                }
            }
        }
        true
    }

    /// Full test: name, type, then metadata.
    pub fn accepts(&self, path: &Path) -> bool {
        let is_dir = path.is_dir();
        if !self.accepts_name(path, is_dir) {
            return false;
        }
        if self.min_size.is_none()
            && self.max_size.is_none()
            && self.min_age.is_none()
            && self.max_age.is_none()
        {
            return true;
        }
        match path.symlink_metadata() {
            Ok(md) => {
                let modified = md.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                self.accepts_metadata(modified, md.len())
            }
            Err(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::definition::model::SearchKind;
    use std::path::PathBuf;

    fn def() -> ActionDef {
        ActionDef {
            command: "delete".into(),
            search: SearchKind::Glob,
            path: "/tmp".into(),
            ..ActionDef::default()
        }
    }

    #[test]
    fn regex_filters() {
        let mut def = def();
        def.regex = Some(r"^.*\.log$".into());
        let filter = ActionFilter::compile(&def).unwrap();

        assert!(filter.accepts_name(Path::new("/tmp/a.log"), false));
        assert!(!filter.accepts_name(Path::new("/tmp/a.txt"), false));
    }

    #[test]
    fn negative_regex_filters() {
        let mut def = def();
        def.nregex = Some(r"^(important|keep)\.log$".into());
        let filter = ActionFilter::compile(&def).unwrap();

        assert!(filter.accepts_name(Path::new("/tmp/a.log"), false));
        assert!(!filter.accepts_name(Path::new("/tmp/keep.log"), false));
    }

    #[test]
    fn type_filter() {
        let mut def = def();
        def.object_type = Some(ObjectType::File);
        let filter = ActionFilter::compile(&def).unwrap();

        assert!(filter.accepts_name(Path::new("/tmp/a"), false));
        assert!(!filter.accepts_name(Path::new("/tmp/a"), true));
    }

    #[test]
    fn size_and_age_filters() {
        let mut def = def();
        def.min_size = Some(1000);
        def.max_age_days = Some(30);
        let filter = ActionFilter::compile(&def).unwrap();

        let old = SystemTime::now() - Duration::from_secs(60 * 60 * 24 * 40);
        assert!(filter.accepts_metadata(old, 2000));
        assert!(!filter.accepts_metadata(old, 10)); // too small
        assert!(!filter.accepts_metadata(SystemTime::now(), 2000)); // too new
    }

    #[test]
    fn whole_path_regex() {
        let mut def = def();
        def.wholeregex = Some(r"/tmp/cache/".into());
        let filter = ActionFilter::compile(&def).unwrap();

        assert!(filter.accepts_name(&PathBuf::from("/tmp/cache/x"), false));
        assert!(!filter.accepts_name(&PathBuf::from("/tmp/other/x"), false));
    }
}
