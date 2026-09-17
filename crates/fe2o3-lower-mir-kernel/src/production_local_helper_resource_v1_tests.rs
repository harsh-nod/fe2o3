use super::*;

#[test]
fn captured_event_constant_and_statement_indexes_have_seven_field_logarithmic_queries() {
    let owner = unit_owner(UnitCase::CastAssert { expected: true }, &[1]);
    let capture = owner.semantic_ssa().occurrences_v1().unwrap();
    let helper = capture
        .function(SemanticFunctionIdV1::from_index(1))
        .unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    budget
        .reserve_storage(FLOOR + owner.retained_analysis_storage_v1())
        .unwrap();
    let incoming = budget.storage();
    with_canonical_call_scratch_v1(&mut budget, |budget| {
        budget.reserve_storage(std::mem::size_of::<[Vec<UnitLocalSourceIndexV1>; 3]>())?;
        let mut events = unit_local_vec_v1(helper.events().len(), budget)?;
        for (index, row) in helper.events().iter().enumerate() {
            unit_local_push_v1(&mut events, UnitLocalSourceIndexV1 {
                key: unit_local_source_key_v1(row.site(), row.operand(), Some(row.role())), index,
            }, budget)?;
        }
        let mut constants = unit_local_vec_v1(helper.constants().len(), budget)?;
        for (index, row) in helper.constants().iter().enumerate() {
            unit_local_push_v1(&mut constants, UnitLocalSourceIndexV1 {
                key: unit_local_source_key_v1(row.site(), row.operand(), None), index,
            }, budget)?;
        }
        let count = owner.correspondence.statement_operation_spans.iter().filter(|row| {
            row.correspondence_owner == ARRAY_ROOT && row.semantic_function.index() == 1
        }).count();
        let mut statements = unit_local_vec_v1(count, budget)?;
        for (index, row) in owner.correspondence.statement_operation_spans.iter().enumerate() {
            if row.correspondence_owner != ARRAY_ROOT || row.semantic_function.index() != 1 { continue; }
            unit_local_push_v1(&mut statements, UnitLocalSourceIndexV1 {
                key: [u64::from(row.semantic_block.index()), u64::from(row.statement_ordinal), 0, 0, 0, 0, 0], index,
            }, budget)?;
        }
        let mut families = [events, constants, statements];
        for rows in &mut families {
            assert!(rows.len() > 8, "actual captured fixture must exercise a multi-level search");
            unit_local_source_sort_v1(rows, budget)?;
            let height = (usize::BITS - rows.len().leading_zeros()) as usize;
            for (position, row) in rows.iter().enumerate() {
                let before = budget.work();
                let found = unit_local_source_find_v1(rows, row.key, budget)?;
                assert_eq!(found, position);
                assert_eq!(rows[found].index, row.index);
                assert!(budget.work() - before <= 8 * height);
            }
            // Querying the first key takes exactly height comparisons. A
            // failed final seven-field debit keeps the paid loop step only.
            let exact = 8 * height;
            let floor = budget.storage();
            for limit in [exact - 1, exact] {
                let mut query_work = CanonicalKernelIrWorkBudgetV1::new(limit);
                let mut query = ArgumentBudgetV1::new(&mut query_work, STORAGE);
                query.reserve_storage(floor)?;
                let result = unit_local_source_find_v1(rows, rows[0].key, &mut query);
                if limit == exact {
                    assert_eq!(result?, 0);
                    assert_eq!(query.work(), exact);
                } else {
                    assert!(matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Work(error)
                    )) if error.actual() == exact && error.limit() == exact - 1));
                    assert_eq!(query.work(), exact - 7);
                }
                assert_eq!((query.storage(), query.peak_storage()), (floor, floor));
            }
            let before = budget.work();
            assert!(matches!(unit_local_source_find_v1(rows, [u64::MAX; 7], budget),
                Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)));
            assert!(budget.work() - before <= 8 * height);
            let first = rows[0];
            let second = rows[1];
            rows[1].key = first.key;
            assert!(matches!(unit_local_source_sort_v1(rows, budget),
                Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)));
            assert_ne!(first.index, second.index);
        }
        drop(families);
        Ok(())
    }).unwrap();
    assert_eq!(budget.storage(), incoming);
    let retained = owner.retained_analysis_storage_v1();
    drop(owner);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

