//! Transaction/component tests only. No source/proof admission is synthesized.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::AtomicUsize;
const LIMIT: usize = MAX_COMPILER_MODULE_HANDOFF_STORAGE_V5;

#[path = "compiler_module_handoff_v5_fixture_tests.rs"]
pub(crate) mod fixture;
use fixture::{Fixture, token};

#[test]
fn conditional_transaction_v5_roundtrip_retains_backing_lock_and_consumes_once() {
    let f = Fixture::new();
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, LIMIT);
    let floor = f.reserve(&mut budget);
    let (publication, storage) = publish_compiler_module_handoff_with_currentness_v5(
        &f.path,
        &f.producer,
        f.attempt,
        &f.handoff,
        &mut budget,
    )
    .unwrap();
    assert_eq!(budget.storage(), floor);
    budget.reserve_storage(storage.0).unwrap();
    let (receipt, lease) = publication.into_parts();
    assert_eq!(f.recover(&mut budget).unwrap(), receipt);
    assert!(!receipt.grants_publication_authority());
    assert!(!receipt.grants_compiler_authority());
    let token = token(&lease, &mut budget);
    let original = backing_snapshot(token.handoff());
    let floor = budget.storage();
    let (mapped, extra) = token
        .try_map_handoff(&mut budget, |h, b| {
            assert!(matches!(lease.acquire_current_token(b), Err(Error::Busy)));
            b.charge_work(17).unwrap();
            Ok::<_, ()>((h, 37))
        })
        .unwrap();
    assert_eq!(budget.storage(), floor);
    budget.reserve_storage(extra.0).unwrap();
    assert_eq!(backing_snapshot(mapped.handoff()), original);
    mapped.revalidate_locked_currentness(&mut budget).unwrap();
    let floor = budget.storage();
    let consumed =
        consume_compiler_module_handoff_with_currentness_v5(&lease, mapped, &mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
    assert_eq!(consumed.receipt(), receipt);
    assert_eq!(backing_snapshot(consumed.handoff()), original);
    assert_eq!(consumed.bytes(), f.handoff.canonical_bytes());
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
fn conditional_transaction_v5_refusal_unwind_and_floor_damage_are_terminal() {
    for mode in 0..5 {
        let f = Fixture::new();
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, LIMIT);
        f.reserve(&mut budget);
        let receipt = f.publish(&mut budget).unwrap();
        let lease = f.lease(receipt, &mut budget);
        let token = token(&lease, &mut budget);
        let floor = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        let mut checkpoint = 0;
        let result = catch_unwind(AssertUnwindSafe(|| {
            token.try_map_handoff(&mut budget, |h, b| {
                checkpoint = b.storage();
                b.charge_work(17).unwrap();
                match mode {
                    0 => Err("refused"),
                    1 => {
                        b.reserve_storage(37).unwrap();
                        Err("refused")
                    }
                    2 => {
                        b.reserve_storage(37).unwrap();
                        panic!("opaque unwind")
                    }
                    3 => {
                        b.release_storage(1).unwrap();
                        Err("floor theft")
                    }
                    _ => {
                        b.reserve_storage(1).unwrap();
                        Ok((h, 0))
                    }
                }
            })
        }));
        assert!(checkpoint > floor);
        match &result {
            Ok(Err(e)) if mode < 2 => assert!(matches!(
                e.cause(),
                CompilerModuleHandoffAdmissionCauseV5::Admission("refused")
            )),
            Err(_) if mode == 2 => {}
            Ok(Err(e)) => assert!(matches!(
                e.cause(),
                CompilerModuleHandoffAdmissionCauseV5::Transaction(Error::Resource(
                    Resource::Accounting
                ))
            )),
            _ => panic!("unexpected map result: {result:?}"),
        }
        assert_eq!(
            budget.storage(),
            match mode {
                1 | 2 => checkpoint + 37,
                3 => checkpoint - 1,
                4 => checkpoint + 1,
                _ => checkpoint,
            }
        );
        assert!(budget.work_ledger_identity_v1() == ledger);
        f.ready();
        // Opaque E/panic payload destruction remains under the original lock.
        assert!(matches!(
            lease.acquire_current_token(&mut budget),
            Err(Error::Busy)
        ));
        drop(result);
        assert_eq!(f.recover(&mut budget).unwrap(), receipt);
        drop(lease.acquire_current_token(&mut budget).unwrap());
    }
}

