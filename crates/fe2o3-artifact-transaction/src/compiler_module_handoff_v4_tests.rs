//! Content-only fixtures. No compiler execution or launch authority is claimed.
use super::*;
use crate::{BuildInvocation, BuildSession, CompilerModuleHandoffErrorV1, begin_build_attempt};
use fe2o3_compiler_ffi::*;
use fe2o3_compiler_lineage::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::os::unix::fs::PermissionsExt;
use std::panic::{AssertUnwindSafe, catch_unwind};

const LIMIT: usize = MAX_COMPILER_MODULE_HANDOFF_STORAGE_V4;

struct Fixture {
    path: PathBuf,
    producer: ProducerIdentity,
    attempt: BuildAttempt,
    handoff: Handoff,
}
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let path = std::env::temp_dir().join(format!(
            "fe2o3-native-v4-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        let producer =
            ProducerIdentity::from_codegen("native", Some(Path::new("/native.rs"))).unwrap();
        let attempt = begin_build_attempt(
            &path,
            &producer,
            BuildInvocation::from_bytes([3; 32]),
            BuildSession::from_bytes([4; 16]),
        )
        .unwrap();
        Self {
            path,
            producer,
            attempt,
            handoff: outer(7),
        }
    }
    fn publish(&self, budget: &mut Budget<'_>) -> Result<CompilerModuleHandoffReceiptV4> {
        publish_compiler_module_handoff_v4(
            &self.path,
            &self.producer,
            self.attempt,
            &self.handoff,
            budget,
        )
    }
    fn recover(&self, budget: &mut Budget<'_>) -> Result<CompilerModuleHandoffReceiptV4> {
        recover_compiler_module_handoff_receipt_v4(&self.path, &self.producer, self.attempt, budget)
    }
    fn lease(
        &self,
        receipt: CompilerModuleHandoffReceiptV4,
        budget: &mut Budget<'_>,
    ) -> CompilerModuleHandoffCurrentnessLeaseV4 {
        let (lease, storage) = acquire_compiler_module_handoff_currentness_lease_v4(
            &self.path,
            &self.producer,
            receipt,
            budget,
        )
        .unwrap();
        budget.reserve_storage(storage.0).unwrap();
        lease
    }
    fn slot(&self) -> PathBuf {
        let producer = producer_identity_for::<Schema>(&self.producer);
        let slot = slot_identity_for::<Schema>(
            producer,
            self.attempt,
            CompilerModuleHandoffSlotV4::Production,
        );
        self.path
            .join(format!("{}{}", Schema::PARENT_PREFIX, hex(&producer)))
            .join(format!("{}{}", Schema::SLOT_PREFIX, hex(&slot)))
    }
    fn reserve(&self, budget: &mut Budget<'_>) -> usize {
        let floor = payload_storage(&self.handoff).unwrap();
        budget.reserve_storage(floor).unwrap();
        floor
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.path).unwrap();
    }
}

pub(crate) fn outer(seed: u8) -> Handoff {
    outer_with_carrier(seed, seed)
}

pub(crate) fn outer_with_carrier(seed: u8, payload_seed: u8) -> Handoff {
    let legacy = super::super::semantic_v3::tests::outer(seed);
    wrap(&legacy, payload_seed)
}

pub(crate) fn wrap(legacy: &InertSemanticCompilerModuleHandoffV3, payload_seed: u8) -> Handoff {
    let base = legacy.capsule();
    let pair = NativeRefinedForwardingCarrierLayoutV1::new::<()>(19, 64).unwrap();
    let inner = InertProductionSemanticCapsuleLayoutV4::new::<()>(
        base.canonical_bytes().len(),
        pair.encoded_len(),
    )
    .unwrap();
    let mut bytes = vec![payload_seed; inner.encoded_len()];
    bytes[inner.base_range()].copy_from_slice(base.canonical_bytes());
    seal_native_refined_forwarding_carrier_v1(
        pair,
        &mut bytes[inner.carrier_range()],
        LIMIT,
        |_| Ok::<_, ()>(()),
    )
    .unwrap();
    seal_inert_production_semantic_capsule_v4(inner, &mut bytes, LIMIT, |_| Ok::<_, ()>(()))
        .unwrap();
    let capsule = InertProductionSemanticCapsuleV4::decode_owned(bytes).unwrap();
    let module = legacy.module_handoff();
    let layout = InertSemanticCompilerModuleHandoffLayoutV4::new(
        capsule.canonical_bytes().len(),
        module.canonical_bytes().len(),
    )
    .unwrap();
    let mut bytes = vec![0; layout.encoded_len()];
    bytes[layout.capsule_range()].copy_from_slice(capsule.canonical_bytes());
    bytes[layout.module_handoff_range()].copy_from_slice(module.canonical_bytes());
    seal_inert_semantic_compiler_module_handoff_v4(
        layout,
        &mut bytes,
        capsule.identity(),
        module.identity(),
        |_| Ok::<_, ()>(()),
    )
    .unwrap();
    Handoff::decode_owned(bytes).unwrap()
}