fn helper_payload(owner: &ProductionPreRankedKirOwnerV1) -> usize {
    let helper = &owner.helper_memory;
    let source = &helper.unit_source;
    std::mem::size_of::<SealedHelperMemoryV1>()
        + helper.functions.capacity() * std::mem::size_of::<RetainedHelperKindV1>()
        + helper.associations.capacity() * std::mem::size_of::<RetainedHelperAssociationV1>()
        + helper.allocations.capacity() * std::mem::size_of::<RetainedLocalAllocationV1>()
        + helper.accesses.capacity() * std::mem::size_of::<RetainedLocalAccessV1>()
        + helper.control.capacity() * std::mem::size_of::<RetainedLocalControlV1>()
        + helper.edge_bindings.capacity() * std::mem::size_of::<RetainedLocalEdgeBindingV1>()
        + source.bodies.capacity() * std::mem::size_of::<UnitLocalBodyRowV1>()
        + source.associations.capacity() * std::mem::size_of::<UnitLocalAssociationRowV1>()
        + source.values.capacity() * std::mem::size_of::<UnitLocalValueRowV1>()
        + source.memory.capacity() * std::mem::size_of::<UnitLocalMemoryRowV1>()
        + source.control.capacity() * std::mem::size_of::<UnitLocalControlRowV1>()
        + source.calls.capacity() * std::mem::size_of::<UnitLocalCallRowV1>()
}

