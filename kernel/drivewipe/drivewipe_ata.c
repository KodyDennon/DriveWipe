// SPDX-License-Identifier: GPL-2.0
/*
 * DriveWipe Kernel Module — ATA passthrough via SCSI subsystem
 *
 * Executes ATA commands using scsi_execute_cmd() with ATA_16 CDBs
 * (SCSI-ATA Translation, SAT-4 / T10). This bypasses the SCSI layer's
 * command filtering that may block security commands and vendor-specific
 * operations.
 *
 * Also provides internal helper functions used by drivewipe_hpa.c for
 * HPA/DCO manipulation and ATA security state queries.
 */

#include <linux/kernel.h>
#include <linux/fs.h>
#include <linux/uaccess.h>
#include <linux/slab.h>
#include <linux/blkdev.h>
#include <linux/dma-mapping.h>
#include <scsi/scsi.h>
#include <scsi/scsi_device.h>
#include <scsi/scsi_host.h>
#include <scsi/sg.h>

#include "drivewipe_internal.h"
#include "drivewipe_ioctl.h"

/* ── ATA constants ────────────────────────────────────────────────────────── */

/* ATA_16 CDB opcode (SCSI-ATA Translation, SAT-4) */
#define ATA_16_OPCODE    0x85

/* ATA protocol values for CDB byte 1 (bits 4:1) */
#define ATA_PROTO_NON_DATA     (3 << 1)
#define ATA_PROTO_PIO_DATA_IN  (4 << 1)
#define ATA_PROTO_PIO_DATA_OUT (5 << 1)
#define ATA_PROTO_DMA          (6 << 1)

/* ATA commands we allow through passthrough */
#define ATA_CMD_IDENTIFY       0xEC
#define ATA_CMD_SEC_SET_PASS   0xF1
#define ATA_CMD_SEC_ERASE_UNIT 0xF4
#define ATA_CMD_SEC_DISABLE    0xF6
#define ATA_CMD_READ_NATIVE    0xF8
#define ATA_CMD_SET_MAX        0xF9
#define ATA_CMD_READ_NATIVE_EXT 0x27
#define ATA_CMD_SET_MAX_EXT    0x37
#define ATA_CMD_DEV_CONFIG     0xB1

/* CDB byte 2 flags (T10 SAT-4) */
#define CDB2_CK_COND          (1 << 5)  /* Return ATA regs in sense */
#define CDB2_T_DIR_IN         (1 << 3)  /* Data from device */
#define CDB2_BYTE_BLOCK       (1 << 2)  /* Transfer in blocks */
#define CDB2_T_LEN_SECTORS    (2 << 0)  /* T_LENGTH = sector count */

/* Timeout for standard ATA commands */
#define ATA_TIMEOUT            (30 * HZ)
#define ATA_RETRIES            3

/* ── Command validation ───────────────────────────────────────────────────── */

static bool dw_ata_cmd_allowed(u8 cmd)
{
	switch (cmd) {
	case ATA_CMD_IDENTIFY:
	case ATA_CMD_SEC_SET_PASS:
	case ATA_CMD_SEC_ERASE_UNIT:
	case ATA_CMD_SEC_DISABLE:
	case ATA_CMD_READ_NATIVE:
	case ATA_CMD_SET_MAX:
	case ATA_CMD_READ_NATIVE_EXT:
	case ATA_CMD_SET_MAX_EXT:
	case ATA_CMD_DEV_CONFIG:
		return true;
	default:
		return false;
	}
}

/* ── ATA_16 CDB construction ─────────────────────────────────────────────── */

