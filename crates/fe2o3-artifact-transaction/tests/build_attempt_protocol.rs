use fe2o3_artifact_transaction::{
    BuildAttempt, BuildInvocation, BuildSession, EmitError, ProducerIdentity,
    begin_build_attempt as begin_build_attempt_for_invocation, emit_artifact_transaction,
    emit_artifact_transaction_for_attempt, fail_build_attempt, finish_build_attempt,
};
use std::fs;
use std::io;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Barrier, mpsc};
use std::thread;

static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new() -> Self {
        loop {
            let id = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "fe2o3-build-attempt-integration-{}-{id}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self { path },
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("failed to create test directory: {error}"),
            }
        }
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[derive(Clone, Copy)]
struct TestKernel {
    name: &'static str,
    generation: &'static str,
}

fn make_producer(crate_name: &str, source: &str) -> ProducerIdentity {
    ProducerIdentity::from_codegen(crate_name, Some(Path::new(source))).unwrap()
}

fn session(discriminator: u8) -> BuildSession {
    assert_ne!(discriminator, 0);
    let mut bytes = [0; 16];
    bytes[15] = discriminator;
    BuildSession::from_bytes(bytes)
}

fn invocation(discriminator: u8) -> BuildInvocation {
    assert_ne!(discriminator, 0);
    BuildInvocation::from_bytes([discriminator; 32])
}

fn begin_build_attempt(
    output: &Path,
    producer: &ProducerIdentity,
    session: BuildSession,
) -> Result<BuildAttempt, EmitError> {
    let discriminator = session.as_bytes()[15];
    begin_build_attempt_for_invocation(output, producer, invocation(discriminator), session)
}

fn one(name: &'static str, generation: &'static str) -> [TestKernel; 1] {
    [TestKernel { name, generation }]
}

fn fake_compile(llvm_ir_path: &Path, hsaco_path: &Path) -> Result<(), EmitError> {
    let llvm_ir = fs::read_to_string(llvm_ir_path)?;
    fs::write(hsaco_path.with_extension("o"), format!("object:{llvm_ir}"))?;
    fs::write(hsaco_path, format!("hsaco:{llvm_ir}"))?;
    Ok(())
}

fn emit_direct(output: &Path, producer: &ProducerIdentity, kernels: &[TestKernel]) {
    emit_artifact_transaction(
        output,
        producer,
        kernels,
        |kernel| kernel.name,
        |kernel| Ok(format!("{}:{}", kernel.generation, kernel.name)),
        fake_compile,
    )
    .unwrap();
}

fn emit_authorized(
    output: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    kernels: &[TestKernel],
) {
    emit_artifact_transaction_for_attempt(
        output,
        producer,
        attempt,
        kernels,
        |kernel| kernel.name,
        |kernel| Ok(format!("{}:{}", kernel.generation, kernel.name)),
        fake_compile,
    )
    .unwrap();
}

fn assert_generation(output: &Path, kernels: &[&str], generation: &str) {
    for kernel in kernels {
        assert_eq!(
            fs::read_to_string(output.join(format!("{kernel}.ll"))).unwrap(),
            format!("{generation}:{kernel}")
        );
        assert_eq!(
            fs::read_to_string(output.join(format!("{kernel}.o"))).unwrap(),
            format!("object:{generation}:{kernel}")
        );
        assert_eq!(
            fs::read_to_string(output.join(format!("{kernel}.hsaco"))).unwrap(),
            format!("hsaco:{generation}:{kernel}")
        );
    }
}

fn assert_absent(output: &Path, kernels: &[&str]) {
    for kernel in kernels {
        for extension in ["ll", "o", "hsaco"] {
            assert!(!output.join(format!("{kernel}.{extension}")).exists());
        }
    }
}

fn read_triplet(output: &Path, kernel: &str) -> [Vec<u8>; 3] {
    ["ll", "o", "hsaco"]
        .map(|extension| fs::read(output.join(format!("{kernel}.{extension}"))).unwrap())
}

