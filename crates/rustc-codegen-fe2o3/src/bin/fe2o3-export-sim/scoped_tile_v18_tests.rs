fn tile_args(order: &str) -> Vec<OsString> {
    let mut args = diagnostic_args();
    args[0] = "--diagnostic-kir-v18".into();
    args.extend(["--diagnostic-tile-order", order].map(OsString::from));
    args
}

#[test]
fn tile_v18_export_is_explicit_and_preserves_each_declared_cpu_profile() {
    for order in ["blocked", "striped"] {
        for target in ["gfx942", "gfx950"] {
            let mut args = tile_args(order);
            args.extend(["--target", target].map(OsString::from));
            let options = parse(args, &env::temp_dir()).unwrap();
            assert_eq!(options.format, ExportFormat::DiagnosticKirV18);
            assert_eq!(options.format.output_env(), DIAGNOSTIC_KIR_ENV_V18);
            assert_eq!(options.diagnostic_tile_order.as_deref(), Some(order));
            let flags = options.format.rustflags(options.target_profile);
            assert!(flags.ends_with(" -Coverflow-checks=on"));
            assert!(flags.contains(&format!("-Ctarget-cpu={target}")));
            assert!(!flags.contains("debuginfo"));
        }
    }
    let conflicts = conflicting_extraction_environment();
    assert!(conflicts.contains(&DIAGNOSTIC_KIR_ENV_V18));
    assert!(conflicts.contains(&DIAGNOSTIC_TILE_ORDER_ENV_V18));
}

#[test]
fn tile_v18_export_pins_mir_without_changing_other_formats() {
    for target in [
        ProductionAmdTargetProfileV1::Gfx942,
        ProductionAmdTargetProfileV1::Gfx950,
    ] {
        let flags = ExportFormat::DiagnosticKirV18.rustflags(target);
        assert_eq!(
            flags,
            fixed_target_rustflags(target, 1) + " -Zmir-opt-level=0 -Coverflow-checks=on"
        );
        assert_eq!(flags.matches("-Zmir-opt-level=").count(), 1);
        assert_eq!(flags.matches("-Coverflow-checks=").count(), 1);
        assert!(!flags.contains("debug-assertions"));
        for version in 1..=6 {
            assert_eq!(
                ExportFormat::SimulationBundle(version).rustflags(target),
                fixed_target_rustflags(target, version)
            );
        }
        for format in [
            ExportFormat::DiagnosticKirV16,
            ExportFormat::DiagnosticKirV17,
        ] {
            assert_eq!(format.rustflags(target), fixed_target_rustflags(target, 1));
        }
    }
}

#[test]
fn tile_v18_export_rejects_missing_duplicate_and_foreign_selectors() {
    for extra in [
        vec!["--diagnostic-kir-v18"],
        vec!["--diagnostic-kir-v17"],
        vec!["--diagnostic-kir-v16"],
        vec!["--bundle-version", "6"],
        vec!["--diagnostic-tile-order", "striped"],
        vec!["--diagnostic-ordered-origin-v1", "/unopened-origin"],
    ] {
        let mut args = tile_args("blocked");
        args.extend(extra.into_iter().map(OsString::from));
        assert!(parse(args, &env::temp_dir()).is_err());
    }
    for order in ["", "Blocked", "auto", "blocked,striped"] {
        assert!(parse(tile_args(order), &env::temp_dir()).is_err());
    }
    let mut missing = diagnostic_args();
    missing[0] = "--diagnostic-kir-v18".into();
    assert!(parse(missing, &env::temp_dir()).is_err());
    let mut stray = diagnostic_args();
    stray.extend(["--diagnostic-tile-order", "blocked"].map(OsString::from));
    assert!(parse(stray, &env::temp_dir()).is_err());
}

#[test]
fn tile_v18_export_rejects_target_and_profile_overrides() {
    for extra in [
        vec!["--", "--target=gfx950"],
        vec!["--", "--profile", "release"],
        vec!["--", "--config", "build.rustflags=[]"],
        vec!["--target", "gfx1100"],
    ] {
        let mut args = tile_args("striped");
        args.extend(extra.into_iter().map(OsString::from));
        assert!(parse(args, &env::temp_dir()).is_err());
    }
}
