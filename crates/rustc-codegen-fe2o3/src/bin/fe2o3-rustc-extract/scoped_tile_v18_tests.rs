#[test]
fn scoped_tile_v18_environment_is_paired_closed_and_disjoint_before_passthrough() {
    use fe2o3_lower_mir_kernel::ProductionScopedTileObservationOrderV29 as Order;
    use std::ffi::OsStr;
    for (name, order) in [("blocked", Order::Blocked), ("striped", Order::Striped)] {
        assert_eq!(
            scoped_tile_v18::validate_options(
                Some(OsStr::new("out")),
                Some(OsStr::new(name)),
                [false; 26]
            )
            .unwrap(),
            Some(order)
        );
        for conflict in 0..26 {
            let mut others = [false; 26];
            others[conflict] = true;
            assert!(
                scoped_tile_v18::validate_options(
                    Some(OsStr::new("out")),
                    Some(OsStr::new(name)),
                    others
                )
                .is_err()
            );
        }
    }
    for (output, order) in [
        (None, Some("blocked")),
        (Some("out"), None),
        (Some(""), Some("blocked")),
        (Some("out"), Some("auto")),
    ] {
        assert!(
            scoped_tile_v18::validate_options(
                output.map(OsStr::new),
                order.map(OsStr::new),
                [false; 26]
            )
            .is_err()
        );
    }
    assert!(
        scoped_tile_v18::validate_options(None, None, [true; 26])
            .unwrap()
            .is_none()
    );
}

#[test]
fn scoped_tile_v18_selection_preserves_actual_bindings_and_refuses_other_modes() {
    use fe2o3_lower_mir_kernel::ProductionScopedTileObservationOrderV29 as Order;
    for order in [Order::Blocked, Order::Striped] {
        let selected = selected_compile("unit", &["actual"]);
        let binding = selected.crate_binding;
        let observation = selected.metadata_observation;
        let args = selected.args.clone();
        let PreparedExtractionV1::Selected(actual) = scoped_tile_v18::select_mode(
            PreparedExtractionV1::Selected(selected),
            Some("tile.kir".into()),
            Some(order),
        )
        .unwrap() else {
            panic!("selected invocation")
        };
        assert_eq!(actual.crate_binding, binding);
        assert_eq!(actual.metadata_observation, observation);
        assert_eq!(actual.args, args);
        assert!(
            matches!(actual.mode, ExtractionModeV1::DiagnosticKirV18(path, got) if path == "tile.kir" && got == order)
        );
    }
    for mode in [
        ExtractionModeV1::RankedMemory,
        ExtractionModeV1::SimulationBundle("other".into()),
        ExtractionModeV1::DiagnosticKirV17("other".into()),
        ExtractionModeV1::DiagnosticKirV19("other".into()),
        ExtractionModeV1::PhysicalEntryDiagnosticV20("other".into()),
        ExtractionModeV1::OrderedCompositionDiagnosticV1("other".into()),
        ExtractionModeV1::Bf16TileSourceV1("other".into()),
        ExtractionModeV1::Bf16GeneratedSourceV1("other".into()),
    ] {
        let mut selected = selected_compile("unit", &["actual"]);
        selected.mode = mode;
        assert!(
            scoped_tile_v18::select_mode(
                PreparedExtractionV1::Selected(selected),
                Some("tile".into()),
                Some(Order::Blocked)
            )
            .is_err()
        );
    }
    let mut selected = selected_compile("unit", &["actual"]);
    selected.crate_binding_output = Some("binding".into());
    assert!(
        scoped_tile_v18::select_mode(
            PreparedExtractionV1::Selected(selected),
            Some("tile".into()),
            Some(Order::Blocked)
        )
        .is_err()
    );
}

#[test]
fn scoped_tile_v18_passthrough_scrubs_both_diagnostic_controls() {
    use fe2o3_lower_mir_kernel::ProductionScopedTileObservationOrderV29 as Order;
    let prepared = PreparedExtractionV1::Passthrough {
        executable: "rustc".into(),
        forwarded_args: vec!["--version".into()],
    };
    let prepared =
        scoped_tile_v18::select_mode(prepared, Some("tile".into()), Some(Order::Striped)).unwrap();
    let PreparedExtractionV1::Passthrough {
        executable,
        forwarded_args,
    } = prepared
    else {
        panic!("passthrough")
    };
    let command = passthrough_command(executable, forwarded_args);
    for key in [
        scoped_tile_v18::OUTPUT_ENV,
        scoped_tile_v18::ORDER_ENV,
        EXTRACT_ENGINEERING_CAPTURE_ENV_V1,
        EXTRACT_BF16_TILE_SOURCE_DIRECTORY_ENV_V1,
        EXTRACT_BF16_TILE_PROMOTION_REQUEST_ENV_V1,
        EXTRACT_BF16_GENERATED_SOURCE_DIRECTORY_ENV_V1,
    ] {
        assert!(
            command
                .get_envs()
                .any(|(name, value)| name == key && value.is_none())
        );
    }
}

