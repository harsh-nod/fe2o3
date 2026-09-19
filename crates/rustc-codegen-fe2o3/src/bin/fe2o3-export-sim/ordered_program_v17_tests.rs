fn program_diagnostic_args() -> Vec<OsString> {
    let mut args = diagnostic_args();
    args[0] = OsString::from("--diagnostic-kir-v17");
    args
}

#[test]
fn program_export_selects_exact_v17_without_source_capture_or_bundle_relabel() {
    let mut args = program_diagnostic_args();
    args.extend(["--", "--features", "ordered-program-v32", "--offline"].map(OsString::from));
    let options = parse(args, &env::temp_dir()).unwrap();
    assert_eq!(options.format, ExportFormat::DiagnosticKirV17);
    assert_eq!(options.format.output_env(), DIAGNOSTIC_KIR_ENV_V17);
    assert_eq!(options.format.label(), "diagnostic canonical KIR V17");
    assert_eq!(options.target_profile, ProductionAmdTargetProfileV1::Gfx942);
    assert_eq!(
        options.cargo_args,
        ["--features", "ordered-program-v32", "--offline"].map(OsString::from)
    );
    assert_eq!(
        options.format.rustflags(options.target_profile),
        fixed_target_rustflags(options.target_profile, 1)
    );
    assert!(conflicting_extraction_environment().contains(&DIAGNOSTIC_KIR_ENV_V17));
    assert_ne!(
        DIAGNOSTIC_KIR_ENV_V17,
        ExportFormat::DiagnosticKirV16.output_env()
    );
    for version in 1..=6 {
        assert_ne!(
            DIAGNOSTIC_KIR_ENV_V17,
            ExportFormat::SimulationBundle(version).output_env()
        );
    }
}

#[test]
fn program_export_refuses_other_modes_targets_duplicates_and_path_like_flag_values() {
    for before in [false, true] {
        for extra in [
            vec!["--diagnostic-kir-v16".to_owned()],
            vec!["--bundle-version".to_owned(), "1".to_owned()],
            vec!["--bundle-version".to_owned(), "6".to_owned()],
        ] {
            let mut args = program_diagnostic_args();
            if before {
                args.splice(0..0, extra.into_iter().map(OsString::from));
            } else {
                args.extend(extra.into_iter().map(OsString::from));
            }
            assert!(
                parse(args, &env::temp_dir())
                    .unwrap_err()
                    .contains("mutually exclusive")
            );
        }
    }
    for extra in [
        vec!["--target", "gfx950"],
        vec!["--diagnostic-kir-v17"],
        vec!["unexpected.kir"],
    ] {
        let mut args = program_diagnostic_args();
        args.extend(extra.into_iter().map(OsString::from));
        assert!(parse(args, &env::temp_dir()).is_err());
    }
}
