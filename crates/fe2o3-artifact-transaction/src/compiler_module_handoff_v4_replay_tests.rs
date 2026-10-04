//! Receipt rederivation is an inert content check, never filesystem custody.
use super::tests::{Fixture, outer, outer_with_carrier};
use super::*;
use crate::{BuildInvocation, BuildSession};
use fe2o3_compiler_ffi::INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_DECODE_METADATA_STORAGE_V4 as METADATA;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

const SLOT: CompilerModuleHandoffSlotV4 = CompilerModuleHandoffSlotV4::Production;
const LIMIT: usize = MAX_COMPILER_MODULE_HANDOFF_STORAGE_V4;

fn producer() -> ProducerIdentity {
    ProducerIdentity::from_codegen("native", Some(Path::new("/native.rs"))).unwrap()
}

fn attempt(generation: u64, session: u8, invocation: u8) -> BuildAttempt {
    BuildAttempt::new(
        generation,
        BuildSession::from_bytes([session; 16]),
        BuildInvocation::from_bytes([invocation; 32]),
    )
    .unwrap()
}

fn expected(
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    handoff: &Handoff,
) -> CompilerModuleHandoffTransactionIdentityV4 {
    let producer = producer_identity_for::<Schema>(producer);
    let slot = slot_identity_for::<Schema>(producer, attempt, SLOT);
    CompilerModuleHandoffTransactionIdentityV4::from_bytes(Schema::derive_identity(
        producer,
        slot,
        attempt,
        handoff.identity().into(),
        handoff.canonical_bytes(),
    ))
}

fn rederive(
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    expected: CompilerModuleHandoffTransactionIdentityV4,
    handoff: &Handoff,
    budget: &mut Budget<'_>,
) -> Result<CompilerModuleHandoffReceiptV4> {
    // Exercise the crate-root export as well as the implementation.
    crate::rederive_compiler_module_handoff_receipt_for_replay_v4(
        producer, attempt, SLOT, expected, handoff, budget,
    )
}

fn mismatch(result: Result<CompilerModuleHandoffReceiptV4>) {
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
fn native_v4_receipt_replay_is_inert_before_publication_and_after_consumption() {
    let f = Fixture::new();
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, LIMIT);
    let floor = f.reserve(&mut budget);
    let transaction = expected(&f.producer, f.attempt, &f.handoff);
    let receipt = rederive(&f.producer, f.attempt, transaction, &f.handoff, &mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
    assert_eq!(receipt.attempt(), f.attempt);
    assert_eq!(receipt.slot(), SLOT);
    assert_eq!(receipt.handoff_identity(), f.handoff.identity());
    assert_eq!(receipt.transaction_identity(), transaction);
    assert_eq!(receipt.length(), f.handoff.canonical_bytes().len());
    assert!(!receipt.grants_compiler_authority());
    assert!(!receipt.grants_publication_authority());
    assert!(
        !f.slot().exists(),
        "pure replay must not create transaction files"
    );
    assert!(matches!(
        acquire_compiler_module_handoff_currentness_lease_v4(
            &f.path,
            &f.producer,
            receipt,
            &mut budget
        ),
        Err(Error::Coordination(
            CompilerModuleHandoffErrorV1::NotPublished
        ))
    ));
    assert_eq!(f.publish(&mut budget).unwrap(), receipt);
    let lease = f.lease(receipt, &mut budget);
    let token = tests::token(&lease, &mut budget);
    let consumed =
        consume_compiler_module_handoff_with_currentness_v4(&lease, token, &mut budget).unwrap();
    assert_eq!(consumed.receipt(), receipt);
    let floor = budget.storage();
    assert_eq!(
        rederive(&f.producer, f.attempt, transaction, &f.handoff, &mut budget).unwrap(),
        receipt
    );
    assert_eq!(budget.storage(), floor);
    assert!(f.slot().join(CONSUMED_ENTRY).exists());
    assert!(!f.slot().join(READY_ENTRY).exists());
    assert!(matches!(
        lease.revalidate(&mut budget),
        Err(Error::Coordination(
            CompilerModuleHandoffErrorV1::AlreadyConsumed
        ))
    ));
    assert!(matches!(
        acquire_compiler_module_handoff_currentness_lease_v4(
            &f.path,
            &f.producer,
            receipt,
            &mut budget
        ),
        Err(Error::Coordination(
            CompilerModuleHandoffErrorV1::AlreadyConsumed
        ))
    ));
}

#[test]
fn native_v4_receipt_replay_rejects_every_producer_and_attempt_axis() {
    let producer = producer();
    let attempt = attempt(9, 4, 3);
    let handoff = outer(7);
    let expected = expected(&producer, attempt, &handoff);
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, LIMIT);
    let floor = payload_storage(&handoff).unwrap();
    budget.reserve_storage(floor).unwrap();
    let other_name =
        ProducerIdentity::from_codegen("renamed", Some(Path::new("/native.rs"))).unwrap();
    let other_source =
        ProducerIdentity::from_codegen("native", Some(Path::new("/different.rs"))).unwrap();
    for (producer, changed) in [
        (&other_name, attempt),
        (&other_source, attempt),
        (&producer, self::attempt(10, 4, 3)),
        (&producer, self::attempt(9, 5, 3)),
        (&producer, self::attempt(9, 4, 6)),
    ] {
        let before = budget.work();
        mismatch(rederive(producer, changed, expected, &handoff, &mut budget));
        assert_eq!(budget.storage(), floor);
        assert!(budget.work() > before, "refused hashes must remain paid");
    }
    rederive(&producer, attempt, expected, &handoff, &mut budget).unwrap();
}

