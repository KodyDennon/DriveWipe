// SPDX-License-Identifier: GPL-2.0
/*
 * DriveWipe Kernel Module — NVMe admin command passthrough
 *
 * Executes NVMe admin commands by opening the NVMe character device
 * (e.g., /dev/nvme0) from kernel space and issuing NVME_IOCTL_ADMIN_CMD
 * via vfs_ioctl(). This is the correct approach for out-of-tree modules
 * because:
 *
 *   1. nvme_submit_sync_cmd() requires access to nvme_ctrl->admin_q,
 *      which lives in the internal nvme.h header (not exported).
 *
 *   2. set_fs() was removed in kernel 5.18, so we can't use
 *      copy_from_user() with kernel addresses.
 *
 *   3. The NVMe character device (/dev/nvme0) exposes
 *      NVME_IOCTL_ADMIN_CMD which does exactly what we need.
 *
 * The module validates commands against an allowlist before forwarding
 * them to the NVMe character device.
 *
 * Command flow:
 *   userspace -> /dev/drivewipe ioctl -> validate -> open /dev/nvmeN
 *   -> NVME_IOCTL_ADMIN_CMD -> return result
 */

#include <linux/kernel.h>
#include <linux/fs.h>
#include <linux/uaccess.h>
#include <linux/slab.h>
#include <linux/nvme_ioctl.h>
#include <linux/version.h>

#include "drivewipe_internal.h"
#include "drivewipe_ioctl.h"

/* NVMe admin command opcodes we allow through. */
#define NVME_ADM_IDENTIFY    0x06
#define NVME_ADM_GET_LOG     0x02
#define NVME_ADM_FORMAT_NVM  0x80
#define NVME_ADM_SANITIZE    0x84
#define NVME_ADM_SEC_SEND    0x81
#define NVME_ADM_SEC_RECV    0x82

static bool dw_nvme_cmd_allowed(u8 opcode)
{
	switch (opcode) {
	case NVME_ADM_IDENTIFY:
	case NVME_ADM_GET_LOG:
	case NVME_ADM_FORMAT_NVM:
	case NVME_ADM_SANITIZE:
	case NVME_ADM_SEC_SEND:
	case NVME_ADM_SEC_RECV:
		return true;
	default:
		return false;
	}
}

/*
 * Determine if an NVMe admin opcode transfers data TO the device.
 * Used to decide whether to pre-fill the data buffer from userspace.
 */
static bool dw_nvme_is_write_cmd(u8 opcode)
{
	switch (opcode) {
	case NVME_ADM_SEC_SEND:
	case NVME_ADM_FORMAT_NVM:
	case NVME_ADM_SANITIZE:
		return true;
	default:
		return false;
	}
}

/*
 * Determine if an NVMe admin opcode transfers data FROM the device.
 * Used to decide whether to copy the data buffer back to userspace.
 */
static bool dw_nvme_is_read_cmd(u8 opcode)
{
	switch (opcode) {
	case NVME_ADM_IDENTIFY:
	case NVME_ADM_GET_LOG:
	case NVME_ADM_SEC_RECV:
		return true;
	default:
		return false;
	}
}

/*
 * Derive the NVMe character device path from a namespace device path.
 *
 * Input:  /dev/nvme0n1  ->  Output: /dev/nvme0
 * Input:  /dev/nvme0    ->  Output: /dev/nvme0  (unchanged)
 * Input:  /dev/nvme12n1 ->  Output: /dev/nvme12
 *
 * NVMe admin commands must be sent to the controller char device,
 * not the namespace block device.
 */
static int dw_nvme_ctrl_path(const char *device_path, char *ctrl_path,
			     size_t ctrl_path_size)
{
	const char *p;
	size_t base_len;

	/* Find "nvme" in the path */
	p = strstr(device_path, "nvme");
	if (!p)
		return -EINVAL;

	/* Skip past "nvme" */
	p += 4;

	/* Skip digits (controller number) */
	while (*p >= '0' && *p <= '9')
		p++;

	/* If we hit 'n' followed by digits, that's the namespace — strip it */
	if (*p == 'n' && *(p + 1) >= '0' && *(p + 1) <= '9') {
		base_len = p - device_path;
	} else {
		/* Already a controller path, or no namespace suffix */
		base_len = strlen(device_path);
	}

	if (base_len >= ctrl_path_size)
		return -ENAMETOOLONG;

	memcpy(ctrl_path, device_path, base_len);
	ctrl_path[base_len] = '\0';
	return 0;
}

/**
 * dw_nvme_command - Execute a raw NVMe admin command.
 *
 * Validates the command against the allowlist, then opens the NVMe
 * controller character device and issues NVME_IOCTL_ADMIN_CMD.
 *
 * The data buffer is managed in kernel space: data is copied from
 * userspace before write commands and copied back after read commands.
 */
