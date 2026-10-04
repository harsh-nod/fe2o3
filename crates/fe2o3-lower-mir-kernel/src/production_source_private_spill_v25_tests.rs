use super::*;

#[path = "production_source_private_shared_capture_v26_tests.rs"]
mod shared_capture_v26;

fn mixed_owner(neutral: bool) -> ProductionSemanticSsaOwnerV1 {
    let base = private_entry_owner_v20(if neutral {
        PrivateEntryFixtureV20::Neutral
    } else {
        PrivateEntryFixtureV20::NonNeutral
    });
    let semantic = base.source_semantic();
    let mut types = semantic.types().to_vec();
    let shared = reference(&mut types, U32, SemanticMutabilityV1::Immutable, false);
    let mut functions = semantic.functions().to_vec();
    let leaf = &functions[3];
    let mut locals = leaf.locals().to_vec();
    locals[2] = local(162, shared, SemanticLocalRoleV1::Temporary);
    locals.push(local(164, shared, SemanticLocalRoleV1::Temporary));
    let mut tail = leaf.blocks()[0].statements()[1..].to_vec();
    // Keep the original shared holder live across a genuine edge while the
    // scalar access itself remains the exact original local occurrence.
    tail.insert(
        0,
        assign(
            place(4, shared),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(2, shared))),
        ),
    );
    functions[3] = function(
        150,
        leaf.role(),
        leaf.abi().clone(),
        locals,
        vec![
            block(
                170,
                vec![assign(
                    place(2, shared),
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Shared,
                        place: place(1, U32),
                    },
                )],
                SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::Goto,
                    SemanticBlockIdV1::from_index(1),
                )),
            ),
            block(171, tail, SemanticTerminatorKindV1::Return),
        ],
    );
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        semantic.callables().to_vec(),
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    let owner = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let plan = owner
        .plan_for_function(SemanticFunctionIdV1::from_index(3))
        .unwrap()
        .plan();
    assert!(
        plan.promoted_variables()
            .iter()
            .all(|variable| variable.get() != 1)
    );
    assert!(
        plan.live_in(fe2o3_mir_model::SsaBlockIdV1::new(1))
            .unwrap()
            .iter()
            .any(|variable| variable.get() == 2)
    );
    owner
}

fn mixed_neutral() -> ProductionSemanticSsaOwnerV1 {
    mixed_owner(true)
}
fn mixed_non_neutral() -> ProductionSemanticSsaOwnerV1 {
    mixed_owner(false)
}

fn census(owner: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18) -> [usize; 4] {
    let mut rows = [0; 4];
    for operation in owner
        .module()
        .functions
        .iter()
        .filter_map(|function| function.body.as_ref())
        .flat_map(|body| &body.blocks)
        .flat_map(|block| &block.operations)
    {
        match operation.kind {
            OperationKind::Load { .. } => rows[0] += 1,
            OperationKind::Store { .. } => rows[1] += 1,
            OperationKind::Storage(ScopedObjectOperationV29::ReadValue { .. }) => rows[2] += 1,
            OperationKind::Storage(ScopedObjectOperationV29::WriteValue { .. }) => rows[3] += 1,
            _ => {}
        }
    }
    rows
}

fn first_spill(
    original: &ProductionSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<(usize, usize)> {
    for root in 0..original.source.root_count(budget)? {
        for (slot, row) in original
            .source
            .root_row(root)?
            .source_slots
            .slots
            .iter()
            .enumerate()
        {
            budget.charge_work(1)?;
            if matches!(
                row.origin.identity,
                ScopedAllocationIdentityV29::LegacyLocal(_)
            ) && matches!(
                row.representation,
                ScopedSlotRepresentationV29::ScalarArray(_)
            ) {
                return Ok((root, slot));
            }
        }
    }
    panic!("genuine fixture has a retained scalar spill");
}

fn source_cause<'a>(
    mut error: &'a (dyn std::error::Error + 'static),
) -> &'a ProductionSourceOwnedViewErrorV18 {
    loop {
        if let Some(source) = error.downcast_ref::<ProductionSourceOwnedViewErrorV18>() {
            return source;
        }
        error = error.source().expect("typed original-source error chain");
    }
}

