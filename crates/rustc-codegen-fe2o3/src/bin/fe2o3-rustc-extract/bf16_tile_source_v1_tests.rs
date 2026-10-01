#[test]
#[cfg(target_os = "linux")]
fn bf16_source_selector_preserves_current_invocation_and_bindings() {
    let selected = selected_compile("unit", &["actual-cargo-metadata"]);
    let (binding, metadata, args) = (
        selected.crate_binding,
        selected.metadata_observation,
        selected.args.clone(),
    );
    let PreparedExtractionV1::Selected(actual) = select_bf16_tile_source_v1_mode(
        PreparedExtractionV1::Selected(selected),
        Some("fresh-bf16-source".into()),
    )
    .unwrap() else {
        panic!("lost selected source")
    };
    assert_eq!(actual.crate_binding, binding);
    assert_eq!(actual.metadata_observation, metadata);
    assert_eq!(actual.args, args);
    assert!(
        matches!(actual.mode, ExtractionModeV1::Bf16TileSourceV1(p) if p == "fresh-bf16-source")
    );
}
#[test]
fn bf16_source_rejects_other_outputs_and_binding_sidecar() {
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
        ExtractionModeV1::DiagnosticKirV18(
            p(),
            fe2o3_lower_mir_kernel::ProductionScopedTileObservationOrderV29::Blocked,
        ),
        ExtractionModeV1::DiagnosticKirV19(p()),
        ExtractionModeV1::PhysicalEntryDiagnosticV20(p()),
        ExtractionModeV1::PhysicalGlobalCopyDiagnosticV21(p()),
        ExtractionModeV1::PhysicalLdsExchangeDiagnosticV22(p()),
        ExtractionModeV1::OrderedCompositionDiagnosticV1(p()),
        ExtractionModeV1::Bf16TileSourceV1(p()),
    ] {
        let mut selected = selected_compile("unit", &["metadata"]);
        selected.mode = mode;
        assert!(
            select_bf16_tile_source_v1_mode(PreparedExtractionV1::Selected(selected), Some(p()),)
                .is_err()
        );
    }
    let mut selected = selected_compile("unit", &["metadata"]);
    selected.crate_binding_output = Some("binding".into());
    assert!(
        select_bf16_tile_source_v1_mode(PreparedExtractionV1::Selected(selected), Some(p()),)
            .is_err()
    );
}
#[test]
fn bf16_source_disjointness_covers_every_diagnostic_bit() {
    for mask in 0..256u16 {
        let others = std::array::from_fn(|bit| mask & (1 << bit) != 0);
        assert_eq!(
            require_disjoint_bf16_tile_source_v1(true, others).is_ok(),
            mask == 0
        );
        assert!(require_disjoint_bf16_tile_source_v1(false, others).is_ok());
    }
}
#[test]
#[cfg(target_os = "linux")]
fn bf16_source_absence_and_passthrough_are_not_actions() {
    let dependency = || PreparedExtractionV1::Passthrough {
        executable: "real-rustc".into(),
        forwarded_args: vec!["--version".into()],
    };
    assert!(
        matches!(select_bf16_tile_source_v1_mode(dependency(), Some("fresh".into())).unwrap(),
        PreparedExtractionV1::Passthrough { executable, forwarded_args }
        if executable == "real-rustc" && forwarded_args == [OsString::from("--version")])
    );
    assert!(matches!(
        select_bf16_tile_source_v1_mode(
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
fn bf16_source_paths_are_bounded_before_io() {
    for path in [
        OsString::new(),
        OsString::from("x".repeat(4097)),
        OsString::from("a\0b"),
    ] {
        assert!(bf16_tile_path_v1(&path).is_err());
    }
    assert!(bf16_tile_path_v1(&OsString::from("x".repeat(4096))).is_ok());
}
#[test]
#[cfg(target_os = "linux")]
fn bf16_source_promotion_requires_its_explicit_mode() {
    assert!(
        selected_bf16_tile_promotion_request_v1(&ExtractionModeV1::KernelIr, None)
            .unwrap()
            .is_none()
    );
    for mode in [
        ExtractionModeV1::KernelIr,
        ExtractionModeV1::OrderedCompositionDiagnosticV1("out".into()),
        ExtractionModeV1::DiagnosticKirV17("out".into()),
        ExtractionModeV1::DiagnosticKirV18(
            "out".into(),
            fe2o3_lower_mir_kernel::ProductionScopedTileObservationOrderV29::Blocked,
        ),
        ExtractionModeV1::AmdgpuCompilerHandoff("out".into()),
    ] {
        assert!(
            selected_bf16_tile_promotion_request_v1(&mode, Some("request.json".into())).is_err()
        );
    }
    assert_eq!(
        selected_bf16_tile_promotion_request_v1(
            &ExtractionModeV1::Bf16TileSourceV1("out".into()),
            Some("request.json".into()),
        )
        .unwrap(),
        Some(PathBuf::from("request.json"))
    );
}
#[test]
fn bf16_source_rejects_legacy_mutation_origin_and_normal_flags() {
    let mode = ExtractionModeV1::Bf16TileSourceV1("out".into());
    assert!(
        selected_composition_promotion_request_v1(&mode, Some("other-request".into())).is_err()
    );
    assert!(ordered_origin_v1::selected_output(&mode, Some("origin".into())).is_err());
    assert!(
        require_normal_composition_mode_v1(&mode, false, Some(std::ffi::OsStr::new("1"))).is_err()
    );
    let mut selected = selected_compile("unit", &["metadata"]);
    selected.mode = mode;
    assert!(
        select_ordered_composition_diagnostic_v1_mode(
            PreparedExtractionV1::Selected(selected),
            Some("old".into()),
        )
        .is_err()
    );
}
#[test]
fn bf16_source_opt_ins_are_removed_from_passthrough() {
    let command = passthrough_command("real-rustc".into(), vec!["--version".into()]);
    let removed: Vec<_> = command
        .get_envs()
        .filter_map(|(key, value)| value.is_none().then_some(key))
        .collect();
    assert!(removed.contains(&std::ffi::OsStr::new(
        EXTRACT_BF16_TILE_SOURCE_DIRECTORY_ENV_V1
    )));
    assert!(removed.contains(&std::ffi::OsStr::new(
        EXTRACT_BF16_TILE_PROMOTION_REQUEST_ENV_V1
    )));
}