#[test]
fn source_scope_rejects_a_different_real_owner_inventory_before_its_header() {
    let owner = unit_owner(UnitCase::Initializer, &[1]);
    let other = unit_owner(UnitCase::Initializer, &[1]);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    let retained = owner.retained_analysis_storage_v1() + other.retained_analysis_storage_v1();
    budget.reserve_storage(FLOOR + retained).unwrap();
    let (inventory, storage) =
        CanonicalKirInventoryV1::derive(other.executable(), &mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    assert!(!inventory.belongs_to(owner.executable()));
    let (before, floor, peak) = (budget.work(), budget.storage(), budget.peak_storage());
    assert!(matches!(
        owner.with_checked_unit_local_source_v1(
            &inventory,
            &mut budget,
            |_, _| -> Result<(), ProductionSemanticKirErrorV1> {
                panic!("foreign inventory must not enter")
            }
        ),
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (before + 9, floor, peak)
    );
    assert_eq!(budget.failed_storage(), None);
    drop(inventory);
    budget.release_storage(storage.retained_storage()).unwrap();
    drop(owner);
    drop(other);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn captured_source_payload_is_transferred_once_or_preserves_the_existing_reservation() {
    for preexisting in [false, true] {
        let (mut ssa, launch) = unit_source(UnitCase::Initializer, &[1]);
        assert!(ssa.occurrence_storage().is_none());
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        let old_capture = if preexisting {
            let receipt = ssa
                .try_capture_occurrences_with_budget_v1(&mut budget)
                .unwrap();
            assert_eq!(budget.storage(), FLOOR);
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            Some(receipt)
        } else {
            None
        };
        let incoming = budget.storage();
        let owner = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
            ssa,
            launch,
            ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        )
        .unwrap();
        assert_eq!(budget.storage(), incoming);
        let receipt = owner.semantic_ssa().occurrence_storage().unwrap();
        assert!(receipt.retained_storage() > 0);
        match (preexisting, owner.helper_memory.capture) {
            (true, HelperOccurrenceCaptureV1::Preexisting(actual)) => {
                assert_eq!(
                    actual.retained_storage(),
                    old_capture.unwrap().retained_storage()
                );
            }
            (false, HelperOccurrenceCaptureV1::Transferred(actual)) => {
                assert_eq!(actual.retained_storage(), receipt.retained_storage());
            }
            _ => panic!("exact capture custody must survive construction"),
        }
        let helper = helper_payload(&owner);
        assert_eq!(owner.helper_memory_storage_v1().retained_storage(), helper);
        let total = owner.executable_storage().retained_storage()
            + owner.assert_origin_storage().payload_storage()
            + helper
            + if preexisting {
                0
            } else {
                receipt.retained_storage()
            };
        assert_eq!(owner.retained_analysis_storage_v1(), total);
        budget.reserve_storage(total).unwrap();
        assert_eq!(
            budget.storage(),
            FLOOR
                + owner.executable_storage().retained_storage()
                + owner.assert_origin_storage().payload_storage()
                + helper
                + receipt.retained_storage()
        );
        if preexisting {
            let (inventory, inventory_storage) =
                CanonicalKirInventoryV1::derive(owner.executable(), &mut budget).unwrap();
            budget
                .reserve_storage(inventory_storage.retained_storage())
                .unwrap();
            let full = budget.storage();
            let short = total + receipt.retained_storage() - 1;
            let missing = full - short;
            budget.release_storage(missing).unwrap();
            let before = budget.work();
            assert!(matches!(
                owner.with_checked_unit_local_source_v1(
                    &inventory,
                    &mut budget,
                    |_, _| -> Result<(), ProductionSemanticKirErrorV1> {
                        panic!("missing capture cannot enter")
                    }
                ),
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Accounting
                    )
                )
            ));
            assert_eq!((budget.work(), budget.storage()), (before + 17, short));
            budget.reserve_storage(missing).unwrap();
            owner
                .with_checked_unit_local_source_v1(&inventory, &mut budget, |view, budget| {
                    assert!(
                        view.association(ARRAY_ROOT, SemanticFunctionIdV1::from_index(1), budget)?
                            .is_some()
                    );
                    Ok(())
                })
                .unwrap();
            assert_eq!(budget.storage(), full);
            drop(inventory);
            budget
                .release_storage(inventory_storage.retained_storage())
                .unwrap();
        }
        drop(owner);
        budget.release_storage(total).unwrap();
        if let Some(receipt) = old_capture {
            budget.release_storage(receipt.retained_storage()).unwrap();
        }
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn constructor_capture_preflight_is_two_work_before_storage_or_pending_lowering() {
    let (ssa, launch) = unit_source(UnitCase::Initializer, &[1]);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    assert!(
        matches!(ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa, launch, ProductionSemanticKirLimitsV1::default(), &mut budget,
    ), Err(ProductionPreRankedKirErrorV1::Lowering(
        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
            fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Work(error)
        )
    )) if error.actual() == 2 && error.limit() == 1)
    );
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (0, FLOOR, FLOOR)
    );
    assert_eq!(budget.failed_storage(), None);
}

#[test]
fn preexisting_capture_missing_one_byte_is_not_recharged_or_recreated() {
    let (mut ssa, launch) = unit_source(UnitCase::Initializer, &[1]);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    let capture = ssa
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(capture.retained_storage()).unwrap();
    budget.release_storage(1).unwrap();
    let before_work = budget.work();
    let short = capture.retained_storage() - 1;
    assert!(matches!(
        ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
            ssa,
            launch,
            ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        ),
        Err(ProductionPreRankedKirErrorV1::Lowering(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Accounting
            )
        ))
    ));
    assert_eq!(budget.work(), before_work + 2);
    assert_eq!(budget.storage(), short);
    assert_eq!(budget.failed_storage(), None);
    budget.release_storage(short).unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn helper_header_refusal_keeps_live_source_capture_and_reports_the_actual_header() {
    let owner = unit_owner(UnitCase::ReadThenWrite, &[1]);
    let floor = FLOOR + owner.retained_analysis_storage_v1();
    let header = std::mem::size_of::<SealedHelperMemoryV1>();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, floor + header - 1);
    budget.reserve_storage(floor).unwrap();
    assert!(matches!(SealedHelperMemoryV1::derive_with_origins_v1(
        CanonicalCallSubjectV1::owner(&owner), Some(&owner.assert_origins), &mut budget,
    ), Err(ProductionPreRankedKirErrorV1::Lowering(
        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
            fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Storage(error)
        )
    )) if error.actual() == floor + header && error.limit() == floor + header - 1));
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (4, floor, floor)
    );
    assert_eq!(budget.failed_storage(), Some(floor + header));
}

