use super::ReportGenerator;
use crate::error::{DriveWipeError, Result};
use crate::types::{format_bytes, WipeResult};

use genpdf::elements::{Break, Paragraph};
use genpdf::fonts;
use genpdf::style::Style;
use genpdf::{Document, Element, SimplePageDecorator};

pub struct PdfReportGenerator;

impl PdfReportGenerator {
    /// Build the PDF document content from a WipeResult.
    fn build_document(
        result: &WipeResult,
    ) -> std::result::Result<Document, genpdf::error::Error> {
        // Try several common font locations, falling back through each.
        let font_family = fonts::from_files("./fonts", "LiberationSans", None)
            .or_else(|_| {
                fonts::from_files(
                    "/usr/share/fonts/truetype/liberation",
                    "LiberationSans",
                    None,
                )
            })
            .or_else(|_| {
                fonts::from_files(
                    "/usr/share/fonts/liberation-sans",
                    "LiberationSans",
                    None,
                )
            })
            .or_else(|_| fonts::from_files(".", "LiberationSans", None))?;

        let mut doc = Document::new(font_family);
        doc.set_title("Data Sanitization Certificate");

        let mut decorator = SimplePageDecorator::new();
        decorator.set_margins(20);
        doc.set_page_decorator(decorator);

        // Title
        doc.push(
            Paragraph::new("DATA SANITIZATION CERTIFICATE")
                .styled(Style::new().bold().with_font_size(18)),
        );
        doc.push(Break::new(1.5));

        // Session info
        doc.push(
            Paragraph::new(format!("Session ID: {}", result.session_id))
                .styled(Style::new().with_font_size(10)),
        );
        doc.push(
            Paragraph::new(format!(
                "Date: {}",
                result.completed_at.format("%Y-%m-%d %H:%M:%S UTC")
            ))
            .styled(Style::new().with_font_size(10)),
        );
        doc.push(Break::new(1.0));

        // Device information section
        doc.push(
            Paragraph::new("DEVICE INFORMATION")
                .styled(Style::new().bold().with_font_size(14)),
        );
        doc.push(Break::new(0.5));
        doc.push(Paragraph::new(format!("Model:    {}", result.device_model)));
        doc.push(Paragraph::new(format!("Serial:   {}", result.device_serial)));
        doc.push(Paragraph::new(format!(
            "Capacity: {}",
            format_bytes(result.device_capacity)
        )));
        doc.push(Paragraph::new(format!(
            "Path:     {}",
            result.device_path.display()
        )));
        doc.push(Break::new(1.0));

        // Method section
        doc.push(
            Paragraph::new("SANITIZATION METHOD")
                .styled(Style::new().bold().with_font_size(14)),
        );
        doc.push(Break::new(0.5));
        doc.push(Paragraph::new(format!(
            "Method: {} ({})",
            result.method_name, result.method_id
        )));
        doc.push(Paragraph::new(format!("Passes: {}", result.passes.len())));
        doc.push(Break::new(1.0));

        // Pass details
        doc.push(
            Paragraph::new("PASS DETAILS")
                .styled(Style::new().bold().with_font_size(14)),
        );
        doc.push(Break::new(0.5));

        for pass in &result.passes {
            doc.push(Paragraph::new(format!(
                "Pass {}: {} - {:.1}s @ {:.1} MB/s",
                pass.pass_number, pass.pattern_name, pass.duration_secs, pass.throughput_mbps,
            )));
        }
        doc.push(Break::new(1.0));

        // Verification
        doc.push(
            Paragraph::new("VERIFICATION")
                .styled(Style::new().bold().with_font_size(14)),
        );
        doc.push(Break::new(0.5));
        let verification_text = match result.verification_passed {
            Some(true) => "PASSED",
            Some(false) => "FAILED",
            None => "Not performed",
        };
        doc.push(Paragraph::new(format!("Verification: {verification_text}")));
        doc.push(Break::new(1.0));

        // Outcome
        doc.push(
            Paragraph::new("RESULT")
                .styled(Style::new().bold().with_font_size(14)),
        );
        doc.push(Break::new(0.5));
        doc.push(
            Paragraph::new(format!("Outcome: {}", result.outcome))
                .styled(Style::new().bold().with_font_size(12)),
        );
        doc.push(Paragraph::new(format!(
            "Total Duration: {:.1} seconds",
            result.total_duration_secs,
        )));
        doc.push(Break::new(1.0));

        // System info
        doc.push(
            Paragraph::new("SYSTEM INFORMATION")
                .styled(Style::new().bold().with_font_size(14)),
        );
        doc.push(Break::new(0.5));
        doc.push(Paragraph::new(format!("Hostname: {}", result.hostname)));
        if let Some(ref operator) = result.operator {
            doc.push(Paragraph::new(format!("Operator: {operator}")));
        }

        Ok(doc)
    }
}

impl ReportGenerator for PdfReportGenerator {
    fn generate(&self, result: &WipeResult) -> Result<Vec<u8>> {
        let doc = Self::build_document(result)
            .map_err(|e| DriveWipeError::ReportError(format!("Failed to build PDF document: {e}")))?;

        let mut buf = Vec::new();
        doc.render(&mut buf)
            .map_err(|e| DriveWipeError::ReportError(format!("Failed to render PDF: {e}")))?;

        Ok(buf)
    }

    fn file_extension(&self) -> &str {
        "pdf"
    }
}
