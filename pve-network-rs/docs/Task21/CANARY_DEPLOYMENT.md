# Canary Deployment Playbook (Test Nodes)

## Scope
Establish controlled canary rollout of the Rust-based `pve-network` on dedicated test nodes before production rollout. This document assumes Debian-based Proxmox VE environment and availability of both Perl and Rust implementations.

## 1. Prerequisites
- Identify two test nodes (CANARY_A, CANARY_B) representing typical cluster roles.
- Ensure SSH access and Proxmox cluster membership.
- Collect baseline data (Perl version, current configs, automation touchpoints).
- Prepare signed `.deb` packages for Rust implementation and store in `~/artifacts/`.
- Confirm monitoring stack (Prometheus/Grafana or equivalent) can scrape canary nodes.

## 2. Preparation Steps (per node)
1. **Snapshot configs:**
   ```bash
   mkdir -p /var/lib/pve-network-rs/manual-backups/$(date +%Y%m%d-%H%M%S)
   rsync -a /etc/network/interfaces /etc/pve/sdn.cfg /etc/pve/storage.cfg /var/lib/pve-network-rs/manual-backups/<ts>/
   ```
2. **Install prerequisites:** `apt-get update && apt-get install -y dh-cargo ifupdown2 jq`
3. **Install package:** `dpkg -i ~/artifacts/pve-network-rs_<version>_amd64.deb`
4. **Verify systemd unit present** (should be disabled initially): `systemctl status pve-network-rs.service`

## 3. Migration Configuration
- Create `/etc/pve/network/migration.toml` (replicated via pmxcfs):
  ```toml
  phase = "perl-only"
  fallback_enabled = true
  perl_api_base_url = "https://<cluster>:8006"
  perl_api_timeout = 60
  features = { rust_cli = false, rust_daemon = false }
  log_migration_decisions = true
  ```
- Commit to cluster: run `pvesh set /configuration/cluster/migration --file /etc/pve/network/migration.toml` (placeholder command) or simply ensure file synchronized.

## 4. Shadow Mode (Rust Read-Only)
1. Set phase: `migration-ctl phase set rust-read-only`
2. Restart middleware: `systemctl restart pve-network-migration.service`
3. Run contract tests:
   ```bash
   contract-test --node CANARY_A --perl-url https://<cluster>:8006 --output /var/log/pve-network/contract-canary-a.json
   ```
4. Execute CLI smoke:
   ```bash
   pvenet status --verbose
   pvenet list --format json
   ```
5. Monitor logs for fallback usage (should remain false).

## 5. Promotion to Basic Write
1. Confirm =24h of clean metrics (no fallback, apply latency <= Perl baseline).
2. Set phase: `migration-ctl phase set rust-basic-write`
3. Dry-run apply: `pvenet apply --dry-run`
4. Prepare change window; optionally limit to CANARY_A first, then CANARY_B after success.

## 6. Monitoring Checklist
- Prometheus scrape target: `<node>:9187/metrics` (if exporter configured).
  - Alerts:
    - `pve_network_fallback_total > 0` (critical).
    - `increase(pve_network_apply_duration_seconds_sum[5m]) / increase(..._count[5m]) > baseline` (warning).
- Systemd logs: `journalctl -u pve-network-rs -u pve-network-migration -f`
- CLI logs: `/var/log/pve-network/` (fallback & contract test reports).

## 7. Failure Handling
- Immediate fallback: `migration-ctl phase set perl-only`, `systemctl stop pve-network-rs.service`.
- Restore latest backup from `/var/lib/pve-network-rs/backups/<ts>`.
- Reapply via Perl CLI: `pvenet-legacy apply`.
- Collect diagnostics (`migration-ctl diagnostics`, `journalctl`) for analysis before retry.

## 8. Promotion to Advanced/Full
- After 7-day soak in `rust-basic-write`, if metrics clean, proceed to `rust-advanced`, then `rust-sdn`, then `rust-full` with same process.
- Keep CANARY_B one phase behind until CANARY_A stable to maintain early warning capability.

## 9. Rollout to Remaining Nodes
1. Update documentation/automation referencing Rust CLI.
2. Schedule rolling install per node replicating steps 2–8.
3. Remove Perl package only after cluster-wide `rust-full` stability confirmed.

## 10. Evidence & Reporting
- Store contract test outputs, migration config snapshots, and monitoring dashboards under `docs/Task21/evidence/`.
- Provide weekly status with fallback counts, apply latency, and outstanding issues.

## References
- `docs/Task20/MIGRATION_RUNBOOK.md`
- `docs/Task20/TROUBLESHOOTING.md`
- `docs/Task20/CONTAINER_STORAGE_INTEGRATION.md`
