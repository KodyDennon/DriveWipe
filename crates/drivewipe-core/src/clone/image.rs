use std::io::{Read, Write};

use serde::{Deserialize, Serialize};

use super::CompressionMode;

/// Magic bytes identifying a DriveWipe clone image.
const IMAGE_MAGIC: &[u8; 8] = b"DWCLONE\x01";

/// Header for a DriveWipe clone image file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloneImageHeader {
    /// Image format version.
    pub version: u32,
    /// Source device info.
    pub source_model: String,
    pub source_serial: String,
    pub source_capacity: u64,
    /// Block size used for chunks.
    pub block_size: u32,
    /// Total number of data chunks.
    pub chunk_count: u64,
    /// Compression mode.
    pub compression: CompressionMode,
    /// Whether the data is encrypted.
    pub encrypted: bool,
    /// BLAKE3 hash of the uncompressed source data.
    pub source_hash: Option<String>,
    /// Creation timestamp.
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// A clone image consisting of a header followed by data chunks.
pub struct CloneImage;

impl CloneImage {
    /// Write image header to a writer.
    pub fn write_header<W: Write>(
        writer: &mut W,
        header: &CloneImageHeader,
    ) -> crate::error::Result<()> {
        writer.write_all(IMAGE_MAGIC).map_err(|e| {
            crate::error::DriveWipeError::Clone(format!("Failed to write image magic: {e}"))
        })?;

        let header_json = serde_json::to_vec(header).map_err(|e| {
            crate::error::DriveWipeError::Clone(format!("Failed to serialize header: {e}"))
        })?;

        let header_len = header_json.len() as u32;
        writer.write_all(&header_len.to_le_bytes()).map_err(|e| {
            crate::error::DriveWipeError::Clone(format!("Failed to write header length: {e}"))
        })?;
        writer.write_all(&header_json).map_err(|e| {
            crate::error::DriveWipeError::Clone(format!("Failed to write header: {e}"))
        })?;

        Ok(())
    }

    /// Read image header from a reader.
    pub fn read_header<R: Read>(reader: &mut R) -> crate::error::Result<CloneImageHeader> {
        let mut magic = [0u8; 8];
        reader.read_exact(&mut magic).map_err(|e| {
            crate::error::DriveWipeError::Clone(format!("Failed to read image magic: {e}"))
        })?;

        if &magic != IMAGE_MAGIC {
            return Err(crate::error::DriveWipeError::Clone(
                "Invalid clone image: bad magic bytes".to_string(),
            ));
        }

        let mut len_buf = [0u8; 4];
        reader.read_exact(&mut len_buf).map_err(|e| {
            crate::error::DriveWipeError::Clone(format!("Failed to read header length: {e}"))
        })?;
        let header_len = u32::from_le_bytes(len_buf) as usize;

        let mut header_buf = vec![0u8; header_len];
        reader.read_exact(&mut header_buf).map_err(|e| {
            crate::error::DriveWipeError::Clone(format!("Failed to read header: {e}"))
        })?;

        let header: CloneImageHeader = serde_json::from_slice(&header_buf).map_err(|e| {
            crate::error::DriveWipeError::Clone(format!("Failed to parse header: {e}"))
        })?;

        Ok(header)
    }

    /// Write a compressed data chunk.
    pub fn write_chunk<W: Write>(
        writer: &mut W,
        data: &[u8],
        compression: CompressionMode,
    ) -> crate::error::Result<()> {
        let compressed = match compression {
            CompressionMode::None => data.to_vec(),
            CompressionMode::Gzip => {
                use flate2::Compression;
                use flate2::write::GzEncoder;
                let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
                encoder.write_all(data).map_err(|e| {
                    crate::error::DriveWipeError::Compression(format!("Gzip compress failed: {e}"))
                })?;
                encoder.finish().map_err(|e| {
                    crate::error::DriveWipeError::Compression(format!("Gzip finish failed: {e}"))
                })?
            }
            CompressionMode::Zstd => zstd::encode_all(data, 3).map_err(|e| {
                crate::error::DriveWipeError::Compression(format!("Zstd compress failed: {e}"))
            })?,
        };

        let chunk_len = compressed.len() as u32;
        writer.write_all(&chunk_len.to_le_bytes()).map_err(|e| {
            crate::error::DriveWipeError::Clone(format!("Failed to write chunk length: {e}"))
        })?;
        writer.write_all(&compressed).map_err(|e| {
            crate::error::DriveWipeError::Clone(format!("Failed to write chunk data: {e}"))
        })?;

        Ok(())
    }

    /// Read and decompress a data chunk.
    pub fn read_chunk<R: Read>(
        reader: &mut R,
        compression: CompressionMode,
    ) -> crate::error::Result<Vec<u8>> {
        let mut len_buf = [0u8; 4];
        reader.read_exact(&mut len_buf).map_err(|e| {
            crate::error::DriveWipeError::Clone(format!("Failed to read chunk length: {e}"))
        })?;
        let chunk_len = u32::from_le_bytes(len_buf) as usize;

        let mut compressed = vec![0u8; chunk_len];
        reader.read_exact(&mut compressed).map_err(|e| {
            crate::error::DriveWipeError::Clone(format!("Failed to read chunk data: {e}"))
        })?;

        let data = match compression {
            CompressionMode::None => compressed,
            CompressionMode::Gzip => {
                use flate2::read::GzDecoder;
                let mut decoder = GzDecoder::new(&compressed[..]);
                let mut decompressed = Vec::new();
                decoder.read_to_end(&mut decompressed).map_err(|e| {
                    crate::error::DriveWipeError::Compression(format!(
                        "Gzip decompress failed: {e}"
                    ))
                })?;
                decompressed
            }
            CompressionMode::Zstd => zstd::decode_all(&compressed[..]).map_err(|e| {
                crate::error::DriveWipeError::Compression(format!("Zstd decompress failed: {e}"))
            })?,
        };

        Ok(data)
    }
}
