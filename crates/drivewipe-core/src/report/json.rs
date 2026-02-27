use super::data::WipeReport;
use super::ReportGenerator;
use crate::error::Result;
use crate::types::WipeResult;

pub struct JsonReportGenerator;

impl ReportGenerator for JsonReportGenerator {
    fn generate(&self, result: &WipeResult) -> Result<Vec<u8>> {
        let report = WipeReport::from_result(result.clone());
        let json = report.to_json()?;
        Ok(json.into_bytes())
    }

    fn file_extension(&self) -> &str {
        "json"
    }
}
