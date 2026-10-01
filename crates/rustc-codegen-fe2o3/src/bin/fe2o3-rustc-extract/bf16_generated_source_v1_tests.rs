#[test]
#[cfg(target_os = "linux")]
fn generated_bf16_selector_keeps_actual_argv_and_bindings() {
    let selected = selected_compile("unit", &["actual-cargo-metadata"]);
    let (binding, metadata, args) = (
        selected.crate_binding,
        selected.metadata_observation,
        selected.args.clone(),
    );
    let PreparedExtractionV1::Selected(actual) = select_bf16_generated_source_v1_mode(
        PreparedExtractionV1::Selected(selected),
        Some("fresh-generated".into()),
    )
    .unwrap() else {
        panic!("lost actual selected invocation")
    };
    assert_eq!(actual.crate_binding, binding);
    assert_eq!(actual.metadata_observation, metadata);
    assert_eq!(actual.args, args);
    assert!(
        matches!(actual.mode, ExtractionModeV1::Bf16GeneratedSourceV1(p) if p == "fresh-generated")
    );
}

#[test]
fn generated_bf16_and_direct_source_modes_are_disjoint() {
    for direct in [false, true] {
        for generated in [false, true] {
            assert_eq!(
                require_disjoint_bf16_source_modes_v1(direct, generated).is_ok(),
                !(direct && generated)
            );
        }
    }
}

#[test]
fn generated_bf16_rejects_all_alternate_modes_and_binding_sidecar() {
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
        ExtractionModeV1::Bf16TileSourceV1(p()),
        ExtractionModeV1::Bf16GeneratedSourceV1(p()),
    ] {
        let mut selected = selected_compile("unit", &["metadata"]);
        selected.mode = mode;
        assert!(
            select_bf16_generated_source_v1_mode(
                PreparedExtractionV1::Selected(selected),
                Some(p())
            )
            .is_err()
        );
    }
    let mut selected = selected_compile("unit", &["metadata"]);
    selected.crate_binding_output = Some("binding".into());
    assert!(
        select_bf16_generated_source_v1_mode(PreparedExtractionV1::Selected(selected), Some(p()))
            .is_err()
    );
}

#[test]
#[cfg(target_os = "linux")]
fn generated_bf16_absence_and_dependency_probe_are_not_actions() {
    let passthrough = PreparedExtractionV1::Passthrough {
        executable: "real-rustc".into(),
        forwarded_args: vec!["--version".into()],
    };
    assert!(
        matches!(select_bf16_generated_source_v1_mode(passthrough, Some("fresh".into())).unwrap(),
        PreparedExtractionV1::Passthrough { executable, forwarded_args }
        if executable == "real-rustc" && forwarded_args == [OsString::from("--version")])
    );
    assert!(matches!(
        select_bf16_generated_source_v1_mode(
            PreparedExtractionV1::Selected(selected_compile("unit", &["metadata"])),
            None,
        )
        .unwrap(),
        PreparedExtractionV1::Selected(SelectedExtractionV1 {
            mode: ExtractionModeV1::KernelIr,
            ..
        })
    ));
}

#[test]
fn generated_bf16_paths_reject_before_io() {
    for path in [
        OsString::new(),
        OsString::from("x".repeat(4097)),
        OsString::from("a\0b"),
    ] {
        assert!(
            select_bf16_generated_source_v1_mode(
                PreparedExtractionV1::Selected(selected_compile("unit", &["metadata"])),
                Some(path),
            )
            .is_err()
        );
    }
}

#[test]
fn generated_bf16_cannot_publish_or_select_origin_or_normal_composition() {
    let mode = ExtractionModeV1::Bf16GeneratedSourceV1("out".into());
    assert!(selected_bf16_tile_promotion_request_v1(&mode, Some("request".into())).is_err());
    assert!(selected_composition_promotion_request_v1(&mode, Some("request".into())).is_err());
    assert!(ordered_origin_v1::selected_output(&mode, Some("origin".into())).is_err());
    assert!(
        require_normal_composition_mode_v1(&mode, false, Some(std::ffi::OsStr::new("1"))).is_err()
    );
    let mut selected = selected_compile("unit", &["metadata"]);
    selected.mode = mode;
    assert!(
        select_bf16_tile_source_v1_mode(
            PreparedExtractionV1::Selected(selected),
            Some("direct".into())
        )
        .is_err()
    );
}

#[test]
fn generated_bf16_opt_in_does_not_escape_to_rustc_passthrough() {
    let command = passthrough_command("real-rustc".into(), vec!["--version".into()]);
    assert!(command.get_envs().any(|(name, value)| name
        == EXTRACT_BF16_GENERATED_SOURCE_DIRECTORY_ENV_V1
        && value.is_none()));
}
