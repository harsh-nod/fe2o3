use super::tests::{FLOOR, LIMIT, Shape, fixed_fixture, fixture, prepare};
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1;
use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as ViewError;

#[test]
fn original_packing_cursor_has_independent_exact_work_and_no_heap_credit() {
    for deferred in [false, true] {
        let fixture = if deferred {
            fixture(Shape::Record, [0, 1, 2, 3], 1)
        } else {
            fixed_fixture()
        };
        let count = fixture.roots[0].arguments.len();
        let required = 12 + 12 * count;
        for limit in [required, required - 1] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
            let mut budget = Budget::new(&mut work, FLOOR);
            budget.reserve_storage(FLOOR).unwrap();
            let result = Packing::new(
                &fixture.roots[0],
                fixture.owner.source_semantic(),
                &fixture.owner.source_semantic().functions()[0],
                &mut budget,
            );
            if limit == required {
                let mut cursor = result.unwrap();
                for ordinal in 0..count {
                    cursor.offset(ordinal).unwrap();
                }
                assert!(cursor.finish().unwrap().bytes > 0);
                assert_eq!(budget.work(), required);
            } else {
                assert!(
                    matches!(result, Err(Error::SourceOwnedEntrance(ViewError::Resource(Resource::Work(error))))
                    if error.actual() == required && error.limit() == limit)
                );
                assert_eq!(budget.work(), 0);
            }
            assert_eq!((budget.storage(), budget.peak_storage()), (FLOOR, FLOOR));
        }
    }
}

#[test]
fn actual_preparation_pays_exact_projection_header_before_any_argument_storage() {
    // Independent field layout: one three-word Vec header, then two u32 extent
    // fields, rounded to the maximum field alignment. No measured peak oracle.
    let alignment = std::mem::align_of::<usize>().max(std::mem::align_of::<u32>());
    let unpadded = 3 * std::mem::size_of::<usize>() + 2 * std::mem::size_of::<u32>();
    let header = (unpadded + alignment - 1) & !(alignment - 1);
    assert_eq!(std::mem::size_of::<RootArguments>(), header);
    // Each fallible vector also owns its three-word header before allocation.
    let vector_header = 3 * std::mem::size_of::<usize>();
    for roots in [1, 2] {
        for short in [true, false] {
            let fixture = fixture(Shape::Record, [0, 1, 2, 3], roots);
            let projection_storage = vector_header + roots * header;
            let limit = FLOOR + projection_storage - usize::from(short);
            let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
            let mut budget = Budget::new(&mut work, limit);
            budget.reserve_storage(FLOOR).unwrap();
            let result = prepare(fixture, &mut budget);
            let expected = FLOOR
                + projection_storage
                + if short {
                    0
                } else {
                    vector_header + 4 * std::mem::size_of::<Argument>()
                };
            assert!(
                matches!(result, Err(Error::SourceOwnedEntrance(ViewError::Resource(Resource::Storage(error))))
                if error.actual() == expected && error.limit() == limit)
            );
            assert_eq!(
                budget.peak_storage(),
                if short { FLOOR } else { FLOOR + projection_storage }
            );
            assert_eq!(budget.storage(), FLOOR);
        }
    }
}

#[test]
fn original_cursor_rejects_skipped_repeated_and_incomplete_visits_without_credit_growth() {
    let fixture = fixture(Shape::Record, [0, 1, 2, 3], 1);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = Budget::new(&mut work, FLOOR);
    budget.reserve_storage(FLOOR).unwrap();
    for mode in 0..3 {
        let mut cursor = Packing::new(
            &fixture.roots[0],
            fixture.owner.source_semantic(),
            &fixture.owner.source_semantic().functions()[0],
            &mut budget,
        )
        .unwrap();
        match mode {
            0 => assert!(cursor.offset(1).is_err()),
            1 => {
                assert_eq!(cursor.offset(0).unwrap(), 0);
                assert!(cursor.offset(0).is_err());
            }
            2 => {
                assert_eq!(cursor.offset(0).unwrap(), 0);
                assert!(cursor.finish().is_err());
            }
            _ => unreachable!(),
        }
    }
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn actual_prepared_capture_adopts_original_occurrences_and_cleans_success_error_and_panic_once() {
    for inherited in [false, true] {
        for outcome in 0..3 {
            let mut fixture = fixture(Shape::Record, [0, 1, 2, 3], 1);
            let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            budget.reserve_storage(FLOOR).unwrap();
            let inherited_credit = if inherited {
                let receipt = fixture
                    .owner
                    .try_capture_occurrences_with_budget_v1(&mut budget)
                    .unwrap();
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                receipt.retained_storage()
            } else {
                0
            };
            let prepared = prepare(fixture, &mut budget).unwrap();
            assert!(prepared.adopted_storage() > inherited_credit);
            assert_eq!(budget.storage(), FLOOR + prepared.adopted_storage());
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                prepared.with_checked_source_v18(&mut budget, |view, budget| {
                    assert!(view.source_ssa(budget)?.occurrence_storage().is_some());
                    assert!(
                        view.kernel_argument_by_value_abi_v29(0, 1, budget)?
                            .is_some()
                    );
                    match outcome {
                        0 => Ok(()),
                        1 => Err(ViewError::Binding("original callback refusal")),
                        2 => std::panic::panic_any(0x52494e4cu64),
                        _ => unreachable!(),
                    }
                })
            }));
            match outcome {
                0 => result.unwrap().unwrap(),
                1 => assert!(matches!(
                    result.unwrap(),
                    Err(ViewError::Binding("original callback refusal"))
                )),
                2 => assert_eq!(
                    result.unwrap_err().downcast_ref::<u64>(),
                    Some(&0x52494e4cu64)
                ),
                _ => unreachable!(),
            }
            assert_eq!(budget.storage(), FLOOR);
        }
    }
}

#[test]
fn refused_original_registration_drops_unadopted_occurrence_credit_without_new_source_owner() {
    let mut fixture = fixture(Shape::Record, [0, 1, 2, 3], 1);
    fixture.roots[0].kernarg_alignment_bytes = 8;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let receipt = fixture
        .owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    assert!(matches!(
        prepare(fixture, &mut budget),
        Err(Error::DescriptorEvidence(_))
    ));
    assert_eq!(budget.storage(), FLOOR);
}
