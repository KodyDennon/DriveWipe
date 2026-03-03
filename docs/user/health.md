# Drive Health Monitoring

DriveWipe provides comprehensive drive health monitoring using SMART (ATA) and NVMe health log data.

## CLI Usage

```bash
# Show health snapshot for a drive
sudo drivewipe health /dev/sda

# Save a health snapshot to file
sudo drivewipe health /dev/sda --save snapshot.json

# Compare two snapshots
sudo drivewipe health --compare before.json after.json
```

## Health Snapshot

A health snapshot captures:
- **SMART attributes** — Reallocated sectors, pending sectors, uncorrectable errors, power-on hours, temperature, etc.
- **NVMe health log** — Available spare, temperature, data units read/written, critical warnings, unsafe shutdowns
- **Error log** — Recent error events from the drive's internal log
- **Temperature** — Current drive temperature in Celsius
- **Benchmark results** — Sequential read/write throughput (optional)

## Pre/Post Wipe Health

When `auto_health_pre_wipe` is enabled in config, DriveWipe automatically:
1. Takes a health snapshot before the wipe
2. Executes the wipe
3. Takes a health snapshot after the wipe
4. Compares the two snapshots
5. Reports a PASS/FAIL verdict

The comparison checks for:
- New reallocated sectors
- Increased pending sector count
- Temperature spikes beyond safe range
- Any new critical SMART warnings

## Health Verdicts

| Verdict | Meaning |
|---|---|
| PASS | Drive health is within normal parameters after wipe |
| WARN | Minor degradation detected (e.g., slight temperature increase) |
| FAIL | Significant degradation detected (new bad sectors, critical warnings) |

## TUI / GUI

The Drive Health screen shows a list of detected drives with their SMART status. Select a drive to view detailed health data. The TUI uses color coding:
- Green: Healthy / PASS
- Yellow: Warning
- Red: Failed / Critical
