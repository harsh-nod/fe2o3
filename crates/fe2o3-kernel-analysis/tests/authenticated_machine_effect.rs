#![cfg(target_os = "linux")]

use fe2o3_kernel_analysis::{
    AuthenticatedPhysicalMachineEffectErrorKindV1, AuthenticatedPhysicalMachineEffectLimitsV1,
    AuthenticatedPhysicalMachineEffectWorkerV1, PhysicalMachineEffectBudgetV1,
    PhysicalMachineEffectEntryRequestV1, PhysicalMachineEffectEvidenceErrorV1,
    inspect_physical_machine_effect_worker_candidate_v1,
};
use rustix::fs::{OFlags, SealFlags};
use std::{
    fs::{self, File, OpenOptions},
    io::{Seek, SeekFrom, Write},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
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
    AuthenticatedPhysicalMachineEffectLimitsV1::new(Duration::from_secs(2), 1024 * 1024, 16 * 1024)
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
    let candidate = inspect_physical_machine_effect_worker_candidate_v1(&path, limits()).unwrap();
    let worker =
        AuthenticatedPhysicalMachineEffectWorkerV1::open(&path, candidate.policy(), limits())
            .unwrap();
    assert_eq!(worker.policy(), candidate.policy());
    assert_eq!(worker.analyzer_identity(), candidate.analyzer_identity());
    assert_eq!(worker.toolchain_identity(), candidate.toolchain_identity());
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
        first.evidence().request_identity(),
        second.evidence().request_identity()
    );
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
            .evidence()
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
    replay.extend_from_slice(first.evidence().canonical_bytes());
    let stale = worker.analyze(replay, vec![entry()], limits()).unwrap_err();
    assert!(matches!(
        stale.kind(),
        AuthenticatedPhysicalMachineEffectErrorKindV1::Evidence(
            PhysicalMachineEffectEvidenceErrorV1::ExecutionChallengeMismatch
                | PhysicalMachineEffectEvidenceErrorV1::RequestIdentityMismatch
        )
    ));

    for (mode, expected) in [
        (
            5,
            PhysicalMachineEffectEvidenceErrorV1::AnalyzerIdentityMismatch,
        ),
        (6, PhysicalMachineEffectEvidenceErrorV1::LengthMismatch),
        (
            7,
            PhysicalMachineEffectEvidenceErrorV1::ExecutionChallengeMismatch,
        ),
    ] {
        let error = worker
            .analyze(vec![mode], vec![entry()], limits())
            .unwrap_err();
        assert!(matches!(
            error.kind(),
            AuthenticatedPhysicalMachineEffectErrorKindV1::Evidence(actual)
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
