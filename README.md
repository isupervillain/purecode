# PureCode

A fast, language-aware code analysis tool that distinguishes "pure code" from "noise" (comments, whitespace, boilerplates). It can analyze git diffs (for PR reviews) or scan file directories (for codebase stats).

## Features

- **Language Aware**: Distinguishes comments, docstrings, and pure code for over 20 languages.
- **Diff Analysis**: Analyzes git diffs to show the "net pure code" contribution of a change.
- **Snapshot Analysis**: Scans directories to generate codebase statistics.
- **Complexity Metrics**: Calculates a review complexity score based on churn and code type.
- **Unified Output**: Supports Human-readable, Plain text, and JSON formats.
- **CI Friendly**: Strict threshold checking, exit codes, and machine-readable summaries.

## Architecture

PureCode operates in a pipeline:

1. **Parser**: Reads a git diff (Diff Mode) or file contents (Snapshot Mode).
2. **Classifier**: A stateful engine that processes content line-by-line. It detects the language based on file extension and applies language-specific rules to classify each line as `Pure`, `Comment`, `Docstring`, or `Blank`. Comment markers inside string, template and regex literals are ignored; a Python triple-quoted string is a docstring only when it starts a statement (`x = """…` is code); `<script>`/`<style>` blocks in HTML and Vue use C-style rules. A line with any code on it is `Pure`; shebangs count as code.
3. **Stats Aggregator**: Accumulates metrics per file and per language.
4. **Reporter**: Outputs the data in the requested format (Human, JSON, Plain).

## Installation

### Quickstart

Install with a single command (macOS / Linux):

```bash
curl -LsSf https://raw.githubusercontent.com/isupervillain/purecode/main/install.sh | sh
```

For Windows (PowerShell):

```powershell
powershell -ExecutionPolicy ByPass -c "irm https://raw.githubusercontent.com/isupervillain/purecode/main/install.ps1 | iex"
```

### From Source

```bash
cargo install --path .
```

## Usage

### Diff Mode (Default)

Analyzes the changes between two git references.

```bash
# Analyze changes on HEAD since it diverged from main (git diff base...head)
purecode diff --base origin/main --head HEAD

# Shortcut (uses default origin/main -> HEAD)
purecode

# Read diff from stdin
git diff --unified=0 origin/main | purecode diff --stdin
```

### Files Mode (Snapshot)

Analyzes files in the current directory or specified paths.

```bash
# Analyze all files in current directory
purecode files

# Analyze specific directories
purecode files src/ lib/

# Skip more files via .purecode.toml (see Configuration)
```

Files ignored by git (`.gitignore` at any level, `.git/info/exclude`, the global gitignore) or by `.ignore` are skipped, and ignored directories such as build output are never walked. Also skipped: as are binary files, `.git/`, `node_modules`, `target`, `dist` and lock files (`*.lock`, `package-lock.json`, `pnpm-lock.yaml`). Hidden files such as `.github/` workflows are analyzed. Files in an unrecognized language are counted under `Other`, with every non-blank line as pure.

When stdin is used (`purecode files --stdin`), one file path per line is read and include/exclude are not applied.

### Options

- `--format <human|plain|json>`: Output format.
- `--per-file`: Show detailed statistics per file.
- `--max-noise-ratio <0.0-1.0>`: Fail if the noise ratio exceeds this value.
- `--min-pure-lines <N>`: Fail if net pure lines count is less than N.
- `--fail-on-decrease`: Fail if net pure code contribution is negative.
- `--warn-only`: Print validation failures but exit with 0 (useful for non-blocking CI).
- `--ci`: Enable CI mode (no colors, deterministic output, summary lines).

## Configuration

You can configure defaults via a `.purecode.toml` file in your project root:

```toml
[purecode]
base = "origin/main"
format = "human"
max_noise_ratio = 0.6
min_pure_lines = 5
fail_on_decrease = true
warn_only = false
ci = false

include = ["src/**"]
exclude = ["**/*.lock", "**/dist/**", "**/target/**", "**/node_modules/**"]
```

Setting `include` or `exclude` replaces its default list. Keys may also be written at the top level without the `[purecode]` header. Unknown keys, invalid values or unparsable TOML are an error (exit code 1), so a typo cannot silently disable a CI threshold.

CLI flags always override configuration values. `include`/`exclude` are glob patterns matched against paths relative to the working directory (or, for a scanned path outside it, relative to that path); directories above it never match. Use a `**/` prefix to match at any depth.

## Exit Codes

| Code | Meaning |
| ---- | ------- |
| 0 | Success (or threshold failure with `--warn-only`) |
| 1 | Runtime error (e.g. `git diff` failed, bad input) |
| 2 | A threshold check failed, or invalid command-line usage |

## Limitations

Classification is heuristic, line-based, and needs no compiler. Known edge cases:

- A diff hunk shows only part of a file, so a hunk that starts inside a block comment is recognised only by its leading `*` lines (`* text`, `*/`). A hunk whose first line is a wrapped `* operand` is counted as a comment.
- A Python triple-quoted string that starts a line is treated as a docstring, even when it is a call argument.
- Heredocs and code embedded in YAML (`run: |`) are classified by the host language's rules.
- In JavaScript, a regex literal is recognised after an operator, an opening bracket or a keyword such as `return`; elsewhere `/` is division.

## Integration

### Pre-commit Hook

Add to `.pre-commit-config.yaml`:

```yaml
repos:
  - repo: https://github.com/isupervillain/purecode
    rev: v0.2.2
    hooks:
      - id: purecode
        args: ["--format", "human"]
```

### GitHub Actions

Run `purecode` to check PR quality:

```yaml
steps:
  - uses: actions/checkout@v4
    with:
      fetch-depth: 0 # Need history for diff

  - name: Install PureCode
    run: curl -LsSf https://raw.githubusercontent.com/isupervillain/purecode/main/install.sh | sh

  - name: Run Analysis
    run: |
      # Check against the PR base
      purecode --base origin/${{ github.base_ref }} --head HEAD --format human --max-noise-ratio 0.6
```

## Output Formats

### JSON

Use `--format json` for a fully structured output suitable for automated processing:

```json
{
  "summary": {
    "total_added": 120,
    "pure_added": 100,
    ...
  },
  "language_stats": { ... },
  "complexity_score": 145.2,
  "token_estimate": 2340,
  "mode": "diff"
}
```

### CI Mode

Use `--ci` to get machine-readable summary lines at the end of output:

```bash
PURECODE_SUMMARY noise_ratio=0.15 pure_added=100 pure_removed=5 files_changed=8 complexity=145.2
```

On failure:

```bash
PURECODE_FAIL reason=noise_ratio_exceeded noise_ratio=0.62 max_noise_ratio=0.50
```

## Contributing

1. Clone the repository: `git clone https://github.com/isupervillain/purecode`
2. Run tests: `cargo test`
3. Submit a PR.

## License

MIT
