//! Size accounting and human readable formatting.
//!
//! BleachBit reports `st_blocks * 512` on POSIX because that is what the file
//! actually costs on disk. We do the same and additionally expose the apparent
//! size, so `--json` output can distinguish "freed" from "logical".

use std::fs::Metadata;
use std::path::Path;

use crate::core::error::{Error, Result};
use crate::fsutil::walk::ScanOptions;
use crate::platform;

/// Bytes actually allocated on disk (blocks × 512 on POSIX, `len()` elsewhere).
pub fn allocated_size(md: &Metadata) -> u64 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        md.blocks().saturating_mul(512)
    }
    #[cfg(not(unix))]
    {
        md.len()
    }
}

/// Apparent (logical) file size.
pub fn apparent_size(md: &Metadata) -> u64 {
    md.len()
}

/// Allocated size of `path`, following the BleachBit convention:
/// permission errors count as zero rather than failing the scan.
pub fn size_of(path: &Path) -> Result<u64> {
    let md = match std::fs::symlink_metadata(path) {
        Ok(md) => md,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => return Ok(0),
        Err(err) => return Err(Error::io(path, err)),
    };
    Ok(allocated_size(&md))
}

/// Allocated size of `path`, returning 0 for anything that vanished mid-scan.
pub fn size_of_or_zero(path: &Path) -> u64 {
    size_of(path).unwrap_or_else(|err| {
        if !err.is_benign() {
            log::debug!("size_of({}) failed: {}", path.display(), err);
        }
        0
    })
}

/// Total allocated size of everything inside `dir` (recursively).
pub fn dir_size(dir: &Path) -> u64 {
    tree_size(dir, &ScanOptions::files())
}

/// Free space available to the current user at `path`.
pub fn free_space(path: &Path) -> Result<u64> {
    platform::free_space(path)
}

/// Total capacity of the filesystem holding `path` (best effort).
pub fn total_space(path: &Path) -> Result<u64> {
    platform::total_space(path)
}

const SI: [&str; 7] = ["", "k", "M", "G", "T", "P", "E"];
const IEC: [&str; 7] = ["", "Ki", "Mi", "Gi", "Ti", "Pi", "Ei"];

/// Format a byte count the way BleachBit does (`1.4MB` / `1.3MiB`).
///
/// * `>= 10^9` → 2 decimals
/// * `>= 10^3` → 1 decimal
/// * otherwise → integer
pub fn bytes_to_human(bytes: u64, iec: bool) -> String {
    if bytes == 0 {
        return "0B".into();
    }

    let (prefixes, base): (&[&str], f64) = if iec { (&IEC, 1024.0) } else { (&SI, 1000.0) };

    let decimals = if bytes as f64 >= base.powi(3) {
        2
    } else if bytes as f64 >= base {
        1
    } else {
        0
    };

    let mut value = bytes as f64;
    for prefix in prefixes {
        if value < base {
            return format!("{value:.decimals$}{prefix}B", decimals = decimals);
        }
        value /= base;
    }
    "a lot".to_string()
}

/// Parse `1G`, `500M`, `2.5GiB`, `1024` … into a byte count.
///
/// Used by the `--min-size` / `--max-size` filters and the config file.
pub fn human_to_bytes(text: &str) -> Result<u64> {
    let text = text.trim();
    if text.is_empty() {
        return Err(Error::msg("empty size"));
    }

    let split = text.find(|c: char| !(c.is_ascii_digit() || c == '.'));
    let (number, suffix) = match split {
        Some(idx) => (&text[..idx], text[idx..].trim()),
        None => (text, ""),
    };
    let number: f64 = number
        .parse()
        .map_err(|_| Error::msg(format!("not a number: '{number}'")))?;

    let multiplier: f64 = match suffix.to_lowercase().as_str() {
        "" | "b" => 1.0,
        "k" | "kb" => 1_000.0,
        "kib" => 1024.0,
        "m" | "mb" => 1_000_000.0,
        "mib" => 1024.0 * 1024.0,
        "g" | "gb" => 1_000_000_000.0,
        "gib" => 1024.0 * 1024.0 * 1024.0,
        "t" | "tb" => 1_000_000_000_000.0,
        "tib" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        other => {
            return Err(Error::msg(format!("unknown size suffix '{other}'")));
        }
    };

    Ok((number * multiplier).round() as u64)
}

