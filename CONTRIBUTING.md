# Contributing to PureCode

Thank you for your interest in contributing to `purecode`! We want to make this tool the standard for semantic diff analysis.

## Prerequisites

- **Rust**: You will need a stable Rust toolchain installed. We recommend using [rustup](https://rustup.rs/).
- **Git**: For version control.

## Local Development

1.  **Clone the repository**:
    ```bash
    git clone https://github.com/yourusername/purecode.git
    cd purecode
    ```

2.  **Build the project**:
    ```bash
    cargo build
    ```

3.  **Run tests**:
    ```bash
    cargo test
    ```

4.  **Run the tool locally**:
    You can run the CLI against the repo itself to test changes:
    ```bash
    cargo run -- --base HEAD~1 --head HEAD
    ```

## Coding Guidelines

- **Idiomatic Rust**: We strive for clean, idiomatic Rust code (Edition 2021).
- **Formatting**: Please run `cargo fmt` before submitting.
- **Linting**: We use `clippy` to catch common mistakes. Please run `cargo clippy` and address warnings.
    ```bash
    cargo clippy -- -D warnings
    ```
- **Documentation**: Public structs and functions should have documentation comments (`///`).

## Branching and Pull Requests

1.  Create a new branch for your feature or fix. We recommend naming it `feature/your-feature-name` or `fix/issue-description`.
2.  Make your changes.
3.  Add tests for any new logic (especially new classifiers).
4.  Ensure all tests pass.
5.  Push your branch and open a Pull Request against `main`.

## CI/CD

Our GitHub Actions workflow automatically:
- Builds the project on Linux, macOS, and Windows.
- Runs the test suite.
- (On PRs) Generates binary artifacts that you can download to verify behavior on different OSes.

## Reporting Issues

If you find a bug or have a feature request, please open an issue in the GitHub repository describing the problem and how to reproduce it.
