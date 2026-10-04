#[test]
fn ordered_composition_selector_retains_invocation_and_bindings() {
    let selected = selected_compile("unit", &["actual-cargo-metadata"]);
    let (binding, metadata, args) = (
        selected.crate_binding,
        selected.metadata_observation,
        selected.args.clone(),
    );
    let PreparedExtractionV1::Selected(actual) = select_ordered_composition_diagnostic_v1_mode(
        PreparedExtractionV1::Selected(selected),
        Some("fresh-composition".into()),
    )
    .unwrap() else {
        panic!("lost selected source")
    };
    assert_eq!(actual.crate_binding, binding);
    assert_eq!(actual.metadata_observation, metadata);
    assert_eq!(actual.args, args);
    assert!(
        matches!(actual.mode,ExtractionModeV1::OrderedCompositionDiagnosticV1(p) if p=="fresh-composition")
    );
}
#[test]
fn ordered_composition_selector_rejects_every_other_output() {
    let p = || OsString::from("other");
    for mode in [
        ExtractionModeV1::RankedMemory,
        ExtractionModeV1::AmdgpuLlvm(p()),
        ExtractionModeV1::Gfx942Llvm(p()),
        ExtractionModeV1::Gfx942CompilerHandoff(p()),
        ExtractionModeV1::AmdgpuCompilerHandoff(p()),
        ExtractionModeV1::SimulationBundle(p()),
        ExtractionModeV1::SimulationBundleV2(p()),
        ExtractionModeV1::SimulationBundleV3(p()),
        ExtractionModeV1::SimulationBundleV4(p()),
        ExtractionModeV1::SimulationBundleV5(p()),
        ExtractionModeV1::SimulationBundleV6(p()),
        ExtractionModeV1::DiagnosticKirV16(p()),
        ExtractionModeV1::DiagnosticKirV17(p()),
        ExtractionModeV1::DiagnosticKirV19(p()),
        ExtractionModeV1::PhysicalEntryDiagnosticV20(p()),
        ExtractionModeV1::PhysicalGlobalCopyDiagnosticV21(p()),
        ExtractionModeV1::PhysicalLdsExchangeDiagnosticV22(p()),
        ExtractionModeV1::OrderedCompositionDiagnosticV1(p()),
    ] {
        let mut s = selected_compile("unit", &["metadata"]);
        s.mode = mode;
        assert!(
            select_ordered_composition_diagnostic_v1_mode(
                PreparedExtractionV1::Selected(s),
                Some(p())
            )
            .unwrap_err()
            .contains("mutually exclusive")
        );
    }
    let mut s = selected_compile("unit", &["metadata"]);
    s.crate_binding_output = Some(PathBuf::from("binding"));
    assert!(
        select_ordered_composition_diagnostic_v1_mode(PreparedExtractionV1::Selected(s), Some(p()))
            .unwrap_err()
            .contains("crate-binding")
    );
    for mask in 0..64u8 {
        let others = std::array::from_fn(|bit| mask & (1 << bit) != 0);
        assert_eq!(
            require_disjoint_ordered_composition_diagnostic_v1(true, others).is_ok(),
            mask == 0
        );
        assert!(require_disjoint_ordered_composition_diagnostic_v1(false, others).is_ok());
    }
}
#[test]
fn ordered_composition_selector_keeps_dependencies_and_explicit_mode() {
    let fresh = || PreparedExtractionV1::Selected(selected_compile("unit", &["metadata"]));
    assert!(matches!(
        select_ordered_composition_diagnostic_v1_mode(fresh(), None).unwrap(),
        PreparedExtractionV1::Selected(SelectedExtractionV1 {
            mode: ExtractionModeV1::KernelIr,
            ..
        })
    ));
    assert!(select_ordered_composition_diagnostic_v1_mode(fresh(), Some(OsString::new())).is_err());
    let dependency = || PreparedExtractionV1::Passthrough {
        executable: "real-rustc".into(),
        forwarded_args: vec!["--version".into()],
    };
    assert!(
        select_ordered_composition_diagnostic_v1_mode(dependency(), Some(OsString::new())).is_err()
    );
    assert!(
        matches!(select_ordered_composition_diagnostic_v1_mode(dependency(),Some("fresh".into())).unwrap(),PreparedExtractionV1::Passthrough{executable,forwarded_args} if executable=="real-rustc" && forwarded_args==[OsString::from("--version")])
    );
}
#[test]
fn older_selector_cannot_replace_composition_or_mint_origin() {
    let c = select_ordered_composition_diagnostic_v1_mode(
        PreparedExtractionV1::Selected(selected_compile("unit", &["metadata"])),
        Some("fresh".into()),
    )
    .unwrap();
    assert!(select_physical_lds_exchange_diagnostic_v22_mode(c, Some("old".into())).is_err());
    assert!(
        ordered_origin_v1::selected_output(
            &ExtractionModeV1::OrderedCompositionDiagnosticV1("fresh".into()),
            Some("forged-origin".into())
        )
        .is_err()
    );
}