struct TrackedOwner {
    handoff: Handoff,
    binding: Arc<currentness::Current<Schema>>,
    reads: Arc<AtomicUsize>,
    drops: Arc<AtomicUsize>,
}
impl fmt::Debug for TrackedOwner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("TrackedOwner")
    }
}
impl AsRef<Handoff> for TrackedOwner {
    fn as_ref(&self) -> &Handoff {
        self.reads.fetch_add(1, Ordering::Relaxed);
        &self.handoff
    }
}
impl Drop for TrackedOwner {
    fn drop(&mut self) {
        assert!(
            self.binding.output.try_lock().unwrap().is_none(),
            "owner/error destroyed after unlock"
        );
        self.drops.fetch_add(1, Ordering::Relaxed);
    }
}

#[test]
fn conditional_transaction_v5_opaque_error_and_panic_own_lock_until_destruction() {
    for panic in [false, true] {
        let f = Fixture::new();
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, LIMIT);
        f.reserve(&mut budget);
        let receipt = f.publish(&mut budget).unwrap();
        let lease = f.lease(receipt, &mut budget);
        let token = token(&lease, &mut budget);
        let reads = Arc::new(AtomicUsize::new(0));
        let drops = Arc::new(AtomicUsize::new(0));
        let before = budget.storage();
        let result = catch_unwind(AssertUnwindSafe(|| {
            token.try_map_handoff(&mut budget, |handoff, b| {
                b.reserve_storage(37).unwrap();
                let owner = TrackedOwner {
                    handoff,
                    binding: Arc::clone(&lease.binding),
                    reads: reads.clone(),
                    drops: drops.clone(),
                };
                if panic {
                    std::panic::panic_any(owner);
                }
                Err::<(Handoff, usize), _>(owner)
            })
        }));
        assert_eq!(drops.load(Ordering::Relaxed), 0);
        assert!(budget.storage() >= before + 37);
        assert!(matches!(
            lease.acquire_current_token(&mut budget),
            Err(Error::Busy)
        ));
        if !panic {
            let e = result.as_ref().unwrap().as_ref().unwrap_err();
            assert!(std::error::Error::source(e).is_none());
            assert!(matches!(
                e.cause(),
                CompilerModuleHandoffAdmissionCauseV5::Admission(_)
            ));
        }
        drop(result);
        assert_eq!(drops.load(Ordering::Relaxed), 1);
        assert_eq!(reads.load(Ordering::Relaxed), 0);
        f.ready();
        assert_eq!(f.recover(&mut budget).unwrap(), receipt);
    }
}

#[test]
fn conditional_transaction_v5_late_currentness_overrides_success_and_opaque_error() {
    for rejected in [false, true] {
        let f = Fixture::new();
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, LIMIT);
        f.reserve(&mut budget);
        let receipt = f.publish(&mut budget).unwrap();
        let lease = f.lease(receipt, &mut budget);
        let token = token(&lease, &mut budget);
        let reads = Arc::new(AtomicUsize::new(0));
        let drops = Arc::new(AtomicUsize::new(0));
        let before = budget.storage();
        let result = token.try_map_handoff(&mut budget, |handoff, b| {
            let file = fs::OpenOptions::new()
                .write(true)
                .open(f.slot().join(PAYLOAD_ENTRY))
                .unwrap();
            file.write_all_at(&[0xff], 0).unwrap();
            let owner = TrackedOwner {
                handoff,
                binding: Arc::clone(&lease.binding),
                reads: reads.clone(),
                drops: drops.clone(),
            };
            if rejected {
                b.reserve_storage(37).unwrap();
                Err(owner)
            } else {
                Ok((owner, 37))
            }
        });
        let error = result.unwrap_err();
        assert!(matches!(
            error.cause(),
            CompilerModuleHandoffAdmissionCauseV5::Transaction(Error::Coordination(_))
        ));
        assert_eq!(drops.load(Ordering::Relaxed), 1);
        assert_eq!(reads.load(Ordering::Relaxed), usize::from(!rejected));
        assert!(budget.storage() >= before + 37);
        f.ready();
        assert!(lease.binding.output.try_lock().unwrap().is_none());
        drop(error);
        assert!(lease.binding.output.try_lock().unwrap().is_some());
    }
}

