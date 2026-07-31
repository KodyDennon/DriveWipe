//! PDF sanitization certificate.
//!
//! Built on `printpdf` using the PDF base-14 fonts, so no font files need to be
//! present on the machine generating the certificate.

use super::ReportGenerator;
use crate::error::{DriveWipeError, Result};
use crate::types::{WipeResult, format_bytes};

use printpdf::{
    BuiltinFont, DateTime, Mm, Op, PdfDocument, PdfFontHandle, PdfPage, PdfSaveOptions, Point, Pt,
    TextItem,
};

/// A4, in millimetres.
const PAGE_WIDTH_MM: f32 = 210.0;
const PAGE_HEIGHT_MM: f32 = 297.0;
const MARGIN_MM: f32 = 20.0;

const TITLE_PT: f32 = 18.0;
const HEADING_PT: f32 = 13.0;
const BODY_PT: f32 = 10.0;

pub struct PdfReportGenerator;

/// Lays text out top-down, tracking the cursor and starting a new page when the
/// bottom margin is reached.
struct Layout {
    pages: Vec<PdfPage>,
    ops: Vec<Op>,
    /// Distance from the top of the page, in millimetres.
    y_mm: f32,
}

impl Layout {
    fn new() -> Self {
        let mut layout = Self {
            pages: Vec::new(),
            ops: Vec::new(),
            y_mm: MARGIN_MM,
        };
        layout.begin_page();
        layout
    }

    fn begin_page(&mut self) {
        self.ops = vec![
            Op::SaveGraphicsState,
            Op::StartTextSection,
            // The cursor is placed once per page. Every later move is relative
            // to it, because `SetTextCursor` emits PDF's `Td`, which offsets
            // from the current line's origin rather than the page origin.
            Op::SetTextCursor {
                pos: Point::new(Mm(MARGIN_MM), Mm(PAGE_HEIGHT_MM - MARGIN_MM)),
            },
        ];
        self.y_mm = MARGIN_MM;
    }

    fn end_page(&mut self) {
        self.ops.push(Op::EndTextSection);
        self.ops.push(Op::RestoreGraphicsState);
        let ops = std::mem::take(&mut self.ops);
        self.pages
            .push(PdfPage::new(Mm(PAGE_WIDTH_MM), Mm(PAGE_HEIGHT_MM), ops));
    }

    /// Advance the cursor, breaking to a new page if the line would not fit.
    fn advance(&mut self, height_mm: f32) {
        if self.y_mm + height_mm > PAGE_HEIGHT_MM - MARGIN_MM {
            self.end_page();
            self.begin_page();
        } else {
            self.feed(height_mm);
        }
        self.y_mm += height_mm;
    }

    fn line(&mut self, text: impl Into<String>, size_pt: f32, bold: bool) {
        // Point sizes are converted to millimetres for cursor maths; the 1.45
        // factor is ordinary single-spaced leading.
        let height_mm = size_pt * 0.3528 * 1.45;
        self.advance(height_mm);

        let font = if bold {
            BuiltinFont::HelveticaBold
        } else {
            BuiltinFont::Helvetica
        };

        self.ops.push(Op::SetFont {
            font: PdfFontHandle::Builtin(font),
            size: Pt(size_pt),
        });
        self.ops.push(Op::ShowText {
            items: vec![TextItem::Text(text.into())],
        });
    }

    /// Move the cursor down by `height_mm`, relative to the current line.
    fn feed(&mut self, height_mm: f32) {
        self.ops.push(Op::SetTextCursor {
            pos: Point::new(Mm(0.0), Mm(-height_mm)),
        });
    }

    fn blank(&mut self, height_mm: f32) {
        self.advance(height_mm);
    }

    fn heading(&mut self, text: &str) {
        self.blank(3.0);
        self.line(text, HEADING_PT, true);
        self.blank(1.0);
    }

    /// A `Label: value` row. Helvetica is proportional, so padding the label to
    /// a fixed character width would not line the values up; the label simply
    /// runs into the value instead.
    fn field(&mut self, label: &str, value: impl Into<String>) {
        let value = value.into();
        let text = if label.is_empty() {
            value
        } else {
            format!("{label} {value}")
        };
        self.line(text, BODY_PT, false);
    }

    fn finish(mut self) -> Vec<PdfPage> {
        self.end_page();
        self.pages
    }
}

