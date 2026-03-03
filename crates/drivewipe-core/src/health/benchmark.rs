use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::io::RawDeviceIo;

/// Results of a sequential I/O micro-benchmark.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    /// Sequential read speed in MiB/s.
    pub sequential_read_mbps: f64,
    /// Sequential write speed in MiB/s (if tested).
    pub sequential_write_mbps: Option<f64>,
    /// Block size used for the benchmark.
    pub block_size: usize,
    /// Number of blocks read/written.
    pub blocks_tested: u64,
    /// Total bytes transferred.
    pub bytes_transferred: u64,
    /// Duration of the benchmark in seconds.
    pub duration_secs: f64,
}

/// Run a quick sequential read benchmark on a device.
///
/// Reads `num_blocks` blocks of `block_size` bytes from the start of the device.
/// This is non-destructive (read-only).
pub fn benchmark_sequential_read(
    device: &mut dyn RawDeviceIo,
    block_size: usize,
    num_blocks: u64,
) -> crate::error::Result<BenchmarkResult> {
    let mut buf = vec![0u8; block_size];
    let mut bytes_read: u64 = 0;
    let capacity = device.capacity();

    let start = Instant::now();

    for i in 0..num_blocks {
        let offset = i * block_size as u64;
        if offset + block_size as u64 > capacity {
            break;
        }
        match device.read_at(offset, &mut buf) {
            Ok(n) => bytes_read += n as u64,
            Err(e) => {
                log::warn!("Benchmark read error at offset {}: {}", offset, e);
                break;
            }
        }
    }

    let duration = start.elapsed().as_secs_f64();
    let mbps = if duration > 0.0 {
        (bytes_read as f64 / (1024.0 * 1024.0)) / duration
    } else {
        0.0
    };

    Ok(BenchmarkResult {
        sequential_read_mbps: mbps,
        sequential_write_mbps: None,
        block_size,
        blocks_tested: bytes_read / block_size as u64,
        bytes_transferred: bytes_read,
        duration_secs: duration,
    })
}