#[test]
fn begin_invalidates_only_the_selected_producers_owned_triplets() {
    let temp = TestDirectory::new();
    let output = temp.path.join("output");
    let selected = make_producer("selected", "/workspace/src/selected.rs");
    let preserved = make_producer("preserved", "/workspace/src/preserved.rs");

    emit_direct(
        &output,
        &selected,
        &[
            TestKernel {
                name: "selected_a",
                generation: "old-selected",
            },
            TestKernel {
                name: "selected_b",
                generation: "old-selected",
            },
        ],
    );
    emit_direct(
        &output,
        &preserved,
        &one("preserved_kernel", "preserved-generation"),
    );
    let preserved_before = read_triplet(&output, "preserved_kernel");

    let attempt = begin_build_attempt(&output, &selected, session(1)).unwrap();

    assert_absent(&output, &["selected_a", "selected_b"]);
    assert_eq!(read_triplet(&output, "preserved_kernel"), preserved_before);
    fail_build_attempt(&output, &selected, attempt).unwrap();
    assert_generation(&output, &["preserved_kernel"], "preserved-generation");
}

#[test]
fn same_invocation_begin_is_idempotent_until_the_backend_claims_it() {
    let temp = TestDirectory::new();
    let output = temp.path.join("output");
    let producer = make_producer("idempotent", "/workspace/src/idempotent.rs");
    let build_session = session(2);

    let first = begin_build_attempt(&output, &producer, build_session).unwrap();
    let second = begin_build_attempt(&output, &producer, build_session).unwrap();
    assert_eq!(second, first);
    assert_eq!(first.session(), build_session);
    assert_ne!(first.generation(), 0);

    emit_authorized(
        &output,
        &producer,
        first,
        &one("idempotent_kernel", "authorized"),
    );
    assert!(matches!(
        begin_build_attempt(&output, &producer, build_session),
        Err(EmitError::BuildAttempt { .. })
    ));
    finish_build_attempt(&output, &producer, first).unwrap();
    finish_build_attempt(&output, &producer, first).unwrap();
    assert!(matches!(
        begin_build_attempt(&output, &producer, build_session),
        Err(EmitError::BuildAttempt { .. })
    ));
    assert_generation(&output, &["idempotent_kernel"], "authorized");
}

#[test]
fn distinct_invocation_fingerprint_supersedes_instead_of_aliasing() {
    let temp = TestDirectory::new();
    let output = temp.path.join("output");
    let producer = make_producer("variants", "/workspace/src/variants.rs");
    let build_session = session(17);
    let first =
        begin_build_attempt_for_invocation(&output, &producer, invocation(17), build_session)
            .unwrap();
    let second =
        begin_build_attempt_for_invocation(&output, &producer, invocation(18), build_session)
            .unwrap();

    assert!(second.generation() > first.generation());
    assert_ne!(second.invocation(), first.invocation());
    let prepare_called = AtomicBool::new(false);
    let error = emit_artifact_transaction_for_attempt(
        &output,
        &producer,
        first,
        &one("stale_variant", "must-not-publish"),
        |kernel| kernel.name,
        |kernel| {
            prepare_called.store(true, Ordering::SeqCst);
            Ok(format!("{}:{}", kernel.generation, kernel.name))
        },
        fake_compile,
    )
    .unwrap_err();
    assert!(matches!(error, EmitError::BuildAttempt { .. }));
    assert!(!prepare_called.load(Ordering::SeqCst));
    assert_absent(&output, &["stale_variant"]);
    fail_build_attempt(&output, &producer, second).unwrap();
}

#[test]
fn backend_claim_survives_process_like_unwind_and_is_not_reusable() {
    let temp = TestDirectory::new();
    let output = temp.path.join("output");
    let producer = make_producer("claimed", "/workspace/src/claimed.rs");
    let attempt = begin_build_attempt(&output, &producer, session(19)).unwrap();

    let unwind = catch_unwind(AssertUnwindSafe(|| {
        let _ = emit_artifact_transaction_for_attempt(
            &output,
            &producer,
            attempt,
            &one("claimed_kernel", "never-published"),
            |kernel| kernel.name,
            |_kernel| -> Result<String, EmitError> {
                panic!("simulated backend process termination after durable claim")
            },
            fake_compile,
        );
    }));
    assert!(unwind.is_err());

    let prepare_called = AtomicBool::new(false);
    let error = emit_artifact_transaction_for_attempt(
        &output,
        &producer,
        attempt,
        &one("claimed_kernel", "must-not-publish"),
        |kernel| kernel.name,
        |kernel| {
            prepare_called.store(true, Ordering::SeqCst);
            Ok(format!("{}:{}", kernel.generation, kernel.name))
        },
        fake_compile,
    )
    .unwrap_err();
    assert!(matches!(error, EmitError::BuildAttempt { .. }));
    assert!(!prepare_called.load(Ordering::SeqCst));
    assert_absent(&output, &["claimed_kernel"]);
    fail_build_attempt(&output, &producer, attempt).unwrap();
}

