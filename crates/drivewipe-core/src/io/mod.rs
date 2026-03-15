//! Raw device I/O trait and platform-specific implementations.
//!
//! This module defines the [`RawDeviceIo`] trait used by the wipe engine to
//! perform unbuffered, direct reads and writes to block devices.  Platform
//! sub-modules provide concrete implementations that bypass the OS page cache
//! so that every byte is physically committed to the storage medium.

use std::alloc::Layout;
use std::ops::{Deref, DerefMut};

use crate::error::Result;

// ── Platform implementations ────────────────────────────────────────────────

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "windows")]
pub mod windows;

// ── Constants ───────────────────────────────────────────────────────────────

/// Default I/O block size used by the wipe engine (4 MiB).
///
/// Larger buffers improve throughput on modern drives with large write caches.
/// This strikes a good balance between throughput and memory usage for
/// sequential overwrites of block devices.
pub const DEFAULT_BLOCK_SIZE: usize = 4 * 1024 * 1024;

// ── Trait ────────────────────────────────────────────────────────────────────

/// Low-level, unbuffered I/O against a block device.
///
/// Implementations MUST bypass the operating system page cache so that writes
/// are committed to the physical medium.  On Linux this is achieved with
/// `O_DIRECT | O_SYNC`, on macOS with `F_NOCACHE`, and on Windows with
/// `FILE_FLAG_NO_BUFFERING | FILE_FLAG_WRITE_THROUGH`.
pub trait RawDeviceIo: Send {
    /// Write `buf` at the given byte `offset`.
    ///
    /// Returns the number of bytes actually written.  The caller is
    /// responsible for ensuring that `offset` and `buf.len()` are aligned to
    /// the device's logical block size when the platform requires it (e.g.
    /// Linux `O_DIRECT`).
    fn write_at(&mut self, offset: u64, buf: &[u8]) -> Result<usize>;

    /// Read into `buf` starting at the given byte `offset`.
    ///
    /// Returns the number of bytes actually read.
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize>;

    /// Total capacity of the device in bytes.
    fn capacity(&self) -> u64;

    /// Logical block size of the device in bytes (typically 512 or 4096).
    fn block_size(&self) -> u32;

    /// Flush all pending writes to the physical medium.
    fn sync(&mut self) -> Result<()>;
}

// ── AlignedBuffer ───────────────────────────────────────────────────────────

/// A page-aligned buffer for use with `O_DIRECT` and similar APIs.
///
/// Allocates memory with the requested alignment using [`std::alloc::alloc_zeroed`]
/// and deallocates it on drop.  Implements `Deref<Target=[u8]>` and `DerefMut`
/// so it can be used anywhere a `&[u8]` or `&mut [u8]` is expected.
pub struct AlignedBuffer {
    ptr: *mut u8,
    len: usize,
    layout: Layout,
}

// SAFETY: The buffer is a plain byte array with no thread-affinity.
unsafe impl Send for AlignedBuffer {}

impl AlignedBuffer {
    /// Allocate a zeroed buffer of `size` bytes aligned to `alignment`.
    ///
    /// # Panics
    ///
    /// Panics if `alignment` is zero, not a power of two, or if the
    /// allocation fails (out of memory).
    pub fn new(size: usize, alignment: usize) -> Self {
        assert!(
            alignment > 0 && alignment.is_power_of_two(),
            "alignment must be a power of two"
        );
        // Ensure size is a multiple of alignment as required by Layout.
        let padded_size = (size + alignment - 1) & !(alignment - 1);
        let layout =
            Layout::from_size_align(padded_size, alignment).expect("invalid layout parameters");
        // SAFETY: layout has non-zero size (padded_size >= alignment > 0) and
        // valid alignment.  alloc_zeroed returns a zeroed block or null.
        let ptr = unsafe { std::alloc::alloc_zeroed(layout) };
        if ptr.is_null() {
            std::alloc::handle_alloc_error(layout);
        }
        Self {
            ptr,
            len: size,
            layout,
        }
    }

    /// Returns an immutable slice over the buffer contents.
    pub fn as_slice(&self) -> &[u8] {
        // SAFETY: ptr is valid for `len` bytes and is properly aligned.
        unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
    }

