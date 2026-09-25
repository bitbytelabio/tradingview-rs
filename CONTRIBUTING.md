# Contributing

Thank you for your interest in contributing to `tradingview-rs`!

## Workspace Architecture

`tradingview-rs` is structured as a virtual Cargo workspace:

- **`crates/tradingview`**: Core high-performance Rust library (`tradingview-rs` on crates.io).
- **`crates/tradingview-py`**: Native Python extension bindings (`tradingview-rs` on PyPI) powered by PyO3, maturined with `abi3` stable ABI support.

## Getting Started

```bash
git clone https://github.com/bitbytelabio/tradingview-rs.git
cd tradingview-rs
cp .env.example .env  # fill in optional credentials for live tests

# Rust core
cargo build --workspace --all-targets --all-features
cargo test --all-features --locked

# Or use just recipes
just checks
```

### Python Development Setup

```bash
cd crates/tradingview-py
python3 -m venv .venv
source .venv/bin/activate
pip install maturin pytest pytest-asyncio mypy ruff polars pandas

# Build extension in editable mode
maturin develop --release

# Run python tests and linters
pytest -v
ruff check python tests
mypy python/ tests/
```

## Development Workflow

1. **Find an issue** — Pick an unclaimed issue from the [current milestone](https://github.com/bitbytelabio/tradingview-rs/milestones). Comment to claim it.
2. **Create a branch** — `git checkout -b feature/your-feature` or `fix/your-bugfix`.
3. **Write tests** — All new features need tests. Bug fixes need regression tests.
4. **Document** — Add `///` doc comments to all new public items. Run `cargo doc --no-deps` and fix any warnings.
5. **Run checks** — `just checks` (or `cargo fmt -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --all-features`).
6. **Sign commits** — All commits must be cryptographically signed with GPG or SSH (`git commit -S`).
7. **Open a PR** — Reference the issue number in the description.
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

1. Bump version in `crates/tradingview/Cargo.toml`, `crates/tradingview-py/Cargo.toml`, and `crates/tradingview-py/pyproject.toml`.
2. Update `CHANGELOG.md` with release date and notes.
3. Run full verification gates: `just checks`.
4. Dry-run publish check: `cargo publish -p tradingview-rs --dry-run --locked`.
5. Commit with conventional message: `git commit -S -m "chore(release): bump version to X.Y.Z"`.
6. Tag with cryptographic signature: `git tag -s vX.Y.Z -m "Release vX.Y.Z"`.
7. Push branch and tag: `git push origin main vX.Y.Z`.
8. The GitHub Actions CI/CD pipeline (`publish.yml`) will automatically verify, package, publish to crates.io, build Python binary wheels for PyPI, and create the GitHub Release.
## Questions?

Open a [discussion](https://github.com/bitbytelabio/tradingview-rs/discussions) or join the [VNQuant community](https://github.com/bitbytelabio).