int dw_nvme_command(struct dw_nvme_cmd __user *ucmd)
{
	struct dw_nvme_cmd cmd;
	struct nvme_passthru_cmd pcmd;
	struct file *nvme_file;
	char ctrl_path[68];
	void *data_buf = NULL;
	int ret;

	if (copy_from_user(&cmd, ucmd, sizeof(cmd)))
		return -EFAULT;

	cmd.device_path[sizeof(cmd.device_path) - 1] = '\0';

	/* Validate command against allowlist. */
	if (!dw_nvme_cmd_allowed(cmd.opcode)) {
		pr_warn("drivewipe: NVMe opcode %#04x not in allowlist\n",
			cmd.opcode);
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
		if (dw_nvme_is_write_cmd(cmd.opcode)) {
			if (copy_from_user(data_buf,
					   (void __user *)cmd.data_ptr,
					   cmd.data_len)) {
				kvfree(data_buf);
				return -EFAULT;
			}
		}
	}

	/* Derive the NVMe controller char device path. */
	ret = dw_nvme_ctrl_path(cmd.device_path, ctrl_path, sizeof(ctrl_path));
	if (ret) {
		pr_err("drivewipe: cannot derive NVMe controller path from %s\n",
		       cmd.device_path);
		kvfree(data_buf);
		return ret;
	}

	/* Open the NVMe controller character device. */
	nvme_file = filp_open(ctrl_path, O_RDWR, 0);
	if (IS_ERR(nvme_file)) {
		pr_err("drivewipe: failed to open NVMe device %s: %ld\n",
		       ctrl_path, PTR_ERR(nvme_file));
		kvfree(data_buf);
		return PTR_ERR(nvme_file);
	}

	/* Build the NVMe passthrough command struct.
	 *
	 * struct nvme_passthru_cmd is defined in <linux/nvme_ioctl.h>:
	 *   __u8  opcode, flags
	 *   __u16 rsvd1
	 *   __u32 nsid, cdw2, cdw3
	 *   __u64 metadata, addr
	 *   __u32 metadata_len, data_len
	 *   __u32 cdw10..cdw15
	 *   __u32 timeout_ms
	 *   __u32 result
	 */
	memset(&pcmd, 0, sizeof(pcmd));
	pcmd.opcode     = cmd.opcode;
	pcmd.flags      = cmd.flags;
	pcmd.nsid       = cmd.nsid;
	pcmd.cdw10      = cmd.cdw10;
	pcmd.cdw11      = cmd.cdw11;
	pcmd.cdw12      = cmd.cdw12;
	pcmd.cdw13      = cmd.cdw13;
	pcmd.cdw14      = cmd.cdw14;
	pcmd.cdw15      = cmd.cdw15;
	pcmd.data_len   = cmd.data_len;
	pcmd.timeout_ms = cmd.timeout_ms ? cmd.timeout_ms : 30000;

	/*
	 * The NVMe char device ioctl (NVME_IOCTL_ADMIN_CMD) expects the
	 * data buffer address in pcmd.addr as a userspace pointer. Since
	 * we're calling from kernel space, we need to handle this carefully.
	 *
	 * On kernels with set_fs() (pre-5.18), we could use set_fs(KERNEL_DS).
	 * On modern kernels, the NVMe driver's ioctl path calls
	 * copy_from_user/copy_to_user internally on pcmd.addr.
	 *
	 * The correct approach is to pass the ORIGINAL userspace pointer
	 * (cmd.data_ptr) directly, since the NVMe driver will handle the
	 * user/kernel copy itself. We don't need to copy data ourselves
	 * when going through the ioctl path — the NVMe driver does it.
	 */
	pcmd.addr = cmd.data_ptr; /* Pass the userspace pointer directly */

	/* Issue NVME_IOCTL_ADMIN_CMD via the NVMe char device's ioctl. */
	if (!nvme_file->f_op || !nvme_file->f_op->unlocked_ioctl) {
		pr_err("drivewipe: NVMe device %s has no ioctl handler\n",
		       ctrl_path);
		fput(nvme_file);
		kvfree(data_buf);
		return -ENODEV;
	}

	ret = nvme_file->f_op->unlocked_ioctl(nvme_file,
					       NVME_IOCTL_ADMIN_CMD,
					       (unsigned long)&pcmd);

	fput(nvme_file);

	/* Map kernel error codes. */
	if (ret < 0) {
		pr_err("drivewipe: NVMe admin command %#04x failed: %d\n",
		       cmd.opcode, ret);
		cmd.status = 0xFFFF;
		cmd.result = 0;
	} else {
		/*
		 * The NVMe ioctl returns the NVMe status code in the
		 * return value: 0 = success, positive = NVMe status.
		 */
		cmd.status = (u16)ret;
		cmd.result = pcmd.result;
		ret = 0; /* Normalize: we report NVMe status in cmd.status */
	}

	/* Copy updated command struct back to userspace. */
	if (copy_to_user(ucmd, &cmd, sizeof(cmd))) {
		kvfree(data_buf);
		return -EFAULT;
	}

	kvfree(data_buf);
	return ret;
}
