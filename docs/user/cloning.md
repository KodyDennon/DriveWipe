# Drive Cloning

DriveWipe supports three cloning modes: block-level, partition-aware, and image I/O.

## Block-Level Clone

Copies every sector from source to target. Target must be >= source size.

```bash
sudo drivewipe clone /dev/sda /dev/sdb --mode block
```

Features:
- Sector-by-sector copy with hash verification
- Asymmetric size handling (pad with zeros if target is larger)
- Resume support on interruption
- Live progress with throughput display

## Partition-Aware Clone

Reads the source partition table and resizes partitions to fit the target drive.

```bash
sudo drivewipe clone /dev/sda /dev/sdb --mode partition
```

Features:
- Automatic partition resizing (proportional or specified)
- Handles System Reserved / EFI partitions
- GUID and partition attribute preservation
- Supports GPT and MBR partition tables

## Image Operations

### Create a drive image

```bash
sudo drivewipe clone /dev/sda image.dwi --compress zstd
```

### Create an encrypted image

```bash
sudo drivewipe clone /dev/sda image.dwi --compress zstd --encrypt
```

You will be prompted for a passphrase.

### Restore from image

```bash
sudo drivewipe clone image.dwi /dev/sdb
```

## Image Format

DriveWipe images (`.dwi`) use a structured format:
- Header: magic bytes, version, source drive info, compression type, encryption flag
- Data: chunked sectors with optional compression and AES-256 encryption

## Safety

- DriveWipe refuses to clone to/from the boot drive
- Source and target cannot be the same drive
- Existing data on the target drive will be destroyed
- A confirmation prompt is shown before writing begins

## TUI / GUI

Both the TUI and GUI provide a Clone interface accessible from the main menu. Select source and target drives from the drive list, choose a mode, and start the operation.
