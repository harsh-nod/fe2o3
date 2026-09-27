use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
use std::mem::{align_of, size_of};

#[test]
fn private_bridge_entrance_headers_precede_root_query_and_source_only_closures() {
    type Call<'a> = (
        &'a Original<'a>,
        &'a Optimized<'a>,
        &'a Request<'a>,
        &'a mut Budget<'a>,
    );
    type Outer<'a> = (&'a Original<'a>, &'a Request<'a>);
    type Args<'a> = (
        &'a fe2o3_lower_mir_kernel::ProductionOptimizedSourceScalarLeavesV18<'a>,
        &'a mut Budget<'a>,
    );
    type Inner<'a> = (
        &'a Original<'a>,
        &'a fe2o3_lower_mir_kernel::ProductionOptimizedSourceScalarLeavesV18<'a>,
        &'a Request<'a>,
    );
    let expected = size_of::<Call<'_>>()
        + 2 * align_of::<Call<'_>>()
        + size_of::<Outer<'_>>()
        + 2 * align_of::<Outer<'_>>()
        + size_of::<Args<'_>>()
        + 2 * align_of::<Args<'_>>()
        + size_of::<Inner<'_>>()
        + 2 * align_of::<Inner<'_>>()
        + size_of::<usize>()
        + size_of::<Result<usize, SourceError>>()
        + size_of::<&SourceLeaves<'_>>()
        + size_of::<Result<&SourceLeaves<'_>, SourceError>>()
        + 3 * size_of::<Result<(), ProductionRankedProjectionErrorV1>>()
        + 3 * size_of::<Result<(), SourceError>>();
    assert_eq!(private_root_bridge_entrance_headers_v18(), Some(expected));
}

#[test]
fn private_bridge_scratch_header_equation_and_overflow_are_independent() {
    let fixed = 2 * size_of::<&Original<'_>>()
        + size_of::<&Optimized<'_>>()
        + size_of::<&Request<'_>>()
        + size_of::<&SourceLeaves<'_>>()
        + 2 * size_of::<&mut Budget<'_>>()
        + size_of::<&fe2o3_lower_mir_kernel::ProductionOptimizedSourceScalarLeavesV18<'_>>()
        + size_of::<Result<&SourceLeaves<'_>, SourceError>>()
        + size_of::<&fe2o3_lower_mir_kernel::ProductionSourceOwnedViewV18<'_>>()
        + size_of::<Result<&fe2o3_lower_mir_kernel::ProductionSourceOwnedViewV18<'_>, SourceError>>(
        )
        + size_of::<&AdmittedInertSemanticMirV1>()
        + size_of::<Result<&AdmittedInertSemanticMirV1, SourceError>>()
        + size_of::<Result<usize, SourceError>>()
        + 4 * size_of::<usize>()
        + size_of::<Option<usize>>()
        + 2 * size_of::<Result<(), SourceError>>()
        + 4 * size_of::<Result<(), ProductionRankedProjectionErrorV1>>()
        + 2 * size_of::<Result<(), Resource>>()
        + size_of::<std::thread::Result<Result<(), ProductionRankedProjectionErrorV1>>>()
        + 3 * size_of::<bool>()
        + 2 * size_of::<&()>()
        + 2 * align_of::<&()>();
    assert_eq!(private_root_bridge_headers(13, 8), Some(fixed + 13 + 16));
    assert_eq!(private_root_bridge_headers(usize::MAX, 1), None);
    assert_eq!(private_root_bridge_headers(0, usize::MAX), None);
}