fn token(
    lease: &CompilerModuleHandoffCurrentnessLeaseV4,
    budget: &mut Budget<'_>,
) -> CompilerModuleHandoffConsumptionTokenV4 {
    let (token, storage) = lease.acquire_current_token(budget).unwrap();
    budget.reserve_storage(storage.0).unwrap();
    token
}

#[test]
fn native_transaction_v4_roundtrip_retains_backing_lock_and_consumes_once() {
    use crate::{CompilerExecutionSubjectErrorV2, InertCompilerExecutionSubjectV2 as Subject};
    let f = Fixture::new();
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, LIMIT);
    let floor = f.reserve(&mut budget);
    let receipt = f.publish(&mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
    let (published, storage) = Subject::from_publication(receipt, &f.handoff, &mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let recovered = f.recover(&mut budget).unwrap();
    assert_eq!(recovered, receipt);
    let (subject, storage) = Subject::from_publication(recovered, &f.handoff, &mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    assert_eq!(subject, published);
    drop(subject);
    budget.release_storage(storage.retained_storage()).unwrap();
    let mut wrong_length = receipt;
    wrong_length.length += 1;
    let checkpoint = budget.storage();
    assert!(matches!(
        Subject::from_publication(wrong_length, &f.handoff, &mut budget),
        Err(CompilerExecutionSubjectErrorV2::HandoffLengthMismatch)
    ));
    assert_eq!(budget.storage(), checkpoint);
    let wrong = outer_with_carrier(7, 29);
    let wrong_storage = payload_storage(&wrong).unwrap();
    budget.reserve_storage(wrong_storage).unwrap();
    let checkpoint = budget.storage();
    assert!(matches!(
        Subject::from_publication(receipt, &wrong, &mut budget),
        Err(CompilerExecutionSubjectErrorV2::HandoffIdentityMismatch)
    ));
    assert_eq!(budget.storage(), checkpoint);
    drop(wrong);
    budget.release_storage(wrong_storage).unwrap();
    let lease = f.lease(receipt, &mut budget);
    lease.revalidate(&mut budget).unwrap();
    let token = token(&lease, &mut budget);
    assert!(matches!(
        lease.acquire_current_token(&mut budget),
        Err(Error::Busy)
    ));
    let pointer = token.handoff().canonical_bytes().as_ptr();
    let before = budget.storage();
    let (mapped, additional) = token
        .try_map_handoff(&mut budget, |h, _| Ok::<_, ()>((h, 0)))
        .unwrap();
    assert_eq!(budget.storage(), before);
    budget.reserve_storage(additional.0).unwrap();
    assert_eq!(mapped.handoff().canonical_bytes().as_ptr(), pointer);
    mapped.revalidate_locked_currentness(&mut budget).unwrap();
    assert!(matches!(
        lease.acquire_current_token(&mut budget),
        Err(Error::Busy)
    ));
    let before = budget.storage();
    let consumed =
        consume_compiler_module_handoff_with_currentness_v4(&lease, mapped, &mut budget).unwrap();
    assert_eq!(budget.storage(), before);
    assert_eq!(consumed.receipt(), receipt);
    assert_eq!(consumed.handoff().canonical_bytes().as_ptr(), pointer);
    assert_eq!(consumed.bytes(), f.handoff.canonical_bytes());
    let (subject, storage) = Subject::from_consumed(&consumed, &mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    assert_eq!(subject, published);
    assert!(!consumed.grants_compiler_authority());
    assert!(!consumed.grants_launch_authority());
    assert!(matches!(
        f.recover(&mut budget),
        Err(Error::Coordination(
            CompilerModuleHandoffErrorV1::AlreadyConsumed
        ))
    ));
    assert!(f.slot().join(CONSUMED_ENTRY).exists());
    assert!(!f.slot().join(READY_ENTRY).exists());
}

#[test]
fn native_transaction_v4_admission_refusal_unwind_and_owner_substitution_leave_ready() {
    for mode in 0..6 {
        let f = Fixture::new();
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, LIMIT);
        f.reserve(&mut budget);
        let receipt = f.publish(&mut budget).unwrap();
        let lease = f.lease(receipt, &mut budget);
        let token = token(&lease, &mut budget);
        let before = budget.storage();
        let result = catch_unwind(AssertUnwindSafe(|| {
            token.try_map_handoff(&mut budget, |h, b| {
                b.charge_work(17).unwrap();
                match mode {
                    0 => Err("rejected"),
                    1 => {
                        b.reserve_storage(37).unwrap();
                        panic!("admission unwind");
                    }
                    2 => Ok((
                        Handoff::decode_owned(h.canonical_bytes().to_vec()).unwrap(),
                        0,
                    )),
                    3 => {
                        b.reserve_storage(37).unwrap();
                        Ok((h, 0))
                    }
                    4 => {
                        b.reserve_storage(1).unwrap();
                        Err("bad error checkpoint")
                    }
                    _ => {
                        b.release_storage(1).unwrap();
                        Err("bad error checkpoint")
                    }
                }
            })
        }));
        match mode {
            0 => assert!(matches!(
                result.unwrap(),
                Err(CompilerModuleHandoffAdmissionErrorV4::Admission("rejected"))
            )),
            1 => assert!(result.is_err()),
            2 => assert!(matches!(
                result.unwrap(),
                Err(CompilerModuleHandoffAdmissionErrorV4::Transaction(
                    Error::HandoffIdentityMismatch
                ))
            )),
            _ => assert!(matches!(
                result.unwrap(),
                Err(CompilerModuleHandoffAdmissionErrorV4::Transaction(
                    Error::Resource(Resource::Accounting)
                ))
            )),
        }
        assert_eq!(budget.storage(), before);
        assert!(f.slot().join(READY_ENTRY).exists());
        assert!(!f.slot().join(CONSUMED_ENTRY).exists());
        assert_eq!(f.recover(&mut budget).unwrap(), receipt);
        drop(lease.acquire_current_token(&mut budget).unwrap());
    }
}

struct TrackedOwner {
    handoff: Handoff,
    reads: std::rc::Rc<std::cell::Cell<usize>>,
    drops: std::rc::Rc<std::cell::Cell<usize>>,
}
impl AsRef<Handoff> for TrackedOwner {
    fn as_ref(&self) -> &Handoff {
        self.reads.set(self.reads.get() + 1);
        &self.handoff
    }
}
impl Drop for TrackedOwner {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}

#[test]
fn native_transaction_v4_mapping_prepays_headers_and_owner_before_inspection() {
    for stop_before_callback in [true, false] {
        let f = Fixture::new();
        let mut setup_work = Work::new(usize::MAX);
        let mut setup = Budget::new(&mut setup_work, LIMIT);
        f.reserve(&mut setup);
        let receipt = f.publish(&mut setup).unwrap();
        let lease = f.lease(receipt, &mut setup);
        let token = token(&lease, &mut setup);
        let floor = setup.storage();
        let headers = token_headers::<TrackedOwner>();
        let semantic = 4096;
        let cap =
            floor + FRAME_STORAGE + headers + if stop_before_callback { 0 } else { semantic } - 1;
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, cap);
        budget.reserve_storage(floor).unwrap();
        let reads = std::rc::Rc::new(std::cell::Cell::new(0));
        let drops = std::rc::Rc::new(std::cell::Cell::new(0));
        let mut called = false;
        let result = token.try_map_handoff(&mut budget, |handoff, _| {
            called = true;
            Ok::<_, ()>((
                TrackedOwner {
                    handoff,
                    reads: reads.clone(),
                    drops: drops.clone(),
                },
                semantic,
            ))
        });
        assert!(matches!(
            result,
            Err(CompilerModuleHandoffAdmissionErrorV4::Transaction(
                Error::Resource(Resource::Storage(_))
            ))
        ));
        assert_eq!(called, !stop_before_callback);
        assert_eq!(reads.get(), 0);
        assert_eq!(drops.get(), usize::from(!stop_before_callback));
        assert_eq!(budget.storage(), floor);
        assert!(f.slot().join(READY_ENTRY).exists());
        assert!(!f.slot().join(CONSUMED_ENTRY).exists());
        assert_eq!(f.recover(&mut setup).unwrap(), receipt);
    }
}

#[test]
fn native_transaction_v4_mapping_exact_and_one_short_resources() {
    let run = |work_limit: usize, extra: usize| {
        let f = Fixture::new();
        let mut setup_work = Work::new(usize::MAX);
        let mut setup = Budget::new(&mut setup_work, LIMIT);
        f.reserve(&mut setup);
        let receipt = f.publish(&mut setup).unwrap();
        let lease = f.lease(receipt, &mut setup);
        let token = token(&lease, &mut setup);
        let old = token.storage();
        let floor = setup.storage();
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, (floor + extra).min(LIMIT));
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(11).unwrap();
        let result = token.try_map_handoff(&mut budget, |h, b| {
            b.charge_work(37)?;
            Ok::<_, Resource>((h, 4096))
        });
        assert_eq!(budget.storage(), floor);
        let used = budget.work();
        let peak = budget.peak_storage() - floor;
        let result = result.map(|(mapped, additional)| {
            assert_eq!(mapped.storage().0, old.0 + additional.0);
            assert_eq!(additional.0, 4096 + token_headers::<Handoff>());
            setup.reserve_storage(additional.0).unwrap();
            let consumed =
                consume_compiler_module_handoff_with_currentness_v4(&lease, mapped, &mut setup)
                    .unwrap();
            assert_eq!(consumed.storage().0, old.0 + additional.0);
        });
        if result.is_err() {
            assert!(f.slot().join(READY_ENTRY).exists());
            assert!(!f.slot().join(CONSUMED_ENTRY).exists());
        }
        (result, used, peak)
    };
    let (result, work, storage) = run(usize::MAX, LIMIT);
    result.unwrap();
    run(work, storage).0.unwrap();
    assert!(matches!(
        run(work - 1, storage).0,
        Err(CompilerModuleHandoffAdmissionErrorV4::Transaction(
            Error::Resource(Resource::Work(_))
        ))
    ));
    assert!(matches!(
        run(work, storage - 1).0,
        Err(CompilerModuleHandoffAdmissionErrorV4::Transaction(
            Error::Resource(Resource::Storage(_))
        ))
    ));
}

