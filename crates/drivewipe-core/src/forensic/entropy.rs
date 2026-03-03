use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::io::RawDeviceIo;

/// Summary statistics from entropy analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntropyStats {
    /// Average entropy across all sectors (0.0 = uniform, 8.0 = max random).
    pub average_entropy: f64,
    /// Minimum entropy found.
    pub min_entropy: f64,
    /// Maximum entropy found.
    pub max_entropy: f64,
    /// Percentage of sectors with high entropy (>7.5).
    pub high_entropy_pct: f64,
    /// Percentage of sectors with low entropy (<1.0).
    pub low_entropy_pct: f64,
    /// Percentage of zero-filled sectors.
    pub zero_pct: f64,
    /// Number of sectors analyzed.
    pub sectors_analyzed: u64,
    /// Per-region entropy values for heatmap generation.
    pub heatmap: Vec<f64>,
}

/// Calculate Shannon entropy of a byte buffer.
pub fn shannon_entropy(data: &[u8]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }

    let mut counts = [0u64; 256];
    for &byte in data {
        counts[byte as usize] += 1;
    }

    let len = data.len() as f64;
    let mut entropy = 0.0;
    for &count in &counts {
        if count > 0 {
            let p = count as f64 / len;
            entropy -= p * p.log2();
        }
    }

    entropy
}

/// Analyze entropy across the entire device.
pub fn analyze_entropy(
    device: &mut dyn RawDeviceIo,
    block_size: usize,
) -> Result<EntropyStats> {
    let capacity = device.capacity();
    let mut buf = vec![0u8; block_size];

    let mut total_entropy = 0.0;
    let mut min_entropy = f64::MAX;
    let mut max_entropy = 0.0f64;
    let mut high_count: u64 = 0;
    let mut low_count: u64 = 0;
    let mut zero_count: u64 = 0;
    let mut sectors_analyzed: u64 = 0;
    let mut heatmap = Vec::new();

    // Sample every Nth block for efficiency on large drives
    let total_blocks = capacity / block_size as u64;
    let step = if total_blocks > 1024 {
        total_blocks / 1024
    } else {
        1
    };

    let mut block_idx: u64 = 0;
    while block_idx < total_blocks {
        let offset = block_idx * block_size as u64;
        if offset >= capacity {
            break;
        }

        let read_len = ((capacity - offset) as usize).min(block_size);
        match device.read_at(offset, &mut buf[..read_len]) {
            Ok(n) if n > 0 => {
                let e = shannon_entropy(&buf[..n]);
                total_entropy += e;
                min_entropy = min_entropy.min(e);
                max_entropy = max_entropy.max(e);

                if e > 7.5 {
                    high_count += 1;
                }
                if e < 1.0 {
                    low_count += 1;
                }
                if buf[..n].iter().all(|&b| b == 0) {
                    zero_count += 1;
                }

                heatmap.push(e);
                sectors_analyzed += 1;
            }
            _ => break,
        }

        block_idx += step;
    }

    let average_entropy = if sectors_analyzed > 0 {
        total_entropy / sectors_analyzed as f64
    } else {
        0.0
    };

    if min_entropy == f64::MAX {
        min_entropy = 0.0;
    }

    let total = sectors_analyzed.max(1) as f64;

    Ok(EntropyStats {
        average_entropy,
        min_entropy,
        max_entropy,
        high_entropy_pct: (high_count as f64 / total) * 100.0,
        low_entropy_pct: (low_count as f64 / total) * 100.0,
        zero_pct: (zero_count as f64 / total) * 100.0,
        sectors_analyzed,
        heatmap,
    })
}
