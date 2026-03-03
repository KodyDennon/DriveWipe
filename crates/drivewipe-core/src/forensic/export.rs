use super::ForensicResult;
use crate::error::Result;

/// Export forensic results in DFXML format.
pub fn export_dfxml(results: &ForensicResult) -> Result<String> {
    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str("<dfxml version=\"1.0\">\n");
    xml.push_str("  <creator>\n");
    xml.push_str("    <program>DriveWipe</program>\n");
    xml.push_str("    <version>0.1.5</version>\n");
    xml.push_str("  </creator>\n");

    xml.push_str("  <source>\n");
    xml.push_str(&format!("    <image_filename>{}</image_filename>\n", results.device_path));
    xml.push_str(&format!("    <serial_number>{}</serial_number>\n", results.device_serial));
    xml.push_str("  </source>\n");

    // File objects from signature hits
    for hit in &results.signature_hits {
        xml.push_str("  <fileobject>\n");
        xml.push_str(&format!("    <byte_run offset=\"{}\" />\n", hit.offset));
        xml.push_str(&format!("    <name_type>{}</name_type>\n", hit.file_type));
        xml.push_str("  </fileobject>\n");
    }

    xml.push_str("</dfxml>\n");
    Ok(xml)
}

/// Export signature hits as a hash set (NSRL-compatible CSV format).
pub fn export_hash_set(results: &ForensicResult) -> Result<String> {
    let mut csv = String::from("\"offset\",\"file_type\",\"magic\",\"confidence\"\n");

    for hit in &results.signature_hits {
        csv.push_str(&format!(
            "\"{}\",\"{}\",\"{}\",\"{}\"\n",
            hit.offset, hit.file_type, hit.magic_hex, hit.confidence,
        ));
    }

    Ok(csv)
}
