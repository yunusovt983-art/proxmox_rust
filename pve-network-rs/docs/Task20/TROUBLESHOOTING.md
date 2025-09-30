# Proxmox VE Network Migration Troubleshooting Guide

> Use this guide to diagnose and resolve issues during or after migrating pve-network from Perl to Rust.

## 1. Quick Health Checks

### 1.1 Migration Middleware State
```bash
systemctl status pve-network-migration.service
journalctl -u pve-network-migration.service -n 100
migration-ctl health
```
- ? **Symptoms**: service failing ? check `migration.toml` syntax, ensure Perl API reachable.

### 1.2 Rust Service Status
```bash
systemctl status pve-network-rs.service
journalctl -u pve-network-rs.service -f
```
- ? **ExecStart failure**: confirm `/usr/bin/pve-network-rs-daemon` exists and migration phase >= `rust-basic-write`.
- ? **Permission denied**: ensure service has access to `/etc/network`, `/etc/pve`, `/run`, `/var/log`.

### 1.3 Fallback Metrics
- Inspect `/var/log/pve-network/migration.log` for `used_fallback=true` entries.
- Prometheus counters: `pve_network_fallback_total`, `pve_network_apply_duration_seconds`.
- If fallback rate >0 ? treat as degraded, revert phase to previous level.

## 2. Common Issues & Solutions

### 2.1 Contract Tests Fail (Rust vs Perl mismatch)
**Symptoms:** `contract-test` reports diff or failure.
**Actions:**
1. Set `phase = "perl-only"` via `migration-ctl`.
2. Capture failing JSON with `--output` option.
3. Compare schemas, adjust serialization (`serde` rename) and update tests.
4. Re-run contract tests; move back to `rust-read-only` only after pass.

### 2.2 `pvenet apply` Panics or Falls Back
- Check `pvenet --debug apply --dry-run` for detailed logs.
- Ensure `NetworkApplier::new` receives valid `pmxcfs` (no `.default()` in CLI).
- Verify `ifupdown2` installed (`ifquery --version`).
- Turn on verbose migration logging (`migration.toml` > `log_migration_decisions = true`).

### 2.3 Configuration Out of Sync Across Cluster
- Run `pmxcfs` status: `pvecm status`.
- Compare `/etc/pve/network/migration.toml` across nodes.
- Use `migration-ctl phase get` per node; manually align before enabling rust phases.

### 2.4 Storage VLAN/Bond/Bridge Not Applied
- Inspect `journalctl -u pve-network-rs.service` for `StorageVlanManager` messages.
- Validate `pvenet status --verbose` lists VLAN interface.
- If missing: `pvenet rollback --list` ? restore previous version, reapply.
- Confirm `ifupdown2` scripts in place; check `ip -d link show type vlan`.

### 2.5 IPAM Allocation Errors
- Log location: `/var/log/pve-network/ipam.log` (if configured).
- If external IPAM unreachable ? fallback to Perl plugin (ensure `perl_api_base_url`).
- Clear cached allocations (if corrupted) by restoring `pve-network-rs` backups.

## 3. Rollback to Perl Workflow
1. `migration-ctl phase set perl-only`.
2. `systemctl stop pve-network-rs.service`.
3. Restore backups (`/var/lib/pve-network-rs/backups/<ts>`).
4. Apply via legacy CLI `pvenet-legacy apply`.
5. Verify `contract-test` using Perl-only mode (should pass).

## 4. Diagnostics Checklist
- [ ] `migration-ctl health` reports consistent phase.
- [ ] No `used_fallback=true` entries in last 15 minutes.
- [ ] `pvenet apply --dry-run` succeeds on representative node.
- [ ] `/etc/network/interfaces` matches golden fixture (if used).
- [ ] `systemctl status` for migration + rust services active.
- [ ] Storage/LXC integrations functioning (see logs).

## 5. Useful Commands
```bash
# Show last fallback entries
rg "used_fallback" /var/log/pve-network/ -n

# Re-run contract tests for specific endpoint
contract-test --node <node> --perl-url https://<host>:8006 --endpoint /api2/json/nodes/<node>/network

# Dump current migration config
cat /etc/pve/network/migration.toml

# Restore backup
cp /var/lib/pve-network-rs/backups/<ts>/interfaces /etc/network/interfaces
```

## 6. Support Escalation
- Collect logs: `/var/log/pve-network/*.log`, `journalctl -u pve-network-*`.
- Export `migration-ctl diagnostics` (future command) or manual summary.
- Provide timestamps and fallback counts to engineering/team.
