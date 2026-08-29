#![cfg(target_os = "linux")]

use fe2o3_kernel_analysis::{
    AuthenticatedPhysicalMachineEffectErrorKindV1, AuthenticatedPhysicalMachineEffectLimitsV1,
    AuthenticatedPhysicalMachineEffectWorkerV1, DEFAULT_PHYSICAL_MACHINE_EFFECT_TIMEOUT_V1,
    PhysicalMachineAnalysisEvidenceErrorV1, PhysicalMachineAnalyzerIdentityV1,
    PhysicalMachineEffectBudgetV1, PhysicalMachineEffectEntryRequestV1,
    PhysicalMachineEffectEvidenceErrorV1, PhysicalMachineEffectWorkerPolicyV1,
    PhysicalMachineToolchainIdentityV1, inspect_physical_machine_effect_worker_candidate_v1,
};
use rustix::fs::{OFlags, SealFlags};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Seek, SeekFrom, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

static TEMP_ID: AtomicU64 = AtomicU64::new(1);

fn fixture() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_fe2o3-machine-effect-worker-fixture"))
}

fn substitute_fixture() -> &'static Path {
    Path::new(env!(
        "CARGO_BIN_EXE_fe2o3-machine-effect-worker-substitute-fixture"
    ))
}

fn limits() -> AuthenticatedPhysicalMachineEffectLimitsV1 {
    AuthenticatedPhysicalMachineEffectLimitsV1::new(Duration::from_secs(30), 1024 * 1024, 16 * 1024)
        .unwrap()
}

fn short_limits() -> AuthenticatedPhysicalMachineEffectLimitsV1 {
    AuthenticatedPhysicalMachineEffectLimitsV1::new(
        Duration::from_millis(100),
        1024 * 1024,
        16 * 1024,
    )
    .unwrap()
}

fn hostile_timeout_limits() -> AuthenticatedPhysicalMachineEffectLimitsV1 {
    AuthenticatedPhysicalMachineEffectLimitsV1::new(Duration::from_secs(10), 1024 * 1024, 16 * 1024)
        .unwrap()
}

fn entry() -> PhysicalMachineEffectEntryRequestV1 {
    PhysicalMachineEffectEntryRequestV1::new(
        "alpha",
        PhysicalMachineEffectBudgetV1::new(0, 0, 0, 1, 0),
    )
    .unwrap()
}

fn worker() -> AuthenticatedPhysicalMachineEffectWorkerV1 {
    let candidate =
        inspect_physical_machine_effect_worker_candidate_v1(fixture(), limits()).unwrap();
    AuthenticatedPhysicalMachineEffectWorkerV1::open(fixture(), candidate.policy(), limits())
        .unwrap()
}

