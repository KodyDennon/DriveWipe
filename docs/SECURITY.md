# Security Policy

## Reporting Vulnerabilities

If you discover a security vulnerability in DriveWipe, please report it responsibly:

1. **Do not** open a public GitHub issue for security vulnerabilities
2. Use [GitHub Security Advisories](https://github.com/NuGit/DriveWipe/security/advisories/new) to report privately, or email the maintainers directly
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
- Boot drive detection prevents accidental self-destruction
- Multi-step confirmation prevents accidental data loss
- All operations are logged for audit purposes