/*
 * Build an ATA_16 CDB per SAT-4 (T10/BSR INCITS 491).
 *
 * CDB layout (16 bytes):
 *   [0]  = 0x85 (ATA_16 opcode)
 *   [1]  = protocol << 1
 *   [2]  = flags: CK_COND | T_DIR | BYTE_BLOCK | T_LENGTH
 *   [3]  = feature (ext/HOB byte for 48-bit)
 *   [4]  = feature (current byte)
 *   [5]  = sector_count (ext)
 *   [6]  = sector_count (current)
 *   [7]  = lba_low (ext)      = LBA[24:31]
 *   [8]  = lba_low (current)  = LBA[0:7]
 *   [9]  = lba_mid (ext)      = LBA[32:39]
 *   [10] = lba_mid (current)  = LBA[8:15]
 *   [11] = lba_high (ext)     = LBA[40:47]
 *   [12] = lba_high (current) = LBA[16:23]
 *   [13] = device/head
 *   [14] = command
 *   [15] = control (reserved)
 */
static void dw_build_ata16(u8 *cdb, u8 command, u8 protocol, u8 feature,
			   u16 sector_count, u64 lba, u8 device,
			   bool has_data, int direction)
{
	memset(cdb, 0, 16);
	cdb[0]  = ATA_16_OPCODE;
	cdb[1]  = protocol;

	if (has_data) {
		u8 flags = CDB2_BYTE_BLOCK | CDB2_T_LEN_SECTORS;
		if (direction == DMA_FROM_DEVICE)
			flags |= CDB2_T_DIR_IN;
		cdb[2] = flags;
	} else {
		cdb[2] = CDB2_CK_COND; /* No data, but return ATA registers */
	}

	/* Feature register */
	cdb[3] = 0; /* feature ext (HOB) */
	cdb[4] = feature;

	/* Sector count */
	cdb[5] = (u8)((sector_count >> 8) & 0xFF); /* ext */
	cdb[6] = (u8)(sector_count & 0xFF);

	/* LBA (48-bit, split across current and ext registers) */
	cdb[7]  = (u8)((lba >> 24) & 0xFF); /* lba_low ext */
	cdb[8]  = (u8)(lba & 0xFF);         /* lba_low current */
	cdb[9]  = (u8)((lba >> 32) & 0xFF); /* lba_mid ext */
	cdb[10] = (u8)((lba >> 8) & 0xFF);  /* lba_mid current */
	cdb[11] = (u8)((lba >> 40) & 0xFF); /* lba_high ext */
	cdb[12] = (u8)((lba >> 16) & 0xFF); /* lba_high current */

	/* Device register */
	cdb[13] = device;

	/* Command register */
	cdb[14] = command;
}

/* ── Parse ATA descriptor sense data ──────────────────────────────────────── */

/*
 * After an ATA_16 command with CK_COND set, the ATA registers are returned
 * in the descriptor sense data. The ATA Return Descriptor has code 0x09:
 *
 * Byte  0: descriptor code (0x09)
 * Byte  1: additional length (0x0C)
 * Byte  2: extend (bit 0)
 * Byte  3: error
 * Byte  4: sector_count (ext)
 * Byte  5: sector_count
 * Byte  6: lba_low (ext)
 * Byte  7: lba_low
 * Byte  8: lba_mid (ext)
 * Byte  9: lba_mid
 * Byte 10: lba_high (ext)
 * Byte 11: lba_high
 * Byte 12: device
 * Byte 13: status
 */
static bool dw_parse_ata_sense(const u8 *sense, int sense_len,
			       u8 *out_status, u8 *out_error,
			       u64 *out_lba)
{
	int i;

	/* Look for descriptor format sense data (response code 0x72/0x73) */
	if (sense_len < 8)
		return false;
	if ((sense[0] & 0x7F) != 0x72 && (sense[0] & 0x7F) != 0x73)
		return false;

	/* Walk descriptors looking for ATA Return Descriptor (code 0x09) */
	i = 8;
	while (i + 1 < sense_len) {
		u8 desc_code = sense[i];
		u8 desc_len = sense[i + 1];

		if (desc_code == 0x09 && desc_len >= 0x0C && i + 14 <= sense_len) {
			if (out_error)
				*out_error = sense[i + 3];
			if (out_status)
				*out_status = sense[i + 13];
			if (out_lba) {
				u64 lba = 0;
				lba |= (u64)sense[i + 7];          /* lba_low */
				lba |= (u64)sense[i + 9]  << 8;    /* lba_mid */
				lba |= (u64)sense[i + 11] << 16;   /* lba_high */
				lba |= (u64)sense[i + 6]  << 24;   /* lba_low ext */
				lba |= (u64)sense[i + 8]  << 32;   /* lba_mid ext */
				lba |= (u64)sense[i + 10] << 40;   /* lba_high ext */
				*out_lba = lba;
			}
			return true;
		}

		i += desc_len + 2;
	}

	return false;
}

