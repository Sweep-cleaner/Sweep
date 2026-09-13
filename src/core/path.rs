//! Path expansion, normalisation and comparison helpers.
//!
//! Cleaner definitions are written by humans for humans: they contain `~`,
//! `%LOCALAPPDATA%`, `$XDG_CACHE_HOME` and mixed separators. Everything in the
//! engine works on fully expanded, canonical-ish [`PathBuf`]s, and this module
//! is the only place that performs the translation.

use std::env;
use std::path::{Component, Path, PathBuf};

use crate::platform;

/// Is path comparison case sensitive on this platform?
pub const fn case_sensitive() -> bool {
    !cfg!(windows)
}

/// Result of one expansion pass.
struct Expanded {
    path: PathBuf,
    /// Every `~`, `$NAME`, `${NAME}` and `%NAME%` reference resolved to a real
    /// value. When this is `false` the path is a fragment, not a location.
    complete: bool,
}

fn expand_inner(input: &str) -> Expanded {
    let mut complete = true;
    if input.is_empty() {
        return Expanded {
            path: PathBuf::new(),
            complete,
        };
    }
    let mut out = String::with_capacity(input.len() + 16);
    let mut chars = input.char_indices().peekable();

    // `for` döngüsü gövdedeki peek/next çağrılarıyla çakışır; while zorunlu.
    #[allow(clippy::while_let_on_iterator)]
    while let Some((idx, ch)) = chars.next() {
        if idx == 0 && ch == '~' {
            match chars.peek() {
                None | Some((_, '/')) => {
                    if let Some(home) = platform::home_dir() {
                        out.push_str(&home.to_string_lossy());
                        continue;
                    }
                    complete = false;
                }
                #[cfg(windows)]
                Some((_, std::path::MAIN_SEPARATOR)) => {
                    if let Some(home) = platform::home_dir() {
                        out.push_str(&home.to_string_lossy());
                        continue;
                    }
                    complete = false;
                }
                _ => {}
            }
        }

        match ch {
            '$' => {
                // ${NAME}
                if chars.peek().map(|(_, c)| *c) == Some('{') {
                    chars.next();
                    let mut name = String::new();
                    let mut closed = false;
                    while let Some((_, c)) = chars.next() {
                        if c == '}' {
                            closed = true;
                            break;
                        }
                        name.push(c);
                    }
                    if closed {
                        match env::var(&name) {
                            Ok(v) => out.push_str(&v),
                            Err(_) => complete = false,
                        }
                        continue;
                    }
                    out.push('$');
                    out.push('{');
                    out.push_str(&name);
                    continue;
                }
                // $NAME
                let mut name = String::new();
                while let Some((_, c)) = chars.peek() {
                    if c.is_ascii_alphanumeric() || *c == '_' {
                        name.push(*c);
                        chars.next();
                    } else {
                        break;
                    }
                }
                if name.is_empty() {
                    out.push('$');
                } else {
                    match env::var(&name) {
                        Ok(v) => out.push_str(&v),
                        Err(_) => complete = false,
                    }
                }
            }
            '%' => {
                // %NAME%
                let mut name = String::new();
                let mut closed = false;
                while let Some((_, c)) = chars.next() {
                    if c == '%' {
                        closed = true;
                        break;
                    }
                    name.push(c);
                }
                if closed {
                    match env::var(&name) {
                        Ok(v) => out.push_str(&v),
                        Err(_) => complete = false,
                    }
                } else {
                    out.push('%');
                    out.push_str(&name);
                }
            }
            '/' if cfg!(windows) => out.push(std::path::MAIN_SEPARATOR),
            '\\' if !cfg!(windows) => out.push(std::path::MAIN_SEPARATOR),
            other => out.push(other),
        }
    }

    Expanded {
        path: normalize(&PathBuf::from(out)),
        complete,
    }
}

/// Expand a leading `~` plus every environment variable in `input`.
///
/// Both POSIX (`$NAME`, `${NAME}`) and Windows (`%NAME%`) syntaxes are accepted
/// on every platform, which keeps a single cleaner definition portable.
/// References that cannot be resolved expand to the empty string.
pub fn expand(input: &str) -> PathBuf {
    expand_inner(input).path
}

/// Like [`expand`], but refuse to guess when a variable is not set.
///
/// `expand("%LOCALAPPDATA%\\Foo")` on a machine with no `LOCALAPPDATA` yields
/// `/Foo` — a syntactically absolute path that points somewhere the cleaner
/// author never intended. Callers that are about to *act* on a path (importers,
/// detectors) should use this instead and treat `None` as "does not apply
/// here".
pub fn expand_defined(input: &str) -> Option<PathBuf> {
    let Expanded { path, complete } = expand_inner(input);
    complete.then_some(path)
}