fn with_inventory(
    owner: &ProductionPreRankedKirOwnerV1,
    next: impl FnOnce(&CanonicalKirInventoryV1<'_>, usize),
) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    let owner_storage = owner.retained_analysis_storage_v1();
    budget.reserve_storage(FLOOR + owner_storage).unwrap();
    let (inventory, receipt) =
        CanonicalKirInventoryV1::derive(owner.executable(), &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    next(&inventory, budget.storage());
    drop(inventory);
    budget.release_storage(receipt.retained_storage()).unwrap();
    budget.release_storage(owner_storage).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn scoped_source_lookup_has_literal_entry_and_single_association_boundaries() {
    let owner = unit_owner(UnitCase::Initializer, &[1]);
    with_inventory(&owner, |inventory, floor| {
        let header = std::mem::size_of::<ProductionUnitLocalSourceV1<'_>>();
        // Scratch2 + physical scope7 + source scope8; query guard5 + iteration1 + pair2.
        for (limit, accepted, attempted, entered, success) in [
            (16, 9, Some(17), false, false),
            (21, 17, Some(22), true, false),
            (24, 23, Some(25), true, false),
            (25, 25, None, true, true),
        ] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
            let mut budget = ArgumentBudgetV1::new(&mut work, floor + header);
            budget.reserve_storage(floor).unwrap();
            let mut observed_entry = false;
            let result =
                owner.with_checked_unit_local_source_v1(inventory, &mut budget, |view, budget| {
                    observed_entry = true;
                    assert!(view.belongs_to(inventory));
                    assert_eq!(view.association_count(), 1);
                    let association = view
                        .association(ARRAY_ROOT, SemanticFunctionIdV1::from_index(1), budget)?
                        .unwrap();
                    assert_eq!(
                        (association.root(), association.function()),
                        (ARRAY_ROOT, SemanticFunctionIdV1::from_index(1))
                    );
                    assert_eq!(association.call_count(), 1);
                    assert!(
                        association.value_count() > 0
                            && association.memory_count() > 0
                            && association.control_count() > 0
                    );
                    Ok(())
                });
            if success {
                result.unwrap();
            } else {
                assert!(
                    matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Work(error)
                )) if Some(error.actual()) == attempted && error.limit() == limit)
                );
            }
            assert_eq!(observed_entry, entered);
            assert_eq!((budget.work(), budget.storage()), (accepted, floor));
            assert_eq!(
                budget.peak_storage(),
                floor + if entered { header } else { 0 }
            );
            assert_eq!(budget.failed_storage(), None);
        }
        let mut work = CanonicalKernelIrWorkBudgetV1::new(17);
        let mut budget = ArgumentBudgetV1::new(&mut work, floor + header - 1);
        budget.reserve_storage(floor).unwrap();
        assert!(
            matches!(owner.with_checked_unit_local_source_v1(inventory, &mut budget, |_, _| -> Result<(), ProductionSemanticKirErrorV1> {
            panic!("short header cannot enter callback")
        }), Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
            fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Storage(error)
        )) if error.actual() == floor + header && error.limit() == floor + header - 1)
        );
        assert_eq!(
            (budget.work(), budget.storage(), budget.peak_storage()),
            (17, floor, floor)
        );
    });
}

