# Wipe Methods

List them from the command line at any time:

```bash
drivewipe methods              # table
drivewipe methods --format json
```


DriveWipe supports 27 wipe methods across three categories: software overwrite, firmware commands, and hybrid (DriveWipe Secure).

## Software Methods

Software methods write patterns to every addressable sector. Best for HDDs. For SSDs, firmware methods are recommended due to wear leveling.

| ID | Name | Passes | Description |
|---|---|---|---|
| `zero` | Zero Fill | 1 | Writes 0x00 to all sectors |
| `one` | One Fill | 1 | Writes 0xFF to all sectors |
| `random` | Random Fill | 1 | AES-256-CTR pseudorandom data |
| `dod-short` | DoD 5220.22-M | 3 | Pass 1: 0x00, Pass 2: 0xFF, Pass 3: random |
| `dod-ece` | DoD 5220.22-M ECE | 7 | Extended version with complementary passes |
| `gutmann` | Gutmann | 35 | 4 random, 27 encoding-specific patterns, 4 random |
| `hmg-baseline` | HMG IS5 Baseline | 1 | Single zero pass (UK government baseline) |
| `hmg-enhanced` | HMG IS5 Enhanced | 3 | Three passes: 0x00, 0xFF, random (UK government enhanced) |
| `rcmp` | RCMP TSSIT OPS-II | 7 | Canadian government standard: alternating 0x00/0xFF + random final |
| `nist-800-88-clear` | NIST SP 800-88 Clear | 1 | Single zero pass, verified |
| `nist-800-88-purge` | NIST SP 800-88 Purge (overwrite) | 3 | Random, 0x00, random, verified |
| `afssi-5020` | AFSSI-5020 (U.S. Air Force) | 3 | 0x00, 0xFF, random, verified |
| `ar-380-19` | AR 380-19 (U.S. Army) | 3 | Random, 0x00, 0xFF, verified |
| `navso-p-5239-26` | NAVSO P-5239-26 (U.S. Navy) | 3 | 0x01, 0xFE, random, verified |
| `vsitr` | VSITR (German BSI) | 7 | Three 0x00/0xFF pairs, then 0xAA, verified |

## Verification

Methods whose specification mandates a read-back — DoD, NIST SP 800-88, HMG IS5,
and the service-branch standards — are always verified, regardless of the
`auto_verify` setting. Turning `auto_verify` off suppresses verification only for
methods that do not require it.

Verification is a full-surface byte-for-byte comparison for every pattern type.
Random passes are reproduced from the AES-256-CTR seed used to write them, so a
sector that silently failed to take a random pass is detected rather than merely
sampled over.

By default only the final pass is verified. Pass `--verify-each-pass` (or set
`verify_each_pass = true` in the config) to read the whole surface back after
*every* pass, which produces per-pass evidence at roughly double the wall-clock
time.

```bash
sudo drivewipe wipe --device /dev/sda --method dod-short --verify-each-pass
```

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
| `drivewipe-secure-hdd` | Mechanical HDDs | 4-pass overwrite + full verification |
| `drivewipe-secure-sata-ssd` | SATA SSDs | ATA Secure Erase → 4-pass overwrite → TRIM → verify |
| `drivewipe-secure-nvme` | NVMe SSDs | NVMe Sanitize/Format → 4-pass overwrite → deallocate → verify |
| `drivewipe-secure-usb` | USB drives | 4-pass overwrite → TRIM → verify (USB controller limitations) |

The controller sanitize runs *before* the overwrite passes so that the passes
leave the final, verifiable pattern on the surface. It is best-effort: a drive
that does not support it, or that rejects it, is noted in the report and the
overwrite passes carry the wipe on their own.

## Choosing a Method

**For HDDs:**
- Quick: `nist-800-88-clear` (1 verified pass, fastest, and the current standard)
- Standard: `dod-short` (3 pass, still widely named in contracts)
- Maximum: `drivewipe-secure-hdd` (multi-phase with verification)

**A note on DoD 5220.22-M:** the overwrite matrix was removed from the NISPOM in
2007, and the DoD now defers to NIST SP 800-88. DoD 5220.22-M remains available
here because contracts and customer requirements still name it, but for new work
`nist-800-88-clear` or `nist-800-88-purge` is the better choice. Gutmann's 35
passes target 1990s MFM/RLL encodings and offer nothing over a single verified
pass on any drive manufactured this century.

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
