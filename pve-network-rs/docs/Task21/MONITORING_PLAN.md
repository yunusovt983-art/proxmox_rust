# Monitoring & Metrics Plan (Canary Deployment)

## Exported Metrics
Expose Rust service metrics over HTTP exporter (e.g., `/metrics`). Suggested Prometheus descriptors:
- `pve_network_apply_duration_seconds` (histogram)
- `pve_network_fallback_total` (counter)
- `pve_network_fallback_active` (gauge)
- `pve_network_parse_duration_seconds` (histogram)
- `pve_network_ipam_allocation_duration_seconds` (histogram)
- `pve_network_storage_vlan_operations_total` (counter)

Implementation sketch (Rust):
```rust
lazy_static! {
    static ref APPLY_DURATION: Histogram = register_histogram!("pve_network_apply_duration_seconds", "Configuration apply latency").unwrap();
    static ref FALLBACK_TOTAL: IntCounter = register_int_counter!("pve_network_fallback_total", "Fallback attempts").unwrap();
}
```

## Prometheus Scrape Config
```
  - job_name: 'pve-network-canary'
    static_configs:
      - targets: ['canary-a:9187', 'canary-b:9187']
``` 
*(Assumes exporter listening on 9187)*

## Grafana Dashboard Panels
- Apply latency histogram/percentiles.
- Fallback counter (single stat + alert when >0).
- Parse duration per config + compare with Perl baseline (if available).
- IPAM allocation durations.
- Storage VLAN operation counts.

## Alerts (PromQL)
```
ALERT PveNetworkFallback
  IF increase(pve_network_fallback_total[5m]) > 0
  FOR 5m
  LABELS { severity = "critical" }
  ANNOTATIONS { summary = "Fallback triggered on {{ $labels.instance }}" }

ALERT PveNetworkApplyLatency
  IF histogram_quantile(0.95, sum(rate(pve_network_apply_duration_seconds_bucket[5m])) by (le)) > 5
  FOR 10m
  LABELS { severity = "warning" }
  ANNOTATIONS { summary = "Apply 95th percentile exceeds 5s on {{ $labels.instance }}" }
```

## Log-Based Monitoring
- `journalctl -u pve-network-rs -u pve-network-migration -f`.
- Use `rg` to capture fallback entries: `rg "used_fallback" /var/log/pve-network`.
- Forward logs to ELK/Graylog for correlation.

## Reporting/SLA
- Daily summary (fallback counts, latency trends) stored under `docs/Task21/evidence/`.
- Define SLO: 0 fallback per day, apply p95 < 5s.
- During canary soak, create weekly report for sign-off.
