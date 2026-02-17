# Simon Grafana Dashboards

Pre-built Grafana dashboards for Silicon Monitor (simon) metrics.

## Setup

### 1. Enable Prometheus Endpoint

Start simon with the HTTP server and Prometheus exporter:

```bash
simon --serve --port 9100
```

Or use the daemon mode:

```bash
simon daemon --config simon.toml
```

### 2. Configure Prometheus

Add to `prometheus.yml`:

```yaml
scrape_configs:
  - job_name: 'simon'
    scrape_interval: 5s
    static_configs:
      - targets: ['localhost:9100']
        labels:
          instance: 'my-host'
```

### 3. Import Dashboards

1. Open Grafana → Dashboards → Import
2. Upload JSON file or paste contents
3. Select your Prometheus data source
4. Click Import

## Dashboards

| Dashboard | File | Description |
|-----------|------|-------------|
| Fleet Overview | `fleet-overview.json` | Multi-host fleet monitoring with health scores |
| GPU Detail | `gpu-detail.json` | Per-GPU metrics with temperature, utilization, memory, power |
| Host Detail | `host-detail.json` | Single-host deep dive with CPU, memory, disk, network |

## Metric Reference

All metrics use the `simon_` prefix:

| Metric | Type | Description |
|--------|------|-------------|
| `simon_cpu_usage_percent` | gauge | CPU usage per core |
| `simon_cpu_frequency_mhz` | gauge | CPU frequency per core |
| `simon_cpu_temperature_celsius` | gauge | CPU temperature |
| `simon_memory_used_bytes` | gauge | Used memory |
| `simon_memory_total_bytes` | gauge | Total memory |
| `simon_memory_usage_percent` | gauge | Memory usage percentage |
| `simon_swap_used_bytes` | gauge | Used swap |
| `simon_swap_total_bytes` | gauge | Total swap |
| `simon_gpu_temperature_celsius` | gauge | GPU temperature |
| `simon_gpu_utilization_percent` | gauge | GPU utilization |
| `simon_gpu_memory_used_bytes` | gauge | GPU memory used |
| `simon_gpu_memory_total_bytes` | gauge | GPU memory total |
| `simon_gpu_power_watts` | gauge | GPU power draw |
| `simon_gpu_clock_graphics_mhz` | gauge | GPU graphics clock |
| `simon_gpu_clock_memory_mhz` | gauge | GPU memory clock |
| `simon_gpu_fan_speed_percent` | gauge | GPU fan speed |
| `simon_disk_used_bytes` | gauge | Disk space used |
| `simon_disk_total_bytes` | gauge | Disk space total |
| `simon_disk_usage_percent` | gauge | Disk usage percentage |
| `simon_network_rx_bytes_total` | counter | Network bytes received |
| `simon_network_tx_bytes_total` | counter | Network bytes transmitted |
| `simon_process_count` | gauge | Total process count |
| `simon_load_average_1m` | gauge | 1-minute load average |
| `simon_load_average_5m` | gauge | 5-minute load average |
| `simon_uptime_seconds` | gauge | System uptime |

## Customization

Dashboards use template variables for filtering:
- `$instance` — filter by host
- `$gpu` — filter by GPU index
- `$disk` — filter by disk device
- `$interface` — filter by network interface
