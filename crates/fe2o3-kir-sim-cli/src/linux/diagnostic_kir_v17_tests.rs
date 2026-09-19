//! Synthetic canonical-owner ingress tests, not source or hardware qualification.
use super::*;
use fe2o3_kernel_ir::{Module, OperationKind, ScalarType, Type, encode_module_v17};
use std::fs;
use std::os::unix::fs::symlink;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

#[test]
fn one_three_sixteen_and_all_six_forms_use_independent_logical_oracles() {
    for case in fixture::cases() {
        for used in [true, false] {
            let owner = fixture::owner(&fixture::module_with_program(
                used,
                fixture::program(&case.descriptors),
            ));
            for inputs in [
                [0, 0, 0],
                [u32::MAX, 0, 1],
                [u32::MAX, 1, 2],
                [0x8000_0000, 0, 0x8000_0000],
                [0xaaaa_5555, 0x5555_aaaa, 19],
                [19, 23, 42],
            ] {
                let input = load_debug_simulation_input_bytes_v17(
                    owner.canonical_bytes(),
                    &fixture::request(inputs),
                )
                .unwrap();
                let execution = input
                    .module
                    .simulate(
                        &input.request,
                        input.simulation_target(),
                        input.simulation_limits,
                    )
                    .unwrap();
                let expected = if used {
                    (case.oracle)(inputs)
                } else {
                    inputs[0]
                };
                assert_eq!(
                    execution.buffer(3).unwrap().bytes(),
                    fixture::output(expected),
                    "case={} used={used}",
                    case.name
                );
                assert_eq!(
                    execution.steps_executed(),
                    64 * 5,
                    "internal instructions stay one logical operation"
                );
                assert!(!input.module.grants_execution_authority());
            }
        }
    }
}

#[test]
fn program_count_padding_source_and_binding_bytes_are_checked_at_exact_offsets() {
    let owner = fixture::owner(&fixture::module(true));
    let request = fixture::request([19, 23, 42]);
    let bytes = owner.canonical_bytes();
    let source = bytes
        .windows(32)
        .position(|window| window == [1; 32])
        .unwrap();
    assert_eq!(&bytes[source - 2..source], &[39, 0]);
    let count = source + 128 + 5 + 12;
    assert_eq!(bytes[count], 2);
    assert_eq!(&bytes[count + 1..count + 5], &[0x85, 0, 0x39, 1]);
    for value in [0, 17, 255] {
        let mut changed = bytes.to_vec();
        changed[count] = value;
        assert!(load_debug_simulation_input_bytes_v17(&changed, &request).is_err());
    }
    for slot in 2..16 {
        for bit in 0..16 {
            let mut changed = bytes.to_vec();
            let offset = count + 1 + 2 * slot;
            changed[offset..offset + 2].copy_from_slice(&(1_u16 << bit).to_le_bytes());
            assert!(
                load_debug_simulation_input_bytes_v17(&changed, &request).is_err(),
                "padding slot={slot} bit={bit}"
            );
        }
    }
    for field in 0..4 {
        let mut changed = bytes.to_vec();
        changed[source + 32 * field..source + 32 * (field + 1)].fill(0);
        assert!(load_debug_simulation_input_bytes_v17(&changed, &request).is_err());
    }
    for offset in [
        source + 128,
        source + 129,
        source + 130,
        source + 131,
        source + 132,
    ] {
        let mut changed = bytes.to_vec();
        changed[offset] = 64;
        assert!(load_debug_simulation_input_bytes_v17(&changed, &request).is_err());
    }
    // Nonzero caller-declared source and valid binding edits change identity.
    // They do not authenticate the source or describe final physical state.
    for offset in [source, source + 128] {
        let mut changed = bytes.to_vec();
        changed[offset] = 40;
        let admitted = load_debug_simulation_input_bytes_v17(&changed, &request).unwrap();
        assert_ne!(
            admitted.module.identity().digest(),
            owner.identity().digest()
        );
        assert!(!admitted.module.grants_execution_authority());
    }
}

#[path = "../../tests/fixtures/diagnostic_kir_v16.rs"]
mod legacy_fixture;