#[test]
fn native_transaction_v4_version_namespaces_and_sidecars_do_not_fallback() {
    let f = Fixture::new();
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, LIMIT);
    f.reserve(&mut budget);
    let legacy = super::super::semantic_v3::tests::outer(7);
    super::super::semantic_v3::publish_compiler_module_handoff_v3(
        &f.path,
        &f.producer,
        f.attempt,
        &legacy,
    )
    .unwrap();
    assert!(matches!(
        f.recover(&mut budget),
        Err(Error::Coordination(
            CompilerModuleHandoffErrorV1::NotPublished
        ))
    ));
    let receipt = f.publish(&mut budget).unwrap();
    assert_eq!(f.recover(&mut budget).unwrap(), receipt);
    fs::write(
        f.slot().join("compiler-execution-receipt-v1"),
        b"not a V4 sidecar",
    )
    .unwrap();
    fs::set_permissions(
        f.slot().join("compiler-execution-receipt-v1"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    assert!(f.recover(&mut budget).is_err());
    assert!(f.slot().join(READY_ENTRY).exists());
}

#[test]
fn native_transaction_v4_rejects_stale_mismatched_and_changed_custody() {
    for mode in 0..4 {
        let f = Fixture::new();
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, LIMIT);
        f.reserve(&mut budget);
        let receipt = f.publish(&mut budget).unwrap();
        let lease = f.lease(receipt, &mut budget);
        let other = f.lease(receipt, &mut budget);
        let token = token(&lease, &mut budget);
        match mode {
            0 => {
                assert!(matches!(
                    consume_compiler_module_handoff_with_currentness_v4(&other, token, &mut budget),
                    Err(Error::MismatchedCurrentnessToken)
                ));
            }
            1 | 2 => {
                let entry = if mode == 1 {
                    READY_ENTRY
                } else {
                    PAYLOAD_ENTRY
                };
                let path = f.slot().join(entry);
                let bytes = fs::read(&path).unwrap();
                fs::remove_file(&path).unwrap();
                fs::write(&path, bytes).unwrap();
                fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
                assert!(
                    consume_compiler_module_handoff_with_currentness_v4(&lease, token, &mut budget)
                        .is_err()
                );
            }
            _ => {
                drop(token);
                begin_build_attempt(
                    &f.path,
                    &f.producer,
                    BuildInvocation::from_bytes([9; 32]),
                    BuildSession::from_bytes([8; 16]),
                )
                .unwrap();
                assert!(lease.revalidate(&mut budget).is_err());
            }
        }
        assert!(!f.slot().join(CONSUMED_ENTRY).exists());
    }
}

