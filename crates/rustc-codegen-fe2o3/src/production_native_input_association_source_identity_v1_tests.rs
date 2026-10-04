//! Three unsigned source identity axes only. The caller first checks the strict
//! original-N component; no Middle/correspondence/Verus or signed owner is made.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use sha2::{Digest, Sha256};

const FIELDS: [&str; 3] = [
    "original semantic receipt identity",
    "original KernelIr receipt identity",
    "original FormalMemory receipt identity",
];

fn content(expected: ([u8; 32], u64)) -> Identity {
    Identity::new(expected.0, expected.1).unwrap()
}

fn exact_refusal(result: R<()>, field: &'static str) -> R<()> {
    if let Err(E::Resource(error)) = result {
        return Err(E::Resource(error));
    }
    assert!(
        matches!(result, Err(E::Mismatch(actual)) if actual == field),
        "{result:?}"
    );
    Ok(())
}

// Reference construction calls the existing typed receipt API directly. The
// observed side uses the exact production typed_identity/exact_identity helpers.
fn axis<T>(
    bytes: &[u8],
    limit: usize,
    field: &'static str,
    construct: fn(Vec<u8>) -> Result<T, LineageErrorV3>,
    identity: fn(&T) -> ([u8; 32], u64),
    budget: &mut Budget<'_>,
) -> R<Identity> {
    scoped(budget, |budget| {
        if bytes.is_empty() || bytes.len() > limit {
            return Err(E::Mismatch("unsigned identity reference extent"));
        }
        let storage = bytes
            .len()
            .checked_mul(2)
            .and_then(|n| n.checked_add(size_of::<T>()))
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(storage)?;
        budget.charge_work(bytes.len().checked_mul(2).ok_or(Resource::Arithmetic)?)?;
        let receipt = construct(copy_prepaid(bytes, budget)?)
            .map_err(|_| E::Mismatch("unsigned identity reference receipt"))?;
        let expected = identity(&receipt);
        assert_eq!(expected.1, u64::try_from(bytes.len()).unwrap());
        let observed = typed_identity(bytes, limit, budget, construct, identity)?;
        assert_eq!(observed, expected);
        exact_identity(content(observed), expected, field, budget)?;

        let mut digest = expected.0;
        digest[0] ^= 1;
        exact_refusal(
            exact_identity(content((digest, expected.1)), expected, field, budget),
            field,
        )?;
        let wrong_length = expected.1.checked_add(1).ok_or(Resource::Arithmetic)?;
        exact_refusal(
            exact_identity(content((expected.0, wrong_length)), expected, field, budget),
            field,
        )?;
        budget.charge_work(bytes.len().checked_add(1).ok_or(Resource::Arithmetic)?)?;
        let raw: [u8; 32] = Sha256::digest(bytes).into();
        assert_ne!(raw, expected.0);
        exact_refusal(
            exact_identity(content((raw, expected.1)), expected, field, budget),
            field,
        )?;

        drop(receipt);
        Ok(content(expected))
    })
}

/// Invoked by the real source64 Observe path and genuine constructed source
/// controls, after exact original-N envelope/roster/formal admission succeeds.
pub(crate) fn exercise_unsigned_original_identities_v1(
    semantic: &[u8],
    kernel: &[u8],
    formal: &[u8],
    budget: &mut Budget<'_>,
) -> R<()> {
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let result = scoped(budget, |budget| {
        budget.reserve_storage(size_of::<[Identity; 3]>())?;
        let ids = [
            axis(
                semantic,
                MAX_CANONICAL_SEMANTIC_MIR_BYTES_V3,
                FIELDS[0],
                InertCanonicalSemanticMirReceiptV3::from_canonical_preimage,
                |receipt| (*receipt.identity().sha256(), receipt.identity().byte_len()),
                budget,
            )?,
            axis(
                kernel,
                MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3,
                FIELDS[1],
                InertKernelIrReceiptV3::from_canonical_preimage,
                |receipt| (*receipt.identity().sha256(), receipt.identity().byte_len()),
                budget,
            )?,
            axis(
                formal,
                MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3,
                FIELDS[2],
                InertFormalMemoryReceiptV3::from_canonical_preimage,
                |receipt| (*receipt.identity().sha256(), receipt.identity().byte_len()),
                budget,
            )?,
        ];
        for (axis, expected) in ids.iter().enumerate() {
            for (other, candidate) in ids.iter().enumerate() {
                if axis != other {
                    exact_refusal(
                        exact_identity(
                            *candidate,
                            (expected.sha256(), expected.byte_len()),
                            FIELDS[axis],
                            budget,
                        ),
                        FIELDS[axis],
                    )?;
                }
            }
        }
        Ok(())
    });
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    result
}

