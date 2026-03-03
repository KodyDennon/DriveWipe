// SPDX-License-Identifier: GPL-2.0
/*
 * DriveWipe Kernel Module — Direct block I/O via submit_bio
 *
 * Provides high-throughput block device I/O by submitting bio requests
 * directly to the block layer, bypassing the page cache and VFS overhead.
 *
 * For write operations (wiping):
 *   1. Allocate page-aligned kernel buffer
 *   2. Copy wipe pattern from userspace
 *   3. Build bio with pages from the buffer
 *   4. submit_bio_wait() for synchronous completion
 *
 * For read operations (verification):
 *   1. Allocate page-aligned kernel buffer
 *   2. Build bio for read
 *   3. submit_bio_wait()
 *   4. Copy data back to userspace
 *
 * This approach gives maximum throughput because:
 *   - No page cache pollution (critical for wiping entire drives)
 *   - No VFS locking overhead
 *   - Direct bio submission to the block device queue
 *   - Multi-page bios for large I/O (up to 4 MiB per request)
 */

#include <linux/kernel.h>
#include <linux/fs.h>
#include <linux/uaccess.h>
#include <linux/slab.h>
#include <linux/blkdev.h>
#include <linux/bio.h>
#include <linux/version.h>

#include "drivewipe_internal.h"
#include "drivewipe_ioctl.h"

/* Maximum single I/O size: 4 MiB. */
#define DW_BIO_MAX_SIZE   (4UL * 1024 * 1024)

/* Maximum pages per bio (4 MiB / 4 KiB = 1024 pages). */
#define DW_BIO_MAX_PAGES  (DW_BIO_MAX_SIZE / PAGE_SIZE)

/*
 * Allocate a page-aligned kernel buffer suitable for bio submission.
 * Returns an array of struct page pointers and the kernel virtual address.
 *
 * The buffer is allocated as individual pages so they can be attached
 * to a bio directly without needing virt_to_page() on vmalloc memory.
 */
static struct page **dw_alloc_bio_pages(unsigned int nr_pages, void **vaddr)
{
	struct page **pages;
	unsigned int i;

	pages = kvzalloc(nr_pages * sizeof(struct page *), GFP_KERNEL);
	if (!pages)
		return NULL;

	for (i = 0; i < nr_pages; i++) {
		pages[i] = alloc_page(GFP_KERNEL | __GFP_ZERO);
		if (!pages[i])
			goto fail;
	}

	/* Map the pages into a contiguous virtual address for copy_from/to_user */
	*vaddr = vmap(pages, nr_pages, VM_MAP, PAGE_KERNEL);
	if (!*vaddr)
		goto fail;

	return pages;

fail:
	while (i--)
		__free_page(pages[i]);
	kvfree(pages);
	return NULL;
}

static void dw_free_bio_pages(struct page **pages, void *vaddr,
			      unsigned int nr_pages)
{
	unsigned int i;

	if (vaddr)
		vunmap(vaddr);

	if (pages) {
		for (i = 0; i < nr_pages; i++) {
			if (pages[i])
				__free_page(pages[i]);
		}
		kvfree(pages);
	}
}

/*
 * Build and submit a bio for direct block I/O.
 *
 * @bdev:      Target block device
 * @sector:    Starting sector number
 * @pages:     Array of pages containing/receiving data
 * @nr_pages:  Number of pages
 * @len:       Total bytes to transfer (may be less than nr_pages * PAGE_SIZE)
 * @op:        REQ_OP_READ or REQ_OP_WRITE
 *
 * Returns 0 on success, negative errno on failure.
 */
static int dw_submit_bio(struct block_device *bdev, sector_t sector,
			 struct page **pages, unsigned int nr_pages,
			 unsigned int len, unsigned int op)
{
	struct bio *bio;
	unsigned int i, this_len;
	int ret;

	/*
	 * bio_alloc() allocates a bio with space for nr_pages bio_vecs.
	 * On kernels 5.18+, bio_alloc takes (bdev, nr_vecs, opf, gfp).
	 * On older kernels, bio_alloc takes (gfp, nr_vecs) and we set
	 * bi_bdev and bi_opf separately.
	 */
#if LINUX_VERSION_CODE >= KERNEL_VERSION(5, 18, 0)
	bio = bio_alloc(bdev, nr_pages, op, GFP_KERNEL);
#else
	bio = bio_alloc(GFP_KERNEL, nr_pages);
	if (!bio)
		return -ENOMEM;
	bio_set_dev(bio, bdev);
	bio->bi_opf = op;
#endif
	if (!bio)
		return -ENOMEM;

	bio->bi_iter.bi_sector = sector;

	/* Add pages to the bio */
	for (i = 0; i < nr_pages && len > 0; i++) {
		this_len = min_t(unsigned int, len, PAGE_SIZE);
		if (bio_add_page(bio, pages[i], this_len, 0) != this_len) {
			/* Bio is full — this shouldn't happen with our
			 * pre-calculated nr_pages, but handle it safely. */
			pr_err("drivewipe: bio_add_page failed at page %u\n", i);
			bio_put(bio);
			return -EIO;
		}
		len -= this_len;
	}

	/* Submit and wait for completion. */
	ret = submit_bio_wait(bio);
	bio_put(bio);

	if (ret) {
		pr_err("drivewipe: bio submission failed: %d (sector=%llu op=%s)\n",
		       ret, (unsigned long long)sector,
		       op == REQ_OP_WRITE ? "write" : "read");
	}

	return ret;
}