struct FailAt(FaultPoint);

#[test]
fn native_transaction_v4_rechecks_a_generic_owners_backing_before_commit() {
    struct Switching {
        original: Handoff,
        replacement: Handoff,
        first: std::cell::Cell<bool>,
    }
    impl AsRef<Handoff> for Switching {
        fn as_ref(&self) -> &Handoff {
            if self.first.replace(false) {
                &self.original
            } else {
                &self.replacement
            }
        }
    }
    let f = Fixture::new();
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, LIMIT);
    f.reserve(&mut budget);
    let replacement = outer(7);
    budget
        .reserve_storage(payload_storage(&replacement).unwrap())
        .unwrap();
    let receipt = f.publish(&mut budget).unwrap();
    let lease = f.lease(receipt, &mut budget);
    let token = token(&lease, &mut budget);
    let (mapped, additional) = token
        .try_map_handoff(&mut budget, |original, budget| {
            assert!(matches!(
                lease.acquire_current_token(budget),
                Err(Error::Busy)
            ));
            Ok::<_, ()>((
                Switching {
                    original,
                    replacement,
                    first: std::cell::Cell::new(true),
                },
                0,
            ))
        })
        .unwrap();
    budget.reserve_storage(additional.0).unwrap();
    assert!(matches!(
        consume_compiler_module_handoff_with_currentness_v4(&lease, mapped, &mut budget),
        Err(Error::HandoffIdentityMismatch)
    ));
    assert_eq!(f.recover(&mut budget).unwrap(), receipt);
    assert!(!f.slot().join(CONSUMED_ENTRY).exists());
}

