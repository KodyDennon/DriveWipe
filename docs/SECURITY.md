# Security Policy

## Reporting Vulnerabilities

If you discover a security vulnerability in DriveWipe, please report it responsibly:

1. **Do not** open a public GitHub issue for security vulnerabilities
2. Use [GitHub Security Advisories](https://github.com/KodyDennon/DriveWipe/security/advisories/new) to report privately, or email the maintainers directly
3. Include steps to reproduce, if possible
4. Allow reasonable time for a fix before public disclosure

## Scope

Security concerns for DriveWipe include:

- **Bypasses of safety checks** (e.g., wiping boot drive, skipping confirmation)
- **Data leakage** (e.g., sensitive data in logs, reports, or temporary files)
- **Insufficient erasure** (e.g., patterns not written correctly, sectors skipped)
- **Privilege escalation** from the tool
- **Cryptographic weaknesses** in the PRNG or crypto erase implementations

## Design Principles

- The AES-256-CTR PRNG is seeded from the OS CSPRNG and uses hardware AES-NI acceleration
- Sensitive memory (keys, patterns) is zeroized on drop using the `zeroize` crate
- The tool refuses to operate without elevated privileges
- Boot drive detection prevents accidental self-destruction (Linux: `/proc/mounts`, macOS: `/sbin/mount`, Windows: `IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS`)
- Multi-step confirmation prevents accidental data loss
- All operations are logged for audit purposes

## Firmware Wipe Security

- **ATA Secure Erase** uses a temporary password set and cleared within the same session. If the erase is interrupted, the drive may remain password-locked. The tool attempts to disable the password after erase completion.
- **ATA security state** is checked before issuing erase commands. Frozen drives are rejected with guidance to suspend/resume the system. Locked drives are rejected.
- **NVMe Sanitize** operations are asynchronous at the firmware level. The tool polls sanitize progress and reports completion status. An interrupted sanitize can be recovered by the drive's firmware.
- **TCG Opal crypto erase** destroys the encryption key on self-encrypting drives. Drives with non-default SID passwords require the current password; the tool cannot bypass drive ownership.
- **macOS ATA limitation**: ATA Secure Erase is intentionally not supported on macOS due to the lack of a reliable ATA passthrough mechanism. NVMe commands on macOS require `nvme-cli`.
- **USB bridge warning**: Firmware commands (ATA/NVMe/TCG Opal) may fail or behave unpredictably through USB-to-SATA/NVMe bridge adapters. The tool warns users when USB-attached drives are detected.