/* ── Internal helper: IDENTIFY DEVICE ─────────────────────────────────────── */

/**
 * dw_ata_identify_device - Issue IDENTIFY DEVICE and return 512-byte data.
 * @sdev:         SCSI device (must be a SATA device)
 * @identify_buf: Output buffer for 256 x u16 words (512 bytes)
 *
 * Returns 0 on success, negative errno on failure.
 */
int dw_ata_identify_device(struct scsi_device *sdev, u16 *identify_buf)
{
	u8 cdb[16];
	u8 sense[32];
	int ret;

	dw_build_ata16(cdb, ATA_CMD_IDENTIFY, ATA_PROTO_PIO_DATA_IN,
		       0, 1, 0, 0x40, true, DMA_FROM_DEVICE);

	memset(sense, 0, sizeof(sense));
	memset(identify_buf, 0, 512);

	ret = dw_scsi_execute(sdev, cdb, sizeof(cdb), DMA_FROM_DEVICE,
			      identify_buf, 512, sense, sizeof(sense),
			      ATA_TIMEOUT, ATA_RETRIES);
	if (ret < 0) {
		pr_err("drivewipe: IDENTIFY DEVICE failed: %d\n", ret);
		return ret;
	}

	/* Check for SCSI error status */
	if (ret > 0) {
		u8 ata_status = 0, ata_error = 0;
		dw_parse_ata_sense(sense, sizeof(sense),
				   &ata_status, &ata_error, NULL);
		pr_err("drivewipe: IDENTIFY DEVICE error: status=%#x error=%#x\n",
		       ata_status, ata_error);
		return -EIO;
	}

	return 0;
}

/* ── Internal helper: READ NATIVE MAX ADDRESS ─────────────────────────────── */

/**
 * dw_ata_read_native_max - Issue READ NATIVE MAX ADDRESS.
 * @sdev:       SCSI device
 * @ext48:      true for 48-bit (0x27), false for 28-bit (0xF8)
 * @native_max: Output: native maximum LBA
 *
 * Returns 0 on success, negative errno on failure.
 */
int dw_ata_read_native_max(struct scsi_device *sdev, bool ext48, u64 *native_max)
{
	u8 cdb[16];
	u8 sense[32];
	int ret;
	u8 ata_status = 0, ata_error = 0;
	u64 lba = 0;
	u8 cmd = ext48 ? ATA_CMD_READ_NATIVE_EXT : ATA_CMD_READ_NATIVE;

	/* READ NATIVE MAX ADDRESS is a non-data command.
	 * The result LBA is returned in the ATA registers via sense data. */
	dw_build_ata16(cdb, cmd, ATA_PROTO_NON_DATA,
		       0, 0, 0, 0x40, /* LBA mode */
		       false, DMA_NONE);

	memset(sense, 0, sizeof(sense));

	ret = dw_scsi_execute(sdev, cdb, sizeof(cdb), DMA_NONE,
			      NULL, 0, sense, sizeof(sense),
			      ATA_TIMEOUT, ATA_RETRIES);
	if (ret < 0) {
		pr_err("drivewipe: READ NATIVE MAX ADDRESS failed: %d\n", ret);
		return ret;
	}

	/* Parse the returned LBA from ATA descriptor sense data */
	if (!dw_parse_ata_sense(sense, sizeof(sense),
				&ata_status, &ata_error, &lba)) {
		pr_warn("drivewipe: READ NATIVE MAX ADDRESS: "
			"no ATA descriptor in sense data\n");
		return -EIO;
	}

	if (ata_error) {
		pr_err("drivewipe: READ NATIVE MAX ADDRESS error: "
		       "status=%#x error=%#x\n", ata_status, ata_error);
		return -EIO;
	}

	if (!ext48)
		lba &= 0x0FFFFFFF; /* 28-bit mask */

	*native_max = lba;
	return 0;
}

