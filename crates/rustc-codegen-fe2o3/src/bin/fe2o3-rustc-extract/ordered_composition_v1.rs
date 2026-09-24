// Explicit composition selector; no fallback or protected handoff output.
const EXTRACT_ORDERED_COMPOSITION_DIRECTORY_ENV_V1: &str =
    "FE2O3_EXTRACT_DIAGNOSTIC_ORDERED_COMPOSITION_DIRECTORY_V1";

fn require_disjoint_ordered_composition_diagnostic_v1(
    selected: bool,
    others: [bool; 6],
) -> Result<(), String> {
    if selected && others.into_iter().any(|v| v) {
        Err(
            "ordered composition diagnostics are mutually exclusive with V16/V17/V19/V20/V21/V22"
                .into(),
        )
    } else {
        Ok(())
    }
}
fn select_ordered_composition_diagnostic_v1_mode(
    mut prepared: PreparedExtractionV1,
    output: Option<OsString>,
) -> Result<PreparedExtractionV1, String> {
    let Some(output) = output else {
        return Ok(prepared);
    };
    if output.is_empty() {
        return Err(format!(
            "{EXTRACT_ORDERED_COMPOSITION_DIRECTORY_ENV_V1} must not be empty"
        ));
    }
    if let PreparedExtractionV1::Selected(selected) = &mut prepared {
        if !matches!(selected.mode, ExtractionModeV1::KernelIr)
            || selected.crate_binding_output.is_some()
        {
            return Err(format!(
                "{EXTRACT_ORDERED_COMPOSITION_DIRECTORY_ENV_V1} is mutually exclusive with all other diagnostic, ranked, LLVM, compiler-handoff, simulation-bundle and crate-binding outputs"
            ));
        }
        selected.mode = ExtractionModeV1::OrderedCompositionDiagnosticV1(output);
    }
    Ok(prepared)
}
