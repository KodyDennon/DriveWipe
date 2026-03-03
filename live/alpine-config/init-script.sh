#!/bin/sh
# DriveWipe Live USB init script
# Runs after Alpine base init, launches DriveWipe TUI

# Set terminal type
export TERM=linux

# Wait for storage devices to settle
sleep 2

# Trigger udev to enumerate all devices
udevadm trigger
udevadm settle --timeout=10

# Set hostname
hostname drivewipe-live

# Display banner
clear
echo "=================================================="
echo "  DriveWipe Live USB"
echo "  Secure Drive Management"
echo "=================================================="
echo ""
echo "Detecting drives..."
sleep 1

# Launch DriveWipe TUI
if [ -x /usr/local/bin/drivewipe-tui ]; then
    /usr/local/bin/drivewipe-tui
else
    echo "ERROR: DriveWipe TUI binary not found."
    echo "Dropping to shell."
fi

# After TUI exits, offer shell
echo ""
echo "DriveWipe TUI exited. Type 'drivewipe-tui' to restart."
echo "Type 'poweroff' to shut down."
exec /bin/sh