impl HandoffHooks for FailAt {
    fn hit(&mut self, point: FaultPoint) -> std::io::Result<()> {
        if point == self.0 {
            Err(std::io::Error::other("injected fault"))
        } else {
            Ok(())
        }
    }
}

#[test]
fn native_transaction_v4_faults_preserve_one_shot_commit_boundary() {
    for point in [
        FaultPoint::PayloadValidated,
        FaultPoint::ConsumedRenamed,
        FaultPoint::ConsumedSynced,
    ] {
        let f = Fixture::new();
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, LIMIT);
        f.reserve(&mut budget);
        let receipt = f.publish(&mut budget).unwrap();
        let lease = f.lease(receipt, &mut budget);
        let token = token(&lease, &mut budget);
        let floor = budget.storage();
        assert!(consume(&lease, token, &mut budget, &mut FailAt(point)).is_err());
        assert_eq!(budget.storage(), floor);
        if point == FaultPoint::PayloadValidated {
            assert_eq!(f.recover(&mut budget).unwrap(), receipt);
            let retry = self::token(&lease, &mut budget);
            consume_compiler_module_handoff_with_currentness_v4(&lease, retry, &mut budget)
                .unwrap();
        } else {
            assert!(matches!(
                f.recover(&mut budget),
                Err(Error::Coordination(
                    CompilerModuleHandoffErrorV1::AlreadyConsumed
                ))
            ));
        }
    }
}