#[test]
fn configured_native_worker_uses_authenticated_identity_probe() {
    let Some(path) = std::env::var_os("FE2O3_MACHINE_EFFECT_NATIVE_WORKER") else {
        return;
    };
    let native_limits = AuthenticatedPhysicalMachineEffectLimitsV1::new(
        DEFAULT_PHYSICAL_MACHINE_EFFECT_TIMEOUT_V1 * 2,
        1024 * 1024,
        16 * 1024,
    )
    .unwrap();
    let candidate =
        inspect_physical_machine_effect_worker_candidate_v1(&path, native_limits).unwrap();
    let worker =
        AuthenticatedPhysicalMachineEffectWorkerV1::open(&path, candidate.policy(), native_limits)
            .unwrap();
    worker
        .verify_deployed_no_fork_profile_for_test(native_limits)
        .unwrap();
    assert_eq!(worker.policy(), candidate.policy());
    assert_eq!(worker.analyzer_identity(), candidate.analyzer_identity());
    assert_eq!(worker.toolchain_identity(), candidate.toolchain_identity());

    let Some(payload_path) = std::env::var_os("FE2O3_MACHINE_ANALYSIS_NATIVE_HSACO") else {
        return;
    };
    let generous = PhysicalMachineEffectBudgetV1::new(64, 32, 16, 16, 8);
    let entries = match std::env::var("FE2O3_MACHINE_ANALYSIS_NATIVE_ENTRY") {
        Ok(symbol) => vec![PhysicalMachineEffectEntryRequestV1::new(symbol, generous).unwrap()],
        Err(std::env::VarError::NotPresent) => vec![
            PhysicalMachineEffectEntryRequestV1::new("alpha", generous).unwrap(),
            PhysicalMachineEffectEntryRequestV1::new("zeta", generous).unwrap(),
        ],
        Err(std::env::VarError::NotUnicode(_)) => {
            panic!("FE2O3_MACHINE_ANALYSIS_NATIVE_ENTRY is not UTF-8")
        }
    };
    let expected_entry_count = entries.len();
    let execution = worker
        .analyze(fs::read(payload_path).unwrap(), entries, native_limits)
        .unwrap();
    assert_eq!(
        execution.analysis().effects().entry_points().len(),
        expected_entry_count
    );
    assert!(execution.analysis().trace().instructions().len() > 2);
    assert!(execution.analysis().trace().blocks().iter().any(|block| {
        block
            .successors()
            .iter()
            .any(|successor| *successor <= block.ordinal())
    }));
    assert!(execution.authenticates_analyzer_execution());
    assert!(!execution.grants_publication_authority());
    assert!(!execution.grants_load_authority());
    assert!(!execution.grants_launch_authority());

    persist_configured_native_record(
        "FE2O3_MACHINE_ANALYSIS_NATIVE_REQUEST_PATH",
        execution.request().canonical_bytes(),
    );
    persist_configured_native_record(
        "FE2O3_MACHINE_ANALYSIS_NATIVE_BUNDLE_PATH",
        execution.analysis().canonical_bytes(),
    );
    if let Some(path) = std::env::var_os("FE2O3_MACHINE_ANALYSIS_NATIVE_RECEIPT_PATH") {
        execution.persist_create_new(&path).unwrap();
        assert_eq!(fs::read(path).unwrap(), execution.canonical_receipt_bytes());
    }

    let mut opcodes = BTreeMap::<&str, usize>::new();
    for instruction in execution.analysis().trace().instructions() {
        *opcodes.entry(instruction.opcode()).or_default() += 1;
    }
    eprintln!(
        "native gfx942 analysis: request={:?} bundle={:?} receipt={:?} blocks={} instructions={} opcodes={opcodes:?}",
        execution.request().identity(),
        execution.analysis().identity(),
        execution.identity(),
        execution.analysis().trace().blocks().len(),
        execution.analysis().trace().instructions().len(),
    );
}

fn persist_configured_native_record(variable: &str, bytes: &[u8]) {
    let Some(path) = std::env::var_os(variable) else {
        return;
    };
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
    drop(file);
    assert_eq!(fs::read(path).unwrap(), bytes);
}

#[test]
fn caller_policy_rejects_analyzer_and_toolchain_substitution() {
    let candidate =
        inspect_physical_machine_effect_worker_candidate_v1(fixture(), limits()).unwrap();
    let exact = candidate.policy();
    for substituted in [
        PhysicalMachineEffectWorkerPolicyV1::new(
            exact.executable(),
            exact.runtime_closure(),
            PhysicalMachineAnalyzerIdentityV1::from_sha256_bytes([0xc3; 32]),
            exact.toolchain(),
        )
        .unwrap(),
        PhysicalMachineEffectWorkerPolicyV1::new(
            exact.executable(),
            exact.runtime_closure(),
            exact.analyzer(),
            PhysicalMachineToolchainIdentityV1::from_sha256_bytes([0xd4; 32]),
        )
        .unwrap(),
    ] {
        assert!(matches!(
            AuthenticatedPhysicalMachineEffectWorkerV1::open(fixture(), substituted, limits())
                .unwrap_err()
                .kind(),
            AuthenticatedPhysicalMachineEffectErrorKindV1::IdentityMismatch
        ));
    }
}

fn temp_dir(name: &str) -> PathBuf {
    let id = TEMP_ID.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "fe2o3-machine-effect-{name}-{}-{id}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir(&path).unwrap();
    path
}

