## What changed?

<!-- Briefly describe the change and why it is needed. Link related issues. -->

## Change type

- [ ] New cleaner / cleaner fix
- [ ] CLI / engine behavior
- [ ] GUI
- [ ] Docs / packaging
- [ ] Other

## Safety checklist (required for any change to deletion paths)

- [ ] I inspected the `preview` output against a disposable fixture tree
- [ ] No protected roots touched (`/proc /sys /dev /boot /etc /usr`, home data)
- [ ] `python3 verify_tomls.py` passes (cleaner changes)
- [ ] `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test` pass (Rust changes)
- [ ] `CHANGELOG.md` updated under `[Unreleased]`
- [ ] Docs updated (`docs/`, README, or cleaner-format as applicable)

## Testing

<!-- Commands run + results. Never report a result that was not actually run. -->

```text
# paste command + output summary here
```

## Notes for reviewers

<!-- Risks, trade-offs, follow-ups. For cleaner PRs: which fixture layout was used? -->
