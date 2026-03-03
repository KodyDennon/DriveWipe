use serde::{Deserialize, Serialize};

/// Detected filesystem type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FilesystemType {
    Ntfs,
    Fat32,
    Fat16,
    ExFat,
    Ext2,
    Ext3,
    Ext4,
    Xfs,
    Btrfs,
    Hfs,
    Apfs,
    Swap,
    Unknown,
}

impl std::fmt::Display for FilesystemType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ntfs => write!(f, "NTFS"),
            Self::Fat32 => write!(f, "FAT32"),
            Self::Fat16 => write!(f, "FAT16"),
            Self::ExFat => write!(f, "exFAT"),
            Self::Ext2 => write!(f, "ext2"),
            Self::Ext3 => write!(f, "ext3"),
            Self::Ext4 => write!(f, "ext4"),
            Self::Xfs => write!(f, "XFS"),
            Self::Btrfs => write!(f, "Btrfs"),
            Self::Hfs => write!(f, "HFS+"),
            Self::Apfs => write!(f, "APFS"),
            Self::Swap => write!(f, "Linux Swap"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Detect filesystem type from magic bytes at the start of a partition.
pub fn detect_filesystem(data: &[u8]) -> FilesystemType {
    if data.len() < 1024 {
        return FilesystemType::Unknown;
    }

    // NTFS: "NTFS    " at offset 3
    if data.len() > 11 && &data[3..7] == b"NTFS" {
        return FilesystemType::Ntfs;
    }

    // FAT32: "FAT32   " at offset 82
    if data.len() > 90 && &data[82..87] == b"FAT32" {
        return FilesystemType::Fat32;
    }

    // FAT16: "FAT16   " or "FAT12   " at offset 54
    if data.len() > 62 && (&data[54..59] == b"FAT16" || &data[54..59] == b"FAT12") {
        return FilesystemType::Fat16;
    }

    // exFAT: "EXFAT   " at offset 3
    if data.len() > 11 && &data[3..8] == b"EXFAT" {
        return FilesystemType::ExFat;
    }

    // ext2/3/4: magic 0xEF53 at offset 1080 (superblock at 1024 + magic at offset 56)
    if data.len() > 1082 && data[1080] == 0x53 && data[1081] == 0xEF {
        // Distinguish ext2/3/4 by feature flags
        if data.len() > 1120 {
            let compat = u32::from_le_bytes(data[1116..1120].try_into().unwrap_or([0; 4]));
            let incompat = u32::from_le_bytes(data[1120..1124].try_into().unwrap_or([0; 4]));

            if incompat & 0x0040 != 0 {
                // EXTENTS feature
                return FilesystemType::Ext4;
            } else if compat & 0x0004 != 0 {
                // HAS_JOURNAL feature
                return FilesystemType::Ext3;
            }
        }
        return FilesystemType::Ext2;
    }

    // XFS: magic "XFSB" at offset 0
    if data.len() > 4 && &data[0..4] == b"XFSB" {
        return FilesystemType::Xfs;
    }

    // Btrfs: magic "_BHRfS_M" at offset 64
    if data.len() > 72 && &data[64..72] == b"_BHRfS_M" {
        return FilesystemType::Btrfs;
    }

    // Linux swap: magic at offset 4086 (pagesize 4096 - 10)
    if data.len() > 4096 && &data[4086..4096] == b"SWAPSPACE2" {
        return FilesystemType::Swap;
    }

    FilesystemType::Unknown
}
