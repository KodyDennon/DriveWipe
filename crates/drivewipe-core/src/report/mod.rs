pub mod data;
pub mod json;
#[cfg(feature = "pdf-report")]
pub mod pdf;

use crate::error::Result;
use crate::types::WipeResult;

pub trait ReportGenerator {
    fn generate(&self, result: &WipeResult) -> Result<Vec<u8>>;
    fn file_extension(&self) -> &str;
}