#[test]
fn native_v4_receipt_replay_rejects_foreign_transaction_slot_domain_and_complete_outer() {
    let producer = producer();
    let attempt = attempt(9, 4, 3);
    let handoff = outer(7);
    let expected = expected(&producer, attempt, &handoff);
    let changed = outer_with_carrier(7, 29);
    assert_eq!(
        handoff.module_handoff().canonical_bytes(),
        changed.module_handoff().canonical_bytes()
    );
    assert_ne!(handoff.canonical_bytes(), changed.canonical_bytes());
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, LIMIT);
    let floor = payload_storage(&handoff).unwrap() + payload_storage(&changed).unwrap();
    budget.reserve_storage(floor).unwrap();
    mismatch(rederive(
        &producer,
        attempt,
        expected,
        &changed,
        &mut budget,
    ));
    let mut wrong = *expected.as_bytes();
    wrong[0] ^= 1;
    for identity in [wrong, [0; 32]] {
        mismatch(rederive(
            &producer,
            attempt,
            CompilerModuleHandoffTransactionIdentityV4::from_bytes(identity),
            &handoff,
            &mut budget,
        ));
    }

    // V4 intentionally exposes only Production. Do not create an invalid Rust
    // enum: a foreign named-slot transcript must not replay as Production.
    assert_eq!(Schema::ALL_SLOTS, &[SLOT]);
    let producer_id = producer_identity_for::<Schema>(&producer);
    let foreign_slot = sha256_parts(&[
        Schema::NAMED_SLOT_DOMAIN,
        &producer_id,
        &attempt.generation().to_le_bytes(),
        attempt.session().as_bytes(),
        attempt.invocation().as_bytes(),
        &[1],
    ]);
    let foreign = Schema::derive_identity(
        producer_id,
        foreign_slot,
        attempt,
        handoff.identity().into(),
        handoff.canonical_bytes(),
    );
    mismatch(rederive(
        &producer,
        attempt,
        CompilerModuleHandoffTransactionIdentityV4::from_bytes(foreign),
        &handoff,
        &mut budget,
    ));
    assert_eq!(budget.storage(), floor);
    rederive(&producer, attempt, expected, &handoff, &mut budget).unwrap();
}

#[test]
fn native_v4_receipt_replay_requires_entire_backing_capacity_and_metadata() {
    let producer = producer();
    let attempt = attempt(9, 4, 3);
    let original = outer(7);
    let expected = expected(&producer, attempt, &original);
    let n = original.canonical_bytes().len();
    let mut backing = Vec::with_capacity(n + 8192);
    backing.extend([0x35; 37]);
    backing.extend_from_slice(original.canonical_bytes());
    backing.extend([0x53; 41]);
    let handoff = Handoff::decode_shared_vec(Arc::new(backing), 37..37 + n).unwrap();
    assert!(handoff.backing_capacity() > handoff.canonical_bytes().len());
    let pointer = handoff.canonical_bytes().as_ptr();
    let floor = handoff.backing_capacity() + METADATA;
    for paid in [handoff.canonical_bytes().len() + METADATA, floor - 1] {
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(paid).unwrap();
        assert!(matches!(
            rederive(&producer, attempt, expected, &handoff, &mut budget),
            Err(Error::Resource(Resource::Accounting))
        ));
        assert_eq!(budget.storage(), paid);
        assert_eq!(budget.peak_storage(), paid);
        assert_eq!(budget.work(), 8);
    }
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(floor).unwrap();
    let receipt = rederive(&producer, attempt, expected, &handoff, &mut budget).unwrap();
    assert_eq!(receipt.handoff_identity(), original.identity());
    assert_eq!(budget.storage(), floor);
    assert_eq!(handoff.canonical_bytes().as_ptr(), pointer);
}

