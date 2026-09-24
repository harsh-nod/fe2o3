// Explicit opt-in mutation: merely requesting diagnostics never writes source.
const EXTRACT_COMPOSITION_PROMOTION_REQUEST_ENV_V1: &str =
    "FE2O3_EXTRACT_ORDERED_COMPOSITION_PROMOTION_REQUEST_V1";

fn selected_composition_promotion_request_v1(
    mode: &ExtractionModeV1,
    request: Option<OsString>,
) -> Result<Option<PathBuf>, String> {
    let Some(request) = request else {
        return Ok(None);
    };
    if request.is_empty() {
        return Err(format!(
            "{EXTRACT_COMPOSITION_PROMOTION_REQUEST_ENV_V1} must not be empty"
        ));
    }
    if !matches!(mode, ExtractionModeV1::OrderedCompositionDiagnosticV1(_)) {
        return Err(format!(
            "{EXTRACT_COMPOSITION_PROMOTION_REQUEST_ENV_V1} requires the explicit ordered-composition diagnostic mode"
        ));
    }
    #[cfg(not(target_os = "linux"))]
    return Err("ordered-composition source promotion requires Linux".into());
    #[cfg(target_os = "linux")]
    Ok(Some(PathBuf::from(request)))
}
