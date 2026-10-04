#[test]
fn physical_global_copy_diagnostic_selector_retains_exact_invocation_and_bindings() {
    let selected = selected_compile("unit", &["actual-cargo-metadata"]);
    let binding = selected.crate_binding;
    let observation = selected.metadata_observation;
    let arguments = selected.args.clone();
    let prepared = select_physical_global_copy_diagnostic_v21_mode(
        PreparedExtractionV1::Selected(selected),
        Some(OsString::from("fresh-diagnostics")),
    )
    .unwrap();
    let PreparedExtractionV1::Selected(selected) = prepared else {
        panic!("lost source")
    };
    assert_eq!(selected.crate_binding, binding);
    assert_eq!(selected.metadata_observation, observation);
    assert_eq!(selected.args, arguments);
    assert!(
        matches!(selected.mode,ExtractionModeV1::PhysicalGlobalCopyDiagnosticV21(path)if path=="fresh-diagnostics")
    );
}
#[test]
fn physical_global_copy_diagnostic_selector_cannot_mix_output_authorities() {
    let path = || OsString::from("other");
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
        ExtractionModeV1::PhysicalEntryDiagnosticV20(path()),
        ExtractionModeV1::PhysicalGlobalCopyDiagnosticV21(path()),
    ] {
        let mut selected = selected_compile("unit", &["metadata"]);
        selected.mode = mode;
        assert!(
            select_physical_global_copy_diagnostic_v21_mode(
                PreparedExtractionV1::Selected(selected),
                Some(path())
            )
            .unwrap_err()
            .contains("mutually exclusive")
        );
    }
    let mut selected = selected_compile("unit", &["metadata"]);
    selected.crate_binding_output = Some(PathBuf::from("binding"));
    assert!(
        select_physical_global_copy_diagnostic_v21_mode(
            PreparedExtractionV1::Selected(selected),
            Some(path())
        )
        .unwrap_err()
        .contains("crate-binding")
    );
    for older in [
        [false, false, false, false],
        [true, false, false, false],
        [false, true, false, false],
        [false, false, true, false],
        [false, false, false, true],
        [true, true, true, true],
    ] {
        assert_eq!(
            require_disjoint_physical_global_copy_diagnostic_v21(true, older).is_ok(),
            !older.into_iter().any(|v| v)
        );
    }
}
#[test]
fn physical_global_copy_diagnostic_selector_is_explicit_and_preserves_dependencies() {
    let selected = || PreparedExtractionV1::Selected(selected_compile("unit", &["metadata"]));
    assert!(matches!(
        select_physical_global_copy_diagnostic_v21_mode(selected(), None).unwrap(),
        PreparedExtractionV1::Selected(SelectedExtractionV1 {
            mode: ExtractionModeV1::KernelIr,
            ..
        })
    ));
    assert!(
        select_physical_global_copy_diagnostic_v21_mode(selected(), Some(OsString::new())).is_err()
    );
    let passthrough = || PreparedExtractionV1::Passthrough {
        executable: "actual-rustc".into(),
        forwarded_args: vec!["--version".into()],
    };
    assert!(
        select_physical_global_copy_diagnostic_v21_mode(passthrough(), Some(OsString::new()))
            .is_err()
    );
    assert!(
        matches!(select_physical_global_copy_diagnostic_v21_mode(passthrough(),Some("fresh".into())).unwrap(),PreparedExtractionV1::Passthrough{executable,forwarded_args}if executable=="actual-rustc"&&forwarded_args==[OsString::from("--version")])
    );
}

#[test]
fn physical_global_copy_cannot_replace_a_selected_v20_and_v20_cannot_replace_v21() {
    let selected = selected_compile("unit", &["metadata"]);
    let v21 = select_physical_global_copy_diagnostic_v21_mode(
        PreparedExtractionV1::Selected(selected),
        Some("v21".into()),
    )
    .unwrap();
    assert!(
        select_physical_entry_diagnostic_v20_mode(v21, Some("v20".into()))
            .unwrap_err()
            .contains("mutually exclusive")
    );
}