/* ── Internal helper: SET MAX ADDRESS ─────────────────────────────────────── */

/**
 * dw_ata_set_max_address - Issue SET MAX ADDRESS.
 * @sdev:    SCSI device
 * @ext48:   true for 48-bit (0x37), false for 28-bit (0xF9)
 * @max_lba: New maximum LBA to set
 *
 * Returns 0 on success, negative errno on failure.
 * WARNING: This changes the drive's accessible capacity.
 */
int dw_ata_set_max_address(struct scsi_device *sdev, bool ext48, u64 max_lba)
{
	u8 cdb[16];
	u8 sense[32];
	int ret;
	u8 ata_status = 0, ata_error = 0;
	u8 cmd = ext48 ? ATA_CMD_SET_MAX_EXT : ATA_CMD_SET_MAX;

	/* SET MAX ADDRESS is a non-data command.
	 * The target LBA is passed in the LBA registers. */
	dw_build_ata16(cdb, cmd, ATA_PROTO_NON_DATA,
		       0, 0, max_lba, 0x40, false, DMA_NONE);

	/* For 28-bit, the top 4 bits of LBA go in device register bits 3:0 */
	if (!ext48)
		cdb[13] = 0x40 | (u8)((max_lba >> 24) & 0x0F);

	memset(sense, 0, sizeof(sense));

	ret = dw_scsi_execute(sdev, cdb, sizeof(cdb), DMA_NONE,
			      NULL, 0, sense, sizeof(sense),
			      ATA_TIMEOUT, ATA_RETRIES);
	if (ret < 0) {
		pr_err("drivewipe: SET MAX ADDRESS failed: %d\n", ret);
		return ret;
	}

	/* Check for errors in sense data */
	dw_parse_ata_sense(sense, sizeof(sense),
			   &ata_status, &ata_error, NULL);
	if (ata_error) {
		pr_err("drivewipe: SET MAX ADDRESS error: "
		       "status=%#x error=%#x\n", ata_status, ata_error);
		return -EIO;
	}

	pr_info("drivewipe: SET MAX ADDRESS to LBA %llu succeeded\n",
		(unsigned long long)max_lba);
	return 0;
}

/* ── Internal helper: DCO IDENTIFY ────────────────────────────────────────── */

/**
 * dw_ata_dco_identify - Issue DEVICE CONFIGURATION IDENTIFY (0xB1/0xC2).
 * @sdev:     SCSI device
 * @dco_data: Output buffer for 512-byte DCO IDENTIFY response
 *
 * Returns 0 on success, negative errno on failure.
 */
int dw_ata_dco_identify(struct scsi_device *sdev, u8 *dco_data)
{
	u8 cdb[16];
	u8 sense[32];
	int ret;

	/* DCO IDENTIFY: command=0xB1, feature=0xC2, PIO data-in, 1 sector */
	dw_build_ata16(cdb, ATA_CMD_DEV_CONFIG, ATA_PROTO_PIO_DATA_IN,
		       0xC2, 1, 0, 0, true, DMA_FROM_DEVICE);

	memset(sense, 0, sizeof(sense));
	memset(dco_data, 0, 512);

	ret = dw_scsi_execute(sdev, cdb, sizeof(cdb), DMA_FROM_DEVICE,
			      dco_data, 512, sense, sizeof(sense),
			      ATA_TIMEOUT, ATA_RETRIES);
	if (ret < 0) {
		pr_err("drivewipe: DCO IDENTIFY failed: %d\n", ret);
		return ret;
	}
	if (ret > 0) {
		u8 ata_status = 0, ata_error = 0;
		dw_parse_ata_sense(sense, sizeof(sense),
				   &ata_status, &ata_error, NULL);
		pr_err("drivewipe: DCO IDENTIFY error: status=%#x error=%#x\n",
		       ata_status, ata_error);
		return -EIO;
	}

	return 0;
}

