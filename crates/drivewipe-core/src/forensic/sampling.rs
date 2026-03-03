use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::io::RawDeviceIo;

/// Result of statistical random sector sampling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamplingResult {
    /// Number of sectors sampled.
    pub sectors_sampled: u64,
    /// Total sectors on the device.
    pub total_sectors: u64,
    /// Percentage of sectors that are all-zero.
    pub zero_pct: f64,
    /// Percentage of sectors with high entropy (>7.0).
    pub high_entropy_pct: f64,
    /// Percentage of sectors with non-random, non-zero data.
    pub data_remnant_pct: f64,
    /// Confidence level of the sample (0.0-1.0).
    pub confidence: f64,
    /// Sample ratio used.
    pub sample_ratio: f64,
}

/// Perform statistical random sector sampling to estimate data remnants.
pub fn statistical_sample(
    device: &mut dyn RawDeviceIo,
    sample_ratio: f64,
) -> Result<SamplingResult> {
    let capacity = device.capacity();
    let sector_size = device.block_size() as u64;
    let total_sectors = capacity / sector_size;

    let samples = ((total_sectors as f64 * sample_ratio) as u64)
        .max(100)
        .min(total_sectors);

    let mut rng = rand::rng();
    let mut buf = vec![0u8; sector_size as usize];

    let mut zero_count: u64 = 0;
    let mut high_entropy_count: u64 = 0;
    let mut data_remnant_count: u64 = 0;
    let mut sampled: u64 = 0;

    for _ in 0..samples {
        let sector = rng.random_range(0..total_sectors);
        let offset = sector * sector_size;

        match device.read_at(offset, &mut buf) {
            Ok(n) if n > 0 => {
                let is_zero = buf[..n].iter().all(|&b| b == 0);
                let entropy = super::entropy::shannon_entropy(&buf[..n]);

                if is_zero {
                    zero_count += 1;
                } else if entropy > 7.0 {
                    high_entropy_count += 1;
                } else {
                    data_remnant_count += 1;
                }
                sampled += 1;
            }
            _ => continue,
        }
    }

    let total = sampled.max(1) as f64;

    // Confidence based on sample size relative to population
    let confidence = 1.0 - (1.0 - sample_ratio).powf(sampled as f64);

    Ok(SamplingResult {
        sectors_sampled: sampled,
        total_sectors,
        zero_pct: (zero_count as f64 / total) * 100.0,
        high_entropy_pct: (high_entropy_count as f64 / total) * 100.0,
        data_remnant_pct: (data_remnant_count as f64 / total) * 100.0,
        confidence: confidence.min(1.0),
        sample_ratio,
    })
}
