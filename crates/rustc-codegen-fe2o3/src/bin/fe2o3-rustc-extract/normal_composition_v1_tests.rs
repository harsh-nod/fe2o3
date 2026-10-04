#[test]
fn normal_composition_mode_is_closed_and_has_no_diagnostic_fallback() {
    use std::ffi::OsStr;
    for mode in [
        ExtractionModeV1::AmdgpuLlvm("out".into()),
        ExtractionModeV1::Gfx942Llvm("out".into()),
        ExtractionModeV1::Gfx942CompilerHandoff("out".into()),
        ExtractionModeV1::AmdgpuCompilerHandoff("out".into()),
    ] {
        assert!(require_normal_composition_mode_v1(&mode, false, Some(OsStr::new("1"))).is_ok());
        assert!(require_normal_composition_mode_v1(&mode, true, Some(OsStr::new("1"))).is_err());
        for value in ["", "0", "true", "01", " 1", "1 "] {
            assert!(
                require_normal_composition_mode_v1(&mode, false, Some(OsStr::new(value))).is_err()
            );
        }
    }
    for mode in [
        ExtractionModeV1::KernelIr,
        ExtractionModeV1::RankedMemory,
        ExtractionModeV1::SimulationBundle("out".into()),
        ExtractionModeV1::SimulationBundleV2("out".into()),
        ExtractionModeV1::SimulationBundleV3("out".into()),
        ExtractionModeV1::SimulationBundleV4("out".into()),
        ExtractionModeV1::SimulationBundleV5("out".into()),
        ExtractionModeV1::SimulationBundleV6("out".into()),
        ExtractionModeV1::DiagnosticKirV16("out".into()),
        ExtractionModeV1::DiagnosticKirV17("out".into()),
        ExtractionModeV1::DiagnosticKirV19("out".into()),
        ExtractionModeV1::PhysicalEntryDiagnosticV20("out".into()),
        ExtractionModeV1::PhysicalGlobalCopyDiagnosticV21("out".into()),
        ExtractionModeV1::PhysicalLdsExchangeDiagnosticV22("out".into()),
        ExtractionModeV1::OrderedCompositionDiagnosticV1("out".into()),
    ] {
        assert!(require_normal_composition_mode_v1(&mode, false, Some(OsStr::new("1"))).is_err());
        assert!(require_normal_composition_mode_v1(&mode, false, None).is_ok());
    }
}
#[cfg(unix)]
#[test]
fn normal_composition_mode_rejects_non_utf8() {
    use std::os::unix::ffi::OsStrExt;
    assert!(
        require_normal_composition_mode_v1(
            &ExtractionModeV1::AmdgpuLlvm("out".into()),
            false,
            Some(std::ffi::OsStr::from_bytes(&[0xff]))
        )
        .is_err()
    );
}
