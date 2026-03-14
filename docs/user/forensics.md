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
Calculates per-sector entropy to identify areas of non-random data. A properly wiped drive should show uniform entropy matching the wipe pattern (0.0 for zero fill, ~8.0 for random fill). Generates a heatmap vector for visualization.

### File Signature Scanning
Scans for 17 known file magic bytes (JPEG, PNG, PDF, ZIP/DOCX, GIF, EXE/DLL, ELF, RAR, 7z, BMP, GZIP, SQLite, MP3, RIFF/AVI/WAV, Mach-O) to detect residual data. Any signatures found after a wipe indicate incomplete sanitization.

### Statistical Sampling
Uses random sector sampling with configurable sample ratio for rapid assessment of large drives. Reports zero-fill percentage, high-entropy percentage, data remnant percentage, and statistical confidence level.

### Hidden Area Detection
Analyzes the partition table to detect:
- **Unallocated gaps** between partitions (>1 MiB) that may contain hidden data
- **Data remnants** in gap regions (non-zero byte detection)
- **Hidden/diagnostic MBR partitions** by name pattern
- **HPA/DCO status** (full detection via ATA passthrough available in live mode with the kernel module)

## Forensic Reports

Formal forensic reports include:
- Timestamps for all operations
- Chain-of-custody information (examiner, case number)
- Methodology description
- Operator identification
- Automated conclusions derived from analysis results (entropy patterns, signature hits, sampling statistics, hidden area findings)

## Export Formats

- **DFXML** — Digital Forensics XML with signature hits, hidden area analysis, entropy statistics, and sampling results
- **Hash sets** — NSRL-compatible CSV format with offset, file type, magic bytes, and confidence

## TUI / GUI

Both the TUI and GUI provide a Forensic Analysis interface accessible from the main menu. Select a drive and start a scan. Results include entropy averages, detected file signatures, sampling statistics, and hidden area findings.
