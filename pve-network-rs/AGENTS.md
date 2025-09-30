# Repository Guidelines

## Project Structure & Module Organization
The workspace lives under `Cargo.toml` at the root and groups core crates in `crates/`: `net-*` crates cover network API, config parsing, validation, applying changes, while SDN logic sits in `sdn-*`. Shared types reside in `crates/pve-shared-types`. CLI utilities live under `crates/net-cli`, and integration harnesses under `crates/net-test`. Debian packaging lives in `debian/`, long-form docs and task notes in `docs/`, CLI scripts in `scripts/`, and ready-to-run samples in `examples/`. Service unit files are staged in `services/`.

## Build, Test, and Development Commands
Use `cargo check --workspace` for quick feedback. `cargo build --all-features` compiles every crate; add `--release` for packaging. `cargo fmt --all` and `cargo clippy --all-targets --all-features -- -D warnings` mirror CI quality gates. Run `cargo test --workspace --all-features` for the default suite, and `cargo llvm-cov --all-features --workspace --lcov --output-path lcov.info` to reproduce coverage. Debian packages are assembled with `dpkg-buildpackage -us -uc -b` (requires the `debian/` toolchain). Scripts like `scripts/build-cli.sh` wrap common CLI builds; review flags before running.

## Coding Style & Naming Conventions
Rust code follows stable `rustfmt` defaults (4-space indentation, trailing commas where useful). Prefer explicit module paths and snake_case for modules, functions, and locals; use UpperCamelCase for types and enums, SCREAMING_SNAKE_CASE for constants. Feature flags should be kebab-cased in `Cargo.toml`. Keep public API docs dense with `///` comments; internal notes should justify design decisions rather than restating code.

## Testing Guidelines
Unit tests live beside implementation modules, while cross-crate integration tests reside in `crates/net-test` and per-crate `tests/` directories. Name tests using the behavior under test, e.g. `validates_vlan_ranges`. Use `cargo test --test integration` when a crate exposes dedicated integration suites, and gate network-mutation tests behind feature flags or mocks. Maintain coverage parity with CI (see coverage job); document gaps in the PR description when skipping heavy scenarios.

## Commit & Pull Request Guidelines
Commits should use `component: imperative summary` (e.g. `net-config: add bond parser guard`) and wrap details at 72 columns. Group related changes; avoid drive-by refactors. PRs must include a problem summary, testing notes (`cargo test`, `cargo clippy`, etc.), and link to Proxmox tracker issues when applicable. Attach CLI or API output only when it clarifies functional changes. Request review from crate owners when touching shared types or Debian packaging.
