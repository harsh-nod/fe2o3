//! Inert component/accounting controls only, never fabricated signed owners.
use super::*;
use fe2o3_compiler_lineage::{
    InertCanonicalSemanticMirReceiptV3, InertKernelIrReceiptV3,
    InertLineageContentIdentityV3 as Identity, InertMiddleEndReceiptV3,
    InertMirToKirCorrespondenceReceiptV3, InertProofBindingAssociationInputsV4 as Inputs,
    InertProofBindingAssociationV4 as Association,
    MAX_INERT_PROOF_BINDING_VERUS_EVIDENCE_BYTES_V4 as EVIDENCE_CAP,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use sha2::{Digest, Sha256};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

const FORMAL: &[u8] = b"inert original formal fixture, not admitted obligations";
const EXPECTED: [&[u8]; 5] = [
    b"inert semantic",
    b"inert middle",
    b"inert original N envelope",
    b"inert correspondence",
    b"inert evidence, not a signature",
];

fn content(sha: &[u8; 32], length: u64) -> Identity {
    Identity::new(*sha, length).unwrap()
}

fn identities(expected: [&[u8]; 5], formal: &[u8]) -> [Identity; 5] {
    macro_rules! receipt {
        ($kind:ty, $bytes:expr) => {{
            let receipt = <$kind>::from_canonical_preimage($bytes.to_vec()).unwrap();
            content(receipt.identity().sha256(), receipt.identity().byte_len())
        }};
    }
    [
        receipt!(InertCanonicalSemanticMirReceiptV3, expected[0]),
        receipt!(InertMiddleEndReceiptV3, expected[1]),
        receipt!(InertKernelIrReceiptV3, expected[2]),
        receipt!(InertMirToKirCorrespondenceReceiptV3, expected[3]),
        receipt!(InertFormalMemoryReceiptV3, formal),
    ]
}

fn wire(ids: [Identity; 5], evidence: &[u8]) -> Vec<u8> {
    Association::new(
        Inputs::new(ids[0], ids[1], ids[2], ids[3], ids[4]),
        evidence,
    )
    .unwrap()
    .canonical_bytes()
    .to_vec()
}

fn check_inert(
    expected: [&[u8]; 5],
    formal: &[u8],
    wire: &[u8],
    work_limit: usize,
    storage_limit: usize,
) -> (R<()>, usize, usize) {
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(37).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let result = (|| {
        let (owner, storage) =
            NativeOriginalInputAssociationReceiptsPolicy6V1::try_from_preimages_v1(
                formal,
                wire,
                &mut budget,
            )?;
        assert_eq!(budget.storage(), 37);
        budget.reserve_storage(storage.retained_storage())?;
        let result = joins::test_support::check(expected, &owner, &mut budget);
        assert_eq!(budget.storage(), 37 + storage.retained_storage());
        drop(owner);
        budget.release_storage(storage.retained_storage())?;
        result
    })();
    assert_eq!(budget.storage(), 37);
    assert!(budget.work_ledger_identity_v1() == ledger);
    (result, budget.work(), budget.peak_storage())
}

#[test]
fn policy6_input_association_inert_checks_are_deterministic_and_exactly_budgeted() {
    let first = wire(identities(EXPECTED, FORMAL), EXPECTED[4]);
    assert_eq!(first, wire(identities(EXPECTED, FORMAL), EXPECTED[4]));
    let (result, work, peak) = check_inert(EXPECTED, FORMAL, &first, 1_000_000, 1_000_000);
    result.unwrap();
    let (result, again_work, again_peak) = check_inert(EXPECTED, FORMAL, &first, work, peak);
    result.unwrap();
    assert_eq!((again_work, again_peak), (work, peak));
    assert!(matches!(
        check_inert(EXPECTED, FORMAL, &first, work - 1, peak).0,
        Err(E::Resource(_))
    ));
    assert!(matches!(
        check_inert(EXPECTED, FORMAL, &first, work, peak - 1).0,
        Err(E::Resource(_))
    ));
}

#[test]
fn policy6_input_association_refuses_each_wrong_typed_digest_length_and_raw_hash() {
    let ids = identities(EXPECTED, FORMAL);
    let names = [
        "original semantic receipt identity",
        "original Middle receipt identity",
        "original KernelIr receipt identity",
        "original correspondence receipt identity",
        "original FormalMemory receipt identity",
    ];
    for axis in 0..5 {
        for fault in 0..3 {
            let mut wrong = ids;
            let mut digest = ids[axis].sha256();
            let mut length = ids[axis].byte_len();
            match fault {
                0 => digest[0] ^= 1,
                1 => length += 1,
                2 => {
                    let preimage = if axis == 4 { FORMAL } else { EXPECTED[axis] };
                    digest = Sha256::digest(preimage).into();
                    assert_ne!(digest, ids[axis].sha256());
                }
                _ => unreachable!(),
            }
            wrong[axis] = Identity::new(digest, length).unwrap();
            let wire = wire(wrong, EXPECTED[4]);
            let result = check_inert(EXPECTED, FORMAL, &wire, 1_000_000, 1_000_000).0;
            assert!(
                matches!(result, Err(E::Mismatch(name)) if name == names[axis]),
                "{result:?}"
            );
        }
    }
}

#[test]
fn policy6_input_association_keeps_receipt_domains_distinct_for_identical_bytes() {
    let bytes = b"identical opaque bytes";
    let expected = [bytes.as_slice(); 5];
    let ids = identities(expected, bytes);
    for i in 0..5 {
        for j in 0..i {
            assert_ne!(ids[i].sha256(), ids[j].sha256());
            assert_eq!(ids[i].byte_len(), ids[j].byte_len());
        }
    }
    let mut swapped = ids;
    swapped.swap(1, 3);
    let wire = wire(swapped, bytes);
    assert!(matches!(
        check_inert(expected, bytes, &wire, 1_000_000, 1_000_000).0,
        Err(E::Mismatch("original Middle receipt identity"))
    ));
}

#[test]
fn policy6_input_association_refuses_changed_expected_preimages_and_formal_substitution() {
    let wire = wire(identities(EXPECTED, FORMAL), EXPECTED[4]);
    for (axis, error) in [
        (0, "original semantic receipt identity"),
        (1, "original Middle receipt identity"),
        (2, "original KernelIr receipt identity"),
        (3, "original correspondence receipt identity"),
        (4, "exact retained native Verus roster"),
    ] {
        let mut changed = EXPECTED[axis].to_vec();
        changed[0] ^= 1;
        let mut expected = EXPECTED;
        expected[axis] = &changed;
        let result = check_inert(expected, FORMAL, &wire, 1_000_000, 1_000_000).0;
        assert!(
            matches!(result, Err(E::Mismatch(actual)) if actual == error),
            "{result:?}"
        );
    }
    assert!(matches!(
        check_inert(EXPECTED, b"other formal", &wire, 1_000_000, 1_000_000).0,
        Err(E::Mismatch("original FormalMemory receipt identity"))
    ));
}

#[test]
fn policy6_input_association_keeps_complete_evidence_and_existing_64k_cap() {
    let ids = identities(EXPECTED, FORMAL);
    let evidence = vec![0x5a; EVIDENCE_CAP];
    let mut expected = EXPECTED;
    expected[4] = &evidence;
    let encoded = wire(ids, &evidence);
    check_inert(expected, FORMAL, &encoded, 10_000_000, 1_000_000)
        .0
        .unwrap();
    let truncated = wire(ids, &evidence[..evidence.len() - 1]);
    assert!(matches!(
        check_inert(expected, FORMAL, &truncated, 10_000_000, 1_000_000).0,
        Err(E::Mismatch("exact retained native Verus roster"))
    ));
    let oversized = vec![0x5a; EVIDENCE_CAP + 1];
    assert!(
        Association::new(
            Inputs::new(ids[0], ids[1], ids[2], ids[3], ids[4]),
            &oversized
        )
        .is_err()
    );
    expected[4] = &oversized;
    assert!(matches!(
        check_inert(expected, FORMAL, &encoded, 10_000_000, 1_000_000).0,
        Err(E::Mismatch("complete native Verus evidence extent"))
    ));
    expected[4] = b"";
    assert!(matches!(
        check_inert(expected, FORMAL, &encoded, 10_000_000, 1_000_000).0,
        Err(E::Mismatch("complete native Verus evidence extent"))
    ));
}

#[test]
fn policy6_input_association_opaque_copy_does_not_admit_malformed_or_trailing_v4() {
    let good = wire(identities(EXPECTED, FORMAL), EXPECTED[4]);
    let mut trailing = good.clone();
    trailing.push(0);
    let mut malformed = good.clone();
    malformed[0] ^= 1;
    for bad in [
        b"not V4".as_slice(),
        &trailing,
        &malformed,
        &good[..good.len() - 1],
    ] {
        assert!(matches!(
            check_inert(EXPECTED, FORMAL, bad, 1_000_000, 1_000_000).0,
            Err(E::Mismatch("original V4 association encoding"))
        ));
    }
}

#[test]
fn policy6_input_association_owned_copy_is_independent_and_exactly_accounted() {
    let mut formal = FORMAL.to_vec();
    let mut encoded = wire(identities(EXPECTED, FORMAL), EXPECTED[4]);
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    budget.reserve_storage(19).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let (owner, storage) = NativeOriginalInputAssociationReceiptsPolicy6V1::try_from_preimages_v1(
        &formal,
        &encoded,
        &mut budget,
    )
    .unwrap();
    assert_eq!(budget.storage(), 19);
    assert_eq!(
        owner.retained_storage(),
        size_of::<NativeOriginalInputAssociationReceiptsPolicy6V1>() + formal.len() + encoded.len()
    );
    assert_eq!(owner.retained_storage(), storage.retained_storage());
    budget.reserve_storage(storage.retained_storage()).unwrap();
    formal[0] ^= 1;
    encoded[0] ^= 1;
    assert_eq!(owner.formal_memory().canonical_preimage(), FORMAL);
    assert_ne!(owner.proof_binding().canonical_preimage(), encoded);
    joins::test_support::check(EXPECTED, &owner, &mut budget).unwrap();
    drop(owner);
    budget.release_storage(storage.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 19);
    assert!(budget.work_ledger_identity_v1() == ledger);
}

#[test]
fn policy6_input_association_owned_copy_has_exact_work_and_storage_limits() {
    let run = |work_limit, storage_limit| {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(29).unwrap();
        let result = NativeOriginalInputAssociationReceiptsPolicy6V1::try_from_preimages_v1(
            b"formal",
            b"opaque V4",
            &mut budget,
        );
        let accepted = result.is_ok();
        drop(result);
        assert_eq!(budget.storage(), 29);
        (accepted, budget.work(), budget.peak_storage())
    };
    let (accepted, work, peak) = run(10_000, 10_000);
    assert!(accepted);
    assert!(run(work, peak).0);
    assert!(!run(work - 1, peak).0);
    assert!(!run(work, peak - 1).0);
}

#[test]
fn policy6_input_association_copy_refuses_empty_oversized_and_short_reserved_inputs() {
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    budget.reserve_storage(23).unwrap();
    let oversized = vec![1; MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3 + 1];
    for (formal, proof) in [
        (&b""[..], &b"x"[..]),
        (&b"x"[..], &b""[..]),
        (&oversized, &b"x"[..]),
        (&b"x"[..], &oversized),
    ] {
        assert!(matches!(
            NativeOriginalInputAssociationReceiptsPolicy6V1::try_from_preimages_v1(
                formal,
                proof,
                &mut budget
            ),
            Err(E::Mismatch("input association preimage extent"))
        ));
        assert_eq!(budget.storage(), 23);
    }
    let encoded = wire(identities(EXPECTED, FORMAL), EXPECTED[4]);
    let (owner, _) = NativeOriginalInputAssociationReceiptsPolicy6V1::try_from_preimages_v1(
        FORMAL,
        &encoded,
        &mut budget,
    )
    .unwrap();
    assert!(matches!(
        joins::test_support::check(EXPECTED, &owner, &mut budget),
        Err(E::Resource(Resource::Accounting))
    ));
    drop(owner);
    assert_eq!(budget.storage(), 23);
}

#[test]
fn policy6_input_association_temporary_identity_releases_before_the_next_copy() {
    let bytes = vec![7; 4096];
    let expected_receipt =
        InertCanonicalSemanticMirReceiptV3::from_canonical_preimage(bytes.clone()).unwrap();
    let expected = (
        *expected_receipt.identity().sha256(),
        expected_receipt.identity().byte_len(),
    );
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 20_000);
    budget.reserve_storage(41).unwrap();
    for _ in 0..4 {
        assert_eq!(
            joins::test_support::semantic_identity(&bytes, &mut budget).unwrap(),
            expected
        );
        assert_eq!(budget.storage(), 41);
    }
    assert_eq!(
        budget.peak_storage(),
        41 + 2 * bytes.len() + size_of::<InertCanonicalSemanticMirReceiptV3>()
    );
    assert!(matches!(
        joins::test_support::semantic_identity(b"", &mut budget),
        Err(E::Mismatch("original typed identity preimage extent"))
    ));
}