/// Lexical normalisation: collapse `.`, resolve `..`, drop duplicated separators.
///
/// Deliberately *not* `canonicalize()`: cleaner paths frequently do not exist,
/// and canonicalize() would also resolve symlinks, which would silently
/// redirect a delete at the link target.
pub fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    let mut components: Vec<Component<'_>> = Vec::new();

    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => match components.last() {
                Some(Component::Normal(_)) => {
                    components.pop();
                }
                Some(Component::RootDir) | Some(Component::Prefix(_)) | None => {}
                _ => components.push(component),
            },
            other => components.push(other),
        }
    }

    for component in components {
        out.push(component.as_os_str());
    }

    if out.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        out
    }
}

/// Case-aware path equality.
pub fn path_equal(a: &Path, b: &Path) -> bool {
    path_equal_cs(a, b, case_sensitive())
}

/// Path equality with an explicit case-sensitivity switch.
pub fn path_equal_cs(a: &Path, b: &Path, case_sensitive: bool) -> bool {
    if case_sensitive {
        a == b
    } else {
        a.as_os_str().eq_ignore_ascii_case(b.as_os_str())
    }
}

/// True when `path` is `base` or lives underneath it, without ever touching the
/// filesystem and without being fooled by a sibling that merely shares a
/// textual prefix (`/home/foo` is *not* under `/home/fool`).
pub fn path_starts_with(path: &Path, base: &Path) -> bool {
    path_starts_with_cs(path, base, case_sensitive())
}

/// [`path_starts_with`] with an explicit case-sensitivity switch.
pub fn path_starts_with_cs(path: &Path, base: &Path, case_sensitive: bool) -> bool {
    if path_equal_cs(path, base, case_sensitive) {
        return true;
    }
    let mut path_iter = path.components();
    for base_part in base.components() {
        match path_iter.next() {
            None => return false,
            Some(path_part) => {
                let matches = if case_sensitive {
                    path_part.as_os_str() == base_part.as_os_str()
                } else {
                    path_part
                        .as_os_str()
                        .eq_ignore_ascii_case(base_part.as_os_str())
                };
                if !matches {
                    return false;
                }
            }
        }
    }
    true
}

/// On Windows, convert to an extended-length (`\\?\`) path so that the Win32
/// layer stops applying the 260 character limit. No-op elsewhere.
pub fn extended_path(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        use std::ffi::OsString;
        use std::os::windows::ffi::{OsStrExt, OsStringExt};

        let raw: Vec<u16> = path.as_os_str().encode_wide().collect();
        if raw.starts_with(&[b'\\' as u16, b'\\' as u16, b'?' as u16, b'\\' as u16]) {
            return path.to_path_buf();
        }
        if !path.is_absolute() {
            return path.to_path_buf();
        }
        let mut wide: Vec<u16> = Vec::with_capacity(raw.len() + 4);
        wide.extend_from_slice(&[b'\\' as u16, b'\\' as u16, b'?' as u16, b'\\' as u16]);
        wide.extend_from_slice(&raw);
        PathBuf::from(OsString::from_wide(&wide))
    }
    #[cfg(not(windows))]
    {
        path.to_path_buf()
    }
}

/// Undo [`extended_path`], mainly so log output stays readable.
pub fn extended_path_undo(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    if let Some(rest) = text.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        path.to_path_buf()
    }
}

/// Join two path fragments, tolerating an empty right-hand side.
pub fn join_opt(base: &Path, extra: &str) -> PathBuf {
    if extra.is_empty() {
        base.to_path_buf()
    } else {
        base.join(extra)
    }
}

