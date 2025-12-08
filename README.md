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
2. **Classifier**: A stateful engine that processes content line-by-line. It detects the language based on file extension and applies language-specific rules (e.g., Python triple-quotes, C-style block comments) to classify each line as `Pure`, `Comment`, `Docstring`, or `Blank`.
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
# Analyze changes between main and HEAD
purecode diff --base origin/main --head HEAD

# Shortcut (uses default origin/main -> HEAD)
purecode

# Read diff from stdin
git diff origin/main | purecode diff --stdin
```

### Files Mode (Snapshot)

Analyzes files in the current directory or specified paths.

```bash
# Analyze all files in current directory
purecode files

# Analyze specific directories
purecode files src/ lib/

# Exclude node_modules (respected by default, but customizable)
purecode files --exclude "**/node_modules/**"
```

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
exclude = ["**/*.lock", "dist/**", "target/**", "node_modules/**"]
```

CLI flags always override configuration values.

## Integration

### Pre-commit Hook

Add to `.pre-commit-config.yaml`:

```yaml
repos:
  - repo: https://github.com/isupervillain/purecode
    rev: v0.2.0
    hooks:
      - id: purecode
        args: ["--stdin", "--format", "human"]
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

1. Clone the repository: `git clone https://github.com/isupervillain/purecode-priv`
2. Run tests: `cargo test`
3. Submit a PR.

## License

MIT
