#[test]
fn scoped_tile_v18_environment_is_paired_closed_and_disjoint_before_passthrough() {
    use fe2o3_lower_mir_kernel::ProductionScopedTileObservationOrderV29 as Order;
    use std::ffi::OsStr;
    for (name, order) in [("blocked", Order::Blocked), ("striped", Order::Striped)] {
        assert_eq!(
            scoped_tile_v18::validate_options(
                Some(OsStr::new("out")),
                Some(OsStr::new(name)),
                [false; 22]
            )
            .unwrap(),
            Some(order)
        );
        for conflict in 0..22 {
            let mut others = [false; 22];
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
                [false; 22]
            )
            .is_err()
        );
    }
    assert!(
        scoped_tile_v18::validate_options(None, None, [true; 22])
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
    for key in [scoped_tile_v18::OUTPUT_ENV, scoped_tile_v18::ORDER_ENV] {
        assert!(
            command
                .get_envs()
                .any(|(name, value)| name == key && value.is_none())
        );
    }
}
