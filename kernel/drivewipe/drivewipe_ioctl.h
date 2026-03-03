/* SPDX-License-Identifier: GPL-2.0 WITH Linux-syscall-note */
/*
 * DriveWipe Kernel Module — ioctl API
 *
 * Shared header between kernel space and userspace.
 * Character device at /dev/drivewipe. Requires CAP_SYS_RAWIO.
 *
 * Copyright (c) DriveWipe Contributors
 */

#ifndef _DRIVEWIPE_IOCTL_H
#define _DRIVEWIPE_IOCTL_H

#ifdef __KERNEL__
#include <linux/types.h>
#include <linux/ioctl.h>
#else
#include <stdint.h>
#include <sys/ioctl.h>
typedef uint8_t   __u8;
typedef uint16_t  __u16;
typedef uint32_t  __u32;
typedef uint64_t  __u64;
#endif

/* ── Magic number ──────────────────────────────────────────────────────────── */

#define DW_IOC_MAGIC  'D'

/* ── Capability flags (DW_CAP_*) ───────────────────────────────────────────── */

#define DW_CAP_ATA           (1 << 0)
#define DW_CAP_NVME          (1 << 1)
#define DW_CAP_HPA           (1 << 2)
#define DW_CAP_DCO           (1 << 3)
#define DW_CAP_DMA           (1 << 4)
#define DW_CAP_ATA_SECURITY  (1 << 5)

/* ── Structs ───────────────────────────────────────────────────────────────── */

/**
 * struct dw_ata_cmd - Raw ATA command passthrough
 * @device_path:  Target block device path (e.g., "/dev/sda")
 * @command:      ATA command register
 * @feature:      ATA feature register
 * @dev_head:     ATA device/head register
 * @protocol:     ATA protocol (PIO/DMA/non-data)
 * @sector_count: Number of sectors
 * @lba:          48-bit LBA address
 * @data_len:     Length of data transfer in bytes
 * @data_ptr:     Userspace buffer pointer
 * @timeout_ms:   Timeout in milliseconds (0 = default 30s)
 * @status:       (output) ATA status register
 * @error:        (output) ATA error register
 * @result_len:   (output) Actual bytes transferred
 */
struct dw_ata_cmd {
	char  device_path[64];
	__u8  command;
	__u8  feature;
	__u8  dev_head;
	__u8  protocol;
	__u16 sector_count;
	__u16 _pad0;
	__u64 lba;
	__u32 data_len;
	__u32 _pad1;
	__u64 data_ptr;
	__u32 timeout_ms;
	/* Output fields */
	__u8  status;
	__u8  error;
	__u16 _pad2;
	__u32 result_len;
};

/**
 * struct dw_nvme_cmd - Raw NVMe admin command passthrough
 * @device_path: Target NVMe device path (e.g., "/dev/nvme0n1" or "/dev/nvme0")
 */
struct dw_nvme_cmd {
	char  device_path[64];
	__u8  opcode;
	__u8  flags;
	__u16 _pad0;
	__u32 nsid;
	__u32 cdw10;
	__u32 cdw11;
	__u32 cdw12;
	__u32 cdw13;
	__u32 cdw14;
	__u32 cdw15;
	__u32 data_len;
	__u32 _pad1;
	__u64 data_ptr;
	__u32 timeout_ms;
	/* Output */
	__u32 result;
	__u16 status;
	__u16 _pad2;
};

/**
 * struct dw_hpa_info - HPA detection/removal
 * @device:          Device path (e.g., "/dev/sda")
 * @current_max_lba: Current max addressable LBA (from IDENTIFY DEVICE)
 * @native_max_lba:  Native (true hardware) max LBA (from READ NATIVE MAX ADDRESS)
 * @hpa_present:     1 if HPA is present, 0 otherwise
 * @hpa_sectors:     Number of sectors hidden by HPA
 */
struct dw_hpa_info {
	char  device[64];
	__u64 current_max_lba;
	__u64 native_max_lba;
	__u8  hpa_present;
	__u8  _pad0[7];
	__u64 hpa_sectors;
};