struct SpillFault;
impl SpillFault {
    fn new(mode: u8) -> Self {
        scoped_raw_admission_v29::SPILL_FAULT_V25
            .with(|value| assert!(value.replace(Some(mode)).is_none()));
        scoped_raw_admission_v29::SPILL_FAULT_FINISHED_V25.with(|value| value.set(false));
        Self
    }
    fn finished(&self) -> bool {
        scoped_raw_admission_v29::SPILL_FAULT_FINISHED_V25.with(std::cell::Cell::get)
    }
}
impl Drop for SpillFault {
    fn drop(&mut self) {
        scoped_raw_admission_v29::SPILL_FAULT_V25.with(|value| value.set(None));
    }
}

#[test]
fn private_spill_completion_preserves_genuine_mixed_scalar_and_object_memory() {
    for factory in [mixed_neutral as fn() -> _, mixed_non_neutral] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let (prepared, fixture) = integer_handoff_prepared_v18(factory, &mut budget);
        let roots = fixture.roots();
        let completed = std::cell::Cell::new(false);
        prepared
            .with_source_consumer_v18(&mut budget, |source, budget| {
                assert!(census(source.canonical(budget)?).into_iter().all(|n| n > 0));
                let floor = budget.storage();
                let handoff = source.private_completed_integer_output_v20(
                    ProductionKernelArgumentAbiInputV18 { roots: &roots },
                    budget,
                )?;
                handoff.check_original_source(source.source_ssa(budget)?, budget)?;
                assert!(
                    census(handoff.output(budget)?.owner())
                        .into_iter()
                        .all(|n| n > 0)
                );
                assert!(!handoff.output(budget)?.grants_authority());
                handoff.discard(budget)?;
                assert_eq!(budget.storage(), floor);
                completed.set(true);
                Ok::<_, ProductionPrivateSourceHandoffErrorV20>(())
            })
            .unwrap();
        assert!(completed.get());
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn private_spill_origin_is_borrowed_and_distinct_from_object_or_array_extent() {
    for copy in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = private_memory_prepared_v18(mixed_neutral, &mut budget).unwrap();
        let settled = std::cell::Cell::new(false);
        let result =
            with_production_optimizer_result_v18(prepared, &mut budget, |original, _, budget| {
                let (root, slot) = first_spill(original, budget)?;
                let mut object = false;
                for root in 0..original.source.root_count(budget)? {
                    for row in &original.source.root_row(root)?.source_slots.slots {
                        budget.charge_work(1)?;
                        object |= matches!(
                            row.origin.identity,
                            ScopedAllocationIdentityV29::OriginalObject { .. }
                        );
                    }
                }
                assert!(object, "typed companion remains a distinct original object");
                let floor = budget.storage();
                let checked = scoped_raw_admission_v29::test_spill_shape_v25(
                    original, root, slot, copy, budget,
                );
                if copy {
                    assert!(matches!(
                        &checked,
                        Err(ProductionSourceOwnedViewErrorV18::Binding(
                            "private scalar spill is not the retained original slot"
                        ))
                    ));
                    let stopped = budget.work();
                    assert!(matches!(
                        original.query(budget),
                        Err(ProductionSourceOwnedViewErrorV18::Binding(
                            "private scalar spill is not the retained original slot"
                        ))
                    ));
                    assert_eq!(budget.work(), stopped);
                } else {
                    checked.as_ref().unwrap();
                }
                budget.release_storage(budget.storage() - floor)?;
                assert_eq!(budget.storage(), floor);
                settled.set(true);
                checked
            });
        assert_eq!(result.is_ok(), !copy, "{result:?}");
        assert!(settled.get());
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn private_spill_does_not_widen_historical_object_only_profile() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = private_memory_prepared_v18(mixed_neutral, &mut budget).unwrap();
    let finished = std::cell::Cell::new(false);
    let result = with_production_optimizer_result_v18(
        prepared,
        &mut budget,
        |original, optimized, budget| {
            let _ = first_spill(original, budget)?;
            let max_cells = optimized.output_inventory(budget)?.definitions().len();
            let floor = budget.storage();
            let result = scoped_raw_admission_v29::with_source_private_physical_v18(
                original,
                optimized,
                fe2o3_kernel_analysis::CanonicalKirPrivateMemoryLimitsV1 { max_cells },
                budget,
                |_, _| -> SourceOwnedResultV18<()> {
                    panic!("old profile admitted a scalar spill")
                },
            );
            assert!(
                matches!(
                    &result,
                    Err(ProductionSourceOwnedViewErrorV18::Binding(
                        "source private entry family requires typed scalar object slots"
                    ))
                ),
                "{result:?}"
            );
            assert_eq!(budget.storage(), floor);
            finished.set(true);
            result
        },
    );
    assert!(matches!(
        source_cause(result.as_ref().unwrap_err()),
        ProductionSourceOwnedViewErrorV18::Binding(
            "source private entry family requires typed scalar object slots"
        )
    ));
    assert!(finished.get());
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn private_spill_complete_census_rejects_omission_duplicate_writer_and_root_substitution() {
    for (fault, expected) in [
        (0, None),
        (
            1,
            Some("source private physical/source operation census is incomplete"),
        ),
        (
            2,
            Some("private spill operation has repeated source coverage"),
        ),
        (
            3,
            Some("source private read-from Store has another source allocation or role"),
        ),
        (
            4,
            Some("source private operation moved to a different output"),
        ),
    ] {
        let fault = SpillFault::new(fault);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let (prepared, fixture) = integer_handoff_prepared_v18(mixed_neutral, &mut budget);
        let roots = fixture.roots();
        let settled = std::cell::Cell::new(false);
        let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            let floor = budget.storage();
            let result = source.private_completed_integer_output_v20(
                ProductionKernelArgumentAbiInputV18 { roots: &roots }, budget);
            let result = match result {
                Ok(handoff) => {
                    assert!(expected.is_none());
                    handoff.discard(budget)?;
                    Ok(())
                },
                Err(error) => {
                    assert!(matches!(source_cause(&error), ProductionSourceOwnedViewErrorV18::Binding(reason)
                        if Some(*reason) == expected), "{error:?}");
                    Err(error)
                },
            };
            assert_eq!(budget.storage(), floor);
            settled.set(true);
            result
        });
        assert_eq!(result.is_ok(), expected.is_none(), "{result:?}");
        assert!(settled.get());
        assert!(fault.finished());
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn private_spill_entry_destination_has_independent_twelve_work_exact_and_short_cuts() {
    for short in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = private_memory_prepared_v18(mixed_neutral, &mut budget).unwrap();
        let settled = std::cell::Cell::new(false);
        let result =
            with_production_optimizer_result_v18(prepared, &mut budget, |original, _, budget| {
                let (root, slot) = first_spill(original, budget)?;
                let instance = original.source.root_row(root)?.source_slots.slots[slot]
                    .instance
                    .index();
                let anchors = original
                    .source
                    .sidecar(root, instance, budget)?
                    .scoped_memory_anchors
                    .as_ref()
                    .unwrap();
                let anchor = anchors
                    .rows
                    .iter()
                    .position(|row| {
                        matches!(
                            row.kind,
                            ScopedMemoryAnchorKindV29::Access {
                                payload: Some(ScopedMemoryPayloadV29::Store {
                                    source: ScopedMemoryStoreSourceV29::EntryArgument { .. },
                                    ..
                                }),
                                ..
                            }
                        )
                    })
                    .expect("genuine entry Store");
                let floor = budget.storage();
                let result = scoped_raw_admission_v29::test_spill_entry_destination_v25(
                    original,
                    root,
                    instance,
                    slot,
                    anchor,
                    short,
                    OPTIMIZED_SOURCE_WORK_LIMIT_V18,
                    budget,
                );
                assert!(result.is_err());
                budget.release_storage(budget.storage() - floor)?;
                assert_eq!(budget.storage(), floor);
                settled.set(true);
                result
            });
        match source_cause(result.as_ref().unwrap_err()) {
            ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(refusal))
                if short =>
            {
                assert_eq!(
                    (refusal.actual(), refusal.limit()),
                    (
                        OPTIMIZED_SOURCE_WORK_LIMIT_V18 + 1,
                        OPTIMIZED_SOURCE_WORK_LIMIT_V18
                    )
                );
            }
            ProductionSourceOwnedViewErrorV18::Binding("selected spill entry boundary stop")
                if !short => {}
            other => panic!("wrong spill entry boundary: {other:?}"),
        }
        assert!(settled.get());
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn private_spill_query_refuses_failed_foreign_accounts_before_debit_and_keeps_custody_failure() {
    for storage in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = private_memory_prepared_v18(mixed_neutral, &mut budget).unwrap();
        let finished = std::cell::Cell::new(false);
        let result =
            with_production_optimizer_result_v18(prepared, &mut budget, |original, _, budget| {
                let (root, slot) = first_spill(original, budget)?;
                let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(17);
                let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, 19);
                if storage {
                    assert!(foreign.reserve_storage(20).is_err());
                } else {
                    assert!(foreign.charge_work(18).is_err());
                }
                let before = (
                    budget.work(),
                    budget.storage(),
                    foreign.work(),
                    foreign.storage(),
                );
                let error = scoped_raw_admission_v29::test_spill_shape_v25(
                    original,
                    root,
                    slot,
                    false,
                    &mut foreign,
                )
                .unwrap_err();
                assert!(matches!(
                    error,
                    ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)
                ));
                assert_eq!(
                    (
                        budget.work(),
                        budget.storage(),
                        foreign.work(),
                        foreign.storage()
                    ),
                    before
                );
                assert!(matches!(
                    scoped_raw_admission_v29::test_spill_shape_v25(
                        original, root, slot, false, budget
                    ),
                    Err(ProductionSourceOwnedViewErrorV18::Resource(
                        ArgumentResourceV1::Accounting
                    ))
                ));
                assert_eq!(
                    (
                        budget.work(),
                        budget.storage(),
                        foreign.work(),
                        foreign.storage()
                    ),
                    before
                );
                finished.set(true);
                Err(error)
            });
        assert!(matches!(
            source_cause(result.as_ref().unwrap_err()),
            ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)
        ));
        assert!(finished.get());
        // Observed foreign custody conservatively suppresses containing refunds.
        assert!(budget.storage() > MODULE_FLOOR);
    }
}