#[test]
fn policy6_input_association_wrapper_delta_keeps_incoming_and_unrelated_floors() {
    let mut work = Work::new(1000);
    let mut budget = Budget::new(&mut work, 10_000);
    let incoming = 1700usize;
    budget.reserve_storage(incoming + 43).unwrap();
    let (retained, added) = scoped(&mut budget, |budget| {
        let required = require_incoming(1100, 600, budget)?;
        assert_eq!(required, incoming);
        finish_receipt(required, budget)
    })
    .unwrap();
    assert_eq!(budget.storage(), incoming + 43);
    let delta = size_of::<PreparedNativeAssociatedInputFinalOutputPolicy6V1>()
        - size_of::<PreparedNativeFinalOutputPolicy6V1>()
        - size_of::<NativeOriginalInputAssociationReceiptsPolicy6V1>();
    assert_eq!(added.retained_storage(), delta);
    assert_eq!(retained, incoming + delta);
    assert!(matches!(
        require_incoming(incoming + 43, 1, &mut budget),
        Err(E::Resource(Resource::Accounting))
    ));
    assert!(matches!(
        require_incoming(usize::MAX, 1, &mut budget),
        Err(E::Resource(Resource::Arithmetic))
    ));
    assert!(matches!(
        finish_receipt(usize::MAX, &mut budget),
        Err(E::Resource(Resource::Arithmetic))
    ));
    assert_eq!(budget.storage(), incoming + 43);

    let run = |limit| {
        let mut work = Work::new(1000);
        let mut budget = Budget::new(&mut work, limit);
        budget.reserve_storage(incoming + 43).unwrap();
        let result = scoped(&mut budget, |budget| {
            let required = require_incoming(1100, 600, budget)?;
            finish_receipt(required, budget)
        });
        assert_eq!(budget.storage(), incoming + 43);
        result
    };
    run(incoming + 43 + delta).unwrap();
    assert!(matches!(
        run(incoming + 43 + delta - 1),
        Err(E::Resource(_))
    ));
}

