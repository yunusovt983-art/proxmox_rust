# Testing pve-network-rs on Gentoo Linux

This guide explains how to build and run the basic test suite on a Gentoo system. It does **not** replace the official Proxmox/Debian-based validation, but it can be useful for quick Rust compilation checks.

## 1. Update Portage & Prerequisites
```bash
sudo emerge --sync
sudo emerge --ask --update --deep --newuse @world
```

## 2. Install Required Packages
```bash
sudo emerge --ask \
    dev-vcs/git \
    dev-lang/rust \
    dev-util/cargo \
    dev-util/pkgconfig \
    net-misc/iproute2 \
    net-misc/bridge-utils \
    sys-apps/busybox
```
> Optional: install systemd (`sudo emerge --ask sys-apps/systemd`) if you want to test systemd unit files.

## 3. Clone Repository
```bash
git clone https://github.com/<your-account>/pve-network-rs.git
cd pve-network-rs
```

## 4. Build & Run Tests
```bash
cargo build --workspace --all-features
cargo test --workspace --all-features
```
- Some integration tests require Proxmox-specific components (`ifupdown2`, `pmxcfs`, Perl API). Those may fail or be ignored on Gentoo.
- If systemd is not installed, avoid tests that rely on unit files.

## 5. Contract Tests (Optional)
Contract tests compare Rust API with real Perl endpoints. On Gentoo you typically run them only if you have a reachable Proxmox Perl API:
```bash
cargo run -p net-test --bin contract-test \
    -- --node <node> --perl-url https://<proxmox-host>:8006 \
    --output docs/Task20/reports/<node>-gentoo.json
```
Provide authentication (token/cookie) if needed—modify the test harness accordingly.

## 6. Caveats
- Debian packaging (`dpkg-buildpackage`) is not supported on Gentoo; use Debian-based containers for that.
- Proxmox-specific services (pmxcfs, Perl CLI) are not available; for full integration tests, use official Proxmox/Debian environment.

## 7. Next Steps
For comprehensive testing (contract, canary rollout, production-like load tests) follow the documentation in:
- `docs/Task20/` (migration runbook, troubleshooting, integration)
- `docs/Task21/` (canary deployment, monitoring, rollback)

Use Gentoo primarily for quick Rust compilation checks.