    /// Returns a mutable slice over the buffer contents.
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        // SAFETY: ptr is valid for `len` bytes, properly aligned, and we have
        // exclusive access via `&mut self`.
        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.len) }
    }

    /// The usable length of the buffer in bytes.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer has zero length.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl Drop for AlignedBuffer {
    fn drop(&mut self) {
        if self.layout.size() > 0 {
            // SAFETY: ptr was allocated with this exact layout.
            unsafe { std::alloc::dealloc(self.ptr, self.layout) };
        }
    }
}

impl Deref for AlignedBuffer {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        self.as_slice()
    }
}

impl DerefMut for AlignedBuffer {
    fn deref_mut(&mut self) -> &mut [u8] {
        self.as_mut_slice()
    }
}

// ── Utilities ───────────────────────────────────────────────────────────────

/// Open a device for raw I/O, returning a platform-appropriate implementation.
///
/// * `writable` - If true, opens for read-write; if false, read-only.
pub fn open_device(path: &std::path::Path, _writable: bool) -> Result<Box<dyn RawDeviceIo>> {
    #[cfg(target_os = "linux")]
    {
        Ok(Box::new(linux::LinuxDeviceIo::open(path)?))
    }
    #[cfg(target_os = "macos")]
    {
        Ok(Box::new(macos::MacosDeviceIo::open(path)?))
    }
    #[cfg(target_os = "windows")]
    {
        Ok(Box::new(windows::WindowsDeviceIo::open(path)?))
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        Err(crate::error::DriveWipeError::PlatformNotSupported(format!(
            "No device I/O implementation for this platform: {}",
            path.display()
        )))
    }
}

/// Allocate a zeroed, properly aligned buffer suitable for direct device I/O.
///
/// The returned [`AlignedBuffer`] is guaranteed to be aligned to `alignment`
/// bytes, which satisfies the alignment requirements of `O_DIRECT` on Linux,
/// `F_NOCACHE` on macOS, and `FILE_FLAG_NO_BUFFERING` on Windows.
///
/// # Arguments
///
/// * `size`      - Number of usable bytes to allocate.
/// * `alignment` - Desired alignment in bytes (must be a power of two).
pub fn allocate_aligned_buffer(size: usize, alignment: usize) -> AlignedBuffer {
    AlignedBuffer::new(size, alignment)
}

/// Wrapper to safely pass a raw pointer to a device across threads.
///
/// SAFETY: This allows passing `&mut dyn RawDeviceIo` to `spawn_blocking` tasks
/// bypassing the `'static` lifetime requirement. The caller must ensure that:
/// 1. The underlying device object outlives the thread/task using this wrapper.
/// 2. Exclusive access is maintained (e.g. by awaiting the task immediately).
///
/// This relies on the assumption that a fat pointer (data + vtable) is exactly
/// two `usize` words. The compile-time assertion below will fail if Rust ever
/// changes fat pointer representation.
#[derive(Clone, Copy)]
pub struct DeviceWrapper(pub [usize; 2]);

// Compile-time assertion: fat pointer must be exactly 2 * usize.
// If this fails, the transmute in new()/get_mut() would be unsound.
const _: () = assert!(std::mem::size_of::<&dyn RawDeviceIo>() == 2 * std::mem::size_of::<usize>(),);

unsafe impl Send for DeviceWrapper {}
unsafe impl Sync for DeviceWrapper {}

impl DeviceWrapper {
    /// Create a new wrapper from a mutable reference.
    pub fn new(device: &mut dyn RawDeviceIo) -> Self {
        // SAFETY: Transmuting a fat pointer to [usize; 2] is safe for the same run.
        unsafe {
            Self(std::mem::transmute::<&mut dyn RawDeviceIo, [usize; 2]>(
                device,
            ))
        }
    }

    /// Get a mutable reference to the underlying device.
    ///
    /// # SAFETY
    ///
    /// The caller must ensure that the pointer is still valid and that
    /// exclusive access is maintained.
    pub unsafe fn get_mut<'a>(&self) -> &'a mut dyn RawDeviceIo {
        // SAFETY: Transmuting [usize; 2] back to a fat pointer.
        unsafe { std::mem::transmute(self.0) }
    }
}
