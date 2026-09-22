use super::*;
use fe2o3_kernel_ir::{CanonicalKernelIrWorkBudgetV1 as Work, Module};

const WORK: usize = 1_000_000_000;
const STORAGE: usize = 128 * 1024 * 1024;

fn actual(run: impl FnOnce(&Owner, &Owner, &[u8; 416], &mut Budget<'_>)) {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(19).unwrap();
    let (input, receipt) = Owner::from_module_ref_with_verification_budget_v12(
        &Module::new("integer-claim"),
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let observed =
        crate::optimize_native_neutral_kernel_ir_integer_continuation_v1(&input, &mut budget)
            .unwrap();
    budget
        .reserve_storage(observed.storage().retained_storage())
        .unwrap();
    let checked = observed.try_check_and_finish_v1(&mut budget).unwrap();
    budget
        .reserve_storage(checked.storage().retained_storage())
        .unwrap();
    let floor = budget.storage();
    run(
        &input,
        checked.owner(),
        checked.execution().canonical_bytes(),
        &mut budget,
    );
    assert_eq!(budget.storage(), floor);
}

#[test]
fn actual_record_has_exact_shared_roster_and_inert_borrowed_fields() {
    actual(|input, output, bytes, budget| {
        let before = budget.work();
        let claim =
            read_unauthenticated_integer_continuation_claim_v1(input, output, bytes, budget)
                .unwrap();
        assert!(std::ptr::eq(claim.canonical_bytes(), bytes));
        assert_eq!(claim.declared_map_digest().as_slice(), &bytes[264..296]);
        assert_eq!(
            claim.declared_profile_work(),
            u64::from_le_bytes(bytes[96..104].try_into().unwrap())
        );
        assert!(!claim.grants_authority());
        assert_eq!(budget.work() - before, 418);
        assert_eq!(&bytes[..8], &[6, 0, 1, 0, 2, 0, 0, 0]);
        assert_eq!(bytes[296], 7);
        assert_eq!(bytes[356], 1);
    });
}

#[test]
fn fixed_control_endpoints_caps_tags_booleans_and_padding_are_closed() {
    actual(|input, output, bytes, budget| {
        for offset in (0..88)
            .chain(120..128)
            .chain(136..176)
            .chain([296, 298, 299, 356, 358, 359])
        {
            let mut changed = *bytes;
            changed[offset] ^= 1;
            assert!(
                read_unauthenticated_integer_continuation_claim_v1(input, output, &changed, budget)
                    .is_err(),
                "fixed field byte {offset}"
            );
        }
        for offset in [297, 357] {
            let mut changed = *bytes;
            changed[offset] = 2;
            assert!(matches!(
                read_unauthenticated_integer_continuation_claim_v1(input, output, &changed, budget),
                Err(Error::Pass)
            ));
        }
        for record in [&bytes[..415], &[]] {
            assert!(matches!(
                read_unauthenticated_integer_continuation_claim_v1(input, output, record, budget),
                Err(Error::Framing)
            ));
        }
        let mut extra = [0; 417];
        extra[..416].copy_from_slice(bytes);
        assert!(matches!(
            read_unauthenticated_integer_continuation_claim_v1(input, output, &extra, budget),
            Err(Error::Framing)
        ));
    });
}

#[test]
fn dynamic_work_epoch_and_map_bytes_remain_unauthenticated_u64_claims() {
    actual(|input, output, bytes, budget| {
        for offset in (88..120)
            .chain(128..136)
            .chain(176..296)
            .chain(300..356)
            .chain(360..416)
        {
            let mut changed = *bytes;
            changed[offset] ^= 255;
            assert!(
                read_unauthenticated_integer_continuation_claim_v1(input, output, &changed, budget)
                    .is_ok(),
                "dynamic field byte {offset}"
            );
        }
    });
}

#[test]
fn exact_and_short_reader_work_leave_the_inherited_storage_unchanged() {
    actual(|input, output, bytes, _| {
        for (limit, accepted) in [(418, true), (417, false), (1, false)] {
            let mut work = Work::new(limit);
            let mut budget = Budget::new(&mut work, 19);
            budget.reserve_storage(19).unwrap();
            let result = read_unauthenticated_integer_continuation_claim_v1(
                input,
                output,
                bytes,
                &mut budget,
            );
            assert_eq!(result.is_ok(), accepted);
            assert_eq!(budget.storage(), 19);
            assert_eq!(budget.peak_storage(), 19);
            assert_eq!(
                budget.work(),
                if accepted {
                    418
                } else if limit == 1 {
                    0
                } else {
                    2
                }
            );
        }
    });
}

fn claim_cause_borrow<T: std::error::Error + 'static>(parent: &dyn std::error::Error, child: &T) {
    assert!(std::ptr::eq(
        parent.source().unwrap().downcast_ref::<T>().unwrap(),
        child
    ));
}

#[test]
fn claim_and_map_cause_links_borrow_resources_without_relabeling_markers() {
    use crate::{
        KirOptimizationMapErrorV12 as MapError, Policy3ExecutionClaimErrorV1 as Policy3Error,
    };
    let mut work = Work::new(3);
    let mut budget = Budget::new(&mut work, 5);
    let work_error = budget.charge_work(4).unwrap_err();
    let storage_error = budget.reserve_storage(6).unwrap_err();
    let accounting = budget.release_storage(1).unwrap_err();
    assert_eq!(accounting, Resource::Accounting);
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (0, 0, 0)
    );
    assert_eq!(budget.failed_storage(), Some(6));
    drop(budget);
    assert_eq!(work.failed_work(), Some(4));
    // Typed diagnostic cases are separate from the actual reader failure test.
    for resource in [
        work_error,
        storage_error,
        accounting,
        Resource::Allocation,
        Resource::Arithmetic,
    ] {
        macro_rules! check {
            ($ty:ident, $variant:ident) => {{
                let error = $ty::$variant(resource);
                let $ty::$variant(child) = &error else {
                    unreachable!()
                };
                claim_cause_borrow(&error, child);
                assert_eq!(*child, resource);
                match child {
                    Resource::Work(leaf) => {
                        claim_cause_borrow(child, leaf);
                        assert_eq!((leaf.actual(), leaf.limit()), (4, 3));
                    }
                    Resource::Storage(leaf) => {
                        claim_cause_borrow(child, leaf);
                        assert_eq!((leaf.actual(), leaf.limit()), (6, 5));
                    }
                    Resource::Allocation | Resource::Accounting | Resource::Arithmetic => {
                        assert!(std::error::Error::source(child).is_none())
                    }
                }
            }};
        }
        check!(Error, Resource);
        check!(Policy3Error, Resource);
        check!(MapError, Resources);
    }
    for error in [Error::Framing, Error::Endpoint, Error::Profile, Error::Pass] {
        assert!(std::error::Error::source(&error).is_none());
    }
    for error in [
        Policy3Error::Framing,
        Policy3Error::Endpoint,
        Policy3Error::Profile,
        Policy3Error::Pass,
    ] {
        assert!(std::error::Error::source(&error).is_none());
    }
    for error in [
        MapError::Arithmetic,
        MapError::Allocation,
        MapError::Limit,
        MapError::Identity,
        MapError::Lifecycle,
        MapError::Coverage,
        MapError::Passes,
        MapError::Relation,
        MapError::UnsupportedMutation,
    ] {
        assert!(std::error::Error::source(&error).is_none());
    }
}

