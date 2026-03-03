// SPDX-License-Identifier: GPL-2.0
/*
 * DriveWipe Kernel Module — Main entry point
 *
 * Registers /dev/drivewipe as a misc character device and dispatches
 * ioctl commands to the appropriate subsystem handlers.
 *
 * All operations require CAP_SYS_RAWIO for safety.
 */

#include <linux/module.h>
#include <linux/kernel.h>
#include <linux/init.h>
#include <linux/miscdevice.h>
#include <linux/fs.h>
#include <linux/uaccess.h>
#include <linux/capability.h>
#include <linux/slab.h>

#include "drivewipe_internal.h"
#include "drivewipe_ioctl.h"

MODULE_LICENSE("GPL");
MODULE_AUTHOR("DriveWipe Contributors");
MODULE_DESCRIPTION("DriveWipe direct ATA/NVMe passthrough and hidden area management");
MODULE_VERSION("1.0.0");

/* ── Device file open ──────────────────────────────────────────────────────── */

static int dw_open(struct inode *inode, struct file *filp)
{
	/* Require CAP_SYS_RAWIO for all operations. */
	if (!capable(CAP_SYS_RAWIO)) {
		pr_warn("drivewipe: open denied — CAP_SYS_RAWIO required\n");
		return -EPERM;
	}
	return 0;
}

/* ── Helper: open block device by path ─────────────────────────────────────── */

struct file *dw_open_bdev(const char *path)
{
	struct file *f;

	f = filp_open(path, O_RDWR | O_EXCL, 0);
	if (IS_ERR(f)) {
		pr_err("drivewipe: failed to open %s: %ld\n", path, PTR_ERR(f));
		return f;
	}

	return f;
}

/* ── ioctl dispatcher ──────────────────────────────────────────────────────── */

static long dw_ioctl(struct file *filp, unsigned int cmd, unsigned long arg)
{
	void __user *uarg = (void __user *)arg;

	/* Double-check CAP_SYS_RAWIO on every ioctl. */
	if (!capable(CAP_SYS_RAWIO))
		return -EPERM;

	switch (cmd) {
	case DW_IOC_ATA_CMD:
		return dw_ata_command((struct dw_ata_cmd __user *)uarg);

	case DW_IOC_NVME_CMD:
		return dw_nvme_command((struct dw_nvme_cmd __user *)uarg);

	case DW_IOC_HPA_DETECT:
		return dw_hpa_detect((struct dw_hpa_info __user *)uarg);

	case DW_IOC_HPA_REMOVE:
		return dw_hpa_remove((struct dw_hpa_info __user *)uarg);

	case DW_IOC_DCO_DETECT:
		return dw_dco_detect((struct dw_dco_info __user *)uarg);

	case DW_IOC_DCO_RESTORE:
		return dw_dco_restore((struct dw_dco_info __user *)uarg);

	case DW_IOC_DCO_FREEZE:
		return dw_dco_freeze((struct dw_dco_info __user *)uarg);

	case DW_IOC_DMA_IO:
		return dw_dma_io((struct dw_dma_request __user *)uarg);

	case DW_IOC_ATA_SEC_STATE:
		return dw_ata_security_state(
			(struct dw_ata_security_state __user *)uarg);

	case DW_IOC_MODULE_INFO: {
		struct dw_module_info info = {
			.version_major = DW_VERSION_MAJOR,
			.version_minor = DW_VERSION_MINOR,
			.version_patch = DW_VERSION_PATCH,
			.capabilities  = DW_CAP_ATA | DW_CAP_NVME |
					 DW_CAP_HPA | DW_CAP_DCO |
					 DW_CAP_DMA | DW_CAP_ATA_SECURITY,
		};
		if (copy_to_user(uarg, &info, sizeof(info)))
			return -EFAULT;
		return 0;
	}

	default:
		pr_warn("drivewipe: unknown ioctl cmd %u\n", cmd);
		return -ENOTTY;
	}
}

/* ── File operations ───────────────────────────────────────────────────────── */

static const struct file_operations dw_fops = {
	.owner          = THIS_MODULE,
	.open           = dw_open,
	.unlocked_ioctl = dw_ioctl,
	.compat_ioctl   = dw_ioctl,
};

/* ── Misc device registration ──────────────────────────────────────────────── */

static struct miscdevice dw_miscdev = {
	.minor = MISC_DYNAMIC_MINOR,
	.name  = "drivewipe",
	.fops  = &dw_fops,
	.mode  = 0600,
};

/* ── Module init/exit ──────────────────────────────────────────────────────── */

static int __init drivewipe_init(void)
{
	int ret;

	ret = misc_register(&dw_miscdev);
	if (ret) {
		pr_err("drivewipe: failed to register misc device: %d\n", ret);
		return ret;
	}

	pr_info("drivewipe: module loaded v%d.%d.%d — /dev/drivewipe ready\n",
		DW_VERSION_MAJOR, DW_VERSION_MINOR, DW_VERSION_PATCH);
	return 0;
}

static void __exit drivewipe_exit(void)
{
	dw_dma_cleanup();
	misc_deregister(&dw_miscdev);
	pr_info("drivewipe: module unloaded\n");
}

module_init(drivewipe_init);
module_exit(drivewipe_exit);
