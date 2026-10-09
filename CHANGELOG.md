# Changelog

## 0.3.0

### Changed

- Diff mode classifies changed lines in the context of their whole file (read from git), so lines added inside an existing docstring or block comment count as noise.
- Default and configured `include`/`exclude` now apply in diff mode too; lock files, `dist/`, `target/` and `node_modules/` no longer count as code. Thresholds based on earlier numbers may need adjusting.
- An invalid `.purecode.toml` (unknown key, bad value, invalid glob, unparsable TOML) is an error (exit 1) instead of silently falling back to defaults. Keys may be written at the top level or under `[purecode]`.
- `.purecode.toml` is found from subdirectories (searched up to the repository root).
- Files mode respects `.gitignore`, skips binary and lock files, and decodes non-UTF-8 files instead of skipping them.
- `--stdin` cannot be combined with `--base`/`--head`, and `--staged` with neither; `--max-noise-ratio` must be between 0.0 and 1.0; in globs `*` no longer matches across `/`.
- Submodule and symlink changes are not counted.
- Building from source requires Rust 1.88 (declared as `rust-version`).
- With `--format json --ci`, `PURECODE_FAIL` is no longer appended to stdout, so the output stays valid JSON.

### Added

- `purecode diff --staged`, used by the pre-commit hook.
- `--no-config`, so a CI gate cannot be relaxed by the analyzed change's `.purecode.toml`.
- A warning when `.gitattributes` makes git hide a source file as binary.
- Releases publish `SHA256SUMS`; both installers verify the download against it.
- Languages: `.cjs`, `.mts`, `.cts`, `.pyi`, case-insensitive extensions, `makefile`/`GNUmakefile`, PowerShell block comments.

### Fixed

- Lines that look like diff headers (`-- x`, `++i;`) are counted; a text file followed by a binary file is no longer dropped; renames, quoted non-ASCII paths, CRLF and non-UTF-8 diffs are handled.
- Comment markers inside strings, template literals, regex literals and raw strings no longer turn code into comments; Python data strings are no longer counted as docstrings; `<script>`/`<style>` in HTML and Vue are classified as code.
- `git diff` runs with fixed options, so an external diff tool, textconv, `diff.noprefix` or `diff.relative` cannot change the result; refs starting with `-` are rejected.
- Control and bidi characters in file names and error messages are escaped; `.purecode.toml` must be a regular file and its parse errors no longer quote its contents.
- Crafted input can no longer exhaust CPU or memory: regex detection is bounded, whole-file context is limited to files up to 4 MiB, and Files Mode skips files over 32 MiB.
- git runs with fsmonitor disabled; release builds use `--locked`, pinned actions and least-privilege tokens, and releases stay drafts until their assets are uploaded; releases are cut only from merges into `main`, and an unfinished draft from a failed run is recreated.
- `--version` reports the real version; the pre-commit hook analyzes staged changes; the release workflow builds the Intel macOS asset again.
