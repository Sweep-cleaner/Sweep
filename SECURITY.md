# Security Policy

Sweep performs destructive operations. Security reports take priority over
everything else. **Do not open a public issue for a vulnerability.**

> Güvenlik politikası (özet): Güvenlik açığını herkese açık issue olarak
> açmayın; aşağıya yazın. Desteklenen sürümler tablosuna bakın.

## Supported versions

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |
| < 0.1   | :x:                |

Only the latest release line receives security fixes.

## Reporting a vulnerability

Email **`nebulawastaken.dev@proton.me`** with:

- affected version (`sweep --version` or commit hash),
- operating system,
- the `preview` output that demonstrates the problem (never run `clean` on
  real data to reproduce a security bug — use a disposable fixture tree),
- steps to reproduce and impact assessment.

Please give the maintainers reasonable time to fix the issue before any public
disclosure. You will receive an acknowledgment.

The following classes are in scope: unintended directory deletion, guard /
protected-path bypass, privilege escalation, untrusted-definition sandbox
escape (`process` / `winreg` running without `--trust-external`), and
report-log data leaks.

## Security model (summary)

- Every deletion passes `Guard` + critical-directory protection + user
  keep-list + symlink / permission checks.
- Critical directories stay protected even with `--force`.
- External cleaner definitions cannot run `process` / `winreg` actions by
  default (untrusted). `--trust-external` is an explicit override, not a
  safety verdict — inspect the source first.
- `preview` is the default flow; `clean` asks for confirmation;
  `--backup-dir` + `undo` provide recovery for recent cleanups.

See [CONTRIBUTING.md](CONTRIBUTING.md#safety-and-security-rules) for the full
safety rules contributors must follow.
