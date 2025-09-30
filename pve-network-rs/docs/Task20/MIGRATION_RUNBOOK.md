# Proxmox VE Network Migration Runbook (Rust)

> **Scope:** Phased migration of pve-network from Perl to Rust while ensuring zero downtime, full API compatibility, and safe rollback paths.

## 1. Preparation

### 1.1 Inventory & Baseline
- Identify target nodes/clusters and record current pve-network package version.
- Capture Perl API schema snapshots (REST `/network`, `/sdn`, `/storage`) using `pvesh get ... > docs/Task20/fixtures/<node>-<endpoint>.json`.
- Export `/etc/network/interfaces`, `/etc/pve/sdn.cfg`, `/etc/pve/storage.cfg`, `/etc/pve/lxc/*.conf` into the migration evidence folder.
- Document existing automation (Ansible/Terraform/custom scripts) and schedule downtime windows for rollback.

### 1.2 Install Rust Packages (Canary Nodes First)
1. Copy the signed `.deb` artifacts to the node (or use repository).
2. Install prerequisites:
   ```bash
   apt-get update
   apt-get install -y dh-cargo ifupdown2 pve-cluster jq
   ```
3. Install the Rust package in shadow mode:
   ```bash
   dpkg -i pve-network-rs_<version>_amd64.deb
   ```
4. Verify systemd unit is present but inactive:
   ```bash
   systemctl status pve-network-rs.service
   ```

### 1.3 Configure Migration Defaults
- Ensure `/etc/pve/network/migration.toml` (or equivalent) exists. If not, create with:
  ```toml
  phase = "perl-only"
  fallback_enabled = true
  perl_api_base_url = "https://<cluster>:8006"
  perl_api_timeout = 60
  features = { rust_cli = false, rust_daemon = false }
  ```
- Sync across cluster via pmxcfs (confirm file replicated under `/etc/pve/` on all nodes).
- Register planned hooks in `/etc/pve/network/hooks.d/` if other Rust components subscribe to events.

## 2. Shadow Mode (Read-Only)

### 2.1 Enable Rust Read-Only Phase
1. Edit migration config and set `phase = "rust-read-only"`; commit via `migration-ctl phase set rust-read-only`.
2. Reload middleware (no network restart):
   ```bash
   systemctl restart pve-network-migration.service
   ```
3. Run contract tests comparing Rust vs Perl outputs:
   ```bash
   contract-test --node <node> --perl-url https://<cluster>:8006 --output docs/Task20/reports/<node>-shadow.json
   ```
   - If any diff detected ? revert to `perl-only`, analyse, patch, re-run.

### 2.2 Shadow Validation
- Execute CLI in read-only mode to confirm parity:
  ```bash
  pvenet status --verbose
  pvenet list --format json
  ```
- Monitor migration logs: `/var/log/pve-network/migration.log` should show `used_fallback=false` for GET endpoints.

## 3. Basic Write Phase

### 3.1 Preconditions
- Ensure backups exist (see Section 6).
- Communicate change window to operators.

### 3.2 Enable Phase
1. `migration-ctl phase set rust-basic-write`.
2. Dry-run apply to confirm behaviour:
   ```bash
   pvenet apply --dry-run
   pvenet validate --config /etc/network/interfaces
   ```
3. Check fallback metrics (`/var/log/pve-network/migration.log`, `systemctl status pve-network-rs`). Any fallback triggered ? roll back to `rust-read-only` and investigate.

## 4. Advanced Functionality (SDN, Storage, Containers)

### 4.1 Rust Advanced Phase
- `migration-ctl phase set rust-advanced` to route bridge/bond/vlan operations through Rust.
- Validate SDN configs:
  ```bash
  contract-test --sdn --node <node>
  pvesh get /nodes/<node>/sdn/zones
  ```
- Run container hotplug smoke tests (future Rust hooks) and ensure logs show hook execution.

### 4.2 SDN Phase
- `migration-ctl phase set rust-sdn` enabling all SDN endpoints.
- Monitor IPAM allocations (`journalctl -u pve-network-rs -f`).

### 4.3 Rust Full
- After soak period with zero fallbacks (recommended =7 days), promote to `rust-full`.
- Disable Perl CLI/services where applicable (optional):
   ```bash
   systemctl disable --now pve-network.service   # legacy
   ```

## 5. Verification Checklist (per node)
- [ ] Contract tests report 100% pass.
- [ ] `pvenet status`, `pvenet apply`, `pvenet rollback --list` run without fallback.
- [ ] `/proc/net/bonding/*`, `ip -details link` reflect applied configuration.
- [ ] Storage VLAN interfaces present (`ip link show <iface>.<tag>`).
- [ ] LXC hotplug events processed (hooks log success).

## 6. Backups & Rollback

### 6.1 Automated Backups
- Debian postinst creates `/var/lib/pve-network-rs/backups/<timestamp>` containing:
  - `/etc/network/interfaces`
  - `/etc/pve/sdn.cfg`
  - `/etc/pve/storage.cfg`
  - `/etc/pve/lxc/*.conf`
- Verify backups periodically:
   ```bash
   ls /var/lib/pve-network-rs/backups
   ```

### 6.2 Manual Snapshot
- Before major changes:
   ```bash
   mkdir -p /var/lib/pve-network-rs/manual-backups/$(date +%Y%m%d-%H%M%S)
   cp -a /etc/network/interfaces ...
   ```

### 6.3 Rollback to Perl
1. Set `phase = "perl-only"` using `migration-ctl`.
2. Restart migration middleware `systemctl restart pve-network-migration.service`.
3. Restore most recent backup and apply via original Perl CLI:
   ```bash
   cp /var/lib/pve-network-rs/backups/<ts>/interfaces /etc/network/interfaces
   pvenet-legacy apply
   ```
4. Stop Rust service if running: `systemctl stop pve-network-rs.service`.

## 7. Cluster Coordination
- Ensure all nodes share identical `migration.toml` (pmxcfs replication).
- When promoting to new phase, run `migration-ctl phase set ... --all-nodes` (future feature) or manual per node.
- Use `migration-ctl health` to confirm phase uniformity.

## 8. Monitoring & Metrics
- `journalctl -u pve-network-rs -f` for runtime logs.
- Prometheus endpoint (if enabled) exposes `pve_network_fallback_total`, `pve_network_apply_duration_seconds`.
- Alert if fallback rate >0 or apply latency exceeds Perl baseline.

## 9. Post-Migration Tasks
- Update documentation/automation to reference Rust CLI/service.
- Remove Perl binaries when confident:
   ```bash
   dpkg --purge pve-network
   ```
- Archive migration evidence (`docs/Task20/`).

## Appendix A: Command Reference
- `migration-ctl phase get` / `set`
- `migration-ctl feature enable <flag>`
- `contract-test --help`
- `pvenet --help`

## Appendix B: Failure Scenarios
- **Rust panic** ? fallback triggered, check migration logs, revert to previous phase.
- **Service crash** ? systemd auto-restart; if persistent, `journalctl` + revert.
- **Inconsistent configs** ? restore from backups, re-run validation.

## Appendix C: Evidence Tracking
- Store reports, config snapshots, contract logs under `docs/Task20/` for audit.