#[test]
fn old_owner_inputs_and_mixed_operations_never_silently_promote() {
    let old = legacy_fixture::owner(&legacy_fixture::module(true));
    assert_eq!(
        load_debug_simulation_input_bytes_v17(
            old.canonical_bytes(),
            &legacy_fixture::request([19, 23, 42])
        )
        .unwrap_err()
        .code,
        "kir_v17_wrong_version"
    );
    let new = fixture::owner(&fixture::module(true));
    assert_eq!(
        crate::load_debug_simulation_input_bytes_v16(
            new.canonical_bytes(),
            &fixture::request([19, 23, 42])
        )
        .unwrap_err()
        .code,
        "kir_v16_wrong_version"
    );
    let mut changed = old.canonical_bytes().to_vec();
    changed[8..10].copy_from_slice(&17_u16.to_le_bytes());
    assert!(
        load_debug_simulation_input_bytes_v17(&changed, &legacy_fixture::request([19, 23, 42]))
            .is_err()
    );
    let mut mixed = fixture::module(true);
    let mut old_operation = legacy_fixture::module(true).functions[0]
        .body
        .as_ref()
        .unwrap()
        .blocks[0]
        .operations[0]
        .clone();
    old_operation.results[0].id = fe2o3_kernel_ir::ValueId(9);
    mixed.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .insert(1, old_operation);
    assert!(encode_module_v17(&mixed).is_err());
}

#[test]
fn retained_dead_steps_bind_identity_even_when_output_and_operation_count_match() {
    let mut identities = std::collections::BTreeSet::new();
    for descriptors in [vec![8], vec![0, 8], vec![8, 8], vec![8, 72], vec![8; 16]] {
        let owner = fixture::owner(&fixture::module_with_program(
            true,
            fixture::program(&descriptors),
        ));
        assert!(identities.insert(*owner.identity().digest()));
        let input = load_debug_simulation_input_bytes_v17(
            owner.canonical_bytes(),
            &fixture::request([19, 23, 42]),
        )
        .unwrap();
        let execution = input
            .module
            .simulate(
                &input.request,
                input.simulation_target(),
                input.simulation_limits,
            )
            .unwrap();
        assert_eq!(execution.buffer(3).unwrap().bytes(), fixture::output(19));
        assert_eq!(execution.steps_executed(), 64 * 5);
    }
    assert_eq!(identities.len(), 5);
}
#[path = "../../tests/fixtures/diagnostic_kir_v17.rs"]
mod fixture;

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "fe2o3-diagnostic-v17-unit-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn exact_owner_identity_request_identity_and_oracle_survive_shared_ingress() {
    for used in [true, false] {
        let owner = fixture::owner(&fixture::module(used));
        for inputs in [
            [7, 11, 13],
            [u32::MAX, 0, 1],
            [0x7fff_ffff, 0, 1],
            [0xa5a5_a5a5, 0x5a5a_5a5a, 2],
            [u32::MAX; 3],
            [0; 3],
        ] {
            let request = fixture::request(inputs);
            let input =
                load_debug_simulation_input_bytes_v17(owner.canonical_bytes(), &request).unwrap();
            assert_eq!(input.module.identity().wire_version(), 17);
            assert_eq!(input.module.identity().digest(), owner.identity().digest());
            assert_eq!(
                input.module.identity().canonical_length(),
                owner.identity().canonical_length()
            );
            assert_eq!(input.kir_sha256, *owner.identity().digest());
            assert_eq!(
                input.request_sha256,
                <[u8; 32]>::from(Sha256::digest(&request))
            );
            assert_eq!(input.request_bytes(), request.len() as u64);
            assert_eq!(input.simulation_bundle_identity(), None);
            assert_eq!(input.simulation_bundle_subject(), None);
            assert_eq!(input.simulation_bundle_evidence(), None);
            assert!(!input.module.grants_execution_authority());
            assert!(matches!(
                input.module.module().functions[0]
                    .body
                    .as_ref()
                    .unwrap()
                    .blocks[0]
                    .operations[0]
                    .kind,
                OperationKind::Gfx942OrderedProgram(_)
            ));
            let execution = input
                .module
                .simulate(
                    &input.request,
                    input.simulation_target(),
                    input.simulation_limits,
                )
                .unwrap();
            assert_eq!(
                execution.buffer(3).unwrap().bytes(),
                fixture::expected_output(inputs, used)
            );
            assert_eq!(execution.steps_executed(), 64 * 5);
        }
    }
}

