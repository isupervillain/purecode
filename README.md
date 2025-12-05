# PureCode

`purecode` is a production-grade Rust CLI tool that analyzes **git diffs** to provide meaningful insights into your code changes. Unlike standard line counters, it distinguishes between "pure code" and "noise" (comments, docstrings, and blank lines), helping you understand the real impact of a pull request or commit.

## Features

- **Language-Aware Analysis**: Automatically detects languages and applies specific rules for identifying comments and code.
- **Pure Code Metrics**: Calculates `TOTAL` lines changed and `PURE` lines changed (excluding noise).
- **Flexible Input**:
  - Run directly on git repositories (comparing branches/commits).
  - Pipe unified diffs via stdin (ideal for pre-commit hooks and CI).
- **Single Binary**: Easy to distribute and install.
- **Fast**: Built in Rust for high performance.

## Supported Languages

`purecode` supports detecting and classifying code for:
- Python
- C-style languages (C, C++, Java, C#, JS, TS, Go, PHP, Swift, Kotlin, Scala)
- Shell / PowerShell
- Ruby
- HTML, CSS, Vue (basic support)

## Installation

### From GitHub Releases (Recommended)

Download the pre-compiled binary for your platform from the [Releases page](https://github.com/yourusername/purecode/releases).

1. Download the archive for your OS (Linux, Windows, or macOS).
2. Extract the archive.
3. Move the binary to a directory in your PATH (e.g., `/usr/local/bin`).

**Linux/macOS example:**

```bash
# Download (replace VERSION and PLATFORM)
wget https://github.com/yourusername/purecode/releases/download/v0.1.0/purecode-linux-amd64.tar.gz

# Extract
tar -xvf purecode-linux-amd64.tar.gz

# Install
chmod +x purecode
sudo mv purecode /usr/local/bin/
```

### From Source

Ensure you have Rust installed (version 1.70+ recommended).

```bash
git clone https://github.com/yourusername/purecode.git
cd purecode
cargo install --path .
```

## Usage

### Basic Usage

Compare `origin/main` (default base) with `HEAD` (default head):

```bash
purecode
```

### Specific Commits/Branches

Compare two specific references:

```bash
purecode --base v1.0 --head v2.0
```

### Stdin Mode (CI / Pre-commit)

Pipe a unified diff into `purecode`. This is useful for pre-commit hooks or when you've already generated a diff file.
**Note**: The diff must be generated with `--unified=0` for accurate line counting context.

```bash
git diff --cached --unified=0 --no-color | purecode --stdin
```

## Example Output

```text
=== OVERALL TOTAL (all languages, all lines) ===
TOTAL lines changed : +15            -5             (net 10)

=== PER LANGUAGE ===
Python:
  TOTAL : +10  -2  (net 8)
  PURE  : +8   -1  (net 7)

Rust:
  TOTAL : +5   -3  (net 2)
  PURE  : +5   -3  (net 2)

=== PURE CODE ONLY (all languages) ===
PURE code lines     : +13            -4             (net 9)
```

## Integration

### Pre-commit Hook

Add this to your `.git/hooks/pre-commit` (or use a framework like `pre-commit`):

```bash
#!/bin/sh
# Check purely code changes before committing
git diff --cached --unified=0 --no-color | purecode --stdin
```

### CI Pipeline

You can use `purecode` in your CI to post stats on PRs. Since it accepts stdin, you can easily run it against the changed files in a PR.

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for details on how to set up your development environment and submit PRs.

## License

This project is open source.
