//! Synthetic wire ingress tests only: these graphs never authenticate source.
use super::*;
#[path = "../../tests/fixtures/diagnostic_kir_v19.rs"]
mod fixture;

#[test]
fn exact_v19_identity_reaches_the_existing_cpu_engine_for_both_branches() {
    for diamond in [false, true] {
        for selector in [0, 1, u32::MAX] {
            let owner = fixture::owner(&if diamond {
                fixture::diamond()
            } else {
                fixture::single()
            });
            let raw = fixture::request(selector);
            let input =
                load_debug_simulation_input_bytes_v19(owner.canonical_bytes(), &raw).unwrap();
            assert_eq!(input.module.identity().wire_version(), 19);
            assert_eq!(input.module.identity().digest(), owner.identity().digest());
            assert!(!input.module.grants_execution_authority());
            assert!(input.simulation_bundle_evidence().is_none());
            assert!(input.simulation_bundle_subject().is_none());
            assert!(input.persisted_schedule_binding().is_err());
            let original = input.request.clone();
            let result = input
                .module
                .simulate(
                    &input.request,
                    input.simulation_target(),
                    input.simulation_limits,
                )
                .unwrap();
            assert_eq!(input.request, original);
            let expected = if diamond && selector != 0 {
                42_u32
            } else {
                19_u32
            };
            assert_eq!(result.buffer(0).unwrap().bytes(), fixture::output(expected));
        }
    }
}

#[test]
fn old_grammars_wrong_versions_tails_and_invalid_requests_do_not_fall_back() {
    let owner = fixture::owner(&fixture::single());
    let raw = fixture::request(0);
    assert!(crate::load_debug_simulation_input_bytes_v17(owner.canonical_bytes(), &raw).is_err());
    assert!(crate::load_debug_simulation_input_bytes_v16(owner.canonical_bytes(), &raw).is_err());
    for version in [7_u16, 12, 16, 17, 18, 20] {
        let mut changed = owner.canonical_bytes().to_vec();
        changed[8..10].copy_from_slice(&version.to_le_bytes());
        assert!(load_debug_simulation_input_bytes_v19(&changed, &raw).is_err());
    }
    let mut trailing = owner.canonical_bytes().to_vec();
    trailing.push(0);
    assert!(load_debug_simulation_input_bytes_v19(&trailing, &raw).is_err());
    let mut request: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    request["invented_source_owner"] = serde_json::json!(true);
    assert!(
        load_debug_simulation_input_bytes_v19(
            owner.canonical_bytes(),
            &serde_json::to_vec(&request).unwrap()
        )
        .is_err()
    );
}

#[test]
fn resource_failure_restores_the_caller_floor_without_minting_input() {
    let owner = fixture::owner(&fixture::single());
    let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
    let mut budget =
        CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, MAX_CANONICAL_STORAGE_V19);
    budget.reserve_storage(97).unwrap();
    let error = admit_with_budget(
        owner.canonical_bytes(),
        owner.canonical_bytes().len(),
        cli_simulation_limits(),
        &mut budget,
    )
    .unwrap_err();
    assert_eq!(budget.storage(), 97);
    assert_eq!(error.0.kind, ErrorKind::KirV19WorkLimit);
}

#[test]
fn filesystem_ingress_reuses_no_follow_regular_file_capture() {
    let result =
        load_debug_simulation_input_v19(Path::new("/dev/null"), Path::new("/missing/request"));
    assert!(result.is_err());
    let bytes = vec![0; MAX_KIR_BYTES + 1];
    assert!(load_debug_simulation_input_bytes_v19(&bytes, &fixture::request(0)).is_err());
}

#[test]
fn parser_refuses_every_persisted_schedule_mode_before_opening_files() {
    let baseline = [
        "--diagnostic-kir-v19",
        "/missing/program",
        "--request",
        "/missing/request",
    ];
    assert!(matches!(
        parse_options(baseline.map(OsString::from).into_iter())
            .unwrap()
            .program,
        ProgramInput::DiagnosticKirV19(_)
    ));
    for extra in [
        vec!["--record-canonical-schedule", "/missing/output"],
        vec!["--record-seeded-schedule", "/missing/output"],
        vec!["--replay-schedule", "/missing"],
        vec!["--explore-seeded-schedules", "1"],
        vec!["--reduce-failure"],
        vec!["--replay-failure-reduction", "/missing"],
        vec!["--schedule-seed", "0"],
        vec!["--schedule-max-decisions", "1"],
        vec!["--exploration-max-retained-decisions", "1"],
    ] {
        let error = run(baseline.into_iter().chain(extra).map(OsString::from)).unwrap_err();
        assert_eq!(error.0.stage, Stage::Arguments);
        assert_eq!(error.0.kind, ErrorKind::ScheduleInputUnsupported);
    }
    for flag in [
        "--diagnostic-kir-v19",
        "--diagnostic-kir-v17",
        "--diagnostic-kir-v16",
        "--kir-v7",
        "--kir-v12",
        "--bundle",
        "--bundle-v5",
        "--bundle-v6",
    ] {
        assert!(
            parse_options(
                baseline
                    .into_iter()
                    .chain([flag, "other"])
                    .map(OsString::from)
            )
            .is_err()
        );
    }
}