#[test]
fn independently_counted_canonical_budget_restores_nonzero_floor() {
    let bytes = encode_module_v17(&Module::new("m")).unwrap();
    assert_eq!(bytes.len(), 37);
    // Existing exact V17 byte-owner formula: work233, owner inline + bytes37
    // + one module-name byte. This helper adds input extent37 and caller floor7.
    let peak = 7 + 37 + size_of::<VerifiedCanonicalKernelIrModuleV17>() + 37 + 1;
    for (work_limit, storage_limit, expected) in [
        (233, peak, None),
        (232, peak, Some(ErrorKind::KirV17WorkLimit)),
        (0, peak, Some(ErrorKind::KirV17WorkLimit)),
        (233, peak - 1, Some(ErrorKind::KirV17StorageLimit)),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(7).unwrap();
        let result = admit_with_budget(&bytes, 37, cli_simulation_limits(), &mut budget);
        assert_eq!(budget.storage(), 7);
        match expected {
            None => {
                assert_eq!(result.unwrap().identity().wire_version(), 17);
                assert_eq!(budget.work(), 233);
                assert_eq!(budget.peak_storage(), peak);
            }
            Some(kind) => assert_eq!(result.unwrap_err().0.kind, kind),
        }
    }
}

#[test]
fn every_empty_owner_work_prefix_preserves_nested_encode_work_classification() {
    let bytes = encode_module_v17(&Module::new("m")).unwrap();
    for work_limit in 0..233 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1_000_000);
        budget.reserve_storage(7).unwrap();
        let error = admit_with_budget(&bytes, bytes.len(), cli_simulation_limits(), &mut budget)
            .unwrap_err();
        assert_eq!(
            error.0.kind,
            ErrorKind::KirV17WorkLimit,
            "work={work_limit}: {error:?}"
        );
        assert_eq!(budget.storage(), 7);
        assert!(budget.work() <= work_limit);
    }
}

#[test]
fn input_capacity_and_prior_failure_are_not_lost_when_restoring_floor() {
    let bytes = encode_module_v17(&Module::new("m")).unwrap();
    let payload = bytes.len() + 511; // caller-owned capacity excess, not length
    let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 4096);
    budget.reserve_storage(9).unwrap();
    assert!(budget.reserve_storage(4096).is_err());
    let failed = budget.failed_storage();
    admit_with_budget(&bytes, payload, cli_simulation_limits(), &mut budget).unwrap();
    assert_eq!(budget.storage(), 9);
    assert_eq!(budget.failed_storage(), failed);
    assert!(budget.peak_storage() >= 9 + payload + bytes.len());
    let accepted_work = budget.work();
    let error =
        admit_with_budget(&bytes, usize::MAX, cli_simulation_limits(), &mut budget).unwrap_err();
    assert_eq!(error.0.kind, ErrorKind::KirV17StorageLimit);
    assert_eq!(budget.storage(), 9);
    assert_eq!(budget.work(), accepted_work);
    let error = admit_with_budget(
        &bytes,
        bytes.len() - 1,
        cli_simulation_limits(),
        &mut budget,
    )
    .unwrap_err();
    assert_eq!(error.0.kind, ErrorKind::KirV17ResourceAccounting);
    assert_eq!(budget.storage(), 9);
    assert_eq!(arithmetic().0.kind, ErrorKind::KirV17ResourceArithmetic);
}

#[test]
fn cpu_resident_rejection_is_separate_and_drops_charged_canonical_owner() {
    let owner = fixture::owner(&fixture::module(true));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1_000_000);
    budget.reserve_storage(11).unwrap();
    let mut limits = cli_simulation_limits();
    limits.max_resident_bytes = 1;
    let error = admit_with_budget(
        owner.canonical_bytes(),
        owner.canonical_bytes().len(),
        limits,
        &mut budget,
    )
    .unwrap_err();
    assert_eq!(error.0.stage, Stage::SimulatorAdmission);
    assert_eq!(budget.storage(), 11);
    assert!(budget.work() > 0);
    assert!(budget.peak_storage() > owner.canonical_bytes().len());
}

