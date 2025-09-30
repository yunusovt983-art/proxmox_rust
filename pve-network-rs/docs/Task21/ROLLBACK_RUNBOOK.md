# Fast Rollback Procedures

> Use this runbook for immediate rollback to the legacy Perl implementation when Rust deployment shows regressions.

## 1. Immediate Response Checklist
- Detect issue (API mismatch, CLI failure, fallback spike, service crash).
- Communicate incident: notify on-call, freeze further rollout.
- Collect evidence: `/var/log/pve-network/migration.log`, `systemctl status`, contract test diff.

## 2. Revert Migration Phase
```bash
migration-ctl phase set perl-only
systemctl restart pve-network-migration.service
```
- Confirms all new requests handled by Perl.
- Monitor `migration.log` to ensure `used_fallback=true` replaced by `false` (Rust bypassed).

## 3. Stop Rust Daemon
```bash
systemctl stop pve-network-rs.service
systemctl disable pve-network-rs.service
```
- Prevents Rust service from interfering.

## 4. Restore Configurations
1. Identify latest backup (automatic or manual) under `/var/lib/pve-network-rs/backups/`.
2. Restore configs:
   ```bash
   cp <backup>/interfaces /etc/network/interfaces
   cp <backup>/sdn.cfg /etc/pve/sdn.cfg
   cp <backup>/storage.cfg /etc/pve/storage.cfg
   rsync -a <backup>/lxc/ /etc/pve/lxc/
   ```
3. Validate syntax quickly: `pvenet-legacy validate --config /etc/network/interfaces`.
4. Apply via legacy CLI:
   ```bash
   pvenet-legacy apply
   systemctl restart pve-network
   ```

## 5. (Optional) Remove Rust Package
- Either hold package with `apt-mark hold pve-network-rs` or purge:
  ```bash
  dpkg -r pve-network-rs   # keep config
  # or
  dpkg --purge pve-network-rs
  ```

## 6. Verification After Rollback
- Run contract tests in Perl-only mode to ensure baseline intact.
- Monitor `/var/log/syslog`, `journalctl -u pve-network` for errors.
- Confirm automation scripts (Ansible/Terraform) succeed.
- Document outage details in incident report.

## 7. Root Cause Analysis
- Compare Rust vs Perl responses (saved contract output) to identify mismatch.
- Check panic traces (`journalctl -u pve-network-rs`) and fix before next attempt.
- Update migration plan/fixtures if new edge case discovered.

## 8. Re-enable Rust (when fixed)
- Reinstall or re-enable service:
  ```bash
  systemctl enable pve-network-rs.service
  migration-ctl phase set rust-read-only
  ```
- Repeat gradual rollout from canary phase.

## Appendix A: Quick Commands
```
migration-ctl phase get
pvenet-legacy apply
ls /var/lib/pve-network-rs/backups
journalctl -u pve-network -n 200
```

## Appendix B: Communication Template
- Incident summary
- Time of detection/rollback
- User impact
- Next steps / ETA for retry
