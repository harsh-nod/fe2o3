//! Synthetic V18 boundary controls, not source or GPU qualification.
use super::*;
#[path = "../../tests/fixtures/diagnostic_kir_v18.rs"]
mod fixture;

const FLOOR: usize = 97;

#[test]
fn explicit_v18_target_preserves_graph_and_executes_under_exact_profile() {
    let bytes = fixture::bytes(&fixture::module());
    let request = fixture::request(37);
    for name in ["gfx942", "gfx950"] {
        let target = SimulationTargetV1::amdgpu_from_device_target(name).unwrap();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(fixture::BOUND);
        let mut budget = Budget::new(&mut work, fixture::BOUND);
        budget.reserve_storage(FLOOR).unwrap();
        let (input, receipt) = load_bytes_for_target(
            &bytes,
            &request,
            bytes.len() + request.len(),
            &mut budget,
            target,
        )
        .unwrap();
        assert_eq!(budget.storage(), FLOOR);
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        assert_eq!(input.simulation_target(), target);
        with_input(&bytes, &request, |legacy| {
            assert_eq!(input.module.identity(), legacy.module.identity());
            assert_eq!(input.request, legacy.request);
            assert_eq!(legacy.simulation_target().amd_profile(), None);
        });
        let output = input
            .module
            .simulate(&input.request, target, input.simulation_limits)
            .unwrap();
        assert_eq!(output.buffer(0).unwrap().bytes(), fixture::output(37));
        drop(output);
        drop(input);
        budget.release_storage(receipt.retained_storage()).unwrap();
        assert_eq!(budget.storage(), FLOOR);
        let options = parse_options(
            [
                "--diagnostic-kir-v18",
                "graph",
                "--request",
                "request",
                "--diagnostic-target",
                name,
            ]
            .into_iter()
            .map(OsString::from),
        )
        .unwrap();
        assert_eq!(
            options.program,
            ProgramInput::DiagnosticKirV18Target("graph".into(), target)
        );
    }
    for args in [
        vec![
            "--kir-v12",
            "graph",
            "--request",
            "request",
            "--diagnostic-target",
            "gfx942",
        ],
        vec![
            "--diagnostic-kir-v18",
            "graph",
            "--request",
            "request",
            "--diagnostic-target",
            "gfx999",
        ],
        vec![
            "--diagnostic-kir-v18",
            "graph",
            "--request",
            "request",
            "--diagnostic-target",
            "gfx942",
            "--diagnostic-target",
            "gfx950",
        ],
        vec![
            "--diagnostic-kir-v18",
            "graph",
            "--request",
            "request",
            "--diagnostic-target",
            "gfx942",
            "--record-canonical-schedule",
            "schedule",
        ],
    ] {
        assert!(parse_options(args.into_iter().map(OsString::from)).is_err());
    }
}
fn with_input<T>(
    bytes: &[u8],
    request: &[u8],
    observe: impl FnOnce(&crate::AdmittedSimulationInputV1) -> T,
) -> T {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(fixture::BOUND);
    let mut budget = Budget::new(&mut work, fixture::BOUND);
    budget.reserve_storage(FLOOR).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let (input, receipt) =
        load_debug_simulation_input_bytes_v18(bytes, request, &mut budget).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let value = observe(&input);
    drop(input);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    assert!(budget.work_ledger_identity_v1() == ledger);
    value
}

#[test]
fn exact_bytes_keep_layouts_identity_and_generic_scalar_memory_results() {
    let module = fixture::module();
    let bytes = fixture::bytes(&module);
    let domain = fe2o3_kernel_ir::VERIFIED_CANONICAL_KERNEL_IR_V18_IDENTITY_DOMAIN_V1;
    let mut hash = Sha256::new();
    hash.update((domain.len() as u32).to_le_bytes());
    hash.update(domain);
    hash.update(fe2o3_kernel_ir::VERIFIED_CANONICAL_KERNEL_IR_V18_IDENTITY_POLICY_V1.to_le_bytes());
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(&bytes);
    let identity: [u8; 32] = hash.finalize().into();
    for value in [0, 37, u32::MAX] {
        with_input(&bytes, &fixture::request(value), |input| {
            assert_eq!(input.module.module(), &module);
            assert_eq!(input.module.identity().wire_version(), 18);
            assert_eq!(*input.module.identity().digest(), identity);
            assert_eq!(
                input.module.identity().canonical_length(),
                bytes.len() as u64
            );
            assert!(!input.module.grants_execution_authority());
            assert!(input.simulation_bundle_evidence().is_none());
            assert!(input.simulation_bundle_subject().is_none());
            assert!(input.persisted_schedule_binding().is_err());
            let before = input.request.clone();
            let output = input
                .module
                .simulate(
                    &input.request,
                    input.simulation_target(),
                    input.simulation_limits,
                )
                .unwrap();
            assert_eq!(input.request, before);
            let buffer = output.buffer(0).unwrap();
            assert_eq!(buffer.bytes(), fixture::output(value));
            assert!(buffer.initialized()[..256].iter().all(|bit| *bit));
            assert!(buffer.initialized()[256..].iter().all(|bit| !*bit));
        });
    }
}