/**
 * dw_dma_io - Perform direct block I/O via submit_bio.
 *
 * Opens the target block device, validates the request parameters,
 * allocates page buffers, and submits bio requests directly to the
 * block layer for maximum throughput.
 */
int dw_dma_io(struct dw_dma_request __user *ureq)
{
	struct dw_dma_request req;
	struct file *bdev_file;
	struct block_device *bdev;
	struct page **pages = NULL;
	void *vaddr = NULL;
	unsigned int nr_pages;
	sector_t sector;
	int ret;

	if (copy_from_user(&req, ureq, sizeof(req)))
		return -EFAULT;

	req.device[sizeof(req.device) - 1] = '\0';
	req.bytes_transferred = 0;

	/* Validate request. */
	if (req.length == 0 || req.length > DW_BIO_MAX_SIZE)
		return -EINVAL;

	if (!req.data_ptr)
		return -EINVAL;

	/* Block I/O must be sector-aligned (512 bytes). */
	if (req.offset % 512 != 0 || req.length % 512 != 0) {
		pr_err("drivewipe: DMA I/O requires 512-byte alignment "
		       "(offset=%llu len=%llu)\n",
		       (unsigned long long)req.offset,
		       (unsigned long long)req.length);
		return -EINVAL;
	}

	/* Calculate number of pages needed. */
	nr_pages = (req.length + PAGE_SIZE - 1) / PAGE_SIZE;
	if (nr_pages > DW_BIO_MAX_PAGES)
		return -EINVAL;

	/* Convert byte offset to sector number. */
	sector = req.offset >> 9; /* divide by 512 */

	/* Allocate page-aligned buffer. */
	pages = dw_alloc_bio_pages(nr_pages, &vaddr);
	if (!pages)
		return -ENOMEM;

	/* Open the block device. */
	if (req.write)
		bdev_file = dw_open_bdev(req.device);
	else
		bdev_file = dw_open_bdev_ro(req.device);

	if (IS_ERR(bdev_file)) {
		ret = PTR_ERR(bdev_file);
		goto out_free;
	}

	bdev = I_BDEV(file_inode(bdev_file));

	/* Validate that the I/O doesn't exceed device capacity. */
	{
		sector_t dev_sectors = bdev_nr_sectors(bdev);
		sector_t end_sector = sector + (req.length >> 9);

		if (end_sector > dev_sectors) {
			pr_err("drivewipe: DMA I/O past end of device "
			       "(end=%llu capacity=%llu sectors)\n",
			       (unsigned long long)end_sector,
			       (unsigned long long)dev_sectors);
			ret = -ERANGE;
			goto out_close;
		}
	}

	if (req.write) {
		/* Write: copy data from userspace into our page buffer. */
		if (copy_from_user(vaddr, (void __user *)req.data_ptr,
				   req.length)) {
			ret = -EFAULT;
			goto out_close;
		}

		/* Submit write bio. */
		ret = dw_submit_bio(bdev, sector, pages, nr_pages,
				    (unsigned int)req.length, REQ_OP_WRITE);
		if (ret == 0)
			req.bytes_transferred = req.length;
	} else {
		/* Read: submit read bio, then copy to userspace. */
		ret = dw_submit_bio(bdev, sector, pages, nr_pages,
				    (unsigned int)req.length, REQ_OP_READ);
		if (ret == 0) {
			req.bytes_transferred = req.length;
			if (copy_to_user((void __user *)req.data_ptr,
					 vaddr, req.length)) {
				ret = -EFAULT;
				goto out_close;
			}
		}
	}

out_close:
	fput(bdev_file);

out_free:
	dw_free_bio_pages(pages, vaddr, nr_pages);

	/* Always copy back the result struct (bytes_transferred). */
	if (copy_to_user(ureq, &req, sizeof(req)))
		return -EFAULT;

	return ret;
}

/**
 * dw_dma_cleanup - Module unload cleanup.
 *
 * No persistent state to clean up in the submit_bio path — all buffers
 * are allocated and freed per-request. This function exists to satisfy
 * the interface declared in drivewipe_internal.h.
 */
void dw_dma_cleanup(void)
{
	/* Nothing to clean up — per-request allocation model. */
	pr_info("drivewipe: DMA subsystem cleaned up\n");
}