#[test]
fn conditional_transaction_v5_owner_storage_is_paid_before_inspection() {
    let f = Fixture::new();
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, LIMIT);
    f.reserve(&mut budget);
    let receipt = f.publish(&mut budget).unwrap();
    let lease = f.lease(receipt, &mut budget);
    let token = token(&lease, &mut budget);
    let reads = Arc::new(AtomicUsize::new(0));
    let drops = Arc::new(AtomicUsize::new(0));
    let before = budget.storage();
    let error = token
        .try_map_handoff(&mut budget, |handoff, _| {
            Ok::<_, ()>((
                TrackedOwner {
                    handoff,
                    binding: Arc::clone(&lease.binding),
                    reads: reads.clone(),
                    drops: drops.clone(),
                },
                LIMIT,
            ))
        })
        .unwrap_err();
    assert!(matches!(
        error.cause(),
        CompilerModuleHandoffAdmissionCauseV5::Transaction(Error::Resource(Resource::Storage(_)))
    ));
    assert_eq!(reads.load(Ordering::Relaxed), 0);
    assert_eq!(drops.load(Ordering::Relaxed), 1);
    assert!(budget.storage() > before);
    assert_eq!(budget.failed_storage(), Some(budget.storage() + LIMIT));
    f.ready();
}

fn copied_handoff(handoff: &Handoff, budget: &mut Budget<'_>) -> (Handoff, usize) {
    use fe2o3_compiler_ffi::{
        INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_DECODE_METADATA_STORAGE_V5 as METADATA,
        inert_semantic_compiler_module_handoff_decode_work_v5,
    };
    let floor = budget.storage();
    let length = handoff.canonical_bytes().len();
    budget.reserve_storage(length + METADATA).unwrap();
    budget.charge_work(length).unwrap();
    let bytes = handoff.canonical_bytes().to_vec();
    budget.reserve_storage(bytes.capacity() - length).unwrap();
    budget
        .charge_work(inert_semantic_compiler_module_handoff_decode_work_v5(length).unwrap())
        .unwrap();
    let copy = Handoff::decode_owned(bytes).unwrap();
    let retained = budget.storage() - floor;
    budget.release_storage(retained).unwrap();
    (copy, retained)
}

#[test]
fn conditional_transaction_v5_equal_byte_redecoded_backing_is_not_original_custody() {
    let f = Fixture::new();
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, LIMIT);
    f.reserve(&mut budget);
    let receipt = f.publish(&mut budget).unwrap();
    let lease = f.lease(receipt, &mut budget);
    let token = token(&lease, &mut budget);
    let before = budget.storage();
    let error = token
        .try_map_handoff(&mut budget, |handoff, b| {
            let (copy, storage) = copied_handoff(&handoff, b);
            assert_eq!(handoff.identity(), copy.identity());
            assert_ne!(
                handoff.canonical_bytes().as_ptr(),
                copy.canonical_bytes().as_ptr()
            );
            Ok::<_, ()>((copy, storage))
        })
        .unwrap_err();
    assert!(matches!(
        error.cause(),
        CompilerModuleHandoffAdmissionCauseV5::Transaction(Error::HandoffIdentityMismatch)
    ));
    assert!(budget.storage() > before);
    f.ready();
}

struct SwitchingOwner {
    original: Handoff,
    copy: Handoff,
    switched: Arc<std::sync::atomic::AtomicBool>,
}
impl AsRef<Handoff> for SwitchingOwner {
    fn as_ref(&self) -> &Handoff {
        if self.switched.load(Ordering::Relaxed) {
            &self.copy
        } else {
            &self.original
        }
    }
}

