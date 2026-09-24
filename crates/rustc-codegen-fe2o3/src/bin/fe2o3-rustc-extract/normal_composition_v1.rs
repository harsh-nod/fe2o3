// Selects a live source route; this flag never promotes diagnostic artifacts.
const EXTRACT_COMPOSITION_NORMAL_ENV_V1: &str = "FE2O3_EXTRACT_ORDERED_COMPOSITION_V1";
fn require_normal_composition_mode_v1(
    mode: &ExtractionModeV1,
    binding_sidecar: bool,
    value: Option<&std::ffi::OsStr>,
) -> Result<(), String> {
    let Some(value) = value else {
        return Ok(());
    };
    if value != "1" {
        return Err(format!(
            "{EXTRACT_COMPOSITION_NORMAL_ENV_V1} requires exactly 1"
        ));
    }
    if binding_sidecar
        || !matches!(
            mode,
            ExtractionModeV1::AmdgpuLlvm(_)
                | ExtractionModeV1::Gfx942Llvm(_)
                | ExtractionModeV1::Gfx942CompilerHandoff(_)
                | ExtractionModeV1::AmdgpuCompilerHandoff(_)
        )
    {
        return Err("normal ordered composition requires an exclusive normal LLVM or inert compiler-handoff output".into());
    }
    Ok(())
}
