# Forensic Analysis

DriveWipe includes a forensic toolkit for analyzing drives before or after sanitization.

## CLI Usage

```bash
# Full forensic scan
sudo drivewipe forensic scan /dev/sda

# Generate formal forensic report
sudo drivewipe forensic report /dev/sda --output report.json

# Compare two forensic scans
sudo drivewipe forensic compare before.json after.json
```

## Analysis Capabilities

### Entropy Analysis
Calculates per-sector entropy to identify areas of non-random data. A properly wiped drive should show uniform entropy matching the wipe pattern (0.0 for zero fill, ~8.0 for random fill).

### File Signature Scanning
Scans for known file magic bytes (JPEG, PDF, DOCX, EXE, ZIP, PNG, etc.) to detect residual data. Any signatures found after a wipe indicate incomplete sanitization.

### Statistical Sampling
Uses random sector sampling with configurable confidence levels for rapid assessment of large drives without scanning every sector.

### Hidden Area Detection
Scans for HPA (Host Protected Area) and DCO (Device Configuration Overlay) regions that may contain data inaccessible to standard wipe operations.

## Forensic Reports

Formal forensic reports include:
- Timestamps for all operations
- Hash chains for data integrity
- Chain-of-custody information
- Methodology description
- Operator identification

## Export Formats

- **DFXML** — Digital Forensics XML for interoperability with forensic tools
- **Hash sets** — NSRL-compatible format for known-file identification
- **Timeline** — log2timeline/Plaso compatible output
