//! Filesystem primitives: traversal, sizing, deletion, free-space wiping.

pub mod delete;
pub mod freespace;
pub mod size;
pub mod walk;

pub use delete::{delete, delete_all, truncate_file, DeleteOptions};
pub use size::{bytes_to_human, dir_size, human_to_bytes, size_of, size_of_or_zero};
pub use walk::{children, glob_dirs, glob_paths, scan_paths, ScanOptions};

/// Is `program` reachable through `$PATH`?
///
/// Cheaper and safer than spawning a shell: BleachBit's `exe_exists()` plus
/// `General.resolve_exe()` rolled into one.
pub fn path_exists_in_path(program: &str) -> bool {
    if program.contains(std::path::MAIN_SEPARATOR) || program.contains('/') {
        return std::path::Path::new(program).is_file();
    }

    let Some(paths) = std::env::var_os("PATH") else {
        return false;
    };

    #[cfg(unix)]
    let is_executable = |candidate: &std::path::Path| -> bool {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(candidate)
            .map(|md| md.is_file() && (md.permissions().mode() & 0o111) != 0)
            .unwrap_or(false)
    };

    #[cfg(windows)]
    let is_executable = |candidate: &std::path::Path| -> bool { candidate.is_file() };

    for dir in std::env::split_paths(&paths) {
        let candidate = dir.join(program);
        if is_executable(&candidate) {
            return true;
        }
        #[cfg(windows)]
        {
            for ext in ["exe", "com", "bat", "cmd"] {
                let candidate = dir.join(format!("{program}.{ext}"));
                if candidate.is_file() {
                    return true;
                }
            }
        }
    }
    false
}

/// Run an external command and return `(exit_code, stdout, stderr)`.
pub fn run_command(
    program: &str,
    args: &[&str],
    wait: bool,
) -> std::io::Result<(i32, String, String)> {
    use std::process::{Command, Stdio};

    if wait {
        let output = Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .output()?;
        Ok((
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ))
    } else {
        Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        Ok((0, String::new(), String::new()))
    }
}

/// Like [`run_command`], but runs the program inside `cwd`.
///
/// Some tools operate on the working directory itself rather than on a path
/// argument — `git gc --auto` is the canonical example, where the repository
/// *is* the cwd. Returns the same `(exit_code, stdout, stderr)` triple.
pub fn run_command_in(
    program: &str,
    args: &[&str],
    cwd: &std::path::Path,
    wait: bool,
) -> std::io::Result<(i32, String, String)> {
    use std::process::{Command, Stdio};

    let mut command = Command::new(program);
    command
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null());
    if wait {
        let output = command.output()?;
        Ok((
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ))
    } else {
        command
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        Ok((0, String::new(), String::new()))
    }
}