// Genuine original-source ownership is supplied by a fresh actual rustc
// session for each mode. No artificial source/native owner is constructed.
pub(crate) fn inspect_actual_private_bridge_scratch_v18(
    view: &fe2o3_lower_mir_kernel::ProductionSourceOwnedViewV18<'_>,
    budget: &mut Budget<'_>,
    mode: u8,
    observed: &std::cell::Cell<Option<[usize; 6]>>,
) -> Result<(), SourceError> {
    let before_scope = budget.storage();
    let checked = view.with_analysis_v18(budget, |scope| scope.with_inventory_v1(|inventory, budget| {
        view.with_ranked_correspondence_v18(inventory, budget, |original, budget| {
            original.with_source_scalar_leaves_v18(0, budget, |leaves, budget| {
                let before = budget.storage();
                let entered = std::cell::Cell::new(false);
                let run = |budget: &mut Budget<'_>| -> Result<(), ProductionRankedProjectionErrorV1> {
                    entered.set(true);
                    match mode {
                        0 | 6 | 7 => Ok(()),
                        1 | 2 | 3 => {
                            if mode == 2 { budget.reserve_storage(32).unwrap(); }
                            if mode == 3 { budget.release_storage(1).unwrap(); }
                            Err(SourceError::Binding("selected private bridge error").into())
                        }
                        4 | 5 => {
                            if mode == 4 { budget.release_storage(1).unwrap(); }
                            std::panic::panic_any("selected private bridge panic")
                        }
                        8 => {
                            let limit = usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT).unwrap();
                            let error = budget.charge_work(limit - budget.work() + 1).unwrap_err();
                            let _ = original.retain_query_resource_error_v18(error);
                            Ok(())
                        }
                        _ => panic!("unknown private bridge scratch mode"),
                    }
                };
                let headers = private_root_bridge_headers(std::mem::size_of_val(&run),
                    std::mem::align_of_val(&run)).unwrap();
                let fill = if mode == 6 || mode == 7 {
                    let limit = crate::production_canonical_phase_policy_v1::STORAGE_LIMIT;
                    let fill = limit - before - headers + usize::from(mode == 6);
                    budget.reserve_storage(fill).unwrap();
                    fill
                } else { 0 };
                let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    with_private_root_bridge_scratch(original, leaves, budget, run)
                }));
                let after = budget.storage();
                observed.set(Some([before, after, headers, usize::from(entered.get()), before_scope, 0]));
                match mode {
                    0 | 7 => {
                        caught.unwrap()?;
                        assert_eq!(after, before + fill);
                        budget.release_storage(fill).unwrap();
                        Ok(())
                    }
                    1 | 2 | 3 => {
                        let error = caught.unwrap().unwrap_err();
                        assert!(matches!(&error, ProductionRankedProjectionErrorV1::CanonicalAssertions(
                            CanonicalAssertionErrorV1::SourceOwned(SourceError::Binding("selected private bridge error")))));
                        assert_eq!(after, match mode { 1 => before, 2 => before + 32, _ => before + headers - 1 });
                        Err(error)
                    }
                    4 | 5 => {
                        let panic = caught.unwrap_err();
                        assert_eq!(panic.downcast_ref::<&str>(), Some(&"selected private bridge panic"));
                        assert_eq!(after, if mode == 4 { before + headers - 1 } else { before });
                        Err(SourceError::Binding("selected private bridge panic").into())
                    }
                    6 => {
                        assert!(!entered.get());
                        let error = caught.unwrap().unwrap_err();
                        assert!(matches!(&error, ProductionRankedProjectionErrorV1::CanonicalAssertions(
                            CanonicalAssertionErrorV1::SourceOwned(SourceError::Resource(Resource::Storage(limit))))
                            if limit.limit() == crate::production_canonical_phase_policy_v1::STORAGE_LIMIT
                                && limit.actual() == limit.limit() + 1));
                        assert_eq!(after, before + fill);
                        Err(error)
                    }
                    8 => {
                        let error = caught.unwrap().unwrap_err();
                        assert!(matches!(&error, ProductionRankedProjectionErrorV1::CanonicalAssertions(
                            CanonicalAssertionErrorV1::SourceOwned(SourceError::Resource(Resource::Work(limit))))
                            if limit.limit() == usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT).unwrap()
                                && limit.actual() == limit.limit() + 1));
                        assert_eq!(after, before);
                        Err(error)
                    }
                    _ => unreachable!(),
                }
            })
        })
    }));
    let mut row = observed.get().expect("genuine scratch scope did not enter");
    row[5] = budget.storage();
    observed.set(Some(row));
    match checked {
        Ok(()) => {
            assert_eq!(budget.storage(), before_scope);
            Ok(())
        }
        Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
            CanonicalAssertionErrorV1::SourceOwned(error),
        )) => Err(error),
        Err(error) => panic!("unexpected private scratch source error: {error:?}"),
    }
}
