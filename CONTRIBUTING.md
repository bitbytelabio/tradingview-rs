# Contributing

Thank you for your interest in contributing to `tradingview-rs`!

## Getting Started

```bash
git clone https://github.com/bitbytelabio/tradingview-rs.git
cd tradingview-rs
cp .env.example .env  # fill in your TradingView credentials
cargo build
cargo test
```

## Development Workflow

1. **Find an issue** — Pick an unclaimed issue from the [current milestone](https://github.com/bitbytelabio/tradingview-rs/milestones). Comment to claim it.
2. **Create a branch** — `git checkout -b feature/your-feature` or `fix/your-bugfix`.
3. **Write tests** — All new features need tests. Bug fixes need regression tests.
4. **Document** — Add `///` doc comments to all new public items. Run `cargo doc --no-deps` and fix any warnings.
5. **Run checks** — `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `cargo doc --no-deps`.
6. **Open a PR** — Reference the issue number in the description.

## Code Style

- Follow [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/).
- Use `tracing` for logging (never `println!` in library code).
- Prefer `thiserror` for error types; implement `From` for common conversions.
- Use `bon::builder` for types with many configurable fields.
- Async functions should accept `CancellationToken` for graceful shutdown.

## Commit Messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
feat(historical): add rate-limit-aware throttling
fix(websocket): handle ping timeout during reconnect
docs(models): document Interval enum variants
test(loader): add integration test for fan-out
```

## Testing

```bash
cargo test                          # unit + integration tests
cargo test -- --ignored             # tests requiring network/auth
cargo bench                         # benchmarks
```

## Documentation

```bash
cargo doc --no-deps --open          # browse locally
cargo doc --no-deps -D warnings     # strict mode (used in CI)
```

## Release Process

1. Bump version in `Cargo.toml`.
2. Update `CHANGELOG.md`.
3. Run full CI: `cargo test && cargo doc --no-deps -D warnings && cargo clippy -- -D warnings`.
4. `cargo publish --dry-run` → review → `cargo publish`.
5. Tag: `git tag vX.Y.Z && git push origin vX.Y.Z`.
6. Create GitHub Release with changelog notes.

## Questions?

Open a [discussion](https://github.com/bitbytelabio/tradingview-rs/discussions) or join the [VNQuant community](https://github.com/bitbytelabio).
