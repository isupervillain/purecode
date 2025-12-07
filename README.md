# PureCode

`purecode` is a production-grade Rust CLI tool that analyzes **git diffs** to provide meaningful insights into your code changes. Unlike standard line counters, it distinguishes between "pure code" and "noise" (comments, docstrings, and blank lines), helping you understand the real impact of a pull request or commit.

## Features

- **Language-Aware Analysis**: Automatically detects languages and applies specific rules for identifying comments and code.
- **Pure Code Metrics**: Calculates `TOTAL` lines changed and `PURE` lines changed (excluding noise).
- **Rich Stats**: Tracks added/removed words, estimated tokens, and breaks down comments vs docstrings.
- **Flexible Input**:
  - Run directly on git repositories (comparing branches/commits).
  - Pipe unified diffs via stdin (ideal for pre-commit hooks and CI).
- **Thresholds**: Gate CI pipelines by enforcing maximum noise ratios or minimum pure code requirements.
- **Fast**: Built in Rust for high performance.
- **Multiple Formats**: Output Human-readable (colorful), Plain text, or JSON.

## Supported Languages

`purecode` supports detecting and classifying code for:
- Python (distinguishes Docstrings vs Comments)
- C-style languages (C, C++, Java, C#, JS, TS, Go, PHP, Swift, Kotlin, Scala)
- Shell / PowerShell
- Ruby
- HTML, CSS, Vue (basic support)

## Quickstart

Install with a single command (macOS / Linux):

```bash
curl -LsSf https://raw.githubusercontent.com/isupervillain/purecode/main/install.sh | sh
```

For Windows (PowerShell):

```powershell
powershell -ExecutionPolicy ByPass -c "irm https://raw.githubusercontent.com/isupervillain/purecode/main/install.ps1 | iex"
```

Run in a git repository:

```bash
# Compare local changes against main
purecode

# OR pipe a diff manually
git diff --cached --unified=0 --no-color | purecode --stdin
```

## How It Works

1.  **Diff Parsing**: `purecode` takes a unified diff (standard git output). It ignores metadata and focuses on lines starting with `+` or `-`.
2.  **Language Detection**: It identifies the file type for each hunk (e.g., `.py`, `.rs`, `.js`).
3.  **Classification**:
    *   **Pure Code**: Logic, variable definitions, function calls.
    *   **Noise**: Comments (`//`, `#`), Docstrings (`"""`, `/**`), and Blank lines.
4.  **Aggregation**: It sums up these metrics to show you the "Net Pure Code" added or removed.

*Note*: For accurate results, always use `--unified=0` when piping diffs. This ensures context lines aren't counted as "noise" or "code".

## Usage

### Basic Usage

Compare `origin/main` (default base) with `HEAD` (default head):

```bash
purecode
```

### Specific Commits/Branches

```bash
purecode --base v1.0 --head v2.0
```

### Output Formats

```bash
# Default (Colorful with emojis)
purecode --format human

# Plain (CI friendly, no colors)
purecode --format plain

# JSON (Machine readable)
purecode --format json
```

### Thresholds & CI Gates

Fail the command if the PR is "too noisy" or decreases code volume:

```bash
# Fail if more than 50% of changes are comments/blanks
purecode --max-noise-ratio 0.5

# Fail if net pure lines < 10 (ensure significant contribution)
purecode --min-pure-lines 10

# Fail if net pure code is negative
purecode --fail-on-decrease
```

## Integration

### Pre-commit Hook

Add to `.pre-commit-config.yaml`:

```yaml
repos:
  - repo: https://github.com/isupervillain/purecode
    rev: v0.1.0
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

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for details.

## License

This project is open source.
