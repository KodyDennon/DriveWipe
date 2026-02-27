pub mod pattern_verify;
pub mod zero_verify;

use crossbeam_channel::Sender;
use uuid::Uuid;

use crate::error::Result;
use crate::io::RawDeviceIo;
use crate::progress::ProgressEvent;

pub trait Verifier: Send + Sync {
    fn verify(
        &self,
        device: &mut dyn RawDeviceIo,
        session_id: Uuid,
        progress_tx: &Sender<ProgressEvent>,
    ) -> Result<bool>;
}
