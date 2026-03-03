# Quick Start Guide

## First Run

1. Build and install DriveWipe (see [installation.md](installation.md))
2. Run with root/admin privileges

## CLI Basics

### List drives

```bash
sudo drivewipe list
```

Shows all detected drives with model, capacity, interface type, and health status.

### Wipe a drive

```bash
sudo drivewipe wipe --device /dev/sda --method dod-short
```

DriveWipe will:
1. Display drive details and wipe method info
2. Ask you to confirm by typing the device path
3. Run a 3-second countdown
4. Execute the wipe with live progress
5. Generate a JSON report automatically

### Verify a wipe

```bash
sudo drivewipe verify --device /dev/sda --pattern zero
```

### Check drive health

```bash
sudo drivewipe health /dev/sda
```

## TUI

Launch the interactive terminal UI:

```bash
sudo drivewipe-tui
```

Navigate with arrow keys, Enter to select, Esc to go back. The main menu provides access to all features:
- Secure Wipe
- Drive Health
- Drive Clone
- Partition Manager
- Forensic Analysis
- Settings

### Keyboard shortcuts
- `Ctrl-L` — Toggle keyboard lock (prevents accidental input)
- `?` — Help screen
- `q` — Quit (from main menu)

## GUI

Launch the graphical interface:

```bash
sudo drivewipe-gui
```

Point-and-click access to all features with the same main menu layout as the TUI.

## Common Workflows

### Wipe and verify with report

```bash
sudo drivewipe wipe --device /dev/sda --method dod-short --verify true
```

### Batch wipe multiple drives

```bash
sudo drivewipe queue add --device /dev/sda --method dod-short
sudo drivewipe queue add --device /dev/sdb --method zero
sudo drivewipe queue start --parallel 2
```

### Pre-wipe health check then wipe

```bash
sudo drivewipe health /dev/sda
sudo drivewipe wipe --device /dev/sda --method drivewipe-secure-hdd
```

### Clone a drive before wiping

```bash
sudo drivewipe clone /dev/sda /dev/sdb --mode block
sudo drivewipe wipe --device /dev/sda --method zero
```