#[test]
fn conditional_transaction_v5_consumption_rechecks_backing_and_lease() {
    for wrong_lease in [false, true] {
        let f = Fixture::new();
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, LIMIT);
        f.reserve(&mut budget);
        let receipt = f.publish(&mut budget).unwrap();
        let lease = f.lease(receipt, &mut budget);
        let other = f.lease(receipt, &mut budget);
        let token = token(&lease, &mut budget);
        let switch = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (mapped, extra) = token
            .try_map_handoff(&mut budget, |original, b| {
                let (copy, storage) = copied_handoff(&original, b);
                Ok::<_, ()>((
                    SwitchingOwner {
                        original,
                        copy,
                        switched: switch.clone(),
                    },
                    storage,
                ))
            })
            .unwrap();
        budget.reserve_storage(extra.0).unwrap();
        if !wrong_lease {
            switch.store(true, Ordering::Relaxed);
        }
        let floor = budget.storage();
        let result = consume_compiler_module_handoff_with_currentness_v5(
            if wrong_lease { &other } else { &lease },
            mapped,
            &mut budget,
        );
        if wrong_lease {
            assert!(matches!(result, Err(Error::MismatchedCurrentnessToken)));
        } else {
            assert!(matches!(result, Err(Error::HandoffIdentityMismatch)));
        }
        assert!(budget.storage() > floor);
        f.ready();
        assert!(lease.binding.output.try_lock().unwrap().is_some());
    }
}

#[test]
fn conditional_transaction_v5_namespaces_and_execution_sidecars_do_not_fallback() {
    let f = Fixture::new();
    let old = super::super::native_v4::tests::outer(7);
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, LIMIT);
    f.reserve(&mut budget);
    budget.reserve_storage(old.backing_capacity()
        + fe2o3_compiler_ffi::INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_DECODE_METADATA_STORAGE_V4).unwrap();
    super::super::native_v4::publish_compiler_module_handoff_v4(
        &f.path,
        &f.producer,
        f.attempt,
        &old,
        &mut budget,
    )
    .unwrap();
    assert!(f.recover(&mut budget).is_err());
    assert!(Handoff::decode_owned(old.canonical_bytes().to_vec()).is_err());
    assert!(
        fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV4::decode_owned(
            f.handoff.canonical_bytes().to_vec()
        )
        .is_err()
    );
    let receipt = f.publish(&mut budget).unwrap();
    let lease = f.lease(receipt, &mut budget);
    fs::write(
        f.slot().join("compiler-execution-receipt-v2"),
        b"no V2 reinterpretation",
    )
    .unwrap();
    assert!(lease.acquire_current_token(&mut budget).is_err());
    f.ready();
}

#[test]
fn conditional_transaction_v5_mapping_exact_and_one_short_resources_preserve_denials() {
    fn run(work_limit: usize, extra: usize) -> (bool, usize, usize) {
        let f = Fixture::new();
        let mut setup_work = Work::new(usize::MAX);
        let mut setup = Budget::new(&mut setup_work, LIMIT);
        f.reserve(&mut setup);
        let receipt = f.publish(&mut setup).unwrap();
        let lease = f.lease(receipt, &mut setup);
        let token = token(&lease, &mut setup);
        let inherited = token.storage();
        let floor = setup.storage();
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, floor + extra.min(LIMIT - floor));
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(11).unwrap();
        assert!(budget.charge_work(usize::MAX).is_err());
        assert!(budget.reserve_storage(usize::MAX).is_err());
        let ledger = budget.work_ledger_identity_v1();
        let result = token.try_map_handoff(&mut budget, |h, b| {
            b.charge_work(37)?;
            Ok::<_, Resource>((h, 4096))
        });
        let success = result.is_ok();
        let used = budget.work();
        let peak = budget.peak_storage() - floor;
        assert!(budget.work_ledger_identity_v1() == ledger);
        if let Ok((mapped, additional)) = &result {
            assert_eq!(budget.storage(), floor);
            assert_eq!(additional.0, 4096 + token_headers::<Handoff>());
            assert_eq!(mapped.storage().0, inherited.0 + additional.0);
            assert_eq!(budget.failed_work(), Some(usize::MAX));
            assert_eq!(budget.failed_storage(), Some(usize::MAX));
        } else {
            assert!(budget.storage() >= floor);
            f.ready();
        }
        drop(result);
        (success, used, peak)
    }
    let (ok, work, peak) = run(usize::MAX, LIMIT);
    assert!(ok);
    assert!(run(work, peak).0);
    assert!(!run(work - 1, peak).0);
    assert!(!run(work, peak - 1).0);
}