/* ── Internal helper: DCO RESTORE ─────────────────────────────────────────── */

/**
 * dw_ata_dco_restore - Issue DEVICE CONFIGURATION RESTORE (0xB1/0xC3).
 * @sdev: SCSI device
 *
 * WARNING: This resets ALL DCO restrictions to factory defaults.
 * The change is PERMANENT and survives power cycles.
 * The drive must be power-cycled for changes to take effect.
 *
 * Returns 0 on success, negative errno on failure.
 */
int dw_ata_dco_restore(struct scsi_device *sdev)
{
	u8 cdb[16];
	u8 sense[32];
	int ret;
	u8 ata_status = 0, ata_error = 0;

	/* DCO RESTORE: command=0xB1, feature=0xC3, non-data */
	dw_build_ata16(cdb, ATA_CMD_DEV_CONFIG, ATA_PROTO_NON_DATA,
		       0xC3, 0, 0, 0, false, DMA_NONE);

	memset(sense, 0, sizeof(sense));

	ret = dw_scsi_execute(sdev, cdb, sizeof(cdb), DMA_NONE,
			      NULL, 0, sense, sizeof(sense),
			      60 * HZ, /* DCO RESTORE can take up to 60s */
			      1);
	if (ret < 0) {
		pr_err("drivewipe: DCO RESTORE failed: %d\n", ret);
		return ret;
	}

	dw_parse_ata_sense(sense, sizeof(sense),
			   &ata_status, &ata_error, NULL);
	if (ata_error) {
		pr_err("drivewipe: DCO RESTORE error: status=%#x error=%#x\n",
		       ata_status, ata_error);
		return -EIO;
	}

	pr_info("drivewipe: DCO RESTORE succeeded\n");
	return 0;
}

/* ── Internal helper: DCO FREEZE ──────────────────────────────────────────── */

/**
 * dw_ata_dco_freeze - Issue DEVICE CONFIGURATION FREEZE LOCK (0xB1/0xC5).
 * @sdev: SCSI device
 *
 * Prevents further DCO modifications until the next power cycle.
 *
 * Returns 0 on success, negative errno on failure.
 */
int dw_ata_dco_freeze(struct scsi_device *sdev)
{
	u8 cdb[16];
	u8 sense[32];
	int ret;
	u8 ata_status = 0, ata_error = 0;

	/* DCO FREEZE: command=0xB1, feature=0xC5, non-data */
	dw_build_ata16(cdb, ATA_CMD_DEV_CONFIG, ATA_PROTO_NON_DATA,
		       0xC5, 0, 0, 0, false, DMA_NONE);

	memset(sense, 0, sizeof(sense));

	ret = dw_scsi_execute(sdev, cdb, sizeof(cdb), DMA_NONE,
			      NULL, 0, sense, sizeof(sense),
			      ATA_TIMEOUT, ATA_RETRIES);
	if (ret < 0) {
		pr_err("drivewipe: DCO FREEZE failed: %d\n", ret);
		return ret;
	}

	dw_parse_ata_sense(sense, sizeof(sense),
			   &ata_status, &ata_error, NULL);
	if (ata_error) {
		pr_err("drivewipe: DCO FREEZE error: status=%#x error=%#x\n",
		       ata_status, ata_error);
		return -EIO;
	}

	pr_info("drivewipe: DCO FREEZE succeeded\n");
	return 0;
}