#[test]
fn exact_version_payload_and_semantic_rejections_never_fall_back() {
    let owner = fixture::owner(&fixture::module(true));
    let request = fixture::request([7, 11, 13]);
    for version in [1_u16, 7, 9, 10, 11, 12, 13, 14, 15, 16] {
        let mut bytes = owner.canonical_bytes().to_vec();
        bytes[8..10].copy_from_slice(&version.to_le_bytes());
        assert_eq!(
            load_debug_simulation_input_bytes_v17(&bytes, &request)
                .unwrap_err()
                .code,
            "kir_v17_wrong_version"
        );
    }
    let mut bytes = owner.canonical_bytes().to_vec();
    bytes.push(0);
    assert!(load_debug_simulation_input_bytes_v17(&bytes, &request).is_err());
    assert!(load_debug_simulation_input_bytes_v17(&bytes[..10], &request).is_err());
    let source_start = owner
        .canonical_bytes()
        .windows(32)
        .position(|window| window == [1; 32])
        .unwrap();
    for (offset, value) in [
        (source_start - 2, 255),
        (source_start - 1, 1),
        (source_start + 128, 64),
        (source_start + 129, 32),
    ] {
        let mut bytes = owner.canonical_bytes().to_vec();
        bytes[offset] = value;
        assert!(load_debug_simulation_input_bytes_v17(&bytes, &request).is_err());
    }
    let mut module = fixture::module(true);
    module.functions[0].signature.parameters[0] = Type::Scalar(ScalarType::I32);
    let malformed = encode_module_v17(&module).unwrap();
    assert_eq!(
        load_debug_simulation_input_bytes_v17(&malformed, &request)
            .unwrap_err()
            .code,
        "kir_v17_verification_failed"
    );
}

#[test]
fn both_byte_caps_apply_before_decode_and_request_json_remains_strict() {
    let oversized = vec![0; MAX_REQUEST_BYTES + 1];
    assert_eq!(
        load_debug_simulation_input_bytes_v17(&[], &oversized)
            .unwrap_err()
            .code,
        "input_too_large"
    );
    assert_eq!(
        load_debug_simulation_input_bytes_v17(&oversized, &[])
            .unwrap_err()
            .code,
        "input_too_large"
    );
    let owner = fixture::owner(&fixture::module(true));
    for request in [
        br#"{"schema":null}"#.as_slice(),
        br#"{"schema":"x","schema":"x"}"#,
        br#"{"unknown":1}"#,
    ] {
        let error =
            load_debug_simulation_input_bytes_v17(owner.canonical_bytes(), request).unwrap_err();
        assert_eq!(error.stage, "request");
        assert!(error.code.starts_with("request_json_"));
    }
}

#[test]
fn target_launch_and_argument_refusals_still_come_from_cpu_preflight() {
    let owner = fixture::owner(&fixture::module(true));
    let request = fixture::request([7, 11, 13]);
    for case in 0..4 {
        let mut input =
            load_debug_simulation_input_bytes_v17(owner.canonical_bytes(), &request).unwrap();
        match case {
            0 => input.request.workgroup.0 = [32, 1, 1],
            1 => input.request.grid.0 = [65, 1, 1],
            2 => input.request.kernel = fe2o3_kernel_ir::KernelId::new("absent"),
            3 => input.request.arguments.pop().map(drop).unwrap(),
            _ => unreachable!(),
        }
        assert!(
            input
                .module
                .preflight(
                    &input.request,
                    input.simulation_target(),
                    input.simulation_limits
                )
                .is_err()
        );
    }
    let mut module = fixture::module(true);
    module
        .required_capabilities
        .insert(fe2o3_kernel_ir::gfx950_xnack_minus_target_capability());
    let wrong = fixture::owner(&module);
    let input = load_debug_simulation_input_bytes_v17(wrong.canonical_bytes(), &request).unwrap();
    assert!(
        input
            .module
            .preflight(
                &input.request,
                input.simulation_target(),
                input.simulation_limits
            )
            .is_err()
    );
}

#[test]
fn parser_rejects_every_schedule_control_before_any_input_read() {
    let baseline = [
        "--diagnostic-kir-v17",
        "/missing-program",
        "--request",
        "/missing-request",
    ];
    assert!(matches!(
        parse_options(baseline.map(OsString::from).into_iter())
            .unwrap()
            .program,
        ProgramInput::DiagnosticKirV17(_)
    ));
    for extra in [
        vec!["--record-canonical-schedule", "/missing-output"],
        vec!["--record-seeded-schedule", "/missing-output"],
        vec!["--replay-schedule", "/missing"],
        vec!["--explore-seeded-schedules", "1"],
        vec!["--reduce-failure"],
        vec!["--replay-failure-reduction", "/missing"],
        vec!["--schedule-seed", "0"],
        vec!["--schedule-max-decisions", "1"],
        vec!["--exploration-max-retained-decisions", "1"],
    ] {
        let failure = run(baseline.into_iter().chain(extra).map(OsString::from)).unwrap_err();
        assert_eq!(failure.0.stage, Stage::Arguments);
        assert_eq!(failure.0.kind, ErrorKind::ScheduleInputUnsupported);
    }
    for flag in [
        "--diagnostic-kir-v17",
        "--diagnostic-kir-v16",
        "--kir-v7",
        "--kir-v12",
        "--bundle",
        "--bundle-v5",
        "--bundle-v6",
    ] {
        assert_eq!(
            parse_options(
                baseline
                    .into_iter()
                    .chain([flag, "other"])
                    .map(OsString::from)
            )
            .unwrap_err()
            .0
            .kind,
            ErrorKind::InvalidCommandLine
        );
    }
}

