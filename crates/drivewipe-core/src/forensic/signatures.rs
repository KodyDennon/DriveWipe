use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::io::RawDeviceIo;

/// A detected file signature at a specific offset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSignatureHit {
    /// Offset in bytes where the signature was found.
    pub offset: u64,
    /// Type of file detected.
    pub file_type: String,
    /// The magic bytes that were matched.
    pub magic_hex: String,
    /// Confidence level (high, medium, low).
    pub confidence: String,
}

/// Known file signatures (magic bytes).
struct FileSignature {
    name: &'static str,
    magic: &'static [u8],
    offset: usize,
}

const SIGNATURES: &[FileSignature] = &[
    FileSignature { name: "JPEG", magic: b"\xFF\xD8\xFF", offset: 0 },
    FileSignature { name: "PNG", magic: b"\x89PNG\r\n\x1A\n", offset: 0 },
    FileSignature { name: "PDF", magic: b"%PDF", offset: 0 },
    FileSignature { name: "ZIP/DOCX/XLSX", magic: b"PK\x03\x04", offset: 0 },
    FileSignature { name: "GIF87a", magic: b"GIF87a", offset: 0 },
    FileSignature { name: "GIF89a", magic: b"GIF89a", offset: 0 },
    FileSignature { name: "EXE/DLL", magic: b"MZ", offset: 0 },
    FileSignature { name: "ELF", magic: b"\x7FELF", offset: 0 },
    FileSignature { name: "RAR", magic: b"Rar!", offset: 0 },
    FileSignature { name: "7z", magic: b"7z\xBC\xAF\x27\x1C", offset: 0 },
    FileSignature { name: "BMP", magic: b"BM", offset: 0 },
    FileSignature { name: "GZIP", magic: b"\x1F\x8B", offset: 0 },
    FileSignature { name: "SQLite", magic: b"SQLite format 3", offset: 0 },
    FileSignature { name: "MP3/ID3", magic: b"ID3", offset: 0 },
    FileSignature { name: "RIFF/AVI/WAV", magic: b"RIFF", offset: 0 },
    FileSignature { name: "Mach-O 64", magic: b"\xCF\xFA\xED\xFE", offset: 0 },
    FileSignature { name: "Mach-O 32", magic: b"\xCE\xFA\xED\xFE", offset: 0 },
];

/// Scan a device for file signatures.
pub fn scan_signatures(
    device: &mut dyn RawDeviceIo,
    block_size: usize,
) -> Result<Vec<FileSignatureHit>> {
    let capacity = device.capacity();
    let mut buf = vec![0u8; block_size];
    let mut hits = Vec::new();

    // Sample blocks across the device
    let total_blocks = capacity / block_size as u64;
    let step = if total_blocks > 4096 {
        total_blocks / 4096
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
                // Check each signature against the start of this block
                for sig in SIGNATURES {
                    if sig.offset + sig.magic.len() <= n
                        && &buf[sig.offset..sig.offset + sig.magic.len()] == sig.magic
                    {
                        hits.push(FileSignatureHit {
                            offset,
                            file_type: sig.name.to_string(),
                            magic_hex: sig
                                .magic
                                .iter()
                                .map(|b| format!("{b:02X}"))
                                .collect::<Vec<_>>()
                                .join(" "),
                            confidence: "high".to_string(),
                        });
                    }
                }
            }
            _ => break,
        }

        block_idx += step;
    }

    Ok(hits)
}
