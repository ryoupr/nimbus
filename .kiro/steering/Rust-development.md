---
inclusion: always
---

# Rust Development Guidelines

## Build & Test Workflow

**"Check First, Build Last" Strategy** - Optimize development cycle by minimizing full builds.

### Development Phase (MUST follow)

- **Syntax/Type Checking**: Use `cargo check` exclusively during development
  - NEVER use `cargo build` until final verification
  - `cargo check` is 10x faster and sufficient for most development iterations

- **Logic Verification**: Run targeted tests only

  ```bash
  cargo test <test_name>           # Single test
  cargo test <module>::<test_name> # Specific module test
  ```

  - Avoid `cargo test` without arguments during development

- **Diagnostics**: Use `getDiagnostics` tool for immediate feedback on syntax/type errors

### Final Verification Phase

- **Build Verification**: Only after all checks pass

  ```bash
  cargo build --release  # Production build
  cargo run -- [ARGS]    # Test execution
  ```

## Code Optimization Patterns

**Goal**: Minimize compilation time without sacrificing runtime performance where it matters.

### Monomorphization Control

**Problem**: Generics create separate code copies for each concrete type, increasing compile time exponentially.

**Solution**: Use dynamic dispatch except in hot paths.

```rust
// ❌ Avoid: Creates code for every T
pub fn process<T: AsRef<str>>(items: Vec<T>) {
    for item in items {
        // complex logic here
    }
}

// ✅ Prefer: Single code path
pub fn process(items: &[&dyn AsRef<str>]) {
    for item in items {
        // complex logic here
    }
}

// ✅ Or use trait objects
pub fn process(items: Vec<Box<dyn AsRef<str>>>) { ... }
```

### Inner Function Pattern

**When**: Public API requires generics for ergonomics.

**Pattern**: Delegate to non-generic inner function.

```rust
// Public API: Generic for convenience
pub fn heavy_logic<T: AsRef<Path>>(path: T) -> Result<()> {
    heavy_logic_inner(path.as_ref())
}

// Implementation: Non-generic, compiled once
fn heavy_logic_inner(path: &Path) -> Result<()> {
    // All logic here
}
```

### Dependency Management

**Adding Dependencies**:

```toml
# ✅ Minimal features
serde = { version = "1.0", default-features = false, features = ["derive"] }

# ❌ Avoid: Pulls unnecessary dependencies
serde = "1.0"
```

**Procedural Macros**: Use sparingly (they slow compilation significantly)

- `#[derive(Serialize, Deserialize)]`: OK for data structures
- `#[tokio::main]`: OK for entry points
- Custom proc macros: Avoid unless critical

### Type Inference Assistance

**Complex Chains**: Add explicit type annotations to reduce compiler inference work.

```rust
// ❌ Compiler must infer through entire chain
let result = data.iter().filter(|x| x.is_valid()).map(|x| x.process()).collect();

// ✅ Help the compiler
let result: Vec<ProcessedData> = data
    .iter()
    .filter(|x| x.is_valid())
    .map(|x| x.process())
    .collect();
```

## Error Handling Patterns

**This Project Uses**:

- `anyhow::Result<T>` for application errors
- `thiserror` for custom error types
- Context propagation with `.context()` or `.with_context()`

```rust
use anyhow::{Context, Result};

pub fn load_config(path: &Path) -> Result<Config> {
    let content = fs::read_to_string(path)
        .context("Failed to read config file")?;
    
    serde_json::from_str(&content)
        .with_context(|| format!("Invalid JSON in {}", path.display()))
}
```

## Async Patterns

**This Project Uses**: Tokio runtime with full features.

```rust
// ✅ Async functions
pub async fn connect_session(instance_id: &str) -> Result<Session> {
    let client = aws_sdk_ssm::Client::new(&aws_config::load_from_env().await);
    // ...
}

// ✅ Spawning tasks
tokio::spawn(async move {
    monitor_session(session_id).await
});
```

## Configuration Verification

**Before suggesting changes**, verify current setup:

1. Check `.cargo/config.toml` for linker configuration
2. Check `Cargo.toml` for profile settings
3. Only suggest if missing or suboptimal

### Expected Linker Configuration

```toml
# .cargo/config.toml
[target.x86_64-unknown-linux-gnu]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=mold", "-Zshare-generics=y"]

[target.x86_64-apple-darwin]
rustflags = ["-C", "link-arg=-fuse-ld=lld"]

[target.x86_64-pc-windows-msvc]
rustflags = ["-C", "link-arg=-fuse-ld=lld"]
```

### Expected Profile Configuration

```toml
# Cargo.toml
[profile.dev]
opt-level = 0
debug = 0  # Reduce link time
strip = "debuginfo"

# Optimize dependencies (rarely change)
[profile.dev.package."*"]
opt-level = 3

[profile.release]
opt-level = 3
lto = true
codegen-units = 1
panic = "abort"
strip = true
```

## Code Style Conventions

- **Imports**: Group by std, external crates, internal modules
- **Error Messages**: User-facing messages in Japanese, code comments in English
- **Naming**: Follow Rust conventions (snake_case for functions/variables, PascalCase for types)
- **Documentation**: Public APIs must have doc comments with examples

## Performance Targets

When implementing features, maintain:

- Memory usage: < 10MB
- CPU usage: < 0.5% (idle monitoring)
- Connection time: < 150ms
- Reconnection detection: < 5s