/// Best-effort "is this an absolute path" test that also understands Windows
/// drive letters when running on Linux (used by the winapp2 importer).
pub fn looks_absolute(path: &str) -> bool {
    if path.starts_with('/') || path.starts_with('\\') {
        return true;
    }
    let bytes = path.as_bytes();
    // `C:\` / `C:/`. The drive must be a real letter: `1:\foo` and `:C\foo`
    // are relative (or nonsense) and must not be mistaken for a rooted path.
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/')
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A rooted path that is spelled correctly on both platforms. Every test
    /// builds its inputs from this instead of bailing out with
    /// `if cfg!(windows) { return; }`, so the module keeps its coverage on
    /// Windows — which is where the safety guarantees matter most.
    fn root() -> &'static str {
        if cfg!(windows) {
            "C:\\"
        } else {
            "/"
        }
    }

    #[test]
    fn case_sensitivity_follows_the_platform() {
        assert_eq!(case_sensitive(), !cfg!(windows));
    }

    #[test]
    fn normalizes_dot_and_dotdot() {
        let r = root();
        assert_eq!(
            normalize(&PathBuf::from(format!("{r}a/b/../c"))),
            PathBuf::from(r).join("a").join("c")
        );
        assert_eq!(
            normalize(&PathBuf::from(format!("{r}a/./b//c"))),
            PathBuf::from(r).join("a").join("b").join("c")
        );
        assert_eq!(normalize(Path::new("")), PathBuf::from("."));
    }

    /// `..` above the root clamps instead of escaping. This is the property
    /// that makes `..` inside a cleaner path safe to resolve lexically.
    #[test]
    fn dotdot_cannot_escape_the_root() {
        let r = root();
        assert_eq!(
            normalize(&PathBuf::from(format!("{r}a/../../c"))),
            PathBuf::from(r).join("c")
        );
        assert_eq!(
            normalize(&PathBuf::from(format!("{r}../c"))),
            PathBuf::from(r).join("c")
        );
    }

    #[test]
    fn starts_with_is_component_wise() {
        let r = root();
        let base = PathBuf::from(format!("{r}home/foo"));
        assert!(path_starts_with(&base.join("x"), &base));
        assert!(path_starts_with(&base, &base));
        // A sibling that merely shares a textual prefix is not inside `base`.
        assert!(!path_starts_with(
            &PathBuf::from(format!("{r}home/fool")),
            &base
        ));
        // ...and the containment must not hold in reverse either.
        assert!(!path_starts_with(&base, &base.join("x")));
        // A trailing separator on the base is not significant.
        assert!(path_starts_with(
            &PathBuf::from(format!("{r}home/foo/bar")),
            &PathBuf::from(format!("{r}home/foo/"))
        ));
    }

    /// The explicit switch must win over the platform default, because the
    /// guard is also asked "would this match on a case-insensitive volume?".
    #[test]
    fn comparison_honours_the_explicit_case_switch() {
        assert!(path_equal_cs(
            Path::new("/Home/Foo"),
            Path::new("/home/foo"),
            false
        ));
        assert!(!path_equal_cs(
            Path::new("/Home/Foo"),
            Path::new("/home/foo"),
            true
        ));
        assert!(path_starts_with_cs(
            Path::new("/home/foo/x"),
            Path::new("/HOME/FOO"),
            false
        ));
        assert!(!path_starts_with_cs(
            Path::new("/home/foo/x"),
            Path::new("/HOME/FOO"),
            true
        ));
    }

    #[test]
    fn expand_replaces_only_a_leading_tilde() {
        let Some(home) = platform::home_dir() else {
            return;
        };
        assert_eq!(expand("~"), home);
        assert_eq!(expand("~/x"), home.join("x"));
        // `~` anywhere else is an ordinary character.
        assert_eq!(expand("~x"), PathBuf::from("~x"));
    }

    /// All three syntaxes are honoured, and a variable that is not set must
    /// make [`expand_defined`] give up rather than return a root-relative
    /// fragment such as `/sub`.
    #[test]
    fn expand_handles_every_variable_syntax() {
        let name = "SWEEP_TEST_PATH_VAR";
        let value = if cfg!(windows) {
            r"C:\sweep-test"
        } else {
            "/sweep-test"
        };
        std::env::set_var(name, value);

        let expected = PathBuf::from(value).join("sub");
        assert_eq!(expand(&format!("${name}/sub")), expected);
        assert_eq!(expand(&format!("${{{name}}}/sub")), expected);
        assert_eq!(expand(&format!("%{name}%/sub")), expected);
        assert_eq!(expand_defined(&format!("%{name}%/sub")), Some(expected));

        std::env::remove_var(name);

        assert_eq!(expand_defined(&format!("${name}/sub")), None);
        // The non-strict form still produces the (meaningless) fragment.
        let sep = std::path::MAIN_SEPARATOR;
        assert_eq!(
            expand(&format!("${name}/sub")),
            PathBuf::from(format!("{sep}sub"))
        );
    }

    /// `%` and `$` are ordinary characters in file names; only a *closed*
    /// reference is a variable.
    #[test]
    fn expand_leaves_unterminated_references_alone() {
        assert_eq!(expand("100%"), PathBuf::from("100%"));
        assert_eq!(expand("50%OFF"), PathBuf::from("50%OFF"));
        assert_eq!(expand("${UNCLOSED"), PathBuf::from("${UNCLOSED"));
    }

    #[test]
    fn looks_absolute_needs_a_real_root() {
        assert!(looks_absolute("/usr/lib"));
        assert!(looks_absolute(r"\Windows"));
        assert!(looks_absolute(r"C:\Windows"));
        assert!(looks_absolute("C:/Windows"));
        assert!(!looks_absolute("Windows"));
        // Drive-relative, not rooted.
        assert!(!looks_absolute("C:Windows"));
        // A digit is not a drive letter.
        assert!(!looks_absolute(r"1:\Windows"));
        assert!(!looks_absolute("C:"));
        assert!(!looks_absolute(""));
    }

    #[test]
    fn join_opt_tolerates_an_empty_suffix() {
        let base = PathBuf::from(format!("{}a", root()));
        assert_eq!(join_opt(&base, ""), base);
        assert_eq!(join_opt(&base, "b"), base.join("b"));
    }

    #[test]
    fn extended_path_helpers_are_harmless_when_not_extended() {
        let plain = PathBuf::from(format!("{}Users/halley", root()));
        assert_eq!(extended_path_undo(&plain), plain);
        // Relative paths are never rewritten, on any platform.
        assert_eq!(
            extended_path(Path::new("relative/x")),
            PathBuf::from("relative/x")
        );
        assert_eq!(
            extended_path_undo(Path::new(r"\\?\C:\Users\halley")),
            PathBuf::from(r"C:\Users\halley")
        );
    }
}