#[test]
fn exact_file_capture_matches_bytes_and_refuses_links_and_special_files() {
    let bytes = fixture::bytes(&fixture::module());
    let request = fixture::request(37);
    let files = fixture::Files::new(&bytes, &request);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(fixture::BOUND);
    let mut budget = Budget::new(&mut work, fixture::BOUND);
    let (input, receipt) =
        load_debug_simulation_input_v18(&files.kir, &files.request, &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    with_input(&bytes, &request, |other| {
        assert_eq!(input.module.module(), other.module.module());
        assert_eq!(input.module.identity(), other.module.identity());
        assert_eq!(input.request, other.request);
    });
    drop(input);
    budget.release_storage(receipt.retained_storage()).unwrap();
    let link = files.path("link");
    std::os::unix::fs::symlink(&files.kir, &link).unwrap();
    assert!(load_debug_simulation_input_v18(&link, &files.request, &mut budget).is_err());
    assert!(
        load_debug_simulation_input_v18(Path::new("/dev/null"), &files.request, &mut budget)
            .is_err()
    );
    assert_eq!(budget.storage(), 0);
}

#[test]
fn wrong_versions_tails_and_request_fields_do_not_fall_back() {
    let bytes = fixture::bytes(&fixture::module());
    let request = fixture::request(37);
    for version in [7_u16, 12, 16, 17, 19, 20] {
        let mut changed = bytes.clone();
        changed[8..10].copy_from_slice(&version.to_le_bytes());
        let mut work = CanonicalKernelIrWorkBudgetV1::new(fixture::BOUND);
        let mut budget = Budget::new(&mut work, fixture::BOUND);
        let error =
            load_debug_simulation_input_bytes_v18(&changed, &request, &mut budget).unwrap_err();
        assert_eq!(error.code, "kir_v18_wrong_version");
        assert_eq!(budget.storage(), 0);
    }
    for changed in [
        bytes[..bytes.len() - 1].to_vec(),
        [bytes.as_slice(), &[0]].concat(),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(fixture::BOUND);
        let mut budget = Budget::new(&mut work, fixture::BOUND);
        assert!(load_debug_simulation_input_bytes_v18(&changed, &request, &mut budget).is_err());
        assert_eq!(budget.storage(), 0);
    }
    let mut document: serde_json::Value = serde_json::from_slice(&request).unwrap();
    document["invented_source_owner"] = serde_json::json!(true);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(fixture::BOUND);
    let mut budget = Budget::new(&mut work, fixture::BOUND);
    let error = load_debug_simulation_input_bytes_v18(
        &bytes,
        &serde_json::to_vec(&document).unwrap(),
        &mut budget,
    )
    .unwrap_err();
    assert_eq!(error.stage, "request");
    assert_eq!(budget.storage(), 0);
}

#[test]
fn work_and_storage_denials_preserve_floor_and_first_failure() {
    let bytes = fixture::bytes(&fixture::module());
    let request = fixture::request(37);
    for (work_limit, storage_limit, code) in [
        (0, fixture::BOUND, "kir_v18_work_limit"),
        (fixture::BOUND, FLOOR, "kir_v18_storage_limit"),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(FLOOR).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let error =
            load_debug_simulation_input_bytes_v18(&bytes, &request, &mut budget).unwrap_err();
        assert_eq!(error.code, code);
        assert_eq!(budget.storage(), FLOOR);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert!(budget.failed_work().is_some() || budget.failed_storage().is_some());
    }
}

#[test]
fn exact_observed_resource_limits_pass_and_one_short_refuses_without_leak() {
    let bytes = fixture::bytes(&fixture::module());
    let request = fixture::request(37);
    let observe = |work_limit, storage_limit| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(FLOOR).unwrap();
        let result = load_debug_simulation_input_bytes_v18(&bytes, &request, &mut budget);
        let accepted = result.is_ok();
        if let Ok((input, receipt)) = result {
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            drop(input);
            budget.release_storage(receipt.retained_storage()).unwrap();
        }
        assert_eq!(budget.storage(), FLOOR);
        (accepted, budget.work(), budget.peak_storage())
    };
    let (_, work, peak) = observe(fixture::BOUND, fixture::BOUND);
    assert!(observe(work, peak).0);
    assert!(!observe(work - 1, peak).0);
    assert!(!observe(work, peak - 1).0);
}

