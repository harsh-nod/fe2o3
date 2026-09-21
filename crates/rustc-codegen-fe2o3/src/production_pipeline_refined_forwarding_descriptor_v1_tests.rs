//! Actual final-F descriptor predicate; constructed source is not rustc/publication authority.
use super::*;
use crate::compiler_descriptor::checked_output_policy3_v1::refined_forwarding_v1::{
    RefinedForwardingDescriptorErrorV1 as DescriptorError, fixtures,
};
use crate::compiler_descriptor::{CompilerDescriptorError, TypedDescriptorRootV1};

fn borrowed(owner: &Composed) -> FinalOwnerV1<'_> {
    match owner {
        Composed::Direct(owner) => FinalOwnerV1::Direct(owner),
        Composed::Erased(owner) => FinalOwnerV1::Erased(owner),
    }
}
fn with_final(
    erased: bool,
    profile: Profile,
    mutation: bool,
    next: impl FnOnce(&PreparedRefinedForwardingNativeOutputV1, &mut Budget<'_>),
) {
    with_prefix(erased, profile, mutation, |prefix, budget| {
        let floor = budget.storage();
        let (value, receipt) = prepare(
            prefix,
            profile,
            Limits::default(),
            ForwardingLimits::default(),
            budget,
        )
        .unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        next(&value, budget);
        assert_eq!(budget.storage(), floor + receipt.retained_storage());
        drop(value);
        budget.release_storage(receipt.retained_storage()).unwrap();
        assert_eq!(budget.storage(), floor);
    });
}
fn positive(erased: bool) {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for mutation in [false, true] {
            with_final(erased, profile, mutation, |value, budget| {
                let owner = borrowed(&value.owner);
                let roots = fixtures::typed_roots(owner);
                let original = roots.clone();
                let floor = budget.storage();
                fixtures::assert_final_subject(owner);
                assert_eq!(
                    value
                        .refinement_origins()
                        .iter()
                        .filter(|row| matches!(
                            row.canonical_origin(),
                            Origin::CheckedAddSplit { .. }
                        ))
                        .count(),
                    if mutation { 2 } else { 0 }
                );
                assert_eq!(
                    value
                        .origins()
                        .iter()
                        .filter(|row| row.canonical_origin().store.is_some())
                        .count(),
                    if mutation { 2 } else { 0 }
                );
                validate_final_descriptor_evidence_v1(owner, &roots, profile, budget).unwrap();
                assert_eq!(roots, original);
                assert_eq!(budget.storage(), floor);
                value.verify_equivalence(budget).unwrap();
                assert!(!value.grants_artifact_or_launch_authority());
                assert!(!value.llvm_ir().contains(".fe2o3.kd.v1"));
            });
        }
    }
}
fn assert_mismatch(result: std::result::Result<(), DescriptorError>, expected: &'static str) {
    match result {
        Err(DescriptorError::Descriptor(error)) => assert!(matches!(*error,
            CompilerDescriptorError::ProductionDescriptorMismatch(detail) if detail == expected)),
        other => panic!("exact descriptor evidence refusal {expected}: {other:?}"),
    }
}
#[test]
fn refined_forwarding_descriptor_direct_final_f_noop_and_mutation_both_profiles() {
    positive(false);
}
#[test]
fn refined_forwarding_descriptor_unit_local_final_f_noop_and_mutation_both_profiles() {
    positive(true);
}

#[test]
fn refined_forwarding_descriptor_exact_source_roots_launch_abi_and_type_identity() {
    for erased in [false, true] {
        with_final(erased, Profile::Gfx942, true, |value, budget| {
            let owner = borrowed(&value.owner);
            let roots = fixtures::typed_roots(owner);
            let floor = budget.storage();
            for case in 0..8 {
                let mut wrong = roots.clone();
                let expected = fixtures::hostile(&mut wrong, case);
                assert_mismatch(
                    validate_final_descriptor_evidence_v1(owner, &wrong, Profile::Gfx942, budget),
                    expected,
                );
                assert_eq!(budget.storage(), floor);
                assert_eq!(budget.failed_storage(), None);
            }
            validate_final_descriptor_evidence_v1(owner, &roots, Profile::Gfx942, budget).unwrap();
        });
    }
}

#[test]
fn refined_forwarding_descriptor_complete_roster_and_exact_target() {
    for erased in [false, true] {
        with_final(erased, Profile::Gfx942, true, |value, budget| {
            let owner = borrowed(&value.owner);
            let roots = fixtures::typed_roots(owner);
            let floor = budget.storage();
            let mut extra = roots.clone();
            extra.push(roots[0].clone());
            for wrong in [&[][..], &roots[..1], extra.as_slice()] {
                assert_mismatch(
                    validate_final_descriptor_evidence_v1(owner, wrong, Profile::Gfx942, budget),
                    "complete ordered typed/source/output/formal root roster",
                );
                assert_eq!(budget.storage(), floor);
            }
            match validate_final_descriptor_evidence_v1(owner, &roots, Profile::Gfx950, budget) {
                Err(DescriptorError::Descriptor(error)) => assert!(matches!(
                    *error,
                    CompilerDescriptorError::CheckedOutputTarget(
                        dialect_amdgcn::ProductionTargetCoordinateErrorV1::Metadata(_)
                    )
                )),
                other => panic!("exact N/B target refusal: {other:?}"),
            }
            assert_eq!(budget.storage(), floor);
        });
    }
}

