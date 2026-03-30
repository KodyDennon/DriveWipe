use super::ForensicResult;
use crate::error::Result;

/// Export forensic results in DFXML format.
pub fn export_dfxml(results: &ForensicResult) -> Result<String> {
    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str("<dfxml version=\"1.0\">\n");
    xml.push_str("  <creator>\n");
    xml.push_str("    <program>DriveWipe</program>\n");
    xml.push_str(&format!(
        "    <version>{}</version>\n",
        env!("CARGO_PKG_VERSION")
    ));
    xml.push_str("  </creator>\n");

    xml.push_str("  <source>\n");
    xml.push_str(&format!(
        "    <image_filename>{}</image_filename>\n",
        results.device_path
    ));
    xml.push_str(&format!(
        "    <serial_number>{}</serial_number>\n",
        results.device_serial
    ));
    xml.push_str("  </source>\n");

    // File objects from signature hits
    for hit in &results.signature_hits {
        xml.push_str("  <fileobject>\n");
        xml.push_str(&format!("    <byte_run offset=\"{}\" />\n", hit.offset));
        xml.push_str(&format!("    <name_type>{}</name_type>\n", hit.file_type));
        xml.push_str("  </fileobject>\n");
    }

    // Hidden areas
    if let Some(ref hidden) = results.hidden_areas {
        xml.push_str("  <hidden_areas>\n");
        if hidden.hpa_detected {
            xml.push_str(&format!(
                "    <hpa size=\"{}\" />\n",
                hidden.hpa_size.unwrap_or(0)
            ));
        }
        if hidden.dco_detected {
            xml.push_str(&format!(
                "    <dco size=\"{}\" />\n",
                hidden.dco_size.unwrap_or(0)
            ));
        }
        for gap in &hidden.unallocated_gaps {
            xml.push_str(&format!(
                "    <unallocated_gap offset=\"{}\" size=\"{}\" has_data=\"{}\" />\n",
                gap.start_offset, gap.size, gap.has_data
            ));
        }
        for hp in &hidden.hidden_partitions {
            xml.push_str(&format!(
                "    <hidden_partition offset=\"{}\" size=\"{}\" description=\"{}\" />\n",
                hp.start_offset, hp.size, hp.description
            ));
        }
        xml.push_str("  </hidden_areas>\n");
    }

    // Entropy statistics
    if let Some(ref entropy) = results.entropy_stats {
        xml.push_str("  <entropy_analysis>\n");
        xml.push_str(&format!(
            "    <average>{:.4}</average>\n",
            entropy.average_entropy
        ));
        xml.push_str(&format!("    <min>{:.4}</min>\n", entropy.min_entropy));
        xml.push_str(&format!("    <max>{:.4}</max>\n", entropy.max_entropy));
        xml.push_str(&format!(
            "    <high_entropy_pct>{:.2}</high_entropy_pct>\n",
            entropy.high_entropy_pct
        ));
        xml.push_str(&format!(
            "    <low_entropy_pct>{:.2}</low_entropy_pct>\n",
            entropy.low_entropy_pct
        ));
        xml.push_str(&format!(
            "    <zero_pct>{:.2}</zero_pct>\n",
            entropy.zero_pct
        ));
        xml.push_str("  </entropy_analysis>\n");
    }

    // Sampling results
    if let Some(ref sampling) = results.sampling_result {
        xml.push_str("  <statistical_sampling>\n");
        xml.push_str(&format!(
            "    <sectors_sampled>{}</sectors_sampled>\n",
            sampling.sectors_sampled
        ));
        xml.push_str(&format!(
            "    <total_sectors>{}</total_sectors>\n",
            sampling.total_sectors
        ));
        xml.push_str(&format!(
            "    <zero_pct>{:.2}</zero_pct>\n",
            sampling.zero_pct
        ));
        xml.push_str(&format!(
            "    <high_entropy_pct>{:.2}</high_entropy_pct>\n",
            sampling.high_entropy_pct
        ));
        xml.push_str(&format!(
            "    <data_remnant_pct>{:.2}</data_remnant_pct>\n",
            sampling.data_remnant_pct
        ));
        xml.push_str(&format!(
            "    <confidence>{:.4}</confidence>\n",
            sampling.confidence
        ));
        xml.push_str("  </statistical_sampling>\n");
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
