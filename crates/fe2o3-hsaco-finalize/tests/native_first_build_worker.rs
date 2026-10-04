//! Opt-in CPU structural tests for the native V4 Worker consumer.
//!
//! First run the verifier's ignored `export_native_first_build_worker_v4_fixtures`
//! test with FE2O3_NATIVE_WORKER_FIXTURE_DIR naming an existing, caller-owned
//! absolute 0700 directory. This suite requires all four exact exported files
//! and Cargo's existing fe2o3-worker-executor-fixture binary; absence is a failure.
//! Run this target explicitly with `--ignored --test-threads=1` after coordination.
//!
//! The exporter uses PUBLIC TEST keys and a synthetic rustc invocation. The
//! measured fixture Worker synthesizes output and derivation records: these tests
//! grant NO protected compiler origin, production proof authority, real LLVM
//! execution/refinement, executable HSACO validity, or GPU execution credit.
#![cfg(target_os = "linux")]

use std::{
    env,
    fs::{self, File},
    io::Read,
    os::unix::fs::{DirBuilderExt, MetadataExt},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
use fe2o3_artifact_transaction::{
    self as transaction, BuildAttempt, BuildInvocation, BuildSession,
    CompilerModuleHandoffConsumptionTokenV4, CompilerModuleHandoffCurrentnessLeaseV4,
    CompilerModuleHandoffReceiptV4, ConsumedCompilerModuleHandoffV4, ProducerIdentity,
};
use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_compiler_ffi::{
    INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_DECODE_METADATA_STORAGE_V4 as DECODE_METADATA,
    InertSemanticCompilerModuleHandoffV4 as Handoff,
    MAX_INERT_REFINED_FORWARDING_STORAGE_V1 as STORAGE_LIMIT,
    inert_semantic_compiler_module_handoff_decode_work_v4,
};
use fe2o3_hsaco_finalize::{
    ContentIdentityV1, InertDecodedWorkerExchangeV2, LinkOptionV1, NativeFirstBuildWorkerErrorV1,
    NativeFirstBuildWorkerStorageV1, PinnedWorkerV1, PreparedNativeFirstBuildWorkerV1,
    ProtectedCompilerNativeHandoffBindingErrorV1, WorkerExecutionLimitsV1, WorkerInputKindV1,
    WorkerInputV1, WorkerMeasurementV1, WorkerOutputConstraintsV1,
    execute_preflighted_native_reproducible_first_build_worker_v1 as execute,
    preflight_native_reproducible_first_build_worker_v1 as preflight,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_verifier::{
    RecoveredCompilerNativeSemanticHandoffV4 as Recovered,
    RefinedForwardingOriginalSourceProofV1 as Original,
    recover_compiler_native_semantic_handoff_token_v4,
};
use rustix::fs::{Mode, OFlags, open, openat};

const FIXTURE_ENV: &str = "FE2O3_NATIVE_WORKER_FIXTURE_DIR";
const MAX_FIXTURE_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const WORK_LIMIT: usize = 4_000_000_000;
pub(crate) const WORKER_ID: &str = "fixture-worker-v3";
pub(crate) const LLVM_ID: &str = "fixture-llvm-v1";
const OUTPUT: &[u8] = b"fixture-output";
// The existing fixture executable recognizes this marker anywhere in its input.
// It is deliberately in an inert provider, never patched into the signed module.
const PROVIDER: &[u8] = b"workflow_kernel native V4 CPU structural provider";
const OUTPUT_BOUND: u64 = 4096;
pub(crate) const CASES: [(bool, Profile, &str); 4] = [
    (false, Profile::Gfx942, "direct-gfx942.v4"),
    (false, Profile::Gfx950, "direct-gfx950.v4"),
    (true, Profile::Gfx942, "erased-gfx942.v4"),
    (true, Profile::Gfx950, "erased-gfx950.v4"),
];
type Token = CompilerModuleHandoffConsumptionTokenV4<Recovered>;

pub(crate) struct Scratch(pub(crate) PathBuf);
impl Scratch {
    pub(crate) fn new() -> Self {
        transaction::enable_same_mount_namespace_artifact_path_guard_v1();
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = env::temp_dir().join(format!(
            "fe2o3-native-worker-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub(crate) struct Fixtures(File);
impl Fixtures {
    pub(crate) fn open() -> Self {
        let path = PathBuf::from(env::var_os(FIXTURE_ENV).unwrap_or_else(|| {
            panic!(
                "{FIXTURE_ENV} is required; run export_native_first_build_worker_v4_fixtures first"
            )
        }));
        assert!(path.is_absolute(), "fixture directory must be absolute");
        let directory = File::from(
            open(
                &path,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .expect("open caller-owned private fixture directory"),
        );
        let metadata = directory.metadata().unwrap();
        assert_eq!(metadata.uid(), rustix::process::geteuid().as_raw());
        assert_eq!(metadata.mode() & 0o7777, 0o700);
        let fixtures = Self(directory);
        // Even a narrowly selected ignored test requires the complete export.
        for (_, _, name) in CASES {
            fixtures.file(name);
        }
        fixtures
    }

    fn file(&self, name: &str) -> File {
        let file = File::from(
            openat(
                &self.0,
                name,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .unwrap_or_else(|error| panic!("required fixture {name}: {error}")),
        );
        let metadata = file.metadata().unwrap();
        assert!(metadata.is_file(), "{name} must be a regular file");
        assert_eq!(metadata.uid(), rustix::process::geteuid().as_raw());
        assert_eq!(metadata.mode() & 0o7777, 0o600);
        assert!((1..=MAX_FIXTURE_BYTES as u64).contains(&metadata.len()));
        file
    }

    fn decode(&self, name: &str, budget: &mut Budget<'_>) -> Handoff {
        let mut file = self.file(name);
        let length = usize::try_from(file.metadata().unwrap().len()).unwrap();
        assert!((1..=MAX_FIXTURE_BYTES).contains(&length));
        budget.reserve_storage(length + DECODE_METADATA).unwrap();
        budget.charge_work(length).unwrap();
        budget
            .charge_work(inert_semantic_compiler_module_handoff_decode_work_v4(length).unwrap())
            .unwrap();
        let mut wire = vec![0; length];
        assert_eq!(wire.capacity(), length);
        file.read_exact(&mut wire).unwrap();
        assert_eq!(file.read(&mut [0]).unwrap(), 0, "fixture grew during read");
        Handoff::decode_owned(wire).expect("strict public V4 decoder must accept the exported wire")
    }
}

pub(crate) struct Ready {
    lease: CompilerModuleHandoffCurrentnessLeaseV4,
    pub(crate) receipt: CompilerModuleHandoffReceiptV4,
    producer: ProducerIdentity,
    attempt: BuildAttempt,
    directory: Scratch,
}
impl Ready {
    pub(crate) fn publish(
        fixtures: &Fixtures,
        name: &str,
        seed: u8,
        budget: &mut Budget<'_>,
    ) -> Self {
        let directory = Scratch::new();
        let producer = ProducerIdentity::from_codegen(
            "native_worker_structural",
            Some(Path::new("/synthetic-native-worker.rs")),
        )
        .unwrap();
        let attempt = transaction::begin_build_attempt(
            &directory.0,
            &producer,
            BuildInvocation::from_bytes([seed; 32]),
            BuildSession::from_bytes([seed; 16]),
        )
        .unwrap();
        let handoff = fixtures.decode(name, budget);
        let decode_storage = handoff.backing_capacity() + DECODE_METADATA;
        let floor = budget.storage();
        let receipt = transaction::publish_compiler_module_handoff_v4(
            &directory.0,
            &producer,
            attempt,
            &handoff,
            budget,
        )
        .unwrap();
        assert_eq!(budget.storage(), floor);
        assert_eq!(
            transaction::recover_compiler_module_handoff_receipt_v4(
                &directory.0,
                &producer,
                attempt,
                budget,
            )
            .unwrap(),
            receipt
        );
        let (lease, storage) = transaction::acquire_compiler_module_handoff_currentness_lease_v4(
            &directory.0,
            &producer,
            receipt,
            budget,
        )
        .unwrap();
        assert_eq!(budget.storage(), floor);
        budget.reserve_storage(storage.retained_storage()).unwrap();
        // Publication's decoded input is dead. Recovery must load its own exact
        // transaction backing, with the earlier work still paid on this ledger.
        drop(handoff);
        budget.release_storage(decode_storage).unwrap();
        Self {
            lease,
            receipt,
            producer,
            attempt,
            directory,
        }
    }

    pub(crate) fn recover(&self, budget: &mut Budget<'_>) -> Token {
        let floor = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        let (token, storage) = self.lease.acquire_current_token(budget).unwrap();
        assert_eq!(budget.storage(), floor);
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let pointer = token.handoff().canonical_bytes().as_ptr();
        let floor = budget.storage();
        let before = budget.work();
        let (token, additional) = recover_compiler_native_semantic_handoff_token_v4(token, budget)
            .expect("real public signed-source/F verifier must admit exported V4 token");
        assert_eq!(budget.storage(), floor);
        assert!(budget.work() > before);
        assert!(budget.work_ledger_identity_v1() == ledger);
        budget
            .reserve_storage(additional.retained_storage())
            .unwrap();
        assert_eq!(token.handoff().canonical_bytes().as_ptr(), pointer);
        assert_eq!(
            token.storage().retained_storage(),
            storage.retained_storage() + additional.retained_storage()
        );
        token
    }

    pub(crate) fn consume(
        &self,
        token: Token,
        budget: &mut Budget<'_>,
    ) -> ConsumedCompilerModuleHandoffV4<Recovered> {
        let floor = budget.storage();
        let consumed = transaction::consume_compiler_module_handoff_with_currentness_v4(
            &self.lease,
            token,
            budget,
        )
        .unwrap();
        assert_eq!(consumed.receipt(), self.receipt);
        assert_eq!(budget.storage(), floor);
        consumed
    }

    fn assert_ready(&self, budget: &mut Budget<'_>) {
        assert_eq!(
            transaction::recover_compiler_module_handoff_receipt_v4(
                &self.directory.0,
                &self.producer,
                self.attempt,
                budget,
            )
            .expect("preflight refusal must leave the durable ready receipt unconsumed"),
            self.receipt
        );
    }
}

fn drop_token(token: Token, budget: &mut Budget<'_>) {
    let storage = token.storage().retained_storage();
    drop(token);
    budget.release_storage(storage).unwrap();
}

fn pinned(llvm_id: &str) -> PinnedWorkerV1 {
    let path = Path::new(env!("CARGO_BIN_EXE_fe2o3-worker-executor-fixture"));
    let bytes = fs::read(path).expect("existing measured Worker fixture binary is required");
    let measurement =
        WorkerMeasurementV1::new(ContentIdentityV1::calculate(&bytes), WORKER_ID, llvm_id).unwrap();
    PinnedWorkerV1::open(path, measurement).unwrap()
}

fn prepare(
    token: &Token,
    receipt: CompilerModuleHandoffReceiptV4,
    closure: CompilerClosureV2,
    worker: &PinnedWorkerV1,
    budget: &mut Budget<'_>,
) -> Result<
    (
        PreparedNativeFirstBuildWorkerV1,
        NativeFirstBuildWorkerStorageV1,
    ),
    NativeFirstBuildWorkerErrorV1,
> {
    let providers =
        vec![WorkerInputV1::new(WorkerInputKindV1::AmdGpuRelocatable, PROVIDER.to_vec()).unwrap()];
    let options = [
        ("verify-each", "true"),
        ("code-object-version", "6"),
        ("strip-debug", "true"),
        ("opt-level", "2"),
    ]
    .into_iter()
    .map(|(name, value)| LinkOptionV1::new(name, value).unwrap())
    .collect();
    preflight(
        token,
        receipt,
        closure,
        worker,
        providers,
        options,
        WorkerOutputConstraintsV1::new(OUTPUT_BOUND).unwrap(),
        WorkerExecutionLimitsV1::new(Duration::from_secs(5), 16 * 1024, 1024).unwrap(),
        budget,
    )
}

pub(crate) fn closure(owner: &Recovered) -> CompilerClosureV2 {
    *owner.handoff().capsule().base().compiler_closure()
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Backing {
    handoff: usize,
    llvm: usize,
    final_f: usize,
    source_catalog: usize,
    signed_roots: [usize; 2],
}
pub(crate) fn backing(owner: &Recovered, erased: bool) -> Backing {
    let (source_catalog, signed_roots) = match owner.recovered().source_proof() {
        Original::Direct(proof) => {
            assert!(!erased);
            assert_eq!(proof.root_count(), 2);
            assert!(!proof.authenticates_compiler_or_launch_origin());
            (
                proof.source().catalog().canonical_bytes().as_ptr() as usize,
                [0, 1].map(|i| proof.signed_ranked_proof(i).unwrap() as *const _ as usize),
            )
        }
        Original::Erased(proof) => {
            assert!(erased);
            assert_eq!(proof.root_count(), 2);
            assert!(!proof.authenticates_compiler_or_launch_origin());
            (
                proof.source().catalog().canonical_bytes().as_ptr() as usize,
                [0, 1].map(|i| proof.signed_ranked_proof(i).unwrap() as *const _ as usize),
            )
        }
    };
    Backing {
        handoff: owner.handoff().canonical_bytes().as_ptr() as usize,
        llvm: owner.handoff().module_handoff().module_bytes().as_ptr() as usize,
        final_f: owner
            .recovered()
            .output()
            .canonical()
            .canonical_bytes()
            .as_ptr() as usize,
        source_catalog,
        signed_roots,
    }
}

fn resource_error(error: &NativeFirstBuildWorkerErrorV1) -> Resource {
    match error {
        NativeFirstBuildWorkerErrorV1::Resource(resource)
        | NativeFirstBuildWorkerErrorV1::Binding(
            ProtectedCompilerNativeHandoffBindingErrorV1::Resource(resource),
        ) => *resource,
        other => panic!("expected a resource refusal before Worker execution, got {other:?}"),
    }
}

#[test]
#[ignore = "requires exported FE2O3_NATIVE_WORKER_FIXTURE_DIR and measured CPU fixture Worker"]
fn native_worker_retains_original_source_and_f_through_exact_replay() {
    let fixtures = Fixtures::open();
    let worker = pinned(LLVM_ID);
    let mut identities = Vec::new();
    for (i, (erased, profile, name)) in CASES.into_iter().enumerate() {
        let mut work = Work::new(WORK_LIMIT);
        let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
        let ledger = budget.work_ledger_identity_v1();
        let ready = Ready::publish(&fixtures, name, 10 + i as u8, &mut budget);
        let token = ready.recover(&mut budget);
        let original = backing(token.content(), erased);
        let actual_f = *token.content().recovered().output().canonical().identity();
        let expected_closure = closure(token.content());
        let floor = budget.storage();
        let before = budget.work();
        let (prepared, storage) = prepare(
            &token,
            ready.receipt,
            expected_closure,
            &worker,
            &mut budget,
        )
        .expect("native Worker preflight must succeed for each signed structural fixture");
        assert_eq!(budget.storage(), floor);
        assert!(budget.work() > before);
        assert_eq!(prepared.storage(), storage);
        assert_eq!(prepared.binding().receipt(), ready.receipt);
        assert_eq!(prepared.worker_measurement(), worker.measurement());
        assert!(storage.retained_storage() > 0);
        budget.reserve_storage(storage.retained_storage()).unwrap();
        assert_eq!(backing(token.content(), erased), original);
        let consumed = ready.consume(token, &mut budget);
        assert_eq!(backing(consumed.content(), erased), original);
        let floor = budget.storage();
        let before = budget.work();
        let (evidence, storage) = execute(consumed, prepared, &worker, &mut budget).unwrap();
        assert_eq!(
            budget.storage(),
            floor,
            "token and preflight floors must remain paid"
        );
        assert!(budget.work() > before);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(evidence.storage(), storage);
        assert!(storage.retained_storage() > 0);
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let owner = evidence.recovered_handoff();
        assert_eq!(backing(owner, erased), original);
        assert_eq!(evidence.binding().receipt(), ready.receipt);
        assert_eq!(evidence.binding().compiler_closure(), expected_closure);
        assert_eq!(evidence.binding().actual_f_identity(), actual_f);
        assert_eq!(
            evidence.binding().carrier_identity(),
            owner.handoff().capsule().carrier_identity()
        );
        assert_eq!(
            owner.handoff().module_handoff().target().to_string(),
            profile.device_target()
        );
        assert_eq!(evidence.worker_measurement(), worker.measurement());
        assert_eq!(evidence.output_bytes(), OUTPUT);
        assert_eq!(
            evidence.output_identity(),
            ContentIdentityV1::calculate(OUTPUT)
        );
        assert!(!owner.authenticates_execution());
        assert!(!owner.authenticates_rustc_abi());
        assert!(!owner.grants_artifact_or_launch_authority());
        assert!(!evidence.binding().authenticates_compiler_origin());
        assert!(!evidence.binding().grants_publication_authority());
        assert!(!evidence.binding().grants_launch_authority());
        let bootstrap = evidence.bootstrap_request_bytes();
        let replay = evidence.exact_replay_request_bytes();
        assert!(bootstrap.starts_with(b"F3LREQ02"));
        assert!(replay.starts_with(b"F3LREQ02"));
        assert_ne!(
            bootstrap, replay,
            "bootstrap bound and exact output replay must differ"
        );
        // Inspect completed transcripts through the public strict exchange
        // decoder; no test-only Worker request constructor or TLV parser.
        let bootstrap = InertDecodedWorkerExchangeV2::decode(
            bootstrap,
            evidence.bootstrap_response().canonical_bytes(),
        )
        .unwrap();
        let replay = InertDecodedWorkerExchangeV2::decode(
            replay,
            evidence.exact_replay_response().canonical_bytes(),
        )
        .unwrap();
        assert_eq!(
            bootstrap.request().output_constraints().max_bytes(),
            OUTPUT_BOUND
        );
        assert_eq!(
            replay.request().output_constraints().max_bytes(),
            OUTPUT.len() as u64
        );
        assert_ne!(
            bootstrap.request().request_id(),
            replay.request().request_id()
        );
        assert_ne!(bootstrap.request().identity(), replay.request().identity());
        assert_eq!(
            bootstrap.response().derivation(),
            replay.response().derivation()
        );
        for exchange in [&bootstrap, &replay] {
            let request = exchange.request();
            let response = exchange.response();
            assert_eq!(
                request.compiler_module().bytes(),
                owner.handoff().module_handoff().module_bytes()
            );
            assert_eq!(request.target(), owner.handoff().module_handoff().target());
            assert_eq!(
                request.worker_executable(),
                worker.measurement().executable()
            );
            assert_eq!(request.worker_build_identity(), WORKER_ID);
            assert_eq!(request.llvm_build_identity(), LLVM_ID);
            assert_eq!(request.external_providers().len(), 1);
            assert_eq!(request.external_providers()[0].bytes(), PROVIDER);
            assert!(response.binds_request(request));
            assert_eq!(response.output().unwrap().bytes(), OUTPUT);
            assert_eq!(
                response.derivation().unwrap().hsaco(),
                evidence.output_identity()
            );
            assert!(!exchange.grants_worker_execution_authority());
            assert!(!exchange.grants_publication_authority());
            assert!(!exchange.grants_launch_authority());
        }
        let identity = *evidence.identity().as_bytes();
        assert_ne!(identity, [0; 32]);
        assert!(!identities.contains(&identity));
        identities.push(identity);
        assert!(matches!(
            transaction::recover_compiler_module_handoff_receipt_v4(
                &ready.directory.0,
                &ready.producer,
                ready.attempt,
                &mut budget,
            ),
            Err(transaction::CompilerModuleHandoffErrorV4::Coordination(
                transaction::CompilerModuleHandoffErrorV1::AlreadyConsumed
            ))
        ));
    }
}

#[test]
#[ignore = "requires exported FE2O3_NATIVE_WORKER_FIXTURE_DIR and measured CPU fixture Worker"]
fn native_worker_rejects_foreign_receipt_and_closure_before_consume() {
    let fixtures = Fixtures::open();
    let worker = pinned(LLVM_ID);
    let mut work = Work::new(WORK_LIMIT);
    let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
    let ready = Ready::publish(&fixtures, CASES[0].2, 30, &mut budget);
    let foreign = Ready::publish(&fixtures, CASES[0].2, 31, &mut budget);
    assert_eq!(
        ready.receipt.handoff_identity(),
        foreign.receipt.handoff_identity()
    );
    assert_ne!(
        ready.receipt.transaction_identity(),
        foreign.receipt.transaction_identity()
    );
    let token = ready.recover(&mut budget);
    let wrong_closure =
        CompilerClosureV2::new([11; 32], [12; 32], [13; 32], [14; 32], [15; 32], [16; 32]).unwrap();
    for (receipt, expected_closure, field) in [
        (
            foreign.receipt,
            closure(token.content()),
            "locked V4 receipt",
        ),
        (ready.receipt, wrong_closure, "expected compiler closure"),
    ] {
        let floor = budget.storage();
        let before = budget.work();
        let error = prepare(&token, receipt, expected_closure, &worker, &mut budget)
            .err()
            .expect("foreign binding must fail before consume");
        assert!(
            matches!(
                error,
                NativeFirstBuildWorkerErrorV1::Binding(
                    ProtectedCompilerNativeHandoffBindingErrorV1::RelationshipMismatch { field: actual }
                ) | NativeFirstBuildWorkerErrorV1::PreflightMismatch(actual) if actual == field
            ),
            "{error:?}"
        );
        assert_eq!(budget.storage(), floor);
        assert!(budget.work() > before);
        ready.lease.validate_current_token(&token).unwrap();
        token.revalidate_locked_currentness(&mut budget).unwrap();
    }
    drop_token(token, &mut budget);
    ready.assert_ready(&mut budget);
    foreign.assert_ready(&mut budget);
}

#[test]
#[ignore = "requires exported FE2O3_NATIVE_WORKER_FIXTURE_DIR and measured CPU fixture Worker"]
fn native_worker_preflight_rejects_cumulative_storage_without_consuming() {
    let fixtures = Fixtures::open();
    let worker = pinned(LLVM_ID);
    let mut work = Work::new(WORK_LIMIT);
    let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
    let ready = Ready::publish(&fixtures, CASES[2].2, 40, &mut budget);
    let token = ready.recover(&mut budget);
    let ledger = budget.work_ledger_identity_v1();
    let floor = budget.storage();
    // Model unrelated live caller storage on the ORIGINAL admission ledger.
    let unrelated = STORAGE_LIMIT - floor;
    budget.reserve_storage(unrelated).unwrap();
    let before = budget.work();
    let error = prepare(
        &token,
        ready.receipt,
        closure(token.content()),
        &worker,
        &mut budget,
    )
    .err()
    .expect("preflight may not reset the cumulative storage budget");
    assert!(
        matches!(resource_error(&error), Resource::Storage(_)),
        "{error:?}"
    );
    assert_eq!(budget.storage(), STORAGE_LIMIT);
    assert!(budget.work() > before);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert!(budget.failed_storage().unwrap() > STORAGE_LIMIT);
    budget.release_storage(unrelated).unwrap();
    assert_eq!(budget.storage(), floor);
    drop_token(token, &mut budget);
    ready.assert_ready(&mut budget);
}

#[test]
#[ignore = "requires exported FE2O3_NATIVE_WORKER_FIXTURE_DIR and measured CPU fixture Worker"]
fn native_worker_execution_rejects_exhausted_original_work_budget() {
    let fixtures = Fixtures::open();
    let worker = pinned(LLVM_ID);
    let mut work = Work::new(WORK_LIMIT);
    let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
    let ready = Ready::publish(&fixtures, CASES[3].2, 50, &mut budget);
    let token = ready.recover(&mut budget);
    let (prepared, storage) = prepare(
        &token,
        ready.receipt,
        closure(token.content()),
        &worker,
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let consumed = ready.consume(token, &mut budget);
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    budget.charge_work(WORK_LIMIT - budget.work()).unwrap();
    let error = execute(consumed, prepared, &worker, &mut budget)
        .err()
        .expect("execution must charge the original cumulative work ledger before spawn");
    assert!(
        matches!(resource_error(&error), Resource::Work(_)),
        "{error:?}"
    );
    assert_eq!(budget.work(), WORK_LIMIT);
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert!(work.failed_work().unwrap() > WORK_LIMIT);
}

#[test]
#[ignore = "requires exported FE2O3_NATIVE_WORKER_FIXTURE_DIR and measured CPU fixture Worker"]
fn native_worker_rejects_cross_preflight_worker_and_occurrence_before_spawn() {
    let fixtures = Fixtures::open();
    let worker = pinned(LLVM_ID);
    // Same executable bytes, different LLVM measurement: the adapter must reject
    // the sealed preflight before calling the underlying process executor.
    let foreign_worker = pinned("foreign-fixture-llvm-v1");
    for wrong_occurrence in [false, true] {
        let mut work = Work::new(WORK_LIMIT);
        let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
        let ready = Ready::publish(&fixtures, CASES[0].2, 60, &mut budget);
        let token = ready.recover(&mut budget);
        let (prepared, storage) = prepare(
            &token,
            ready.receipt,
            closure(token.content()),
            &worker,
            &mut budget,
        )
        .unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        if wrong_occurrence {
            // Equal content is deliberately insufficient: a second real ready
            // transaction supplies its own locked, semantically verified owner.
            let foreign = Ready::publish(&fixtures, CASES[0].2, 61, &mut budget);
            assert_eq!(
                ready.receipt.handoff_identity(),
                foreign.receipt.handoff_identity()
            );
            assert_ne!(
                ready.receipt.transaction_identity(),
                foreign.receipt.transaction_identity()
            );
            let other_token = foreign.recover(&mut budget);
            assert_ne!(
                token.handoff().canonical_bytes().as_ptr(),
                other_token.handoff().canonical_bytes().as_ptr()
            );
            let consumed = foreign.consume(other_token, &mut budget);
            let floor = budget.storage();
            let ledger = budget.work_ledger_identity_v1();
            let error = execute(consumed, prepared, &worker, &mut budget)
                .err()
                .expect("foreign consumed occurrence must not reach the Worker");
            assert!(
                matches!(
                    error,
                    NativeFirstBuildWorkerErrorV1::Binding(
                        ProtectedCompilerNativeHandoffBindingErrorV1::RelationshipMismatch {
                            field: "consumed V4 receipt"
                        }
                    )
                ),
                "{error:?}"
            );
            assert_eq!(budget.storage(), floor);
            assert!(budget.work_ledger_identity_v1() == ledger);
            drop_token(token, &mut budget);
            ready.assert_ready(&mut budget);
        } else {
            let consumed = ready.consume(token, &mut budget);
            let floor = budget.storage();
            let ledger = budget.work_ledger_identity_v1();
            let error = execute(consumed, prepared, &foreign_worker, &mut budget)
                .err()
                .expect("foreign measurement must fail before Worker spawn");
            assert!(
                matches!(
                    error,
                    NativeFirstBuildWorkerErrorV1::PreflightMismatch("measured worker")
                ),
                "{error:?}"
            );
            assert_eq!(budget.storage(), floor);
            assert!(budget.work_ledger_identity_v1() == ledger);
        }
    }
}
