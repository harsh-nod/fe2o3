// Fixed generated-source inspection; no mutation request or normal output.
const EXTRACT_BF16_GENERATED_SOURCE_DIRECTORY_ENV_V1: &str =
    "FE2O3_EXTRACT_BF16_GENERATED_SOURCE_DIRECTORY_V1";

fn require_disjoint_bf16_source_modes_v1(direct: bool, generated: bool) -> Result<(), String> {
    if direct && generated {
        Err("direct BF16 publication/inspection and generated-source admission are mutually exclusive".into())
    } else {
        Ok(())
    }
}

fn select_bf16_generated_source_v1_mode(
    mut prepared: PreparedExtractionV1,
    output: Option<OsString>,
) -> Result<PreparedExtractionV1, String> {
    let Some(output) = output else {
        return Ok(prepared);
    };
    bf16_tile_path_v1(&output)?;
    #[cfg(not(target_os = "linux"))]
    return Err("generated BF16 source admission requires Linux".into());
    #[cfg(target_os = "linux")]
    {
        if let PreparedExtractionV1::Selected(selected) = &mut prepared {
            if !matches!(selected.mode, ExtractionModeV1::KernelIr)
                || selected.crate_binding_output.is_some()
            {
                return Err("generated BF16 source admission is mutually exclusive with all other output modes".into());
            }
            selected.mode = ExtractionModeV1::Bf16GeneratedSourceV1(output);
        }
        Ok(prepared)
    }
}