#[test]
fn native_transaction_v4_full_backing_capacity_is_an_inherited_floor() {
    let mut f = Fixture::new();
    let wire = f.handoff.canonical_bytes();
    let n = wire.len();
    let mut backing = Vec::with_capacity(n + 8192);
    backing.extend([0; 37]);
    backing.extend_from_slice(wire);
    backing.extend([0; 53]);
    f.handoff = Handoff::decode_shared_vec(Arc::new(backing), 37..37 + n).unwrap();
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, LIMIT);
    let floor = payload_storage(&f.handoff).unwrap();
    budget.reserve_storage(floor - 1).unwrap();
    assert!(matches!(
        f.publish(&mut budget),
        Err(Error::Resource(Resource::Accounting))
    ));
    assert!(!f.slot().exists());
    budget.reserve_storage(1).unwrap();
    f.publish(&mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn native_transaction_v4_valid_transaction_digest_cannot_replace_strict_decode() {
    let f = Fixture::new();
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, LIMIT);
    f.reserve(&mut budget);
    let mut malformed = f.handoff.canonical_bytes().to_vec();
    malformed[0] ^= 1;
    budget.reserve_storage(malformed.capacity()).unwrap();
    let fields = entry(&mut budget, malformed.capacity(), |resources| {
        Ok(publish_in_slot_engine::<Schema>(
            &f.path,
            &f.producer,
            f.attempt,
            CompilerModuleHandoffSlotV4::Production,
            f.handoff.identity().into(),
            &malformed,
            &mut NoFaults,
            resources,
        )?)
    })
    .unwrap();
    let receipt = <Schema as currentness::Schema>::receipt(fields, &f.handoff);
    let lease = f.lease(receipt, &mut budget);
    assert!(matches!(
        lease.acquire_current_token(&mut budget),
        Err(Error::NonCanonicalHandoff(_))
    ));
    assert!(matches!(
        f.recover(&mut budget),
        Err(Error::NonCanonicalHandoff(_))
    ));
    assert!(f.slot().join(READY_ENTRY).exists());
    assert!(!f.slot().join(CONSUMED_ENTRY).exists());
}

#[test]
fn native_transaction_v4_exact_and_one_short_pipeline_budgets() {
    let run = |work_limit, storage_limit| {
        let f = Fixture::new();
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        let initial = f.reserve(&mut budget);
        budget.charge_work(11).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        assert!(budget.reserve_storage(usize::MAX).is_err());
        assert!(budget.charge_work(usize::MAX).is_err());
        let result = (|| -> Result<()> {
            let receipt = f.publish(&mut budget)?;
            assert_eq!(budget.storage(), initial);
            assert_eq!(f.recover(&mut budget)?, receipt);
            let (lease, storage) = acquire_compiler_module_handoff_currentness_lease_v4(
                &f.path,
                &f.producer,
                receipt,
                &mut budget,
            )?;
            budget.reserve_storage(storage.0)?;
            let (token, storage) = lease.acquire_current_token(&mut budget)?;
            budget.reserve_storage(storage.0)?;
            let floor = budget.storage();
            consume_compiler_module_handoff_with_currentness_v4(&lease, token, &mut budget)?;
            assert_eq!(budget.storage(), floor);
            Ok(())
        })();
        assert_eq!(budget.failed_storage(), Some(usize::MAX));
        assert!(budget.work_ledger_identity_v1() == ledger);
        let measured = (result, budget.work(), budget.peak_storage());
        drop(budget);
        assert_eq!(work.failed_work(), Some(usize::MAX));
        measured
    };
    let (result, work, storage) = run(usize::MAX, LIMIT);
    result.unwrap();
    run(work, storage).0.unwrap();
    assert!(matches!(
        run(work - 1, storage).0,
        Err(Error::Resource(Resource::Work(_)))
    ));
    assert!(matches!(
        run(work, storage - 1).0,
        Err(Error::Resource(Resource::Storage(_)))
    ));
}
