/* SPDX-License-Identifier: GPL-2.0 */
/*
 * DriveWipe Kernel Module — Internal headers
 *
 * Shared declarations between module source files.
 * Includes kernel version compatibility macros.
 */

#ifndef _DRIVEWIPE_INTERNAL_H
#define _DRIVEWIPE_INTERNAL_H

#include <linux/types.h>
#include <linux/fs.h>
#include <linux/version.h>
#include <linux/blkdev.h>

#include "drivewipe_ioctl.h"

/* Module version */
#define DW_VERSION_MAJOR  1
#define DW_VERSION_MINOR  0
#define DW_VERSION_PATCH  0

/* ── Kernel version compatibility ─────────────────────────────────────────── */

/*
 * scsi_execute_cmd() was introduced in kernel 6.0, replacing scsi_execute().
 * We provide a compatibility wrapper for older kernels.
 */
#include <scsi/scsi.h>
#include <scsi/scsi_device.h>
#include <scsi/scsi_host.h>

#if LINUX_VERSION_CODE >= KERNEL_VERSION(6, 0, 0)
#include <scsi/scsi_cmnd.h>

/*
 * dw_scsi_execute - Execute a SCSI command using scsi_execute_cmd (6.0+).
 * @sdev:      SCSI device
 * @cdb:       Command Descriptor Block
 * @cdb_len:   CDB length
 * @direction: DMA_TO_DEVICE, DMA_FROM_DEVICE, or DMA_NONE
 * @buffer:    Data buffer (kernel address)
 * @bufflen:   Buffer length
 * @sense:     Sense buffer (at least 32 bytes)
 * @sense_len: Sense buffer length
 * @timeout:   Timeout in jiffies
 * @retries:   Number of retries
 *
 * Returns 0 on success, negative errno or positive SCSI status on failure.
 */
static inline int dw_scsi_execute(struct scsi_device *sdev,
				  const unsigned char *cdb, int cdb_len,
				  int direction,
				  void *buffer, unsigned int bufflen,
				  unsigned char *sense, unsigned int sense_len,
				  int timeout, int retries)
{
	struct scsi_exec_args exec_args = {
		.sense = sense,
		.sense_len = sense_len,
	};
	blk_opf_t opf;

	if (direction == DMA_TO_DEVICE)
		opf = REQ_OP_DRV_OUT;
	else
		opf = REQ_OP_DRV_IN;

	return scsi_execute_cmd(sdev, cdb, opf, buffer, bufflen,
				timeout, retries, &exec_args);
}

#else /* Pre-6.0 kernels: use scsi_execute() */

static inline int dw_scsi_execute(struct scsi_device *sdev,
				  const unsigned char *cdb, int cdb_len,
				  int direction,
				  void *buffer, unsigned int bufflen,
				  unsigned char *sense, unsigned int sense_len,
				  int timeout, int retries)
{
	struct scsi_sense_hdr sshdr;

	return scsi_execute(sdev, cdb, direction, buffer, bufflen,
			    sense, &sshdr, timeout, retries, 0, 0, NULL);
}
#endif /* LINUX_VERSION_CODE >= 6.0 */

/*
 * Get the scsi_device associated with a block device.
 *
 * For SATA/SAS/SCSI drives, the device hierarchy is:
 *   scsi_device -> scsi_disk -> gendisk -> block_device
 *
 * Returns NULL if the block device is not backed by a SCSI device
 * (e.g., for NVMe devices).
 */
static inline struct scsi_device *dw_bdev_to_scsi(struct block_device *bdev)
{
	struct device *parent;

	if (!bdev || !bdev->bd_disk)
		return NULL;

	parent = disk_to_dev(bdev->bd_disk)->parent;
	if (!parent)
		return NULL;

	/*
	 * Verify this is a SCSI device. The parent of a SCSI disk's
	 * gendisk device is the scsi_device's sdev_gendev, which is
	 * on the SCSI bus.
	 */
	if (!parent->bus || strcmp(parent->bus->name, "scsi") != 0)
		return NULL;

	return to_scsi_device(parent);
}

/* ── ATA passthrough (drivewipe_ata.c) ─────────────────────────────────────── */

int dw_ata_command(struct dw_ata_cmd __user *ucmd);

/* Internal ATA helpers used by drivewipe_hpa.c */
int dw_ata_identify_device(struct scsi_device *sdev, u16 *identify_buf);
int dw_ata_read_native_max(struct scsi_device *sdev, bool ext48, u64 *native_max);
int dw_ata_set_max_address(struct scsi_device *sdev, bool ext48, u64 max_lba);
int dw_ata_dco_identify(struct scsi_device *sdev, u8 *dco_data);
int dw_ata_dco_restore(struct scsi_device *sdev);
int dw_ata_dco_freeze(struct scsi_device *sdev);

/* ── NVMe passthrough (drivewipe_nvme.c) ───────────────────────────────────── */

int dw_nvme_command(struct dw_nvme_cmd __user *ucmd);

/* ── HPA/DCO operations (drivewipe_hpa.c) ──────────────────────────────────── */

int dw_hpa_detect(struct dw_hpa_info __user *uinfo);
int dw_hpa_remove(struct dw_hpa_info __user *uinfo);
int dw_dco_detect(struct dw_dco_info __user *uinfo);
int dw_dco_restore(struct dw_dco_info __user *uinfo);
int dw_dco_freeze(struct dw_dco_info __user *uinfo);
int dw_ata_security_state(struct dw_ata_security_state __user *ustate);

/* ── DMA I/O (drivewipe_dma.c) ─────────────────────────────────────────────── */

int dw_dma_io(struct dw_dma_request __user *ureq);
void dw_dma_cleanup(void);

/* ── Helpers ───────────────────────────────────────────────────────────────── */

/**
 * dw_open_bdev - Open a block device by path.
 * @path: Null-terminated device path (e.g., "/dev/sda")
 * @mode: O_RDONLY or O_RDWR
 *
 * Returns a file pointer on success, ERR_PTR on failure.
 * Caller must fput() when done.
 */
struct file *dw_open_bdev(const char *path);
struct file *dw_open_bdev_ro(const char *path);

#endif /* _DRIVEWIPE_INTERNAL_H */