impl PdfReportGenerator {
    fn build_pages(result: &WipeResult) -> Vec<PdfPage> {
        let mut l = Layout::new();

        l.line("DATA SANITIZATION CERTIFICATE", TITLE_PT, true);
        l.blank(4.0);
        l.field("Session ID:", result.session_id.to_string());
        l.field(
            "Date:",
            result
                .completed_at
                .format("%Y-%m-%d %H:%M:%S UTC")
                .to_string(),
        );

        l.heading("DEVICE INFORMATION");
        l.field("Model:", &result.device_model);
        l.field("Serial:", &result.device_serial);
        l.field("Capacity:", format_bytes(result.device_capacity));
        l.field("Path:", result.device_path.display().to_string());

        l.heading("SANITIZATION METHOD");
        l.field(
            "Method:",
            format!("{} ({})", result.method_name, result.method_id),
        );
        l.field("Passes:", result.passes.len().to_string());

        l.heading("PASS DETAILS");
        if result.passes.is_empty() {
            l.field("", "No software passes (firmware erase)");
        }
        for pass in &result.passes {
            let verified = match pass.verification_passed {
                Some(true) => " - verified",
                Some(false) => " - VERIFICATION FAILED",
                None => "",
            };
            l.line(
                format!(
                    "Pass {}: {} - {:.1}s @ {:.1} MB/s{}",
                    pass.pass_number,
                    pass.pattern_name,
                    pass.duration_secs,
                    pass.throughput_mbps,
                    verified,
                ),
                BODY_PT,
                false,
            );
        }

        l.heading("VERIFICATION");
        l.field(
            "Result:",
            match result.verification_passed {
                Some(true) => "PASSED",
                Some(false) => "FAILED",
                None => "Not performed",
            },
        );

        l.heading("RESULT");
        l.line(format!("Outcome: {}", result.outcome), 12.0, true);
        l.field(
            "Duration:",
            format!("{:.1} seconds", result.total_duration_secs),
        );
        l.field("Written:", format_bytes(result.total_bytes_written));

        l.heading("SYSTEM INFORMATION");
        l.field("Hostname:", &result.hostname);
        if let Some(ref operator) = result.operator {
            l.field("Operator:", operator);
        }

        // Warnings belong on the certificate: an unremovable hidden area or a
        // failed verification is exactly what an auditor needs to see.
        if !result.warnings.is_empty() {
            l.heading("WARNINGS");
            for w in &result.warnings {
                for chunk in wrap(w, 95) {
                    l.line(chunk, BODY_PT, false);
                }
            }
        }

        if !result.errors.is_empty() {
            l.heading("ERRORS");
            for e in &result.errors {
                for chunk in wrap(e, 95) {
                    l.line(chunk, BODY_PT, false);
                }
            }
        }

        l.finish()
    }
}

/// Break `text` on whitespace into lines of at most `width` characters, so long
/// warnings do not run off the page.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();

    for word in text.split_whitespace() {
        if current.is_empty() {
            current.push_str(word);
        } else if current.chars().count() + 1 + word.chars().count() <= width {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(std::mem::take(&mut current));
            current.push_str(word);
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

impl ReportGenerator for PdfReportGenerator {
    fn generate(&self, result: &WipeResult) -> Result<Vec<u8>> {
        let mut doc = PdfDocument::new("Data Sanitization Certificate");

        // Stamp the document with when the wipe finished rather than leaving
        // printpdf's epoch default, so the certificate's own metadata agrees
        // with the event it certifies.
        doc.metadata.info.creator = "DriveWipe".to_string();
        doc.metadata.info.producer = format!("DriveWipe {}", env!("CARGO_PKG_VERSION"));
        doc.metadata.info.subject = format!(
            "Sanitization of {} ({})",
            result.device_model, result.device_serial
        );
        if let Some(ref operator) = result.operator {
            doc.metadata.info.author = operator.clone();
        }
        doc.metadata.info.identifier = result.session_id.to_string();
        if let Ok(ts) = DateTime::from_unix_timestamp(result.completed_at.timestamp()) {
            doc.metadata.info.creation_date = ts;
            doc.metadata.info.modification_date = ts;
            doc.metadata.info.metadata_date = ts;
        }

        let pages = Self::build_pages(result);

        let mut warnings = Vec::new();
        let bytes = doc
            .with_pages(pages)
            .save(&PdfSaveOptions::default(), &mut warnings);

        for w in &warnings {
            log::debug!("PDF writer: {w:?}");
        }

        if bytes.is_empty() {
            return Err(DriveWipeError::ReportError(
                "PDF writer produced no output".to_string(),
            ));
        }

        Ok(bytes)
    }

    fn file_extension(&self) -> &str {
        "pdf"
    }
}