/* ── ioctl handler: DW_IOC_ATA_CMD ────────────────────────────────────────── */

/**
 * dw_ata_command - Execute a raw ATA command via the SCSI subsystem.
 *
 * Opens the target block device, gets the associated scsi_device,
 * builds an ATA_16 CDB, and submits via scsi_execute_cmd().
 */
int dw_ata_command(struct dw_ata_cmd __user *ucmd)
{
	struct dw_ata_cmd cmd;
	struct file *bdev_file;
	struct block_device *bdev;
	struct scsi_device *sdev;
	u8 cdb[16];
	u8 sense[32];
	void *data_buf = NULL;
	int direction;
	int ret;

	if (copy_from_user(&cmd, ucmd, sizeof(cmd)))
		return -EFAULT;

	cmd.device_path[sizeof(cmd.device_path) - 1] = '\0';

	/* Validate command against allowlist. */
	if (!dw_ata_cmd_allowed(cmd.command)) {
		pr_warn("drivewipe: ATA command %#04x not in allowlist\n",
			cmd.command);
		return -EPERM;
	}

	/* Validate data length. */
	if (cmd.data_len > (16 * 1024 * 1024))
		return -EINVAL;

	/* Allocate data buffer if needed. */
	if (cmd.data_len > 0 && cmd.data_ptr) {
		data_buf = kvzalloc(cmd.data_len, GFP_KERNEL);
		if (!data_buf)
			return -ENOMEM;

		/* Copy data from userspace for write commands. */
		if (cmd.protocol == ATA_PROTO_PIO_DATA_OUT ||
		    cmd.protocol == ATA_PROTO_DMA) {
			if (copy_from_user(data_buf,
					   (void __user *)cmd.data_ptr,
					   cmd.data_len)) {
				kvfree(data_buf);
				return -EFAULT;
			}
		}
	}

	/* Determine data direction. */
	if (!data_buf)
		direction = DMA_NONE;
	else if (cmd.protocol == ATA_PROTO_PIO_DATA_IN)
		direction = DMA_FROM_DEVICE;
	else
		direction = DMA_TO_DEVICE;

	/* Build the ATA_16 CDB. */
	dw_build_ata16(cdb, cmd.command, cmd.protocol, cmd.feature,
		       cmd.sector_count, cmd.lba, cmd.dev_head,
		       data_buf != NULL, direction);

	/* Open the block device. */
	bdev_file = dw_open_bdev(cmd.device_path);
	if (IS_ERR(bdev_file)) {
		kvfree(data_buf);
		return PTR_ERR(bdev_file);
	}

	bdev = I_BDEV(file_inode(bdev_file));
	sdev = dw_bdev_to_scsi(bdev);
	if (!sdev) {
		pr_err("drivewipe: %s is not a SCSI/SATA device\n",
		       cmd.device_path);
		fput(bdev_file);
		kvfree(data_buf);
		return -ENODEV;
	}

	/* Take a reference to the SCSI device. */
	if (scsi_device_get(sdev)) {
		fput(bdev_file);
		kvfree(data_buf);
		return -ENODEV;
	}

	/* Execute the command via the SCSI subsystem. */
	memset(sense, 0, sizeof(sense));
	ret = dw_scsi_execute(sdev, cdb, sizeof(cdb), direction,
			      data_buf, cmd.data_len,
			      sense, sizeof(sense),
			      cmd.timeout_ms ?
				msecs_to_jiffies(cmd.timeout_ms) : ATA_TIMEOUT,
			      ATA_RETRIES);

	/* Parse results from sense data. */
	cmd.status = 0;
	cmd.error = 0;
	cmd.result_len = 0;

	if (ret < 0) {
		cmd.status = 0xFF;
		cmd.error = 0xFF;
	} else {
		u8 ata_status = 0, ata_error = 0;

		dw_parse_ata_sense(sense, sizeof(sense),
				   &ata_status, &ata_error, NULL);
		cmd.status = ata_status;
		cmd.error = ata_error;

		if (ret == 0 && data_buf)
			cmd.result_len = cmd.data_len;
	}

	scsi_device_put(sdev);

	/* Copy read data back to userspace. */
	if (data_buf && direction == DMA_FROM_DEVICE && cmd.result_len > 0) {
		if (copy_to_user((void __user *)cmd.data_ptr,
				 data_buf, cmd.result_len)) {
			fput(bdev_file);
			kvfree(data_buf);
			return -EFAULT;
		}
	}

	fput(bdev_file);
	kvfree(data_buf);

	if (copy_to_user(ucmd, &cmd, sizeof(cmd)))
		return -EFAULT;

	return ret < 0 ? ret : 0;
}

