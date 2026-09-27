//! Inert transaction replay, not source proofs or protected execution.
use super::tests::fixture::{Fixture, outer_source_variant, outer_variant};
use super::*;
use crate::{BuildInvocation, BuildSession};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
const SLOT: CompilerModuleHandoffSlotV5 = CompilerModuleHandoffSlotV5::Production;
const LIMIT: usize = MAX_COMPILER_MODULE_HANDOFF_STORAGE_V5;

fn expected(f: &Fixture) -> CompilerModuleHandoffTransactionIdentityV5 {
    let producer = producer_identity_for::<Schema>(&f.producer);
    let slot = slot_identity_for::<Schema>(producer, f.attempt, SLOT);
    CompilerModuleHandoffTransactionIdentityV5::from_bytes(Schema::derive_identity(
        producer,
        slot,
        f.attempt,
        f.handoff.identity().into(),
        f.handoff.canonical_bytes(),
    ))
}
fn replay(f: &Fixture, b: &mut Budget<'_>) -> Result<CompilerModuleHandoffReceiptV5> {
    crate::rederive_compiler_module_handoff_receipt_for_replay_v5(
        &f.producer,
        f.attempt,
        SLOT,
        expected(f),
        &f.handoff,
        b,
    )
}
fn mismatch(result: Result<CompilerModuleHandoffReceiptV5>) {
    assert!(
        matches!(
            result,
            Err(Error::Coordination(
                CompilerModuleHandoffErrorV1::DigestMismatch
            ))
        ),
        "{result:?}"
    );
}

#[test]
fn conditional_v5_receipt_replay_never_creates_or_restores_currentness() {
    let f = Fixture::new();
    let mut work = Work::new(usize::MAX);
    let mut b = Budget::new(&mut work, LIMIT);
    let floor = f.reserve(&mut b);
    let receipt = replay(&f, &mut b).unwrap();
    assert_eq!(b.storage(), floor);
    assert_eq!(receipt.attempt(), f.attempt);
    assert_eq!(receipt.slot(), SLOT);
    assert_eq!(receipt.handoff_identity(), f.handoff.identity());
    assert_eq!(receipt.length(), f.handoff.canonical_bytes().len());
    assert!(!receipt.grants_compiler_authority());
    assert!(!receipt.grants_publication_authority());
    assert!(!f.slot().exists());
    assert!(matches!(
        acquire_compiler_module_handoff_currentness_lease_v5(&f.path, &f.producer, receipt, &mut b),
        Err(Error::Coordination(
            CompilerModuleHandoffErrorV1::NotPublished
        ))
    ));
    assert_eq!(f.publish(&mut b).unwrap(), receipt);
    let lease = f.lease(receipt, &mut b);
    let (token, storage) = lease.acquire_current_token(&mut b).unwrap();
    b.reserve_storage(storage.retained_storage()).unwrap();
    let consumed =
        consume_compiler_module_handoff_with_currentness_v5(&lease, token, &mut b).unwrap();
    assert_eq!(consumed.receipt(), receipt);
    let floor = b.storage();
    assert_eq!(replay(&f, &mut b).unwrap(), receipt);
    assert_eq!(b.storage(), floor);
    assert!(f.slot().join(CONSUMED_ENTRY).exists());
    assert!(!f.slot().join(READY_ENTRY).exists());
    assert!(matches!(
        lease.revalidate(&mut b),
        Err(Error::Coordination(
            CompilerModuleHandoffErrorV1::AlreadyConsumed
        ))
    ));
}

#[test]
fn conditional_v5_receipt_replay_binds_producer_attempt_target_and_all_source_bytes() {
    let f = Fixture::new();
    let transaction = expected(&f);
    let mut work = Work::new(usize::MAX);
    let mut b = Budget::new(&mut work, LIMIT);
    let floor = f.reserve(&mut b);
    let renamed =
        ProducerIdentity::from_codegen("other", Some(Path::new("/conditional.rs"))).unwrap();
    let relocated =
        ProducerIdentity::from_codegen("conditional", Some(Path::new("/other.rs"))).unwrap();
    for (producer, attempt) in [
        (&renamed, f.attempt),
        (&relocated, f.attempt),
        (
            &f.producer,
            BuildAttempt::new(
                f.attempt.generation() + 1,
                f.attempt.session(),
                f.attempt.invocation(),
            )
            .unwrap(),
        ),
        (
            &f.producer,
            BuildAttempt::new(
                f.attempt.generation(),
                BuildSession::from_bytes([99; 16]),
                f.attempt.invocation(),
            )
            .unwrap(),
        ),
        (
            &f.producer,
            BuildAttempt::new(
                f.attempt.generation(),
                f.attempt.session(),
                BuildInvocation::from_bytes([99; 32]),
            )
            .unwrap(),
        ),
    ] {
        mismatch(rederive_compiler_module_handoff_receipt_for_replay_v5(
            producer,
            attempt,
            SLOT,
            transaction,
            &f.handoff,
            &mut b,
        ));
        assert_eq!(b.storage(), floor);
    }
    for handoff in [
        outer_variant(true, 7),
        outer_variant(false, 9),
        outer_source_variant(false, 13),
    ] {
        let storage = payload_storage(&handoff).unwrap();
        b.reserve_storage(storage).unwrap();
        mismatch(rederive_compiler_module_handoff_receipt_for_replay_v5(
            &f.producer,
            f.attempt,
            SLOT,
            transaction,
            &handoff,
            &mut b,
        ));
        b.release_storage(storage).unwrap();
    }
    replay(&f, &mut b).unwrap();
}