/**
 * struct dw_dco_info - DCO detection/restoration
 * @device:          Device path
 * @dco_present:     1 if DCO restrictions are active
 * @dco_real_max_lba: Factory maximum LBA
 * @dco_current_max: Current maximum LBA (may be reduced by DCO)
 * @dco_features:    Raw 512-byte DCO IDENTIFY response
 */
struct dw_dco_info {
	char  device[64];
	__u8  dco_present;
	__u8  _pad0[7];
	__u64 dco_real_max_lba;
	__u64 dco_current_max;
	__u8  dco_features[512];
};

/**
 * struct dw_ata_security_state - ATA security state query
 * @device:                  Device path
 * @supported:               ATA security feature set supported
 * @enabled:                 Security is enabled (password set)
 * @locked:                  Drive is locked
 * @frozen:                  Drive is frozen (security commands rejected)
 * @count_expired:           Erase attempt count expired
 * @enhanced_erase_supported: Enhanced erase is supported
 * @erase_time_normal:       Estimated normal erase time (minutes)
 * @erase_time_enhanced:     Estimated enhanced erase time (minutes)
 */
struct dw_ata_security_state {
	char  device[64];
	__u8  supported;
	__u8  enabled;
	__u8  locked;
	__u8  frozen;
	__u8  count_expired;
	__u8  enhanced_erase_supported;
	__u16 erase_time_normal;
	__u16 erase_time_enhanced;
	__u16 _pad0[3];
};

/**
 * struct dw_dma_request - Block I/O via kernel buffers
 * @device:           Device path
 * @offset:           Byte offset on device
 * @length:           Number of bytes to transfer
 * @data_ptr:         Userspace buffer pointer
 * @write:            1 for write, 0 for read
 * @bytes_transferred: (output) Actual bytes transferred
 */
struct dw_dma_request {
	char  device[64];
	__u64 offset;
	__u64 length;
	__u64 data_ptr;
	__u8  write;
	__u8  _pad0[7];
	__u64 bytes_transferred;
};

/**
 * struct dw_module_info - Module version and capabilities
 * @version_major: Major version number
 * @version_minor: Minor version number
 * @version_patch: Patch version number
 * @capabilities:  Bitmask of DW_CAP_* flags
 */
struct dw_module_info {
	__u32 version_major;
	__u32 version_minor;
	__u32 version_patch;
	__u32 capabilities;
};

/* ── ioctl numbers ─────────────────────────────────────────────────────────── */

/* ATA/NVMe passthrough */
#define DW_IOC_ATA_CMD         _IOWR(DW_IOC_MAGIC, 0x01, struct dw_ata_cmd)
#define DW_IOC_NVME_CMD        _IOWR(DW_IOC_MAGIC, 0x02, struct dw_nvme_cmd)

/* HPA operations */
#define DW_IOC_HPA_DETECT      _IOWR(DW_IOC_MAGIC, 0x10, struct dw_hpa_info)
#define DW_IOC_HPA_REMOVE      _IOWR(DW_IOC_MAGIC, 0x11, struct dw_hpa_info)

/* DCO operations */
#define DW_IOC_DCO_DETECT      _IOWR(DW_IOC_MAGIC, 0x20, struct dw_dco_info)
#define DW_IOC_DCO_RESTORE     _IOWR(DW_IOC_MAGIC, 0x21, struct dw_dco_info)
#define DW_IOC_DCO_FREEZE      _IOWR(DW_IOC_MAGIC, 0x22, struct dw_dco_info)

/* Block I/O via kernel buffers */
#define DW_IOC_DMA_IO          _IOWR(DW_IOC_MAGIC, 0x30, struct dw_dma_request)

/* Security / info */
#define DW_IOC_ATA_SEC_STATE   _IOWR(DW_IOC_MAGIC, 0x40, struct dw_ata_security_state)
#define DW_IOC_MODULE_INFO     _IOR(DW_IOC_MAGIC,  0x50, struct dw_module_info)

#endif /* _DRIVEWIPE_IOCTL_H */
