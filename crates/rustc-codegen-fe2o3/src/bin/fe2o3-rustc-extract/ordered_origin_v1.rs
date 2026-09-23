//! Internal propagation from the explicit exporter flag, after normal selection.
use super::{ExtractionModeV1, OsString, PathBuf};
pub(super) const OUTPUT_ENV: &str = "FE2O3_DIAGNOSTIC_ORDERED_ORIGIN_PATH_V1";
pub(super) fn selected_output(
    mode: &ExtractionModeV1,
    requested: Option<OsString>,
) -> Result<Option<PathBuf>, String> {
    let Some(requested) = requested else {
        return Ok(None);
    };
    if !matches!(mode, ExtractionModeV1::DiagnosticKirV17(_)) || requested.is_empty() {
        return Err(
            "ordered origin output requires selected diagnostic KIR V17 and a nonempty path"
                .to_owned(),
        );
    }
    Ok(Some(PathBuf::from(requested)))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn absent_internal_output_keeps_legacy_selection() {
        assert_eq!(
            selected_output(&ExtractionModeV1::KernelIr, None).unwrap(),
            None
        );
        assert_eq!(
            selected_output(&ExtractionModeV1::DiagnosticKirV17("raw".into()), None).unwrap(),
            None
        );
    }
    #[test]
    fn present_internal_output_requires_v17_and_nonempty_path() {
        let request = Some(OsString::from("origin.json"));
        for mode in [
            ExtractionModeV1::KernelIr,
            ExtractionModeV1::RankedMemory,
            ExtractionModeV1::DiagnosticKirV16("raw".into()),
            ExtractionModeV1::SimulationBundle("bundle".into()),
        ] {
            assert!(selected_output(&mode, request.clone()).is_err());
        }
        let mode = ExtractionModeV1::DiagnosticKirV17("raw".into());
        assert!(selected_output(&mode, Some(OsString::new())).is_err());
        assert_eq!(
            selected_output(&mode, request).unwrap(),
            Some(PathBuf::from("origin.json"))
        );
    }
}
