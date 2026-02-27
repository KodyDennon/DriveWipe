use anyhow::{Context, Result, bail};

use drivewipe_core::config::DriveWipeConfig;
use drivewipe_core::report::ReportGenerator;
use drivewipe_core::report::data::WipeReport;

/// Execute `drivewipe report`.
pub fn run(
    _config: &DriveWipeConfig,
    input: &str,
    format: &str,
    output: Option<&str>,
) -> Result<()> {
    // ── Load the input JSON report ──────────────────────────────────────
    let json_contents = std::fs::read_to_string(input)
        .with_context(|| format!("Failed to read input report: {input}"))?;

    let report = WipeReport::from_json(&json_contents)
        .with_context(|| format!("Failed to parse report JSON from {input}"))?;

    match format {
        "json" => {
            // Pretty-print the JSON.
            let pretty = report
                .to_json()
                .context("Failed to serialise report as JSON")?;

            if let Some(out_path) = output {
                std::fs::write(out_path, &pretty)
                    .with_context(|| format!("Failed to write JSON to {out_path}"))?;
                println!(
                    "{} JSON report written to {out_path}",
                    console::style("report:").blue().bold(),
                );
            } else {
                println!("{pretty}");
            }
        }
        "pdf" => {
            generate_pdf(&report, output)?;
        }
        other => {
            bail!("Unknown report format: {other}. Supported: json, pdf");
        }
    }

    Ok(())
}

#[cfg(feature = "pdf-report")]
fn generate_pdf(report: &WipeReport, output: Option<&str>) -> Result<()> {
    use drivewipe_core::report::pdf::PdfReportGenerator;

    let out_path = output.unwrap_or_else(|| {
        // Default output path derived from session id.
        "report.pdf"
    });

    let generator = PdfReportGenerator;
    let pdf_bytes = generator
        .generate(&report.result)
        .context("Failed to generate PDF report")?;

    std::fs::write(out_path, &pdf_bytes)
        .with_context(|| format!("Failed to write PDF to {out_path}"))?;

    println!(
        "{} PDF report written to {out_path}",
        console::style("report:").blue().bold(),
    );
    Ok(())
}

#[cfg(not(feature = "pdf-report"))]
fn generate_pdf(_report: &WipeReport, _output: Option<&str>) -> Result<()> {
    bail!(
        "PDF report generation is not available. \
         Rebuild with the `pdf-report` feature enabled."
    );
}