#[test]
fn scoped_source_cleanup_preserves_payloads_or_reports_every_floor_imbalance() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let owner = unit_owner(UnitCase::Initializer, &[1]);
    with_inventory(&owner, |inventory, floor| {
        for imbalance in 0..4 {
            for exit in 0..3 {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
                let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
                budget.reserve_storage(floor).unwrap();
                let result = catch_unwind(AssertUnwindSafe(|| {
                    owner.with_checked_unit_local_source_v1(inventory, &mut budget, |_, budget| {
                        match imbalance {
                            0 => {}
                            1 => budget.reserve_storage(1)?,
                            2 => budget.release_storage(1)?,
                            _ => budget.release_storage(budget.storage() - (floor - 1))?,
                        }
                        match exit {
                            0 => Ok(37usize),
                            1 => Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch),
                            _ => std::panic::panic_any("source scope panic"),
                        }
                    })
                }));
                if imbalance != 0 {
                    assert!(matches!(result, Ok(Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Accounting
                    )))));
                } else {
                    match exit {
                        0 => assert!(matches!(result, Ok(Ok(37)))),
                        1 => assert!(matches!(
                            result,
                            Ok(Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch))
                        )),
                        _ => assert_eq!(
                            result.unwrap_err().downcast_ref::<&str>(),
                            Some(&"source scope panic")
                        ),
                    }
                }
                assert_eq!(
                    budget.storage(),
                    if imbalance == 3 { floor - 1 } else { floor }
                );
                if imbalance == 3 {
                    budget.reserve_storage(1).unwrap();
                }
                owner
                    .with_checked_unit_local_source_v1(inventory, &mut budget, |view, budget| {
                        assert!(
                            view.association(
                                ARRAY_ROOT,
                                SemanticFunctionIdV1::from_index(1),
                                budget
                            )?
                            .is_some()
                        );
                        Ok(())
                    })
                    .unwrap();
                assert_eq!(budget.storage(), floor);
            }
        }
    });
}

fn replaced_scope<'w>(
    owner: &ProductionPreRankedKirOwnerV1,
    inventory: &CanonicalKirInventoryV1<'_>,
    budget: &mut ArgumentBudgetV1<'w>,
    foreign: &mut ArgumentBudgetV1<'w>,
    query: bool,
    exit: usize,
) {
    let incoming = budget.storage();
    let before = budget.work();
    let header = std::mem::size_of::<ProductionUnitLocalSourceV1<'_>>();
    assert_eq!(foreign.storage(), incoming + header);
    let foreign_work = foreign.work();
    let result = owner.with_checked_unit_local_source_v1(inventory, budget, |view, budget| {
        std::mem::swap(budget, foreign);
        if query {
            assert!(matches!(
                view.association(ARRAY_ROOT, SemanticFunctionIdV1::from_index(1), budget),
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Accounting
                    )
                )
            ));
        }
        match exit {
            0 => Ok(()),
            1 => Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch),
            _ => std::panic::panic_any("foreign source scope panic"),
        }
    });
    assert!(matches!(
        result,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Accounting
            )
        )
    ));
    assert_eq!(budget.storage(), incoming + header);
    assert_eq!(budget.peak_storage(), incoming + header);
    assert_eq!(budget.failed_storage(), None);
    assert_eq!(budget.work(), foreign_work + if query { 5 } else { 0 });
    assert_eq!(foreign.storage(), incoming + header);
    assert_eq!(foreign.work(), before + 17);
    std::mem::swap(budget, foreign);
    // No failed foreign postflight may release the original ledger's live header.
    budget.release_storage(header).unwrap();
    owner
        .with_checked_unit_local_source_v1(inventory, budget, |view, budget| {
            assert!(
                view.association(ARRAY_ROOT, SemanticFunctionIdV1::from_index(1), budget)?
                    .is_some()
            );
            Ok(())
        })
        .unwrap();
    assert_eq!(budget.storage(), incoming);
}

#[test]
fn scoped_source_same_slot_work_replacement_never_cleans_foreign_storage() {
    let owner = unit_owner(UnitCase::Initializer, &[1]);
    with_inventory(&owner, |inventory, floor| {
        for query in [false, true] {
            for exit in 0..3 {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
                let mut other_work = CanonicalKernelIrWorkBudgetV1::new(WORK);
                let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
                let mut foreign = ArgumentBudgetV1::new(&mut other_work, STORAGE);
                budget.reserve_storage(floor).unwrap();
                foreign
                    .reserve_storage(floor + std::mem::size_of::<ProductionUnitLocalSourceV1<'_>>())
                    .unwrap();
                replaced_scope(&owner, inventory, &mut budget, &mut foreign, query, exit);
            }
        }
    });
}
