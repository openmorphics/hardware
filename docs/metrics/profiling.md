# Profiling & Metrics

This document defines the initial profiling schema and a minimal visualization workflow.

Primary metrics (examples)
- spikes_per_sec: effective throughput (float)
- latency_ms: end-to-end latency in milliseconds (float)
- buffer_occupancy_pct: 0-100 (float)
- energy_mj: energy estimate in millijoules (float)

Artifact format
- JSON Lines (JSONL), one record per line
- See the Rust schema in [telemetry.profiling.ProfileRecord](crates/telemetry/src/lib.rs:10)
  Fields:
  - ts_ms: integer epoch milliseconds
  - metric: string metric name
  - value: float
  - labels: object of string key/values (optional)

Example (JSONL)
{"ts_ms": 1736966400000, "metric": "latency_ms", "value": 3.7, "labels": {"target":"loihi2","chip":"0"}}
{"ts_ms": 1736966400050, "metric": "spikes_per_sec", "value": 125000.0, "labels": {"target":"loihi2"}}

Emitting JSONL from Rust
- Use [telemetry.profiling.emit_profile_jsonl()](crates/telemetry/src/lib.rs:25) with a slice of records.

Quick visualization (Python)
- Load JSONL with pandas and plot quickly with Altair or Matplotlib.

Example Python snippet
```python
import pandas as pd
import altair as alt

df = pd.read_json("run/profile.jsonl", lines=True)
chart = alt.Chart(df[df["metric"]=="latency_ms"]).mark_line().encode(
    x="ts_ms:T",
    y="value:Q",
    color="labels.target:N"
)
chart.save("latency.png")
```

Future extensions
- Binary trace format for very high-frequency events
- Aggregations (percentiles) and rollups at runtime
- Live streaming via gRPC/WebSocket exporters