#[test]
fn accounting_refusal_restores_original_floor() {
    let bytes = fixture::bytes(&fixture::module());
    let request = fixture::request(37);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(fixture::BOUND);
    let mut budget = Budget::new(&mut work, fixture::BOUND);
    budget.reserve_storage(FLOOR).unwrap();
    let error = load_bytes(&bytes, &request, bytes.len(), &mut budget).unwrap_err();
    assert_eq!(error.0.kind, ErrorKind::KirV18ResourceAccounting);
    assert_eq!(budget.storage(), FLOOR);
    assert!(budget.work() > 0);
}

#[test]
fn serialized_resource_tags_remain_precise_without_changing_report_shape() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
    let mut budget = Budget::new(&mut work, 0);
    let work_error = budget.charge_work(1).unwrap_err();
    let storage_error = budget.reserve_storage(1).unwrap_err();
    for (error, code) in [
        (work_error, "kir_v18_work_limit"),
        (storage_error, "kir_v18_storage_limit"),
        (Resource::Allocation, "kir_v18_allocation_failed"),
        (Resource::Accounting, "kir_v18_resource_accounting"),
        (Resource::Arithmetic, "kir_v18_resource_arithmetic"),
    ] {
        let failure = resource_failure(error);
        let value = serde_json::to_value(&failure.0).unwrap();
        assert_eq!(value["schema"], "fe2o3-simulation-error-v1");
        assert_eq!(value["stage"], "kir_admission");
        assert_eq!(value["kind"], code);
        assert_eq!(value["input"], "kir_v18");
        assert!(value["message"].as_str().unwrap().contains("V18"));
        assert_eq!(value.as_object().unwrap().len(), 6);
    }
}

#[test]
fn union_storage_objects_and_operations_remain_explicit_preflight_refusals() {
    for operations in [false, true] {
        let bytes = fixture::bytes(&fixture::union_storage_module(operations));
        with_input(&bytes, &fixture::storage_request(), |input| {
            let error = input
                .module
                .preflight(
                    &input.request,
                    input.simulation_target(),
                    input.simulation_limits,
                )
                .unwrap_err();
            let SimulationPreflightErrorV1::Unsupported(report) = error else {
                panic!("{error:?}")
            };
            assert!(
                report
                    .findings()
                    .iter()
                    .any(|finding| finding.feature == UnsupportedFeatureV1::InertStorage)
            );
        });
    }
}

#[test]
fn parser_rejects_schedules_and_competing_inputs_before_io() {
    let baseline = [
        "--diagnostic-kir-v18",
        "/missing/program",
        "--request",
        "/missing/request",
    ];
    for options in [
        vec!["--record-canonical-schedule", "/missing/output"],
        vec![
            "--record-seeded-schedule",
            "/missing/output",
            "--schedule-seed",
            "1",
        ],
        vec!["--replay-schedule", "/missing/schedule"],
        vec!["--explore-seeded-schedules", "1", "--schedule-seed", "1"],
        vec!["--reduce-failure"],
        vec!["--replay-failure-reduction", "/missing/reduction"],
        vec!["--schedule-max-decisions", "1"],
    ] {
        let error =
            parse_options(baseline.into_iter().chain(options).map(OsString::from)).unwrap_err();
        assert_eq!(error.0.stage, Stage::Arguments);
        assert_eq!(error.0.kind, ErrorKind::ScheduleInputUnsupported);
    }
    for flag in [
        "--diagnostic-kir-v18",
        "--diagnostic-kir-v17",
        "--diagnostic-kir-v19",
        "--kir-v7",
        "--bundle",
    ] {
        assert!(
            parse_options(
                baseline
                    .into_iter()
                    .chain([flag, "/missing/second"])
                    .map(OsString::from)
            )
            .is_err()
        );
    }
}

#[test]
fn real_driver_success_and_output_failure_restore_original_budget() {
    let files = fixture::Files::new(&fixture::bytes(&fixture::module()), &fixture::request(37));
    let output = files.path("result.json");
    let mut work = CanonicalKernelIrWorkBudgetV1::new(fixture::BOUND);
    let mut budget = Budget::new(&mut work, fixture::BOUND);
    budget.reserve_storage(FLOOR).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let policy = || RunPolicy {
        output: Some(output.as_os_str().to_owned()),
        schedule: ScheduleOption::None,
        race_evidence: false,
    };
    run_with_budget(&files.kir, &files.request, policy(), &mut budget).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    let published = std::fs::read(&output).unwrap();
    assert!(run_with_budget(&files.kir, &files.request, policy(), &mut budget).is_err());
    assert_eq!(std::fs::read(&output).unwrap(), published);
    assert_eq!(budget.storage(), FLOOR);
    assert!(budget.work_ledger_identity_v1() == ledger);
}
