# Wipe Methods

DriveWipe supports 21 wipe methods across three categories: software overwrite, firmware commands, and hybrid (DriveWipe Secure).

## Software Methods

Software methods write patterns to every addressable sector. Best for HDDs. For SSDs, firmware methods are recommended due to wear leveling.

| ID | Name | Passes | Description |
|---|---|---|---|
| `zero` | Zero Fill | 1 | Writes 0x00 to all sectors |
| `one` | One Fill | 1 | Writes 0xFF to all sectors |
| `random` | Random Fill | 1 | AES-256-CTR pseudorandom data |
| `dod-short` | DoD 5220.22-M | 3 | Pass 1: 0x00, Pass 2: 0xFF, Pass 3: random |
| `dod-ece` | DoD 5220.22-M ECE | 7 | Extended version with complementary passes |
| `gutmann` | Gutmann | 35 | 35 passes including 4 random + 27 deterministic + 4 random |
| `hmg-baseline` | HMG IS5 Baseline | 1 | Single random pass (UK government baseline) |
| `hmg-enhanced` | HMG IS5 Enhanced | 3 | Three passes: random, complement, random (UK government enhanced) |
| `rcmp` | RCMP TSSIT OPS-II | 7 | Canadian government standard: alternating 0x00/0xFF + random final |

## Firmware Methods

Firmware methods issue commands directly to the drive controller. Required for proper SSD sanitization. May not work through USB bridges.

| ID | Name | Target |
|---|---|---|
| `ata-erase` | ATA Secure Erase | SATA HDD/SSD |
| `ata-erase-enhanced` | ATA Enhanced Secure Erase | SATA HDD/SSD |
| `nvme-format-user` | NVMe Format (User Data Erase) | NVMe SSD |
| `nvme-format-crypto` | NVMe Format (Cryptographic Erase) | NVMe SSD |
| `nvme-sanitize-block` | NVMe Sanitize (Block Erase) | NVMe SSD |
| `nvme-sanitize-crypto` | NVMe Sanitize (Cryptographic Erase) | NVMe SSD |
| `nvme-sanitize-overwrite` | NVMe Sanitize (Overwrite) | NVMe SSD |
| `tcg-opal` | TCG Opal Crypto Erase | Self-encrypting drives |

## DriveWipe Secure (Hybrid)

DriveWipe Secure methods combine software overwrite with firmware commands for maximum assurance. They automatically detect available firmware capabilities and adapt.

| ID | Target | Strategy |
|---|---|---|
| `drivewipe-secure-hdd` | Mechanical HDDs | Multi-pass patterns + full verification |
| `drivewipe-secure-sata-ssd` | SATA SSDs | Overwrite + TRIM + overwrite + ATA Secure Erase + verify |
| `drivewipe-secure-nvme` | NVMe SSDs | Overwrite + deallocate + NVMe Format/Sanitize + overwrite + verify |
| `drivewipe-secure-usb` | USB drives | Multi-pass overwrite + verify (USB controller limitations) |

## Choosing a Method

**For HDDs:**
- Quick: `zero` (1 pass, fastest)
- Standard: `dod-short` (3 pass, meets most compliance)
- Maximum: `drivewipe-secure-hdd` (multi-phase with verification)

**For SATA SSDs:**
- Recommended: `ata-erase` or `drivewipe-secure-sata-ssd`
- Software methods alone are insufficient for SSDs due to wear leveling

**For NVMe SSDs:**
- Recommended: `nvme-format-crypto` or `drivewipe-secure-nvme`
- Cryptographic erase is instant and complete

**For self-encrypting drives:**
- Recommended: `tcg-opal` (instant, destroys encryption key)

## Custom Methods

Define custom wipe methods in `config.toml`:

```toml
[[custom_methods]]
id = "my-3pass"
name = "My 3-Pass Method"
description = "Random, zero, random"
verify_after = true

[[custom_methods.passes]]
pattern_type = "random"

[[custom_methods.passes]]
pattern_type = "zero"

[[custom_methods.passes]]
pattern_type = "random"
```

Available pattern types: `zero`, `one`, `random`, `constant` (with `constant_value`), `repeating` (with `repeating_pattern`).