#[test]
fn native_v4_receipt_replay_exact_and_one_short_work_and_storage_budgets() {
    let producer = producer();
    let attempt = attempt(9, 4, 3);
    let handoff = outer(7);
    let expected = expected(&producer, attempt, &handoff);
    let source_floor = payload_storage(&handoff).unwrap();
    let work_prefix = 23;
    let storage_prefix = 47;
    let floor = source_floor + storage_prefix;
    let required_work = 8 + replay_hash_work(
        producer.stable_source.len(),
        producer.crate_name.len(),
        handoff.canonical_bytes().len(),
    )
    .unwrap();
    let peak = floor + FRAME_STORAGE + REPLAY_SCRATCH_STORAGE;
    for (short_work, short_storage) in [(false, false), (true, false), (false, true)] {
        let mut work = Work::new(work_prefix + required_work - usize::from(short_work));
        work.charge_work(work_prefix).unwrap();
        let mut budget = Budget::new(&mut work, peak - usize::from(short_storage));
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = rederive(&producer, attempt, expected, &handoff, &mut budget);
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        if short_storage {
            assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
            assert_eq!(budget.failed_storage(), Some(peak));
            assert_eq!(budget.work(), work_prefix + 8);
        } else if short_work {
            assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
            assert_eq!(budget.work(), work_prefix + 8);
            assert_eq!(budget.peak_storage(), peak);
            assert_eq!(work.failed_work(), Some(work_prefix + required_work));
        } else {
            let receipt = result.unwrap();
            assert_eq!(receipt.transaction_identity(), expected);
            assert_eq!(budget.work(), work_prefix + required_work);
            assert_eq!(budget.peak_storage(), peak);
            budget
                .reserve_storage(size_of::<CompilerModuleHandoffReceiptV4>())
                .unwrap();
            assert_eq!(
                budget.storage(),
                floor + size_of::<CompilerModuleHandoffReceiptV4>()
            );
        }
    }
}

#[test]
fn native_v4_receipt_replay_charges_both_producer_strings_and_preserves_work_prefix() {
    let handoff = outer(7);
    let attempt = attempt(9, 4, 3);
    let path = format!("/{}.rs", "a".repeat(1024));
    let producers = [
        producer(),
        ProducerIdentity::from_codegen("longer_crate_name", Some(Path::new(&path))).unwrap(),
    ];
    let mut charges = Vec::new();
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget
        .reserve_storage(payload_storage(&handoff).unwrap())
        .unwrap();
    for producer in &producers {
        let before = budget.work();
        let expected = expected(producer, attempt, &handoff);
        rederive(producer, attempt, expected, &handoff, &mut budget).unwrap();
        charges.push(budget.work() - before);
    }
    let bytes =
        |producer: &ProducerIdentity| producer.stable_source.len() + producer.crate_name.len();
    assert_eq!(
        charges[1] - charges[0],
        bytes(&producers[1]) - bytes(&producers[0])
    );
    assert_eq!(budget.work(), charges.into_iter().sum::<usize>());
}

#[test]
fn native_v4_receipt_replay_arithmetic_and_enlarged_storage_limit_refuse() {
    for lengths in [(usize::MAX, 0, 0), (0, usize::MAX, 0), (0, 0, usize::MAX)] {
        assert_eq!(
            replay_hash_work(lengths.0, lengths.1, lengths.2),
            Err(Resource::Arithmetic)
        );
    }
    let producer = producer();
    let attempt = attempt(9, 4, 3);
    let handoff = outer(7);
    let expected = expected(&producer, attempt, &handoff);
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, LIMIT + 1);
    let floor = payload_storage(&handoff).unwrap();
    budget.reserve_storage(floor).unwrap();
    assert!(matches!(
        rederive(&producer, attempt, expected, &handoff, &mut budget),
        Err(Error::Resource(Resource::Accounting))
    ));
    assert_eq!(budget.storage(), floor);
    assert_eq!(budget.work(), 8);
}

#[test]
fn native_v4_receipt_replay_preserves_frozen_schema_hash_domains() {
    // Independently computed SHA-256 vectors for the pre-existing schema. The
    // arbitrary payload checks only hashing, not admission of malformed V4 bytes.
    let producer = producer_identity_for::<Schema>(&producer());
    let attempt = attempt(9, 4, 3);
    let slot = slot_identity_for::<Schema>(producer, attempt, SLOT);
    let bytes = b"native-v4-identity-golden";
    let binding = currentness::Binding {
        sha256: [0x17; 32],
        byte_len: bytes.len() as u64,
    };
    let transaction = Schema::derive_identity(producer, slot, attempt, binding, bytes);
    assert_eq!(
        hex(&producer),
        "e9b5bcc96ed84b66280607835ca3529e46c054755aac2f2bbaa8735990476927"
    );
    assert_eq!(
        hex(&slot),
        "56047766fd3757bb1ddf71018b9a377d768d99b2935feed2b2a55954a47f4a3f"
    );
    assert_eq!(
        hex(&transaction),
        "63d29ea51bbb80d2e820847c44b7679282e9a4774adf21f1b083eab109e7e569"
    );
}
