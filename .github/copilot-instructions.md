# EC2 SSM Connect - AI Coding Instructions

## Project Overview
EC2 SSM Connect (v3.0) is a high-performance Rust CLI tool for managing AWS EC2 Session Manager connections. It prioritizes reliability (auto-reconnect), observability (diagnostics/metrics), and usability (TUI).

## Architectural Boundaries
- **Entry Point**: `src/main.rs` parses arguments via `clap` and initializes the runtime.
- **Session Core**: `src/session.rs` manages the lifecycle of a single SSM connection. `src/manager.rs` orchestrates these sessions.
- **Diagnostics Engine**: Located in `src/diagnostic/`. Allows modular checks (IAM, Network, Agent) and provides `FixSuggestion`s via `src/suggestion_generator.rs`.
- **UI Layer**: Built with `ratatui`. `src/ui.rs` (single session) and `src/multi_session_ui.rs` (dashboard). Separation of update logic and rendering is critical.
- **AWS Abstraction**: `src/aws.rs` wraps `aws-sdk-rust` clients. All AWS interaction should go through this layer to mock/stub for tests.

## Development Workflows
- **Build**:
  - `cargo build --release` for production binaries.
  - Use `cargo check` for rapid feedback.
- **Testing**:
  - Unit Tests: `cargo test`
  - Benchmarks: `./run_performance_tests.sh` is the source of truth for performance regressions (latency/memory).
- **Running**:
  - Use `run.sh` to wrapper environment setup.
  - `cargo run -- connect --target <name>` for quick checks.

## Key Conventions & Patterns
- **Feature Flags**: The codebase regularly uses `cfg(feature = "...")` (e.g., `multi-session`, `performance-monitoring`). Always check if your changes affect feature-gated code blocks.
- **Error Handling**:
  - Library Code: Use `thiserror` in `src/error.rs` to define `Ec2ConnectError`.
  - Application Code: Use `anyhow::Result` for flexibility in `main` and command handlers.
- **Async Runtime**: Built on `tokio`. Avoid blocking I/O in async functions.
- **Configuration**:
  - Defined in `src/config.rs`.
  - Uses `serde` for serialization.
  - Defaults must be robust; handle missing fields gracefully.

## Critical Files
- `src/config.rs`: Configuration structs and defaults.
- `src/error.rs`: Centralized error definitions.
- `src/lib.rs`: Public API exports and module structure.
- `Cargo.toml`: Dependency versions and feature definitions.