/// Actual E/I bytes are supplied by the same retained source owner. Equal bytes
/// remain a legal no-op; differing bytes must not occupy original N's identity.
pub(crate) fn exercise_original_kernel_identity_substitution_v1(
    original: &[u8],
    candidate: &[u8],
    budget: &mut Budget<'_>,
) -> R<()> {
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let result = scoped(budget, |budget| {
        budget.reserve_storage(size_of::<[Identity; 2]>())?;
        let expected = typed_identity(
            original,
            MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3,
            budget,
            InertKernelIrReceiptV3::from_canonical_preimage,
            |receipt| (*receipt.identity().sha256(), receipt.identity().byte_len()),
        )?;
        let actual = typed_identity(
            candidate,
            MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3,
            budget,
            InertKernelIrReceiptV3::from_canonical_preimage,
            |receipt| (*receipt.identity().sha256(), receipt.identity().byte_len()),
        )?;
        budget.charge_work(
            original
                .len()
                .checked_add(candidate.len())
                .and_then(|n| n.checked_add(1))
                .ok_or(Resource::Arithmetic)?,
        )?;
        let result = exact_identity(content(actual), expected, FIELDS[1], budget);
        if original == candidate {
            result?;
        } else {
            exact_refusal(result, FIELDS[1])?;
        }
        Ok(())
    });
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    result
}

#[test]
fn unsigned_v4_identity_observer_inert_budget_controls_are_exact_and_cumulative() {
    // These bytes test the helper's resource mechanics, not source admission.
    let run = |work_limit, storage_limit| {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(41).unwrap();
        budget.charge_work(7).unwrap();
        let result = exercise_unsigned_original_identities_v1(
            b"semantic reference",
            b"kernel reference",
            b"formal reference",
            &mut budget,
        );
        assert_eq!(budget.storage(), 41);
        (result, budget.work(), budget.peak_storage())
    };
    let (result, work, peak) = run(100_000, 100_000);
    result.unwrap();
    assert!(work > 7);
    let (result, actual_work, actual_peak) = run(work, peak);
    result.unwrap();
    assert_eq!((actual_work, actual_peak), (work, peak));
    assert!(matches!(
        run(work - 1, peak).0,
        Err(E::Resource(Resource::Work(_)))
    ));
    assert!(matches!(
        run(work, peak - 1).0,
        Err(E::Resource(Resource::Storage(_)))
    ));
}

#[test]
fn unsigned_v4_kernel_identity_substitution_keeps_equal_byte_noop_legal() {
    let mut work = Work::new(100_000);
    let mut budget = Budget::new(&mut work, 100_000);
    budget.reserve_storage(43).unwrap();
    for candidate in [b"original bytes".as_slice(), b"different bytes".as_slice()] {
        exercise_original_kernel_identity_substitution_v1(
            b"original bytes",
            candidate,
            &mut budget,
        )
        .unwrap();
        assert_eq!(budget.storage(), 43);
    }
    assert!(matches!(
        exercise_original_kernel_identity_substitution_v1(b"", b"original bytes", &mut budget),
        Err(E::Mismatch("original typed identity preimage extent"))
    ));
    assert_eq!(budget.storage(), 43);
}