#[test]
fn production_execution_returns_fresh_non_authoritative_durable_receipts() {
    let candidate =
        inspect_physical_machine_effect_worker_candidate_v1(fixture(), limits()).unwrap();
    assert_eq!(candidate.analyzer_identity().as_bytes(), [0xa1; 32]);
    assert_eq!(candidate.toolchain_identity().as_bytes(), [0xb2; 32]);
    let worker =
        AuthenticatedPhysicalMachineEffectWorkerV1::open(fixture(), candidate.policy(), limits())
            .unwrap();
    let first = worker.analyze(vec![1], vec![entry()], limits()).unwrap();
    let second = worker.analyze(vec![1], vec![entry()], limits()).unwrap();
    assert_ne!(first.execution_challenge(), second.execution_challenge());
    assert_ne!(
        first.analysis().effects().request_identity(),
        second.analysis().effects().request_identity()
    );
    assert!(first.analysis().binds_exact_payload_instruction_bytes());
    assert!(!first.analysis().establishes_machine_semantics());
    assert!(first.authenticates_analyzer_execution());
    assert!(!first.grants_publication_authority());
    assert!(!first.grants_load_authority());
    assert!(!first.grants_launch_authority());
    assert_eq!(
        first.identity().byte_len(),
        first.canonical_receipt_bytes().len() as u64
    );

    let directory = temp_dir("receipt");
    let receipt = directory.join("analysis.receipt");
    first.persist_create_new(&receipt).unwrap();
    assert_eq!(fs::read(&receipt).unwrap(), first.canonical_receipt_bytes());
    assert!(first.persist_create_new(&receipt).is_err());
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn sealed_image_rejects_same_uid_write_resize_and_new_seals() {
    let worker = worker();
    let path = worker.retained_executable_descriptor_path_for_test();
    let retained = File::open(path).unwrap();
    let flags = rustix::fs::fcntl_getfl(&retained).unwrap();
    assert_eq!(flags & OFlags::ACCMODE, OFlags::RDONLY);
    assert_eq!(
        rustix::fs::fcntl_get_seals(&retained).unwrap(),
        SealFlags::WRITE | SealFlags::GROW | SealFlags::SHRINK | SealFlags::SEAL
    );
    assert!(retained.set_len(0).is_err());
    assert!(rustix::fs::fcntl_add_seals(&retained, SealFlags::EXEC).is_err());

    if let Ok(mut writable_alias) = OpenOptions::new().read(true).write(true).open(path) {
        assert!(writable_alias.write_all(b"substitute").is_err());
        assert!(writable_alias.set_len(1).is_err());
    }
    worker.analyze(vec![1], vec![entry()], limits()).unwrap();
}

#[test]
fn pathname_replacement_keeps_pinned_image_and_worker_substitution_is_rejected() {
    let directory = temp_dir("replacement");
    let selected = directory.join("worker");
    fs::copy(fixture(), &selected).unwrap();
    fs::set_permissions(&selected, fs::Permissions::from_mode(0o700)).unwrap();
    let candidate =
        inspect_physical_machine_effect_worker_candidate_v1(&selected, limits()).unwrap();
    let worker =
        AuthenticatedPhysicalMachineEffectWorkerV1::open(&selected, candidate.policy(), limits())
            .unwrap();
    fs::copy(substitute_fixture(), &selected).unwrap();
    assert_eq!(
        worker
            .analyze(vec![1], vec![entry()], limits())
            .unwrap()
            .analysis()
            .effects()
            .analyzer_identity()
            .as_bytes(),
        [0xa1; 32]
    );
    assert!(matches!(
        AuthenticatedPhysicalMachineEffectWorkerV1::open(
            substitute_fixture(),
            candidate.policy(),
            limits()
        )
        .unwrap_err()
        .kind(),
        AuthenticatedPhysicalMachineEffectErrorKindV1::WorkerIdentityMismatch { .. }
    ));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn stale_replay_stdout_substitution_and_identity_mismatch_fail_closed() {
    let worker = worker();
    let first = worker.analyze(vec![1], vec![entry()], limits()).unwrap();
    let mut replay = vec![4];
    replay.extend_from_slice(first.analysis().canonical_bytes());
    let stale = worker.analyze(replay, vec![entry()], limits()).unwrap_err();
    assert!(matches!(
        stale.kind(),
        AuthenticatedPhysicalMachineEffectErrorKindV1::Analysis(
            PhysicalMachineAnalysisEvidenceErrorV1::Effects(
                PhysicalMachineEffectEvidenceErrorV1::ExecutionChallengeMismatch
                    | PhysicalMachineEffectEvidenceErrorV1::RequestIdentityMismatch
            )
        )
    ));

    for (mode, expected) in [
        (
            5,
            PhysicalMachineAnalysisEvidenceErrorV1::Effects(
                PhysicalMachineEffectEvidenceErrorV1::AnalyzerIdentityMismatch,
            ),
        ),
        (6, PhysicalMachineAnalysisEvidenceErrorV1::LengthMismatch),
        (
            7,
            PhysicalMachineAnalysisEvidenceErrorV1::Effects(
                PhysicalMachineEffectEvidenceErrorV1::ExecutionChallengeMismatch,
            ),
        ),
    ] {
        let error = worker
            .analyze(vec![mode], vec![entry()], limits())
            .unwrap_err();
        assert!(matches!(
            error.kind(),
            AuthenticatedPhysicalMachineEffectErrorKindV1::Analysis(actual)
                if actual == &expected
        ));
    }
}

#[test]
fn worker_receives_only_the_fixed_environment_allowlist() {
    worker().analyze(vec![9], vec![entry()], limits()).unwrap();
}

#[test]
fn worker_parses_requests_under_bounded_resource_limits() {
    worker().analyze(vec![10], vec![entry()], limits()).unwrap();
}

#[test]
fn timeout_reaps_worker() {
    let worker = worker();
    let started = Instant::now();
    let timeout = worker
        .analyze(vec![2], vec![entry()], short_limits())
        .unwrap_err();
    assert_eq!(
        timeout.kind(),
        &AuthenticatedPhysicalMachineEffectErrorKindV1::Timeout
    );
    assert!(started.elapsed() < Duration::from_secs(3));
}

#[test]
fn rapid_double_fork_and_setsid_are_denied_before_worker_input() {
    worker().analyze(vec![11], vec![entry()], limits()).unwrap();
}

#[test]
fn late_runtime_library_mapping_is_rejected_before_acknowledgement() {
    let error = worker()
        .analyze(vec![12], vec![entry()], limits())
        .unwrap_err();
    assert_eq!(
        error.kind(),
        &AuthenticatedPhysicalMachineEffectErrorKindV1::RuntimeClosureChanged
    );
}

#[test]
fn post_ready_fexecve_replacement_from_spoofed_memfd_is_rejected() {
    let error = worker()
        .analyze(vec![13], vec![entry()], limits())
        .unwrap_err();
    assert_eq!(
        error.kind(),
        &AuthenticatedPhysicalMachineEffectErrorKindV1::RuntimeClosureChanged
    );
}

#[test]
fn persistent_same_object_remap_is_rejected_before_acknowledgement() {
    let error = worker()
        .analyze(vec![14], vec![entry()], limits())
        .unwrap_err();
    assert_eq!(
        error.kind(),
        &AuthenticatedPhysicalMachineEffectErrorKindV1::RuntimeClosureChanged
    );
}

#[test]
fn persistent_anonymous_executable_mapping_is_rejected_before_acknowledgement() {
    let error = worker()
        .analyze(vec![18], vec![entry()], limits())
        .unwrap_err();
    assert_eq!(
        error.kind(),
        &AuthenticatedPhysicalMachineEffectErrorKindV1::RuntimeClosureChanged
    );
}

#[test]
fn transient_remap_between_ready_and_done_is_outside_two_snapshot_guarantee() {
    let execution = worker().analyze(vec![15], vec![entry()], limits()).unwrap();
    assert!(execution.authenticates_analyzer_execution());
}

#[test]
fn failed_ack_delivery_terminates_and_reaps_worker() {
    run_failed_ack_delivery_repeats(1);
}

#[test]
#[ignore = "slow 30-run authenticated teardown stress"]
fn failed_ack_delivery_terminates_and_reaps_worker_30_repeat_stress() {
    run_failed_ack_delivery_repeats(30);
}

fn run_failed_ack_delivery_repeats(repeats: usize) {
    let directory = temp_dir("closed-ack");
    let worker = worker();
    for iteration in 0..repeats {
        let pid_file = directory.join(format!("worker-{iteration}.pid"));
        let mut payload = vec![16];
        payload.extend_from_slice(pid_file.to_str().unwrap().as_bytes());
        let started = Instant::now();
        let error = worker
            .analyze(payload, vec![entry()], limits())
            .unwrap_err();
        assert_eq!(
            error.kind(),
            &AuthenticatedPhysicalMachineEffectErrorKindV1::ControlHandshake
        );
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "ACK failure cleanup {iteration} took {:?}",
            started.elapsed()
        );
        let pid = fs::read_to_string(&pid_file).unwrap();
        assert!(!Path::new(&format!("/proc/{pid}")).exists());
    }
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn parent_session_helper_cannot_join_worker_group_and_survives_cleanup() {
    for (name, mode, execution_limits, expected) in [
        (
            "timeout",
            17,
            hostile_timeout_limits(),
            AuthenticatedPhysicalMachineEffectErrorKindV1::Timeout,
        ),
        (
            "ack",
            16,
            limits(),
            AuthenticatedPhysicalMachineEffectErrorKindV1::ControlHandshake,
        ),
    ] {
        let directory = temp_dir(name);
        let pid_file = directory.join("worker.pid");
        let join_result = directory.join("join-result");
        let mut payload = vec![mode];
        payload.extend_from_slice(pid_file.to_str().unwrap().as_bytes());
        let worker = worker();
        let execution =
            thread::spawn(move || worker.analyze(payload, vec![entry()], execution_limits));
        wait_for_file(&pid_file);
        let pid = fs::read_to_string(&pid_file).unwrap();
        let mut helper = Command::new(fixture())
            .arg(format!("--fe2o3-test-join-process-group={pid}"))
            .arg(format!("--fe2o3-test-result={}", join_result.display()))
            .spawn()
            .unwrap();
        wait_for_file(&join_result);
        assert_eq!(fs::read_to_string(&join_result).unwrap(), "errno=1");

        let error = execution.join().unwrap().unwrap_err();
        assert_eq!(error.kind(), &expected);
        assert!(helper.try_wait().unwrap().is_none(), "helper was signaled");
        helper.kill().unwrap();
        helper.wait().unwrap();
        assert!(!Path::new(&format!("/proc/{pid}")).exists());
        fs::remove_dir_all(directory).unwrap();
    }
}

fn wait_for_file(path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !path.exists() {
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {}",
            path.display()
        );
        thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn in_place_source_mutation_during_capture_never_authenticates() {
    let directory = temp_dir("mutation-race");
    let selected = directory.join("worker");
    fs::copy(fixture(), &selected).unwrap();
    fs::set_permissions(&selected, fs::Permissions::from_mode(0o700)).unwrap();
    let mut extension = OpenOptions::new().append(true).open(&selected).unwrap();
    extension.write_all(&vec![0_u8; 32 * 1024 * 1024]).unwrap();
    extension.sync_all().unwrap();
    drop(extension);

    let running = Arc::new(AtomicBool::new(true));
    let writer_running = Arc::clone(&running);
    let writer_path = selected.clone();
    let writer = thread::spawn(move || {
        let mut file = OpenOptions::new().write(true).open(writer_path).unwrap();
        let end = file.metadata().unwrap().len() - 1;
        let mut byte = 0_u8;
        while writer_running.load(Ordering::Relaxed) {
            file.seek(SeekFrom::Start(end)).unwrap();
            file.write_all(&[byte]).unwrap();
            byte ^= 1;
        }
    });
    thread::sleep(Duration::from_millis(10));
    let result = inspect_physical_machine_effect_worker_candidate_v1(&selected, limits());
    running.store(false, Ordering::Relaxed);
    writer.join().unwrap();
    let error = result.expect_err("mutating source authenticated during capture");
    assert!(matches!(
        error.kind(),
        AuthenticatedPhysicalMachineEffectErrorKindV1::WorkerChangedDuringCapture
    ));
    fs::remove_dir_all(directory).unwrap();
}