#[test]
fn cleanup_failure_after_claim_revokes_the_token_before_returning() {
    use std::os::unix::fs::PermissionsExt;

    let temp = TestDirectory::new();
    let output = temp.path.join("output");
    let producer = make_producer("cleanup_failure", "/workspace/src/cleanup_failure.rs");
    let attempt = begin_build_attempt(&output, &producer, session(20)).unwrap();
    let abandoned = output.join(".fe2o3-stage-999-1");
    fs::create_dir(&abandoned).unwrap();
    fs::set_permissions(&abandoned, fs::Permissions::from_mode(0o755)).unwrap();
    let prepare_called = AtomicBool::new(false);

    let error = emit_artifact_transaction_for_attempt(
        &output,
        &producer,
        attempt,
        &one("cleanup_kernel", "must-not-publish"),
        |kernel| kernel.name,
        |kernel| {
            prepare_called.store(true, Ordering::SeqCst);
            Ok(format!("{}:{}", kernel.generation, kernel.name))
        },
        fake_compile,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        EmitError::InvalidArtifactDestination { .. }
    ));
    assert!(!prepare_called.load(Ordering::SeqCst));
    assert_absent(&output, &["cleanup_kernel"]);

    fs::remove_dir(&abandoned).unwrap();
    let retry = emit_artifact_transaction_for_attempt(
        &output,
        &producer,
        attempt,
        &one("cleanup_kernel", "must-not-publish"),
        |kernel| kernel.name,
        |kernel| Ok(format!("{}:{}", kernel.generation, kernel.name)),
        fake_compile,
    )
    .unwrap_err();
    assert!(matches!(retry, EmitError::BuildAttempt { .. }));
}

#[test]
fn different_session_supersedes_and_stale_token_is_rejected_without_mutation() {
    let temp = TestDirectory::new();
    let output = temp.path.join("output");
    let producer = make_producer("superseded", "/workspace/src/superseded.rs");

    let stale = begin_build_attempt(&output, &producer, session(3)).unwrap();
    emit_authorized(
        &output,
        &producer,
        stale,
        &one("old_kernel", "old-generation"),
    );

    let current = begin_build_attempt(&output, &producer, session(4)).unwrap();
    assert!(current.generation() > stale.generation());
    assert_absent(&output, &["old_kernel"]);
    emit_authorized(
        &output,
        &producer,
        current,
        &one("current_kernel", "current-generation"),
    );
    let current_before = read_triplet(&output, "current_kernel");
    let abandoned_stage = output.join(".fe2o3-stage-999-1");
    fs::create_dir(&abandoned_stage).unwrap();
    fs::write(abandoned_stage.join("sentinel"), b"keep").unwrap();
    let ownership_recovery = output.join(".fe2o3-owners-v1.recovery");
    fs::write(&ownership_recovery, b"keep").unwrap();
    let prepare_called = AtomicBool::new(false);
    let compile_called = AtomicBool::new(false);

    assert!(matches!(
        fail_build_attempt(&output, &producer, stale),
        Err(EmitError::BuildAttempt { .. })
    ));
    assert!(matches!(
        finish_build_attempt(&output, &producer, stale),
        Err(EmitError::BuildAttempt { .. })
    ));
    assert_eq!(read_triplet(&output, "current_kernel"), current_before);
    assert_eq!(fs::read(abandoned_stage.join("sentinel")).unwrap(), b"keep");
    assert_eq!(fs::read(&ownership_recovery).unwrap(), b"keep");

    let error = emit_artifact_transaction_for_attempt(
        &output,
        &producer,
        stale,
        &one("stale_kernel", "must-not-publish"),
        |kernel| kernel.name,
        |kernel| {
            prepare_called.store(true, Ordering::SeqCst);
            Ok(format!("{}:{}", kernel.generation, kernel.name))
        },
        |llvm_ir, hsaco| {
            compile_called.store(true, Ordering::SeqCst);
            fake_compile(llvm_ir, hsaco)
        },
    )
    .unwrap_err();

    assert!(matches!(error, EmitError::BuildAttempt { .. }));
    assert!(!prepare_called.load(Ordering::SeqCst));
    assert!(!compile_called.load(Ordering::SeqCst));
    assert_eq!(read_triplet(&output, "current_kernel"), current_before);
    assert_absent(&output, &["stale_kernel"]);
    finish_build_attempt(&output, &producer, current).unwrap();
}