#[test]
fn conditional_transaction_v5_full_backing_capacity_and_closed_ceiling_are_required() {
    let mut f = Fixture::new();
    let wire = f.handoff.canonical_bytes();
    let length = wire.len();
    let mut backing = Vec::with_capacity(length + 8192);
    backing.extend_from_slice(&[0; 37]);
    backing.extend_from_slice(wire);
    backing.extend_from_slice(&[0; 41]);
    let capacity = backing.capacity();
    f.handoff = Handoff::decode_shared_vec(Arc::new(backing), 37..37 + length).unwrap();
    assert_eq!(f.handoff.backing_capacity(), capacity);
    let needed = payload_storage(&f.handoff).unwrap();
    for (limit, prepaid, accepts) in [
        (LIMIT, needed - 1, false),
        (LIMIT + 1, needed, false),
        (LIMIT, needed, true),
    ] {
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, limit);
        budget.reserve_storage(prepaid).unwrap();
        let result = f.publish(&mut budget);
        assert_eq!(result.is_ok(), accepts);
        assert_eq!(budget.storage(), prepaid);
        if !accepts {
            assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
        }
    }
}

struct FailAt(FaultPoint);
impl HandoffHooks for FailAt {
    fn hit(&mut self, point: FaultPoint) -> std::io::Result<()> {
        if point == self.0 {
            Err(std::io::Error::other("injected V5 journal fault"))
        } else {
            Ok(())
        }
    }
}

#[test]
fn conditional_transaction_v5_existing_journal_crash_boundary_is_preserved() {
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
        assert!(budget.storage() > floor);
        if point == FaultPoint::PayloadValidated {
            f.ready();
            assert_eq!(f.recover(&mut budget).unwrap(), receipt);
            let retry = self::token(&lease, &mut budget);
            consume_compiler_module_handoff_with_currentness_v5(&lease, retry, &mut budget)
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

struct CapturedDrop {
    binding: Arc<currentness::Current<Schema>>,
    drops: Arc<AtomicUsize>,
}
impl Drop for CapturedDrop {
    fn drop(&mut self) {
        assert!(self.binding.output.try_lock().unwrap().is_none());
        self.drops.fetch_add(1, Ordering::Relaxed);
    }
}

#[test]
fn conditional_transaction_v5_upfront_denial_destroys_unused_callback_under_lock() {
    for work_denial in [false, true] {
        let f = Fixture::new();
        let mut setup_work = Work::new(usize::MAX);
        let mut setup = Budget::new(&mut setup_work, LIMIT);
        f.reserve(&mut setup);
        let receipt = f.publish(&mut setup).unwrap();
        let lease = f.lease(receipt, &mut setup);
        let token = token(&lease, &mut setup);
        let floor = setup.storage();
        let mut work = Work::new(if work_denial { 0 } else { usize::MAX });
        let mut budget = Budget::new(&mut work, if work_denial { LIMIT } else { floor });
        budget.reserve_storage(floor).unwrap();
        let drops = Arc::new(AtomicUsize::new(0));
        let guard = CapturedDrop {
            binding: Arc::clone(&lease.binding),
            drops: drops.clone(),
        };
        let mut called = false;
        let error = token
            .try_map_handoff(&mut budget, |h, _| {
                called = true;
                drop(guard);
                Ok::<_, ()>((h, 0))
            })
            .unwrap_err();
        assert!(!called);
        assert_eq!(drops.load(Ordering::Relaxed), 1);
        assert_eq!(budget.storage(), floor);
        if work_denial {
            assert!(matches!(
                error.cause(),
                CompilerModuleHandoffAdmissionCauseV5::Transaction(Error::Resource(
                    Resource::Work(_)
                ))
            ));
        } else {
            assert!(matches!(
                error.cause(),
                CompilerModuleHandoffAdmissionCauseV5::Transaction(Error::Resource(
                    Resource::Storage(_)
                ))
            ));
        }
        f.ready();
        assert!(lease.binding.output.try_lock().unwrap().is_none());
        drop(error);
        assert!(lease.binding.output.try_lock().unwrap().is_some());
    }
}