/* ── ioctl handler: DW_IOC_ATA_SEC_STATE ──────────────────────────────────── */

/**
 * dw_ata_security_state - Query ATA security state via IDENTIFY DEVICE.
 *
 * Issues IDENTIFY DEVICE via scsi_execute_cmd() and parses the
 * security-related words (82, 128, 89, 90) to populate the state struct.
 */
int dw_ata_security_state(struct dw_ata_security_state __user *ustate)
{
	struct dw_ata_security_state state;
	struct file *bdev_file;
	struct block_device *bdev;
	struct scsi_device *sdev;
	u16 *identify;
	u16 word82, word128;
	int ret;

	if (copy_from_user(&state, ustate, sizeof(state)))
		return -EFAULT;

	state.device[sizeof(state.device) - 1] = '\0';

	identify = kvzalloc(512, GFP_KERNEL);
	if (!identify)
		return -ENOMEM;

	/* Open the device read-only (just querying state). */
	bdev_file = dw_open_bdev_ro(state.device);
	if (IS_ERR(bdev_file)) {
		kvfree(identify);
		return PTR_ERR(bdev_file);
	}

	bdev = I_BDEV(file_inode(bdev_file));
	sdev = dw_bdev_to_scsi(bdev);
	if (!sdev) {
		fput(bdev_file);
		kvfree(identify);
		return -ENODEV;
	}

	if (scsi_device_get(sdev)) {
		fput(bdev_file);
		kvfree(identify);
		return -ENODEV;
	}

	/* Issue IDENTIFY DEVICE. */
	ret = dw_ata_identify_device(sdev, identify);
	scsi_device_put(sdev);
	fput(bdev_file);

	if (ret) {
		kvfree(identify);
		return ret;
	}

	/* Parse security-related IDENTIFY DEVICE words.
	 *
	 * Word 82: Command set supported — bit 1 = Security feature set
	 * Word 128: Security status
	 *   bit 0: Security supported
	 *   bit 1: Security enabled
	 *   bit 2: Security locked
	 *   bit 3: Security frozen
	 *   bit 4: Security count expired
	 *   bit 5: Enhanced erase supported
	 * Word 89: Normal erase time estimate (minutes)
	 * Word 90: Enhanced erase time estimate (minutes)
	 */
	word82  = le16_to_cpu(identify[82]);
	word128 = le16_to_cpu(identify[128]);

	state.supported = ((word128 & (1 << 0)) &&
			   (word82  & (1 << 1))) ? 1 : 0;
	state.enabled   = (word128 & (1 << 1)) ? 1 : 0;
	state.locked    = (word128 & (1 << 2)) ? 1 : 0;
	state.frozen    = (word128 & (1 << 3)) ? 1 : 0;
	state.count_expired = (word128 & (1 << 4)) ? 1 : 0;
	state.enhanced_erase_supported = (word128 & (1 << 5)) ? 1 : 0;
	state.erase_time_normal   = le16_to_cpu(identify[89]);
	state.erase_time_enhanced = le16_to_cpu(identify[90]);

	kvfree(identify);

	if (copy_to_user(ustate, &state, sizeof(state)))
		return -EFAULT;

	return 0;
}