#[test]
fn failed_attempt_is_terminal_for_the_same_session() {
    let temp = TestDirectory::new();
    let output = temp.path.join("output");
    let producer = make_producer("failed", "/workspace/src/failed.rs");
    let build_session = session(5);
    let attempt = begin_build_attempt(&output, &producer, build_session).unwrap();
    emit_authorized(
        &output,
        &producer,
        attempt,
        &one("failed_kernel", "before-failure"),
    );

    fail_build_attempt(&output, &producer, attempt).unwrap();

    assert_absent(&output, &["failed_kernel"]);
    assert!(matches!(
        begin_build_attempt(&output, &producer, build_session),
        Err(EmitError::BuildAttempt { .. })
    ));
    assert!(matches!(
        emit_artifact_transaction_for_attempt(
            &output,
            &producer,
            attempt,
            &one("late_kernel", "late"),
            |kernel| kernel.name,
            |kernel| Ok(format!("{}:{}", kernel.generation, kernel.name)),
            fake_compile,
        ),
        Err(EmitError::BuildAttempt { .. })
    ));
    assert_absent(&output, &["late_kernel"]);
}

#[test]
fn successful_backend_then_finish_preserves_the_published_generation() {
    let temp = TestDirectory::new();
    let output = temp.path.join("output");
    let producer = make_producer("finished", "/workspace/src/finished.rs");
    let attempt = begin_build_attempt(&output, &producer, session(6)).unwrap();

    emit_authorized(
        &output,
        &producer,
        attempt,
        &one("finished_kernel", "published-generation"),
    );
    let before_finish = read_triplet(&output, "finished_kernel");
    finish_build_attempt(&output, &producer, attempt).unwrap();

    assert_eq!(read_triplet(&output, "finished_kernel"), before_finish);
    assert_generation(&output, &["finished_kernel"], "published-generation");

    let direct_error = emit_artifact_transaction(
        &output,
        &producer,
        &one("finished_kernel", "later-direct-generation"),
        |kernel| kernel.name,
        |kernel| Ok(format!("{}:{}", kernel.generation, kernel.name)),
        fake_compile,
    )
    .unwrap_err();
    assert!(matches!(direct_error, EmitError::BuildAttempt { .. }));
    assert_eq!(read_triplet(&output, "finished_kernel"), before_finish);

    let next = begin_build_attempt(&output, &producer, session(16)).unwrap();
    assert_absent(&output, &["finished_kernel"]);
    emit_authorized(
        &output,
        &producer,
        next,
        &one("finished_kernel", "later-managed-generation"),
    );
    finish_build_attempt(&output, &producer, next).unwrap();
    assert_generation(&output, &["finished_kernel"], "later-managed-generation");
}

#[test]
fn finish_without_backend_fails_closed_and_leaves_owned_artifacts_removed() {
    let temp = TestDirectory::new();
    let output = temp.path.join("output");
    let producer = make_producer("no_backend", "/workspace/src/no_backend.rs");
    emit_direct(
        &output,
        &producer,
        &one("old_owned_kernel", "old-owned-generation"),
    );

    let build_session = session(7);
    let attempt = begin_build_attempt(&output, &producer, build_session).unwrap();
    assert_absent(&output, &["old_owned_kernel"]);
    let error = finish_build_attempt(&output, &producer, attempt).unwrap_err();

    assert!(matches!(error, EmitError::BuildAttempt { .. }));
    assert_absent(&output, &["old_owned_kernel"]);
    assert!(matches!(
        begin_build_attempt(&output, &producer, build_session),
        Err(EmitError::BuildAttempt { .. })
    ));
}

