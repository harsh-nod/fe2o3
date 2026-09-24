#[test]
fn source_promotion_is_explicit_and_only_in_composition_diagnostics() {
    let mode = ExtractionModeV1::OrderedCompositionDiagnosticV1("out".into());
    assert!(
        selected_composition_promotion_request_v1(&mode, None)
            .unwrap()
            .is_none()
    );
    assert!(selected_composition_promotion_request_v1(&mode, Some(OsString::new())).is_err());
    #[cfg(target_os = "linux")]
    assert_eq!(
        selected_composition_promotion_request_v1(&mode, Some("request.json".into())).unwrap(),
        Some(PathBuf::from("request.json"))
    );
    for mode in [
        ExtractionModeV1::KernelIr,
        ExtractionModeV1::RankedMemory,
        ExtractionModeV1::AmdgpuLlvm("out".into()),
        ExtractionModeV1::Gfx942Llvm("out".into()),
        ExtractionModeV1::Gfx942CompilerHandoff("out".into()),
        ExtractionModeV1::AmdgpuCompilerHandoff("out".into()),
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
    ] {
        assert!(
            selected_composition_promotion_request_v1(&mode, Some("request.json".into())).is_err()
        );
        assert!(
            selected_composition_promotion_request_v1(&mode, None)
                .unwrap()
                .is_none()
        );
    }
}
