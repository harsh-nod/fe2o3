// Closed source-only selector. No edited intermediate or normal artifact route.
const EXTRACT_BF16_TILE_SOURCE_DIRECTORY_ENV_V1: &str =
    "FE2O3_EXTRACT_BF16_TILE_SOURCE_DIRECTORY_V1";
const EXTRACT_BF16_TILE_PROMOTION_REQUEST_ENV_V1: &str =
    "FE2O3_EXTRACT_BF16_TILE_PROMOTION_REQUEST_V1";

fn require_disjoint_bf16_tile_source_v1(selected: bool, others: [bool; 7]) -> Result<(), String> {
    if selected && others.into_iter().any(|value| value) {
        Err("BF16 source action is mutually exclusive with V16/V17/V19/V20/V21/V22 and ordered composition diagnostics".into())
    } else {
        Ok(())
    }
}
fn bf16_tile_path_v1(path: &std::ffi::OsStr) -> Result<(), String> {
    if path.is_empty()
        || path.as_encoded_bytes().len() > 4096
        || path.as_encoded_bytes().contains(&0)
    {
        return Err("BF16 source paths must be nonempty and at most 4096 bytes without NUL".into());
    }
    Ok(())
}
fn select_bf16_tile_source_v1_mode(
    mut prepared: PreparedExtractionV1,
    output: Option<OsString>,
) -> Result<PreparedExtractionV1, String> {
    let Some(output) = output else {
        return Ok(prepared);
    };
    bf16_tile_path_v1(&output)?;
    #[cfg(not(target_os = "linux"))]
    return Err("BF16 source action requires Linux".into());
    #[cfg(target_os = "linux")]
    {
        if let PreparedExtractionV1::Selected(selected) = &mut prepared {
            if !matches!(selected.mode, ExtractionModeV1::KernelIr)
                || selected.crate_binding_output.is_some()
            {
                return Err("BF16 source action is mutually exclusive with all other diagnostic, ranked, LLVM, compiler-handoff, simulation-bundle and crate-binding outputs".into());
            }
            selected.mode = ExtractionModeV1::Bf16TileSourceV1(output);
        }
        Ok(prepared)
    }
}
fn selected_bf16_tile_promotion_request_v1(
    mode: &ExtractionModeV1,
    request: Option<OsString>,
) -> Result<Option<PathBuf>, String> {
    let Some(request) = request else {
        return Ok(None);
    };
    bf16_tile_path_v1(&request)?;
    if !matches!(mode, ExtractionModeV1::Bf16TileSourceV1(_)) {
        return Err(
            "BF16 source promotion requires the explicit BF16 source-only output mode".into(),
        );
    }
    #[cfg(not(target_os = "linux"))]
    return Err("BF16 source promotion requires Linux".into());
    #[cfg(target_os = "linux")]
    Ok(Some(PathBuf::from(request)))
}