fn spill_resource(error: &(dyn std::error::Error + 'static)) -> Option<ArgumentResourceV1> {
    let mut current = Some(error);
    while let Some(error) = current {
        if let Some(ProductionSourceOwnedViewErrorV18::Resource(resource)) =
            error.downcast_ref::<ProductionSourceOwnedViewErrorV18>()
        {
            return Some(*resource);
        }
        if let Some(resource) = error.downcast_ref::<ArgumentResourceV1>() {
            return Some(*resource);
        }
        current = error.source();
    }
    None
}

fn mixed_completion_cut(cut: Option<(bool, usize)>) -> (usize, usize, bool) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let (prepared, fixture) = integer_handoff_prepared_v18(mixed_neutral, &mut budget);
    let roots = fixture.roots();
    let observed = std::cell::Cell::new(None);
    let settled = std::cell::Cell::new(false);
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        let initial = budget.storage();
        // Place this query above the already observed preparation peak, without
        // changing either production limit or erasing cumulative work/history.
        let padding = match cut {
            Some((false, remaining)) => MODULE_LIMIT - initial - remaining,
            _ => budget.peak_storage() - initial + 1,
        };
        budget.reserve_storage(padding)?;
        if let Some((true, remaining)) = cut {
            budget.charge_work(OPTIMIZED_SOURCE_WORK_LIMIT_V18 - budget.work() - remaining)?;
        }
        let before = (budget.work(), budget.storage());
        assert!(before.1 >= budget.peak_storage());
        let attempt = source.private_completed_integer_output_v20(
            ProductionKernelArgumentAbiInputV18 { roots: &roots },
            budget,
        );
        let selected = match attempt {
            Ok(handoff) => {
                let work = budget.work() - before.0;
                let peak = budget.peak_storage() - before.1;
                handoff.discard(budget)?;
                assert_eq!(budget.storage(), before.1);
                observed.set(Some((work, peak, true)));
                ProductionPrivateSourceHandoffErrorV20::from(
                    ProductionSourceOwnedViewErrorV18::Binding(
                        "selected mixed spill constructor boundary",
                    ),
                )
            }
            Err(error) => {
                let (is_work, _) = cut.expect("uncut genuine mixed completion must pass");
                let resource = spill_resource(&error).expect("original typed resource cause");
                match (is_work, resource) {
                    (true, ArgumentResourceV1::Work(refusal)) => {
                        assert_eq!(refusal.limit(), OPTIMIZED_SOURCE_WORK_LIMIT_V18);
                        assert!(refusal.actual() > refusal.limit());
                    }
                    (false, ArgumentResourceV1::Storage(refusal)) => {
                        assert_eq!(refusal.limit(), MODULE_LIMIT);
                        assert!(refusal.actual() > refusal.limit());
                    }
                    _ => panic!("wrong mixed completion boundary: {error:?}"),
                }
                let stopped = (budget.work(), budget.storage());
                let retry = source
                    .private_completed_integer_output_v20(
                        ProductionKernelArgumentAbiInputV18 { roots: &roots },
                        budget,
                    )
                    .err()
                    .unwrap();
                assert_eq!(spill_resource(&retry), Some(resource));
                assert_eq!((budget.work(), budget.storage()), stopped);
                observed.set(Some((0, 0, false)));
                error
            }
        };
        budget.release_storage(padding)?;
        assert_eq!(budget.storage(), initial);
        settled.set(true);
        Err::<(), _>(selected)
    });
    assert!(result.is_err());
    assert!(
        settled.get(),
        "all query/padding cleanup assertions completed"
    );
    assert_eq!(budget.storage(), MODULE_FLOOR);
    observed
        .get()
        .expect("all selected boundary assertions completed")
}

