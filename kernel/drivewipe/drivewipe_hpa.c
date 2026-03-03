// SPDX-License-Identifier: GPL-2.0
/*
 * DriveWipe Kernel Module — HPA/DCO operations
 *
 * Implements Host Protected Area (HPA) and Device Configuration Overlay
 * (DCO) detection and removal via real ATA commands submitted through
 * the SCSI subsystem using scsi_execute_cmd() with ATA_16 CDBs.
 *
 * HPA:
 *   - READ NATIVE MAX ADDRESS (0xF8 / 0x27 for 48-bit)
 *   - SET MAX ADDRESS (0xF9 / 0x37 for 48-bit)
 *
 * DCO:
 *   - DEVICE CONFIGURATION IDENTIFY (0xB1, feature 0xC2)
 *   - DEVICE CONFIGURATION RESTORE (0xB1, feature 0xC3)
 *   - DEVICE CONFIGURATION FREEZE LOCK (0xB1, feature 0xC5)
 *
 * All commands use the internal ATA helpers from drivewipe_ata.c which
 * call scsi_execute_cmd() on the block device's associated scsi_device.
 */

#include <linux/kernel.h>
#include <linux/fs.h>
#include <linux/uaccess.h>
#include <linux/slab.h>
#include <linux/blkdev.h>
#include <scsi/scsi_device.h>

#include "drivewipe_internal.h"
#include "drivewipe_ioctl.h"

/*
 * Open a block device and get its scsi_device, with proper error handling.
 * Returns 0 on success, populating *sdev_out and *bdev_file_out.
 * On success, caller must scsi_device_put(*sdev_out) and fput(*bdev_file_out).
 */
static int dw_open_scsi_dev(const char *device_path, bool readonly,
			    struct scsi_device **sdev_out,
			    struct file **bdev_file_out)
{
	struct file *bdev_file;
	struct block_device *bdev;
	struct scsi_device *sdev;

	bdev_file = readonly ? dw_open_bdev_ro(device_path)
			     : dw_open_bdev(device_path);
	if (IS_ERR(bdev_file))
		return PTR_ERR(bdev_file);

	bdev = I_BDEV(file_inode(bdev_file));
	sdev = dw_bdev_to_scsi(bdev);
	if (!sdev) {
		pr_err("drivewipe: %s is not a SCSI/SATA device\n",
		       device_path);
		fput(bdev_file);
		return -ENODEV;
	}

	if (scsi_device_get(sdev)) {
		fput(bdev_file);
		return -ENODEV;
	}

	*sdev_out = sdev;
	*bdev_file_out = bdev_file;
	return 0;
}

static void dw_close_scsi_dev(struct scsi_device *sdev, struct file *bdev_file)
{
	scsi_device_put(sdev);
	fput(bdev_file);
}

/**
 * dw_hpa_detect - Detect Host Protected Area on a device.
 *
 * Issues IDENTIFY DEVICE via scsi_execute_cmd() to get the current max LBA,
 * then issues READ NATIVE MAX ADDRESS to get the native max LBA.
 * If native > current, an HPA is present.
 */
int dw_hpa_detect(struct dw_hpa_info __user *uinfo)
{
	struct dw_hpa_info info;
	struct scsi_device *sdev;
	struct file *bdev_file;
	u16 *identify;
	u16 word83;
	bool ext48;
	u64 current_max, native_max;
	int ret;

	if (copy_from_user(&info, uinfo, sizeof(info)))
		return -EFAULT;

	info.device[sizeof(info.device) - 1] = '\0';

	ret = dw_open_scsi_dev(info.device, true, &sdev, &bdev_file);
	if (ret)
		return ret;

	/* Allocate buffer for IDENTIFY DEVICE data (512 bytes / 256 words). */
	identify = kvzalloc(512, GFP_KERNEL);
	if (!identify) {
		dw_close_scsi_dev(sdev, bdev_file);
		return -ENOMEM;
	}

	/* Issue IDENTIFY DEVICE to get current max LBA. */
	ret = dw_ata_identify_device(sdev, identify);
	if (ret) {
		kvfree(identify);
		dw_close_scsi_dev(sdev, bdev_file);
		return ret;
	}

	/* Check 48-bit LBA support: word 83 bit 10. */
	word83 = le16_to_cpu(identify[83]);
	ext48 = (word83 & (1 << 10)) != 0;

	/* Current max LBA from IDENTIFY DEVICE.
	 *   48-bit: words 100-103 (4 words, little-endian)
	 *   28-bit: words 60-61 (2 words)
	 */
	if (ext48) {
		current_max = (u64)le16_to_cpu(identify[100])
			    | ((u64)le16_to_cpu(identify[101]) << 16)
			    | ((u64)le16_to_cpu(identify[102]) << 32)
			    | ((u64)le16_to_cpu(identify[103]) << 48);
	} else {
		current_max = (u64)le16_to_cpu(identify[60])
			    | ((u64)le16_to_cpu(identify[61]) << 16);
	}

	kvfree(identify);

	/* Issue READ NATIVE MAX ADDRESS to get true hardware capacity. */
	ret = dw_ata_read_native_max(sdev, ext48, &native_max);
	dw_close_scsi_dev(sdev, bdev_file);

	if (ret)
		return ret;

	/* Populate results. */
	info.current_max_lba = current_max;
	info.native_max_lba = native_max;
	info.hpa_present = (native_max > current_max) ? 1 : 0;
	info.hpa_sectors = (native_max > current_max) ?
			   (native_max - current_max) : 0;

	pr_info("drivewipe: HPA detect on %s: current=%llu native=%llu hpa=%s (%llu sectors)\n",
		info.device,
		(unsigned long long)current_max,
		(unsigned long long)native_max,
		info.hpa_present ? "YES" : "no",
		(unsigned long long)info.hpa_sectors);

	if (copy_to_user(uinfo, &info, sizeof(info)))
		return -EFAULT;

	return 0;
}

