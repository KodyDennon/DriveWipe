# Troubleshooting

## Common Issues

### "Permission denied" or "Access denied"
DriveWipe requires root/administrator privileges for raw device access.
- Linux/macOS: `sudo drivewipe ...`
- Windows: Run Command Prompt as Administrator

### "Drive is frozen" (ATA Secure Erase)
Some BIOS/UEFI implementations freeze the ATA security state on boot. To unfreeze:
1. Suspend the system to RAM (`systemctl suspend` on Linux)
2. Wake the system
3. Immediately run the erase command

### "No drives detected"
- Ensure the drive is physically connected and powered
- Check that the drive appears in your OS disk utility
- On Linux, ensure `libudev` is installed
- On Windows, ensure the drive appears in Disk Management

### Firmware commands fail through USB
USB bridge controllers often don't pass through ATA/NVMe commands. Use software wipe methods for USB-connected drives, or connect the drive directly via SATA/NVMe.

### Slow wipe performance
- Check drive health — degraded drives are slower
- Ensure no other I/O-intensive processes are running
- For SSDs, performance may drop after the SLC cache is exhausted (this is normal)
- Check system temperature — thermal throttling reduces throughput

### Resume not working
- Ensure the state file exists in `~/.local/share/drivewipe/sessions/`
- State files are saved every 10 seconds — data since the last save is lost
- If the drive was physically removed, resume may not be possible

### Notifications not appearing
- Linux: Ensure D-Bus is running and a notification daemon is installed
- macOS: Check System Settings > Notifications for DriveWipe
- Windows: Check notification settings in Windows Settings

### Sleep prevention not working
- Linux: Check `systemd-inhibit --list` to verify the inhibitor is registered
- macOS: Check `pmset -g assertions` for the IOPMAssertion
- Windows: Verify with `powercfg /requests`