struct Mark(Arc<AtomicUsize>);
impl Drop for Mark {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn policy6_input_association_failed_or_panicked_scope_discards_owned_receipts() {
    for panic in [false, true] {
        let mut work = Work::new(10000);
        let mut budget = Budget::new(&mut work, 10000);
        budget.reserve_storage(47).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let drops = Arc::new(AtomicUsize::new(0));
        let result: R<()> = scoped(&mut budget, |budget| {
            let (owner, receipt) =
                NativeOriginalInputAssociationReceiptsPolicy6V1::try_from_preimages_v1(
                    b"formal", b"proof", budget,
                )?;
            budget.reserve_storage(receipt.retained_storage())?;
            let _retained = (owner, Mark(Arc::clone(&drops)));
            if panic {
                panic!("input receipt discard control");
            }
            Err(E::Mismatch("discard control"))
        });
        assert!(if panic {
            matches!(result, Err(E::Panicked))
        } else {
            matches!(result, Err(E::Mismatch("discard control")))
        });
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert_eq!(budget.storage(), 47);
        assert!(budget.work_ledger_identity_v1() == ledger);
    }
}

#[test]
fn policy6_input_association_foreign_ledger_is_not_released_on_any_callback_exit() {
    for exit in 0..3 {
        let mut foreign_work = Work::new(1000);
        let mut work = Work::new(1000);
        let mut budget = Budget::new(&mut work, 1000);
        let mut foreign = Budget::new(&mut foreign_work, 1000);
        budget.reserve_storage(53).unwrap();
        budget.charge_work(11).unwrap();
        foreign.reserve_storage(71).unwrap();
        foreign.charge_work(7).unwrap();
        let original_ledger = budget.work_ledger_identity_v1();
        let foreign_ledger = foreign.work_ledger_identity_v1();
        let drops = Arc::new(AtomicUsize::new(0));
        let result = scoped(&mut budget, |budget| {
            let mark = Mark(Arc::clone(&drops));
            std::mem::swap(budget, &mut foreign);
            match exit {
                0 => Ok(mark),
                1 => Err(E::Mismatch("foreign refusal")),
                _ => panic!("foreign panic"),
            }
        });
        assert!(matches!(result, Err(E::Resource(Resource::Accounting))));
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert!(budget.work_ledger_identity_v1() == foreign_ledger);
        assert!(foreign.work_ledger_identity_v1() == original_ledger);
        assert_eq!(budget.storage(), 71);
        assert_eq!(budget.work(), 7);
        assert_eq!(foreign.storage(), 53);
        assert_eq!(foreign.work(), 11);
        budget.release_storage(71).unwrap();
        foreign.release_storage(53).unwrap();
    }
}

#[test]
fn policy6_input_association_panicking_payload_drop_cannot_skip_cleanup() {
    struct Payload(Arc<AtomicUsize>);
    impl Drop for Payload {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
            panic!("payload destructor");
        }
    }
    let mut work = Work::new(1000);
    let mut budget = Budget::new(&mut work, 1000);
    budget.reserve_storage(59).unwrap();
    let drops = Arc::new(AtomicUsize::new(0));
    let result = catch_unwind(AssertUnwindSafe(|| {
        scoped::<()>(&mut budget, |budget| {
            budget.reserve_storage(83)?;
            std::panic::panic_any(Payload(Arc::clone(&drops)))
        })
    }));
    assert!(result.is_err());
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(budget.storage(), 59);
}