#[test]
fn reached_integer_claim_cause_keeps_literal_work_schedule_and_caller_floor() {
    const PRIOR: usize = 11;
    actual(|input, output, bytes, _| {
        for allowance in [1, 417, 418] {
            let mut work = Work::new(PRIOR + allowance);
            work.charge_work(PRIOR).unwrap();
            let mut budget = Budget::new(&mut work, 19);
            budget.reserve_storage(19).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let result = read_unauthenticated_integer_continuation_claim_v1(
                input,
                output,
                bytes,
                &mut budget,
            );
            if allowance == 418 {
                let claim = result.unwrap();
                assert!(std::ptr::eq(claim.canonical_bytes(), bytes));
                assert!(!claim.grants_authority());
                assert_eq!(budget.work(), PRIOR + 418);
            } else {
                let error = result.err().expect("fixed reader work denial");
                let Error::Resource(child) = &error else {
                    panic!("{error:?}")
                };
                claim_cause_borrow(&error, child);
                let Resource::Work(leaf) = child else {
                    panic!("{child:?}")
                };
                claim_cause_borrow(child, leaf);
                let attempted = PRIOR + if allowance == 1 { 2 } else { 418 };
                assert_eq!(
                    (leaf.actual(), leaf.limit()),
                    (attempted, PRIOR + allowance)
                );
                assert_eq!(budget.work(), PRIOR + if allowance == 1 { 0 } else { 2 });
            }
            assert_eq!((budget.storage(), budget.peak_storage()), (19, 19));
            assert_eq!(budget.failed_storage(), None);
            assert!(budget.work_ledger_identity_v1() == ledger);
            drop(budget);
            assert_eq!(
                work.failed_work(),
                match allowance {
                    1 => Some(PRIOR + 2),
                    417 => Some(PRIOR + 418),
                    _ => None,
                }
            );
        }
    });
}