/// Sum of allocated sizes for a list of paths (parallel).
pub fn total_size(paths: &[std::path::PathBuf]) -> u64 {
    use rayon::prelude::*;
    paths.par_iter().map(|p| size_of_or_zero(p)).sum()
}

/// Recursively sum the size of a tree using the shared scan options.
pub fn tree_size(root: &Path, options: &ScanOptions) -> u64 {
    crate::fsutil::walk::scan_paths(root, options)
        .iter()
        .map(|p| size_of_or_zero(p))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn human_formatting_matches_bleachbit_convention() {
        assert_eq!(bytes_to_human(0, false), "0B");
        assert_eq!(bytes_to_human(999, false), "999B");
        // >= 10^3 gets one decimal, >= 10^9 gets two.
        assert_eq!(bytes_to_human(1_000, false), "1.0kB");
        assert_eq!(bytes_to_human(1_500, false), "1.5kB");
        assert_eq!(bytes_to_human(2_000_000_000, false), "2.00GB");
        // IEC uses 1024 and binary prefixes.
        assert_eq!(bytes_to_human(1_024, true), "1.0KiB");
        assert_eq!(bytes_to_human(1_048_576, true), "1.0MiB");
        assert!(bytes_to_human(u64::MAX, false).ends_with('B'));
    }

    #[test]
    fn parses_si_and_iec_sizes() {
        assert_eq!(human_to_bytes("1024").unwrap(), 1_024);
        assert_eq!(human_to_bytes("1kB").unwrap(), 1_000);
        assert_eq!(human_to_bytes("1kib").unwrap(), 1_024);
        assert_eq!(human_to_bytes("500M").unwrap(), 500_000_000);
        assert_eq!(human_to_bytes("1G").unwrap(), 1_000_000_000);
        // 2.5 * 1024^3 — IEC units are binary, unlike the SI ones above.
        assert_eq!(human_to_bytes("2.5GiB").unwrap(), 2_684_354_560);
        assert_eq!(human_to_bytes("1 T").unwrap(), 1_000_000_000_000);
        assert_eq!(human_to_bytes("42b").unwrap(), 42);
    }

    #[test]
    fn rejects_bad_sizes() {
        for bad in ["", "   ", "abc", "5X", "-1", "1.2.3G"] {
            assert!(
                human_to_bytes(bad).is_err(),
                "'{bad}' should not parse as a size"
            );
        }
    }

    #[test]
    fn rounds_trip_through_the_config_layer() {
        // Config values like `journal_size = "500M"` go through this parser.
        for text in ["0", "1", "500M", "2G"] {
            let bytes = human_to_bytes(text).unwrap();
            assert!(!bytes_to_human(bytes, false).is_empty());
        }
    }

    #[test]
    fn sizes_a_real_file() {
        let dir = std::env::temp_dir().join("sweep_size_test");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("f.bin");
        std::fs::write(&file, vec![0u8; 5000]).unwrap();
        let size = size_of(&file).unwrap();
        // POSIX reports blocks*512, Windows reports len(); both are >= len.
        assert!(size >= 5000, "reported {size} for a 5000 byte file");
        assert_eq!(apparent_size(&std::fs::metadata(&file).unwrap()), 5000);
        assert_eq!(size_of_or_zero(&dir.join("missing")), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn totals_sum_in_parallel() {
        let dir = std::env::temp_dir().join("sweep_total_test");
        let _ = std::fs::create_dir_all(&dir);
        let paths: Vec<PathBuf> = (0..4)
            .map(|i| {
                let p = dir.join(format!("f{i}.bin"));
                std::fs::write(&p, vec![1u8; 2048]).unwrap();
                p
            })
            .collect();
        assert!(total_size(&paths) >= 4 * 2048);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