#[test]
fn backend_failure_marks_attempt_failed_and_invalidates_staged_output() {
    let temp = TestDirectory::new();
    let output = temp.path.join("output");
    let producer = make_producer("backend_failure", "/workspace/src/backend_failure.rs");
    let build_session = session(8);
    let attempt = begin_build_attempt(&output, &producer, build_session).unwrap();

    let error = emit_artifact_transaction_for_attempt(
        &output,
        &producer,
        attempt,
        &one("failing_kernel", "failing-backend"),
        |kernel| kernel.name,
        |kernel| Ok(format!("{}:{}", kernel.generation, kernel.name)),
        |llvm_ir, hsaco| {
            fake_compile(llvm_ir, hsaco)?;
            Err(io::Error::other("injected backend failure").into())
        },
    )
    .unwrap_err();

    assert!(matches!(error, EmitError::Transaction(_)));
    assert_absent(&output, &["failing_kernel"]);
    assert!(matches!(
        begin_build_attempt(&output, &producer, build_session),
        Err(EmitError::BuildAttempt { .. })
    ));
    assert!(matches!(
        finish_build_attempt(&output, &producer, attempt),
        Err(EmitError::BuildAttempt { .. })
    ));
}

#[test]
fn direct_backend_is_rejected_while_wrapped_attempt_is_active() {
    let temp = TestDirectory::new();
    let output = temp.path.join("output");
    let producer = make_producer("wrapped", "/workspace/src/wrapped.rs");
    let attempt = begin_build_attempt(&output, &producer, session(9)).unwrap();
    emit_authorized(
        &output,
        &producer,
        attempt,
        &one("wrapped_kernel", "wrapped-generation"),
    );
    let before_direct = read_triplet(&output, "wrapped_kernel");
    let prepare_called = AtomicBool::new(false);
    let compile_called = AtomicBool::new(false);

    let error = emit_artifact_transaction(
        &output,
        &producer,
        &one("direct_kernel", "direct-generation"),
        |kernel| kernel.name,
        |kernel| {
            prepare_called.store(true, Ordering::SeqCst);
            Ok(format!("{}:{}", kernel.generation, kernel.name))
        },
        |llvm_ir, hsaco| {
            compile_called.store(true, Ordering::SeqCst);
            fake_compile(llvm_ir, hsaco)
        },
    )
    .unwrap_err();

    assert!(matches!(error, EmitError::BuildAttempt { .. }));
    assert!(!prepare_called.load(Ordering::SeqCst));
    assert!(!compile_called.load(Ordering::SeqCst));
    assert_eq!(read_triplet(&output, "wrapped_kernel"), before_direct);
    assert_absent(&output, &["direct_kernel"]);
    finish_build_attempt(&output, &producer, attempt).unwrap();
}