#[test]
fn private_spill_complete_constructor_uses_one_cumulative_budget_with_exact_short_limits() {
    let (work, storage, accepted) = mixed_completion_cut(None);
    assert!(accepted && work > 0 && storage > 0);
    for (is_work, remaining) in [(true, work), (false, storage)] {
        assert!(mixed_completion_cut(Some((is_work, remaining))).2);
        assert!(!mixed_completion_cut(Some((is_work, remaining - 1))).2);
    }
}

#[test]
fn private_spill_keyed_transport_requires_exact_anchor_position_and_root() {
    for (fault, expected) in [
        (0, None),
        (1, Some("private spill access anchor is absent")),
        (2, Some("scalar payload changed physical operation")),
        (3, Some("source scalar access changed root")),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = private_memory_prepared_v18(mixed_neutral, &mut budget).unwrap();
        let settled = std::cell::Cell::new(false);
        let result = with_production_optimizer_result_v18(
            prepared,
            &mut budget,
            |original, optimized, budget| {
                let (root, slot) = first_spill(original, budget)?;
                let backing = &original.source.root_row(root)?.source_slots.slots[slot];
                let instance = backing.instance.index();
                let anchors = original
                    .source
                    .sidecar(root, instance, budget)?
                    .scoped_memory_anchors
                    .as_ref()
                    .unwrap();
                let store = anchors.rows.iter().position(|row| matches!(row.kind,
                ScopedMemoryAnchorKindV29::Access { pointer, payload: Some(ScopedMemoryPayloadV29::Store { .. }) }
                    if pointer == backing.origin.pointer)).unwrap();
                let read = anchors.rows.iter().position(|row| matches!(row.kind,
                ScopedMemoryAnchorKindV29::Access { pointer, payload: Some(ScopedMemoryPayloadV29::Load { .. }) }
                    if pointer == backing.origin.pointer)).unwrap();
                let [position] = original.attachment_range(
                    TileAttachmentKeyV29 {
                        root,
                        family: TileAttachmentFamilyV29::MemoryAnchor,
                        instance,
                        row: store,
                        field: TileAttachmentFieldV29::MemoryPosition,
                        component: 0,
                        part: 0,
                    },
                    budget,
                )?
                else {
                    panic!("one actual Store position");
                };
                let ProductionSourceOperationV18::Operation(input) =
                    original.mapped_source_operation(position.location, budget)?
                else {
                    panic!("actual Store operation");
                };
                let floor = budget.storage();
                budget.reserve_storage(scoped_raw_admission_v29::test_spill_query_headers_v25()?)?;
                // An actual successful keyed query precedes every hostile substitution.
                let genuine = optimized
                    .scalar_memory_access_at_v25(root, input, instance, store, budget)?
                    .unwrap();
                assert_eq!(genuine.input(), input);
                assert!(genuine.output().is_some());
                drop(genuine);
                let result = optimized.scalar_memory_access_at_v25(
                    if fault == 3 { 1 - root } else { root },
                    input,
                    instance,
                    match fault {
                        1 => usize::MAX,
                        2 => read,
                        _ => store,
                    },
                    budget,
                );
                let result = match result {
                    Ok(value) => {
                        assert!(expected.is_none() && value.is_some());
                        Ok(())
                    }
                    Err(error) => {
                        assert!(
                            matches!(&error, ProductionSourceOwnedViewErrorV18::Binding(reason)
                        if Some(*reason) == expected),
                            "{error:?}"
                        );
                        Err(error)
                    }
                };
                budget.release_storage(budget.storage() - floor)?;
                assert_eq!(budget.storage(), floor);
                settled.set(true);
                result
            },
        );
        assert_eq!(result.is_ok(), expected.is_none(), "{result:?}");
        assert!(settled.get());
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}
