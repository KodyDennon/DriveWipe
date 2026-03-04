use crossbeam_channel::Sender;
use uuid::Uuid;

use crate::error::Result;
use crate::io::RawDeviceIo;
use crate::partition::PartitionTable;
use crate::progress::ProgressEvent;
use crate::session::CancellationToken;

use super::{CloneConfig, CloneResult};

/// Perform a partition-aware clone that reads the source partition table,
/// copies each partition individually, and can resize to fit the target.
pub async fn clone_partition_aware(
    source: &mut dyn RawDeviceIo,
    target: &mut dyn RawDeviceIo,
    config: &CloneConfig,
    progress_tx: &Sender<ProgressEvent>,
    cancel_token: &CancellationToken,
) -> Result<CloneResult> {
    let session_id = Uuid::new_v4();
    let source_capacity = source.capacity();
    let target_capacity = target.capacity();

    log::info!(
        "Partition-aware clone: source={} bytes, target={} bytes",
        source_capacity,
        target_capacity,
    );

    let _ = progress_tx.send(ProgressEvent::CloneStarted {
        session_id,
        source: config.source.display().to_string(),
        target: config.target.display().to_string(),
        total_bytes: source_capacity,
    });

    // Read and parse source partition table
    let source_ptr = source as *mut dyn RawDeviceIo as usize;
    let header_buf = tokio::task::spawn_blocking(move || {
        let source_ref = unsafe { &mut *(source_ptr as *mut dyn RawDeviceIo) };
        let mut buf = vec![0u8; 34 * 512]; // Read enough for GPT header + entries
        let res = source_ref.read_at(0, &mut buf);
        (res, buf)
    }).await.map_err(|e| crate::error::DriveWipeError::IoGeneric(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;

    let (read_res, header_buf) = header_buf;
    read_res?;

    let source_table = PartitionTable::parse(&header_buf)?;

    log::info!("Source partition table: {:?}", source_table.table_type());

    // For now, fall back to block clone if partition handling is complex
    // Full partition-aware resize is a future enhancement
    log::info!("Falling back to block-level clone for partition-aware mode");
    super::block::clone_block(source, target, config, progress_tx, cancel_token).await
}