#[test]
fn conditional_v5_receipt_replay_rejects_foreign_slot_and_identity_domains() {
    let f = Fixture::new();
    let producer = producer_identity_for::<Schema>(&f.producer);
    let slot = slot_identity_for::<Schema>(producer, f.attempt, SLOT);
    let foreign_slot = sha256_parts(&[
        Schema::NAMED_SLOT_DOMAIN,
        &producer,
        &f.attempt.generation().to_le_bytes(),
        f.attempt.session().as_bytes(),
        f.attempt.invocation().as_bytes(),
        &[1],
    ]);
    let named = Schema::derive_identity(
        producer,
        foreign_slot,
        f.attempt,
        f.handoff.identity().into(),
        f.handoff.canonical_bytes(),
    );
    let mut v4 = Sha256::new();
    v4.update(b"fe2o3.compiler-module-handoff.transaction-identity.v4\0");
    v4.update(f.handoff.identity().sha256());
    v4.update(f.handoff.identity().byte_len().to_le_bytes());
    v4.update(slot);
    v4.update(producer);
    v4.update(f.attempt.generation().to_le_bytes());
    v4.update(f.attempt.session().as_bytes());
    v4.update(f.attempt.invocation().as_bytes());
    v4.update((f.handoff.canonical_bytes().len() as u64).to_le_bytes());
    v4.update(f.handoff.canonical_bytes());
    let mut work = Work::new(usize::MAX);
    let mut b = Budget::new(&mut work, LIMIT);
    f.reserve(&mut b);
    for digest in [[0; 32], [1; 32], named, v4.finalize().into()] {
        mismatch(rederive_compiler_module_handoff_receipt_for_replay_v5(
            &f.producer,
            f.attempt,
            SLOT,
            CompilerModuleHandoffTransactionIdentityV5::from_bytes(digest),
            &f.handoff,
            &mut b,
        ));
    }
}

#[test]
fn conditional_v5_receipt_replay_exact_short_and_unpaid_original_account() {
    let f = Fixture::new();
    let floor = payload_storage(&f.handoff).unwrap();
    let mut measured_work = 0;
    let mut measured_storage = 0;
    for case in 0..6 {
        let quota = match case {
            0 => usize::MAX,
            2 => measured_work - 1,
            _ => measured_work,
        };
        let limit = match case {
            0 => LIMIT,
            3 => measured_storage - 1,
            5 => LIMIT + 1,
            _ => measured_storage,
        };
        let mut work = Work::new(quota);
        let mut b = Budget::new(&mut work, limit);
        b.reserve_storage(floor - usize::from(case == 4)).unwrap();
        b.charge_work(19).unwrap();
        let ledger = b.work_ledger_identity_v1();
        let result = replay(&f, &mut b);
        assert!(b.work_ledger_identity_v1() == ledger);
        assert_eq!(b.storage(), floor - usize::from(case == 4));
        if case == 0 {
            measured_work = b.work();
            measured_storage = b.peak_storage();
            assert_eq!(
                measured_work,
                19 + 8
                    + currentness::replay_hash_work::<Schema>(
                        f.producer.stable_source.len(),
                        f.producer.crate_name.len(),
                        f.handoff.canonical_bytes().len()
                    )
                    .unwrap()
            );
        }
        if case < 2 {
            result.unwrap();
        } else {
            assert!(matches!(result, Err(Error::Resource(_))), "{result:?}");
        }
    }
    assert!(matches!(
        currentness::replay_hash_work::<Schema>(usize::MAX, 1, 1),
        Err(Resource::Arithmetic)
    ));
    assert!(!f.slot().exists());
}