struct Measurement {
    result: std::result::Result<(), DescriptorError>,
    work: usize,
    peak: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
}
fn measure(
    owner: FinalOwnerV1<'_>,
    roots: &[TypedDescriptorRootV1],
    floor: usize,
    work_limit: usize,
    storage_limit: usize,
) -> Measurement {
    let sibling = vec![0x5au8; 37];
    let floor = floor + size_of_val(&sibling) + sibling.capacity();
    let mut work = Work::new(work_limit);
    let (result, accepted, peak, failed_storage) = {
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(17).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result =
            validate_final_descriptor_evidence_v1(owner, roots, Profile::Gfx942, &mut budget);
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(sibling, [0x5a; 37]);
        (
            result,
            budget.work(),
            budget.peak_storage(),
            budget.failed_storage(),
        )
    };
    Measurement {
        result,
        work: accepted,
        peak,
        failed_work: work.failed_work(),
        failed_storage,
    }
}
#[test]
fn refined_forwarding_descriptor_exact_work_and_measured_storage_preserve_ledger() {
    for erased in [false, true] {
        for mutation in [false, true] {
            with_final(erased, Profile::Gfx942, mutation, |value, budget| {
                let owner = borrowed(&value.owner);
                let roots = fixtures::typed_roots(owner);
                let full = measure(
                    owner,
                    &roots,
                    budget.storage(),
                    1_000_000_000,
                    1024 * 1024 * 1024,
                );
                assert!(full.result.is_ok(), "{:?}", full.result);
                assert_eq!((full.failed_work, full.failed_storage), (None, None));
                let exact = measure(owner, &roots, budget.storage(), full.work, full.peak);
                assert!(exact.result.is_ok(), "{:?}", exact.result);
                assert_eq!(
                    (
                        exact.work,
                        exact.peak,
                        exact.failed_work,
                        exact.failed_storage
                    ),
                    (full.work, full.peak, None, None)
                );
                let short = measure(owner, &roots, budget.storage(), full.work - 1, full.peak);
                assert!(
                    matches!(short.result, Err(DescriptorError::Resource(Resource::Work(error)))
                    if error.actual() == full.work && error.limit() == full.work - 1)
                );
                assert_eq!(
                    (
                        short.work,
                        short.peak,
                        short.failed_work,
                        short.failed_storage
                    ),
                    (full.work - 1, full.peak, Some(full.work), None)
                );
            });
        }
    }
}

#[test]
fn refined_forwarding_descriptor_missing_owner_floor_is_accounting_refusal() {
    for erased in [false, true] {
        with_final(erased, Profile::Gfx942, true, |value, _budget| {
            let owner = borrowed(&value.owner);
            let roots = fixtures::typed_roots(owner);
            let mut work = Work::new(1_000_000);
            let mut budget = Budget::new(&mut work, usize::MAX);
            assert!(matches!(
                validate_final_descriptor_evidence_v1(owner, &roots, Profile::Gfx942, &mut budget),
                Err(DescriptorError::Resource(Resource::Accounting))
            ));
            assert_eq!(
                (budget.storage(), budget.work(), budget.peak_storage()),
                (0, 0, 0)
            );
            assert_eq!(budget.failed_storage(), None);
        });
    }
}

#[test]
fn refined_forwarding_descriptor_header_first_denial_and_panic_cleanup() {
    let sibling = [0x31u8; 37];
    let floor = size_of_val(&sibling);
    let header = fixtures::header();
    for short in [false, true] {
        let mut work = Work::new(18);
        let mut budget = Budget::new(&mut work, floor + header - usize::from(short));
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(17).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = fixtures::header_scope(&mut budget, false);
        if short {
            assert!(
                matches!(result, Err(DescriptorError::Resource(Resource::Storage(error)))
                if error.actual() == floor + header && error.limit() == floor + header - 1)
            );
            assert_eq!(
                (budget.peak_storage(), budget.failed_storage()),
                (floor, Some(floor + header))
            );
        } else {
            result.unwrap();
            assert_eq!(
                (budget.peak_storage(), budget.failed_storage()),
                (floor + header, None)
            );
        }
        assert_eq!((budget.storage(), budget.work()), (floor, 18));
        assert!(budget.work_ledger_identity_v1() == ledger);
    }
    let mut work = Work::new(18);
    let mut budget = Budget::new(&mut work, floor + header);
    budget.reserve_storage(floor).unwrap();
    budget.charge_work(17).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    assert!(matches!(
        fixtures::header_scope(&mut budget, true),
        Err(DescriptorError::Panicked)
    ));
    assert_eq!(
        (
            budget.storage(),
            budget.work(),
            budget.peak_storage(),
            budget.failed_storage()
        ),
        (floor, 18, floor + header, None)
    );
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(sibling, [0x31; 37]);
}
