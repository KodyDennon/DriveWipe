# Drive Cloning

DriveWipe supports three cloning modes: block-level, partition-aware, and image I/O.

## Block-Level Clone

Copies every sector from source to target. Target must be >= source size.

```bash
sudo drivewipe clone /dev/sda /dev/sdb --mode block
```

Features:
- Sector-by-sector copy with BLAKE3 hash verification
- Asymmetric size handling (copies up to the smaller capacity)
- Live progress with throughput display
- Optional bandwidth throttling

## Partition-Aware Clone

Reads the source partition table and copies each partition individually, skipping unallocated space.

```bash
sudo drivewipe clone /dev/sda /dev/sdb --mode partition
```

Features:
- Copies partition table header (GPT or MBR) first
- Copies each partition's data range individually
- Skips unallocated space between partitions for faster cloning
- Warns and skips partitions that exceed target capacity
- Falls back to block clone if partition table parsing fails
- Supports GPT and MBR partition tables

## Image Operations

### Create a drive image

```bash
sudo drivewipe clone /dev/sda backup.dwc --compress zstd
```

### Create an encrypted image

```bash
sudo drivewipe clone /dev/sda backup.dwc --compress zstd --encrypt --password "passphrase"
```

Encrypted images use AES-256-CTR with a SHA-256 iterated key derivation. A random salt and nonce are stored in the image header. Each chunk uses an incrementing nonce for unique IVs.

### Restore from image

```bash
sudo drivewipe clone backup.dwc /dev/sdb
```

For encrypted images, provide the password:

```bash
sudo drivewipe clone backup.dwc /dev/sdb --password "passphrase"
```

## Bandwidth Throttling

Limit clone I/O rate to prevent saturation on shared systems:

```bash
sudo drivewipe clone /dev/sda /dev/sdb --bandwidth-limit 100000000  # 100 MB/s
```

Applies to all clone modes (block, partition, image).

## Image Format

DriveWipe images (`.dwc`) use a structured binary format:
- **Magic bytes**: `DWCLONE\x01` (8 bytes)
- **Header**: JSON-encoded metadata (source model/serial/capacity, chunk count, compression mode, encryption salt/nonce)
- **Data chunks**: length-prefixed, optionally compressed (Gzip/Zstd) and encrypted (AES-256-CTR)

## Safety

- DriveWipe refuses to clone to/from the boot drive
- Source and target cannot be the same drive
- Existing data on the target drive will be destroyed
- A confirmation prompt is shown before writing begins

## TUI / GUI

Both the TUI and GUI provide a Clone interface accessible from the main menu. Select source and target drives from the drive list, choose a mode, and start the operation. The GUI shows a live progress bar with throughput during cloning.
