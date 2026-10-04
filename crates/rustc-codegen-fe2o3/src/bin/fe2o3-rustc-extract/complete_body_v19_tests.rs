#[test]
fn complete_body_diagnostic_selector_preserves_actual_invocation_binding_and_metadata() {
    let selected = selected_compile("unit", &["actual-cargo-metadata"]);
    let binding = selected.crate_binding;
    let observation = selected.metadata_observation;
    let arguments = selected.args.clone();
    let prepared = select_diagnostic_kir_v19_mode(
        PreparedExtractionV1::Selected(selected),
        Some(OsString::from("program.kir")),
    )
    .unwrap();
    let PreparedExtractionV1::Selected(selected) = prepared else {
        panic!("selected source lost");
    };
    assert_eq!(selected.crate_binding, binding);
    assert_eq!(selected.metadata_observation, observation);
    assert_eq!(selected.args, arguments);
    assert!(
        matches!(selected.mode, ExtractionModeV1::DiagnosticKirV19(path) if path == "program.kir")
    );
}

#[test]
fn complete_body_diagnostic_selector_refuses_every_other_output_and_binding_sidecar() {
    let path = || OsString::from("other-output");
    for mode in [
        ExtractionModeV1::RankedMemory,
        ExtractionModeV1::AmdgpuLlvm(path()),
        ExtractionModeV1::Gfx942Llvm(path()),
        ExtractionModeV1::Gfx942CompilerHandoff(path()),
        ExtractionModeV1::AmdgpuCompilerHandoff(path()),
        ExtractionModeV1::SimulationBundle(path()),
        ExtractionModeV1::SimulationBundleV2(path()),
        ExtractionModeV1::SimulationBundleV3(path()),
        ExtractionModeV1::SimulationBundleV4(path()),
        ExtractionModeV1::SimulationBundleV5(path()),
        ExtractionModeV1::SimulationBundleV6(path()),
        ExtractionModeV1::DiagnosticKirV16(path()),
        ExtractionModeV1::DiagnosticKirV17(path()),
        ExtractionModeV1::DiagnosticKirV19(path()),
    ] {
        let mut selected = selected_compile("unit", &["metadata"]);
        selected.mode = mode;
        assert!(
            select_diagnostic_kir_v19_mode(PreparedExtractionV1::Selected(selected), Some(path()))
                .unwrap_err()
                .contains("mutually exclusive")
        );
    }
    let mut selected = selected_compile("unit", &["metadata"]);
    selected.crate_binding_output = Some(PathBuf::from("binding.txt"));
    assert!(
        select_diagnostic_kir_v19_mode(PreparedExtractionV1::Selected(selected), Some(path()))
            .unwrap_err()
            .contains("crate-binding")
    );
    let mut selected = selected_compile("unit", &["metadata"]);
    selected.mode = ExtractionModeV1::DiagnosticKirV19(path());
    assert!(
        select_diagnostic_kir_v16_mode(PreparedExtractionV1::Selected(selected), Some(path()))
            .is_err()
    );
}

#[test]
fn complete_body_diagnostic_selector_is_explicit_nonempty_disjoint_and_passthrough_safe() {
    for (v17, v19) in [(false, false), (true, false), (false, true)] {
        require_disjoint_diagnostic_outputs_v19(false, v17, v19).unwrap();
    }
    assert!(require_disjoint_diagnostic_outputs_v19(false, true, true).is_err());
    let selected = || PreparedExtractionV1::Selected(selected_compile("unit", &["metadata"]));
    assert!(matches!(
        select_diagnostic_kir_v19_mode(selected(), None).unwrap(),
        PreparedExtractionV1::Selected(SelectedExtractionV1 {
            mode: ExtractionModeV1::KernelIr,
            ..
        })
    ));
    assert!(select_diagnostic_kir_v19_mode(selected(), Some(OsString::new())).is_err());
    let passthrough = || PreparedExtractionV1::Passthrough {
        executable: OsString::from("actual-rustc"),
        forwarded_args: vec![OsString::from("--version")],
    };
    assert!(select_diagnostic_kir_v19_mode(passthrough(), Some(OsString::new())).is_err());
    assert!(
        matches!(select_diagnostic_kir_v19_mode(passthrough(), Some(OsString::from("program.kir"))).unwrap(), PreparedExtractionV1::Passthrough { executable, forwarded_args } if executable == "actual-rustc" && forwarded_args == [OsString::from("--version")])
    );
}

#[test]
fn complete_body_three_diagnostic_modes_are_pairwise_exclusive() {
    for v16 in [false, true] {
        for v17 in [false, true] {
            for v19 in [false, true] {
                assert_eq!(
                    require_disjoint_diagnostic_outputs_v19(v16, v17, v19).is_ok(),
                    u8::from(v16) + u8::from(v17) + u8::from(v19) <= 1
                );
            }
        }
    }
}