/**
 * dw_hpa_remove - Remove HPA by setting max address to native max.
 *
 * First detects the HPA (reads current and native max LBA), then issues
 * SET MAX ADDRESS with the native max LBA to expose all hidden sectors.
 *
 * Note: SET MAX ADDRESS is volatile by default — the HPA will be restored
 * on the next power cycle. For a permanent change, the volatile bit must
 * be cleared (not supported by all drives).
 */
int dw_hpa_remove(struct dw_hpa_info __user *uinfo)
{
	struct dw_hpa_info info;
	struct scsi_device *sdev;
	struct file *bdev_file;
	u16 *identify;
	u16 word83;
	bool ext48;
	u64 current_max, native_max;
	int ret;

	if (copy_from_user(&info, uinfo, sizeof(info)))
		return -EFAULT;

	info.device[sizeof(info.device) - 1] = '\0';

	/* Open with write access for SET MAX ADDRESS. */
	ret = dw_open_scsi_dev(info.device, false, &sdev, &bdev_file);
	if (ret)
		return ret;

	identify = kvzalloc(512, GFP_KERNEL);
	if (!identify) {
		dw_close_scsi_dev(sdev, bdev_file);
		return -ENOMEM;
	}

	/* Get current state via IDENTIFY DEVICE. */
	ret = dw_ata_identify_device(sdev, identify);
	if (ret) {
		kvfree(identify);
		dw_close_scsi_dev(sdev, bdev_file);
		return ret;
	}

	word83 = le16_to_cpu(identify[83]);
	ext48 = (word83 & (1 << 10)) != 0;

	if (ext48) {
		current_max = (u64)le16_to_cpu(identify[100])
			    | ((u64)le16_to_cpu(identify[101]) << 16)
			    | ((u64)le16_to_cpu(identify[102]) << 32)
			    | ((u64)le16_to_cpu(identify[103]) << 48);
	} else {
		current_max = (u64)le16_to_cpu(identify[60])
			    | ((u64)le16_to_cpu(identify[61]) << 16);
	}

	kvfree(identify);

	/* Get native max via READ NATIVE MAX ADDRESS. */
	ret = dw_ata_read_native_max(sdev, ext48, &native_max);
	if (ret) {
		dw_close_scsi_dev(sdev, bdev_file);
		return ret;
	}

	/* If no HPA, nothing to remove. */
	if (native_max <= current_max) {
		dw_close_scsi_dev(sdev, bdev_file);
		info.current_max_lba = current_max;
		info.native_max_lba = native_max;
		info.hpa_present = 0;
		info.hpa_sectors = 0;
		if (copy_to_user(uinfo, &info, sizeof(info)))
			return -EFAULT;
		return 0;
	}

	pr_info("drivewipe: removing HPA on %s: setting max from %llu to %llu "
		"(restoring %llu sectors)\n",
		info.device,
		(unsigned long long)current_max,
		(unsigned long long)native_max,
		(unsigned long long)(native_max - current_max));

	/* Issue SET MAX ADDRESS to remove the HPA. */
	ret = dw_ata_set_max_address(sdev, ext48, native_max);
	dw_close_scsi_dev(sdev, bdev_file);

	if (ret)
		return ret;

	/* After removal, current max should equal native max. */
	info.current_max_lba = native_max;
	info.native_max_lba = native_max;
	info.hpa_present = 0;
	info.hpa_sectors = 0;

	if (copy_to_user(uinfo, &info, sizeof(info)))
		return -EFAULT;

	return 0;
}

/**
 * dw_dco_detect - Detect Device Configuration Overlay.
 *
 * Issues DEVICE CONFIGURATION IDENTIFY (0xB1/0xC2) via scsi_execute_cmd()
 * to read the 512-byte DCO response containing the factory maximum LBA
 * and feature restriction bits. Compares with IDENTIFY DEVICE to determine
 * if DCO restrictions are active.
 */
