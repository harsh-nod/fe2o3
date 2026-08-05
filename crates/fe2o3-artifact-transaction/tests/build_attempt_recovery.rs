use fe2o3_artifact_transaction::{
    BuildAttempt, BuildInvocation, BuildSession, EmitError, ProducerIdentity,
    begin_build_attempt as begin_build_attempt_for_invocation,
    emit_artifact_transaction_after_preflight,
    emit_artifact_transaction_after_preflight_for_attempt, finish_build_attempt,
};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

const ATTEMPT_FILE: &str = ".fe2o3-attempts-v1";
const RECOVERY_ATTEMPT_FILE: &str = ".fe2o3-attempts-v1.recovery";
const ATTEMPT_MAGIC: &[u8] = b"FE2O3-ATTEMPTS-V1\0";

static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new() -> Self {
        loop {
            let id = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "fe2o3-build-attempt-recovery-{}-{id}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => {
                    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
                    return Self { path };
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("failed to create private test directory: {error}"),
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

#[derive(Default)]
struct CallbackFlags {
    preflight: AtomicBool,
    kernel_name: AtomicBool,
    prepare: AtomicBool,
    compile: AtomicBool,
}

impl CallbackFlags {
    fn assert_none_called(&self) {
        assert!(!self.preflight.load(Ordering::SeqCst));
        assert!(!self.kernel_name.load(Ordering::SeqCst));
        assert!(!self.prepare.load(Ordering::SeqCst));
        assert!(!self.compile.load(Ordering::SeqCst));
    }
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

fn push_text(bytes: &mut Vec<u8>, text: &str) {
    bytes.extend_from_slice(&u16::try_from(text.len()).unwrap().to_le_bytes());
    bytes.extend_from_slice(text.as_bytes());
}

fn canonical_one_record_registry(
    attempt: BuildAttempt,
    source: &str,
    crate_name: &str,
    phase: u8,
    backend_seen: bool,
) -> Vec<u8> {
    let stable_source = format!("path:{source}");
    let mut bytes = Vec::new();
    bytes.extend_from_slice(ATTEMPT_MAGIC);
    bytes.extend_from_slice(&attempt.generation().to_le_bytes());
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    push_text(&mut bytes, &stable_source);
    push_text(&mut bytes, crate_name);
    bytes.extend_from_slice(attempt.invocation().as_bytes());
    bytes.extend_from_slice(&attempt.generation().to_le_bytes());
    bytes.extend_from_slice(attempt.session().as_bytes());
    bytes.push(phase);
    bytes.push(u8::from(backend_seen));
    bytes
}

fn write_private_recovery(output: &Path, bytes: &[u8]) {
    let path = output.join(RECOVERY_ATTEMPT_FILE);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
}

fn fake_compile(llvm_ir_path: &Path, hsaco_path: &Path) -> Result<(), EmitError> {
    let llvm_ir = fs::read_to_string(llvm_ir_path)?;
    fs::write(hsaco_path.with_extension("o"), format!("object:{llvm_ir}"))?;
    fs::write(hsaco_path, format!("hsaco:{llvm_ir}"))?;
    Ok(())
}

fn emit_authorized(
    output: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    kernels: [TestKernel; 1],
) {
    emit_artifact_transaction_after_preflight_for_attempt(
        output,
        producer,
        attempt,
        || Ok(kernels),
        |kernel| kernel.name,
        |kernel| Ok(format!("{}:{}", kernel.generation, kernel.name)),
        fake_compile,
    )
    .unwrap();
}

fn emit_authorized_with_flags(
    output: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    kernel: TestKernel,
    flags: &CallbackFlags,
) -> Result<(), EmitError> {
    emit_artifact_transaction_after_preflight_for_attempt(
        output,
        producer,
        attempt,
        || {
            flags.preflight.store(true, Ordering::SeqCst);
            Ok([kernel])
        },
        |kernel| {
            flags.kernel_name.store(true, Ordering::SeqCst);
            kernel.name
        },
        |kernel| {
            flags.prepare.store(true, Ordering::SeqCst);
            Ok(format!("{}:{}", kernel.generation, kernel.name))
        },
        |llvm_ir, hsaco| {
            flags.compile.store(true, Ordering::SeqCst);
            fake_compile(llvm_ir, hsaco)
        },
    )
    .map(|_| ())
}

fn emit_direct_with_flags(
    output: &Path,
    producer: &ProducerIdentity,
    kernel: TestKernel,
    flags: &CallbackFlags,
) -> Result<(), EmitError> {
    emit_artifact_transaction_after_preflight(
        output,
        producer,
        || {
            flags.preflight.store(true, Ordering::SeqCst);
            Ok([kernel])
        },
        |kernel| {
            flags.kernel_name.store(true, Ordering::SeqCst);
            kernel.name
        },
        |kernel| {
            flags.prepare.store(true, Ordering::SeqCst);
            Ok(format!("{}:{}", kernel.generation, kernel.name))
        },
        |llvm_ir, hsaco| {
            flags.compile.store(true, Ordering::SeqCst);
            fake_compile(llvm_ir, hsaco)
        },
    )
    .map(|_| ())
}

fn read_triplet(output: &Path, kernel: &str) -> [Vec<u8>; 3] {
    ["ll", "o", "hsaco"]
        .map(|extension| fs::read(output.join(format!("{kernel}.{extension}"))).unwrap())
}

fn assert_absent(output: &Path, kernel: &str) {
    for extension in ["ll", "o", "hsaco"] {
        assert!(!output.join(format!("{kernel}.{extension}")).exists());
    }
}

#[test]
fn failed_recovery_is_replayed_before_backend_authorization() {
    const SOURCE: &str = "/workspace/src/replayed_failure.rs";
    const CRATE_NAME: &str = "replayed_failure";

    let temp = TestDirectory::new();
    let output = temp.path.join("output");
    let producer = make_producer(CRATE_NAME, SOURCE);
    let attempt = begin_build_attempt(&output, &producer, session(1)).unwrap();
    let registry_path = output.join(ATTEMPT_FILE);
    let building = fs::read(&registry_path).unwrap();
    assert_eq!(
        building,
        canonical_one_record_registry(attempt, SOURCE, CRATE_NAME, 1, false)
    );

    let mut failed_recovery = building;
    let phase_index = failed_recovery.len() - 2;
    assert_eq!(failed_recovery[phase_index], 1);
    failed_recovery[phase_index] = 2;
    write_private_recovery(&output, &failed_recovery);

    let flags = CallbackFlags::default();
    let error = emit_authorized_with_flags(
        &output,
        &producer,
        attempt,
        TestKernel {
            name: "must_not_publish",
            generation: "rejected",
        },
        &flags,
    )
    .unwrap_err();

    assert!(matches!(error, EmitError::BuildAttempt { .. }));
    flags.assert_none_called();
    assert_eq!(fs::read(registry_path).unwrap(), failed_recovery);
    assert!(!output.join(RECOVERY_ATTEMPT_FILE).exists());
    assert_absent(&output, "must_not_publish");
}

#[test]
fn completed_recovery_makes_finish_idempotent_and_preserves_artifacts() {
    const SOURCE: &str = "/workspace/src/replayed_completion.rs";
    const CRATE_NAME: &str = "replayed_completion";

    let temp = TestDirectory::new();
    let output = temp.path.join("output");
    let producer = make_producer(CRATE_NAME, SOURCE);
    let attempt = begin_build_attempt(&output, &producer, session(2)).unwrap();
    emit_authorized(
        &output,
        &producer,
        attempt,
        one("completed_kernel", "completed-generation"),
    );
    let triplet = read_triplet(&output, "completed_kernel");
    let registry_path = output.join(ATTEMPT_FILE);
    assert_eq!(
        fs::read(&registry_path).unwrap(),
        canonical_one_record_registry(attempt, SOURCE, CRATE_NAME, 4, true)
    );

    let completed_recovery = canonical_one_record_registry(attempt, SOURCE, CRATE_NAME, 3, true);
    write_private_recovery(&output, &completed_recovery);

    finish_build_attempt(&output, &producer, attempt).unwrap();
    finish_build_attempt(&output, &producer, attempt).unwrap();

    assert_eq!(fs::read(registry_path).unwrap(), completed_recovery);
    assert!(!output.join(RECOVERY_ATTEMPT_FILE).exists());
    assert_eq!(read_triplet(&output, "completed_kernel"), triplet);
}

#[test]
fn malformed_private_recovery_poisons_authorization_until_repaired() {
    let temp = TestDirectory::new();
    let output = temp.path.join("output");
    let producer = make_producer("poisoned", "/workspace/src/poisoned.rs");
    let attempt = begin_build_attempt(&output, &producer, session(3)).unwrap();
    let registry_before = fs::read(output.join(ATTEMPT_FILE)).unwrap();
    let malformed_recovery = b"not a canonical attempt registry";
    write_private_recovery(&output, malformed_recovery);

    for generation in ["first-rejection", "second-rejection"] {
        let flags = CallbackFlags::default();
        let error = emit_authorized_with_flags(
            &output,
            &producer,
            attempt,
            TestKernel {
                name: "poisoned_kernel",
                generation,
            },
            &flags,
        )
        .unwrap_err();

        assert!(matches!(error, EmitError::BuildAttempt { .. }));
        flags.assert_none_called();
        assert_eq!(
            fs::read(output.join(RECOVERY_ATTEMPT_FILE)).unwrap(),
            malformed_recovery
        );
        assert_eq!(
            fs::read(output.join(ATTEMPT_FILE)).unwrap(),
            registry_before
        );
        assert_absent(&output, "poisoned_kernel");
    }
}

#[test]
fn completed_managed_record_rejects_direct_backend_without_mutation() {
    let temp = TestDirectory::new();
    let output = temp.path.join("output");
    let producer = make_producer("managed", "/workspace/src/managed.rs");
    let attempt = begin_build_attempt(&output, &producer, session(4)).unwrap();
    emit_authorized(
        &output,
        &producer,
        attempt,
        one("managed_kernel", "managed-generation"),
    );
    finish_build_attempt(&output, &producer, attempt).unwrap();
    let triplet_before = read_triplet(&output, "managed_kernel");
    let registry_before = fs::read(output.join(ATTEMPT_FILE)).unwrap();
    let flags = CallbackFlags::default();

    let error = emit_direct_with_flags(
        &output,
        &producer,
        TestKernel {
            name: "direct_kernel",
            generation: "must-not-publish",
        },
        &flags,
    )
    .unwrap_err();

    assert!(matches!(error, EmitError::BuildAttempt { .. }));
    flags.assert_none_called();
    assert_eq!(read_triplet(&output, "managed_kernel"), triplet_before);
    assert_eq!(
        fs::read(output.join(ATTEMPT_FILE)).unwrap(),
        registry_before
    );
    assert_absent(&output, "direct_kernel");
}

#[test]
fn one_token_authorizes_only_one_backend_publication() {
    let temp = TestDirectory::new();
    let output = temp.path.join("output");
    let producer = make_producer("single_use", "/workspace/src/single_use.rs");
    let attempt = begin_build_attempt(&output, &producer, session(5)).unwrap();
    emit_authorized(
        &output,
        &producer,
        attempt,
        one("current_kernel", "first-publication"),
    );
    let triplet_before = read_triplet(&output, "current_kernel");
    let registry_before = fs::read(output.join(ATTEMPT_FILE)).unwrap();
    let flags = CallbackFlags::default();

    let error = emit_authorized_with_flags(
        &output,
        &producer,
        attempt,
        TestKernel {
            name: "second_kernel",
            generation: "must-not-publish",
        },
        &flags,
    )
    .unwrap_err();

    assert!(matches!(error, EmitError::BuildAttempt { .. }));
    flags.assert_none_called();
    assert_eq!(read_triplet(&output, "current_kernel"), triplet_before);
    assert_eq!(
        fs::read(output.join(ATTEMPT_FILE)).unwrap(),
        registry_before
    );
    assert_absent(&output, "second_kernel");
    finish_build_attempt(&output, &producer, attempt).unwrap();
}