#[test]
fn scoped_tile_v18_bf16_conflicts_refuse_before_passthrough_preparation() {
    use std::ffi::OsStr;
    for order in ["blocked", "striped"] {
        for (directory, request, generated) in [
            (true, false, false),
            (false, true, false),
            (true, true, false),
            (false, false, true),
            (true, false, true),
            (false, true, true),
            (true, true, true),
        ] {
            let mut others = [false; 26];
            others[22] = directory;
            others[23] = request;
            others[24] = generated;
            let prepared_passthrough = std::cell::Cell::new(false);
            let result = scoped_tile_v18::validate_options(
                Some(OsStr::new("tile.kir")),
                Some(OsStr::new(order)),
                others,
            )
            .and_then(|order| {
                prepared_passthrough.set(true);
                scoped_tile_v18::select_mode(
                    PreparedExtractionV1::Passthrough {
                        executable: "rustc".into(),
                        forwarded_args: vec!["--version".into()],
                    },
                    Some("tile.kir".into()),
                    order,
                )
            });
            assert!(result.is_err());
            assert!(!prepared_passthrough.get());
        }
    }
}

#[test]
#[cfg(target_os = "linux")]
fn scoped_tile_v18_bf16_selector_chains_preserve_standalone_and_refuse_both_orders() {
    use fe2o3_lower_mir_kernel::ProductionScopedTileObservationOrderV29 as Order;
    for order in [Order::Blocked, Order::Striped] {
        for (tile, bf16) in [(false, false), (true, false), (false, true), (true, true)] {
            for tile_first in [false, true] {
                let selected = selected_compile("unit", &["actual-cargo-metadata"]);
                let binding = selected.crate_binding;
                let observation = selected.metadata_observation;
                let args = selected.args.clone();
                let prepared = PreparedExtractionV1::Selected(selected);
                let select_tile = |prepared| {
                    scoped_tile_v18::select_mode(
                        prepared,
                        tile.then(|| "tile.kir".into()),
                        tile.then_some(order),
                    )
                };
                let select_bf16 = |prepared| {
                    select_bf16_tile_source_v1_mode(prepared, bf16.then(|| "bf16-source".into()))
                };
                let result = if tile_first {
                    select_tile(prepared).and_then(select_bf16)
                } else {
                    select_bf16(prepared).and_then(select_tile)
                };
                if tile && bf16 {
                    assert!(result.is_err());
                    continue;
                }
                let PreparedExtractionV1::Selected(actual) = result.unwrap() else {
                    panic!("lost selected source")
                };
                assert_eq!(actual.crate_binding, binding);
                assert_eq!(actual.metadata_observation, observation);
                assert_eq!(actual.args, args);
                match (tile, bf16) {
                    (true, false) => assert!(matches!(
                        actual.mode,
                        ExtractionModeV1::DiagnosticKirV18(path, got)
                            if path == "tile.kir" && got == order
                    )),
                    (false, true) => assert!(matches!(
                        actual.mode,
                        ExtractionModeV1::Bf16TileSourceV1(path) if path == "bf16-source"
                    )),
                    (false, false) => assert!(matches!(actual.mode, ExtractionModeV1::KernelIr)),
                    (true, true) => unreachable!(),
                }
            }
        }
    }
}

#[test]
fn scoped_tile_v18_engineering_capture_refuses_before_passthrough_preparation() {
    use std::ffi::OsStr;

    for order in ["blocked", "striped"] {
        let mut others = [false; 26];
        others[25] = true;
        let prepared_passthrough = std::cell::Cell::new(false);
        let result = scoped_tile_v18::validate_options(
            Some(OsStr::new("tile.kir")),
            Some(OsStr::new(order)),
            others,
        )
        .and_then(|order| {
            prepared_passthrough.set(true);
            scoped_tile_v18::select_mode(
                PreparedExtractionV1::Passthrough {
                    executable: "rustc".into(),
                    forwarded_args: vec!["--version".into()],
                },
                Some("tile.kir".into()),
                order,
            )
        });
        assert!(result.is_err());
        assert!(!prepared_passthrough.get());
    }
}