#[test]
fn programmatic_schedule_paths_refuse_before_open_or_publication_without_panicking() {
    let directory = Directory::new();
    let output = directory.path("must-not-exist");
    let owner = fixture::owner(&fixture::module(true));
    let request = fixture::request([7, 11, 13]);
    let input = load_debug_simulation_input_bytes_v17(owner.canonical_bytes(), &request).unwrap();
    assert_eq!(
        input.persisted_schedule_binding().unwrap_err().code,
        "schedule_input_unsupported"
    );
    assert_eq!(
        crate::load_debug_simulation_schedule_v1(Path::new("/missing-schedule"), &input)
            .unwrap_err()
            .code,
        "schedule_input_unsupported"
    );
    for schedule in [
        ScheduleOption::RecordCanonical {
            output: output.clone().into_os_string(),
            max_decisions: 1,
        },
        ScheduleOption::RecordSeeded {
            output: output.clone().into_os_string(),
            seed: 0,
            max_decisions: 1,
        },
        ScheduleOption::Replay {
            input: OsString::from("/missing-schedule"),
        },
        ScheduleOption::ExploreSeeded {
            first_seed: 0,
            max_schedules: 1,
            max_decisions: 1,
            max_retained_decisions: 1,
        },
        ScheduleOption::ReduceFailure {
            schedule: SimulationFailureScheduleV1::Canonical,
            max_decisions: 1,
        },
        ScheduleOption::ReplayFailureReduction {
            input: OsString::from("/missing-report"),
        },
    ] {
        let input =
            load_debug_simulation_input_bytes_v17(owner.canonical_bytes(), &request).unwrap();
        let error = run_with_admitted_input(
            input,
            RunPolicy {
                output: Some(output.clone().into_os_string()),
                schedule,
                race_evidence: false,
            },
        )
        .unwrap_err();
        assert_eq!(error.0.kind, ErrorKind::ScheduleInputUnsupported);
        assert!(!output.exists());
    }
}

#[test]
fn filesystem_ingress_reuses_regular_no_symlink_and_stable_capture_checks() {
    let directory = Directory::new();
    let owner = fixture::owner(&fixture::module(true));
    let kir = directory.path("program.kir");
    let request = directory.path("request.json");
    fs::write(&kir, owner.canonical_bytes()).unwrap();
    fs::write(&request, fixture::request([7, 11, 13])).unwrap();
    let input = crate::load_debug_simulation_input_v17(&kir, &request).unwrap();
    assert_eq!(input.module.identity().digest(), owner.identity().digest());
    let link = directory.path("link");
    symlink(&kir, &link).unwrap();
    assert_eq!(
        crate::load_debug_simulation_input_v17(&link, &request)
            .unwrap_err()
            .code,
        "input_open_failed"
    );
    let ancestor = directory.path("ancestor");
    symlink(&directory.0, &ancestor).unwrap();
    assert_eq!(
        crate::load_debug_simulation_input_v17(&ancestor.join("program.kir"), &request)
            .unwrap_err()
            .code,
        "input_open_failed"
    );
    assert_eq!(
        crate::load_debug_simulation_input_v17(&directory.0, &request)
            .unwrap_err()
            .code,
        "input_not_regular"
    );
    let fifo = directory.path("fifo");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    assert_eq!(
        crate::load_debug_simulation_input_v17(&fifo, &request)
            .unwrap_err()
            .code,
        "input_not_regular"
    );
    let oversized = directory.path("oversized");
    File::create(&oversized)
        .unwrap()
        .set_len((MAX_KIR_BYTES + 1) as u64)
        .unwrap();
    assert_eq!(
        crate::load_debug_simulation_input_v17(&oversized, &request)
            .unwrap_err()
            .code,
        "input_too_large"
    );
    let failure = secure_read_with_hook(
        &kir,
        MAX_KIR_BYTES,
        InputCode::KirV17,
        "diagnostic V17",
        || {
            fs::write(&kir, b"changed after read").unwrap();
        },
    )
    .unwrap_err();
    assert_eq!(failure.0.kind, ErrorKind::InputChanged);
}
