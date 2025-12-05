# PureCode

A Rust CLI tool that analyzes a **git diff** and computes, **per language**, the number of:
- **TOTAL** lines added and removed.
- **PURE** code lines added and removed (excluding comments, docstrings, and blank lines).

## Installation

```bash
cargo install --path .
```

## Usage

### Analyze diff between origin/main and HEAD (default)
```bash
purecode
```

### Analyze diff between specific refs
```bash
purecode --base v1.0 --head v2.0
```

### Use with pipe (e.g., pre-commit hook)
```bash
git diff --cached --unified=0 --no-color | purecode --stdin
```

### Help
```bash
purecode --help
```