#[test]
fn concurrent_orderings_never_leave_a_stale_publication() {
    let old_first = TestDirectory::new();
    let output = old_first.path.join("output");
    let producer = make_producer("race_old_first", "/workspace/src/race_old_first.rs");
    let stale = begin_build_attempt(&output, &producer, session(10)).unwrap();
    let compile_entered = Arc::new(Barrier::new(2));
    let (release_compile, wait_for_release) = mpsc::channel();

    let old_output = output.clone();
    let old_producer = producer.clone();
    let old_barrier = Arc::clone(&compile_entered);
    let old_publisher = thread::spawn(move || {
        emit_artifact_transaction_for_attempt(
            &old_output,
            &old_producer,
            stale,
            &one("old_racing_kernel", "old-racing-generation"),
            |kernel| kernel.name,
            |kernel| Ok(format!("{}:{}", kernel.generation, kernel.name)),
            |llvm_ir, hsaco| {
                old_barrier.wait();
                wait_for_release.recv().unwrap();
                fake_compile(llvm_ir, hsaco)
            },
        )
    });

    compile_entered.wait();
    let (new_begin_started, observe_new_begin_started) = mpsc::channel();
    let new_output = output.clone();
    let new_producer = producer.clone();
    let newer_begin = thread::spawn(move || {
        new_begin_started.send(()).unwrap();
        begin_build_attempt(&new_output, &new_producer, session(11))
    });
    observe_new_begin_started.recv().unwrap();
    release_compile.send(()).unwrap();

    old_publisher.join().unwrap().unwrap();
    let current = newer_begin.join().unwrap().unwrap();
    assert!(current.generation() > stale.generation());
    assert_absent(&output, &["old_racing_kernel"]);
    fail_build_attempt(&output, &producer, current).unwrap();

    let new_first = TestDirectory::new();
    let output = new_first.path.join("output");
    let producer = make_producer("race_new_first", "/workspace/src/race_new_first.rs");
    let stale = begin_build_attempt(&output, &producer, session(12)).unwrap();
    let start_together = Arc::new(Barrier::new(3));
    let (new_attempt_sender, new_attempt_receiver) = mpsc::channel();

    let new_output = output.clone();
    let new_producer = producer.clone();
    let new_barrier = Arc::clone(&start_together);
    let newer_begin = thread::spawn(move || {
        new_barrier.wait();
        let attempt = begin_build_attempt(&new_output, &new_producer, session(13)).unwrap();
        new_attempt_sender.send(attempt).unwrap();
        attempt
    });

    let prepare_called = Arc::new(AtomicBool::new(false));
    let compile_called = Arc::new(AtomicBool::new(false));
    let stale_output = output.clone();
    let stale_producer = producer.clone();
    let stale_barrier = Arc::clone(&start_together);
    let stale_prepare_called = Arc::clone(&prepare_called);
    let stale_compile_called = Arc::clone(&compile_called);
    let stale_publisher = thread::spawn(move || {
        stale_barrier.wait();
        let _current = new_attempt_receiver.recv().unwrap();
        emit_artifact_transaction_for_attempt(
            &stale_output,
            &stale_producer,
            stale,
            &one("late_stale_kernel", "must-not-publish"),
            |kernel| kernel.name,
            |kernel| {
                stale_prepare_called.store(true, Ordering::SeqCst);
                Ok(format!("{}:{}", kernel.generation, kernel.name))
            },
            |llvm_ir, hsaco| {
                stale_compile_called.store(true, Ordering::SeqCst);
                fake_compile(llvm_ir, hsaco)
            },
        )
    });

    start_together.wait();
    let current = newer_begin.join().unwrap();
    let error = stale_publisher.join().unwrap().unwrap_err();
    assert!(matches!(error, EmitError::BuildAttempt { .. }));
    assert!(!prepare_called.load(Ordering::SeqCst));
    assert!(!compile_called.load(Ordering::SeqCst));
    assert_absent(&output, &["late_stale_kernel"]);
    fail_build_attempt(&output, &producer, current).unwrap();
}

#[test]
fn control_registry_survives_reload_and_retains_generation_watermark() {
    const ATTEMPT_MAGIC: &[u8] = b"FE2O3-ATTEMPTS-V1\0";

    let temp = TestDirectory::new();
    let output = temp.path.join("output");
    let original_producer = make_producer("reload", "/workspace/src/reload.rs");
    let original_session = session(14);
    let first = begin_build_attempt(&output, &original_producer, original_session).unwrap();
    let registry_path = output.join(".fe2o3-attempts-v1");
    let active_registry = fs::read(&registry_path).unwrap();
    assert!(active_registry.starts_with(ATTEMPT_MAGIC));

    let reloaded_producer = make_producer("reload", "/workspace/src/reload.rs");
    let reloaded_session = BuildSession::from_hex(&original_session.to_hex()).unwrap();
    let reloaded_attempt = BuildAttempt::from_env_value(&first.to_env_value()).unwrap();
    assert_eq!(
        begin_build_attempt(&output, &reloaded_producer, reloaded_session).unwrap(),
        reloaded_attempt
    );
    emit_authorized(
        &output,
        &reloaded_producer,
        reloaded_attempt,
        &one("reload_kernel", "reload-generation"),
    );
    finish_build_attempt(&output, &reloaded_producer, reloaded_attempt).unwrap();

    let finalized_registry = fs::read(&registry_path).unwrap();
    assert!(finalized_registry.starts_with(ATTEMPT_MAGIC));
    assert_ne!(finalized_registry, active_registry);
    let second = begin_build_attempt(&output, &reloaded_producer, session(15)).unwrap();
    assert!(second.generation() > first.generation());
    assert_absent(&output, &["reload_kernel"]);
    fail_build_attempt(&output, &reloaded_producer, second).unwrap();
}