int dw_dco_detect(struct dw_dco_info __user *uinfo)
{
	struct dw_dco_info info;
	struct scsi_device *sdev;
	struct file *bdev_file;
	u16 *identify;
	u16 word83;
	u64 current_max, factory_max;
	int ret;

	if (copy_from_user(&info, uinfo, sizeof(info)))
		return -EFAULT;

	info.device[sizeof(info.device) - 1] = '\0';

	ret = dw_open_scsi_dev(info.device, true, &sdev, &bdev_file);
	if (ret)
		return ret;

	/* Issue DCO IDENTIFY to get factory capacity and feature bits. */
	ret = dw_ata_dco_identify(sdev, info.dco_features);
	if (ret) {
		dw_close_scsi_dev(sdev, bdev_file);
		return ret;
	}

	/* Parse factory max LBA from DCO IDENTIFY data.
	 * Words 1-3 (bytes 2-7): factory maximum LBA (48-bit, little-endian).
	 */
	factory_max = (u64)info.dco_features[2]
		    | ((u64)info.dco_features[3] << 8)
		    | ((u64)info.dco_features[4] << 16)
		    | ((u64)info.dco_features[5] << 24)
		    | ((u64)info.dco_features[6] << 32)
		    | ((u64)info.dco_features[7] << 40);

	/* Get current max from IDENTIFY DEVICE for comparison. */
	identify = kvzalloc(512, GFP_KERNEL);
	if (!identify) {
		dw_close_scsi_dev(sdev, bdev_file);
		return -ENOMEM;
	}

	ret = dw_ata_identify_device(sdev, identify);
	dw_close_scsi_dev(sdev, bdev_file);

	if (ret) {
		kvfree(identify);
		return ret;
	}

	word83 = le16_to_cpu(identify[83]);
	if (word83 & (1 << 10)) {
		current_max = (u64)le16_to_cpu(identify[100])
			    | ((u64)le16_to_cpu(identify[101]) << 16)
			    | ((u64)le16_to_cpu(identify[102]) << 32)
			    | ((u64)le16_to_cpu(identify[103]) << 48);
	} else {
		current_max = (u64)le16_to_cpu(identify[60])
			    | ((u64)le16_to_cpu(identify[61]) << 16);
	}

	kvfree(identify);

	/* Populate results. */
	info.dco_real_max_lba = factory_max;
	info.dco_current_max = current_max;
	info.dco_present = (factory_max > current_max && factory_max > 0) ? 1 : 0;

	pr_info("drivewipe: DCO detect on %s: factory=%llu current=%llu dco=%s\n",
		info.device,
		(unsigned long long)factory_max,
		(unsigned long long)current_max,
		info.dco_present ? "YES" : "no");

	if (copy_to_user(uinfo, &info, sizeof(info)))
		return -EFAULT;

	return 0;
}

/**
 * dw_dco_restore - Restore DCO to factory settings.
 *
 * Issues DEVICE CONFIGURATION RESTORE (0xB1/0xC3) via scsi_execute_cmd().
 * This resets all DCO restrictions, restoring the drive to its full
 * factory capacity and feature set.
 *
 * WARNING: This is a PERMANENT, IRREVERSIBLE operation.
 * The drive should be power-cycled afterward.
 */
int dw_dco_restore(struct dw_dco_info __user *uinfo)
{
	struct dw_dco_info info;
	struct scsi_device *sdev;
	struct file *bdev_file;
	int ret;

	if (copy_from_user(&info, uinfo, sizeof(info)))
		return -EFAULT;

	info.device[sizeof(info.device) - 1] = '\0';

	ret = dw_open_scsi_dev(info.device, false, &sdev, &bdev_file);
	if (ret)
		return ret;

	pr_info("drivewipe: DCO RESTORE requested for %s\n", info.device);

	/* Issue DCO RESTORE. */
	ret = dw_ata_dco_restore(sdev);
	dw_close_scsi_dev(sdev, bdev_file);

	if (ret)
		return ret;

	/* After restore, re-detect to return updated state. */
	info.dco_present = 0;
	memset(info.dco_features, 0, sizeof(info.dco_features));

	if (copy_to_user(uinfo, &info, sizeof(info)))
		return -EFAULT;

	return 0;
}

/**
 * dw_dco_freeze - Freeze DCO configuration.
 *
 * Issues DEVICE CONFIGURATION FREEZE LOCK (0xB1/0xC5) via scsi_execute_cmd().
 * Prevents any further DCO modifications until the next power cycle.
 */
int dw_dco_freeze(struct dw_dco_info __user *uinfo)
{
	struct dw_dco_info info;
	struct scsi_device *sdev;
	struct file *bdev_file;
	int ret;

	if (copy_from_user(&info, uinfo, sizeof(info)))
		return -EFAULT;

	info.device[sizeof(info.device) - 1] = '\0';

	ret = dw_open_scsi_dev(info.device, false, &sdev, &bdev_file);
	if (ret)
		return ret;

	pr_info("drivewipe: DCO FREEZE requested for %s\n", info.device);

	ret = dw_ata_dco_freeze(sdev);
	dw_close_scsi_dev(sdev, bdev_file);

	if (ret)
		return ret;

	if (copy_to_user(uinfo, &info, sizeof(info)))
		return -EFAULT;

	return 0;
}
