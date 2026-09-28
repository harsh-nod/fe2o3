fn descriptor_failure_tail_run_v1766(
    work_limit: usize,
    storage_limit: usize,
) -> (SourceOwnedResultV18<()>, usize, usize, bool) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let visited = std::cell::Cell::new(false);
    let result = with_pending_api_owner_v18(
        ModuleFixture::Ordinary,
        false,
        &mut budget,
        || descriptor_source_owner(DescriptorCase::READ),
        |owner, launch, input, _, budget| -> SourceOwnedResultV18<()> {
            let abi = kernel_argument_abi_v18::tests::FixtureKernelAbiV18::new(&owner);
            let roots = abi.roots();
            let prepared =
                ProductionPendingScopedSourceOwnerV29::prepare_source_with_kernel_abi_budget_v18(
                    owner,
                    launch,
                    input,
                    ProductionKernelArgumentAbiInputV18 { roots: &roots },
                    ProductionSemanticKirLimitsV1::default(),
                    budget,
                )?;
            prepared.with_source_consumer_v18(budget, |source, budget| {
                let function = source.instance(0, 0, budget)?.0;
                let owner = source.source_ssa(budget)?;
                let original = &owner.source_semantic().functions()[function.index() as usize];
                let occurrences = owner.occurrences_v1().unwrap().function(function).unwrap();
                let boundary = occurrences
                    .terminal_failure_start(SsaBlockIdV1::new(0))
                    .unwrap();
                let root = source.root_row(0)?;
                assert_eq!(root.sidecars.rows.len(), 1);
                let anchors = root.sidecars.rows[0]
                    .scoped_memory_anchors
                    .as_ref()
                    .unwrap();
                let mut diagnostics = Vec::new();
                let mut values = Vec::new();
                for (anchor, row) in anchors.rows.iter().enumerate() {
                    let ScopedMemoryAnchorKindV29::FailureRead { event, local } = row.kind else {
                        continue;
                    };
                    let source_event = &occurrences.events()[event];
                    let frame = row.source.unwrap();
                    let role = ExecutionOperandV29::AssertMessage(diagnostics.len() as u32);
                    assert_eq!(
                        frame.site,
                        execution_site_v29(SemanticBlockIdV1::from_index(0), None)
                    );
                    assert_eq!(frame.role, Some(ScopedMemoryRoleV29::Operand(role)));
                    assert_eq!(source_event.site(), frame.site);
                    assert_eq!(source_event.operand(), role);
                    assert_eq!(source_event.role(), ExecutionEventV29::BaseUse);
                    assert!(source_event.is_reachable());
                    assert!(source_event.ordinal() as usize >= boundary);
                    assert_eq!(source_event.event().variable().get(), local);
                    let Some(SemanticOperandV1::Copy(place)) =
                        scoped_source_operand_v29(original, frame.site, role)
                    else {
                        panic!("actual descriptor diagnostic operand");
                    };
                    assert_eq!(place.local().index(), local);
                    assert!(place.projections().is_empty());
                    assert!(source_event.is_promoted());
                    let Some(fe2o3_mir_model::SsaResolvedEventV1::Use { variable, value }) =
                        source_event.resolved()
                    else {
                        panic!("the original diagnostic must resolve to its SSA value");
                    };
                    assert_eq!(variable.get(), local);
                    let archive = root.sidecars.rows[0]
                        .execution_observation
                        .as_ref()
                        .unwrap();
                    let Some(SemanticValueBindingV1::Value { id, ty }) =
                        archive.bindings.get(&value)
                    else {
                        panic!("the diagnostic must retain its emitted scalar value");
                    };
                    // The length is emitted by SliceLength as Index, whereas
                    // the original by-value index argument retains U64.
                    let (expected_local, expected_type) = match diagnostics.len() {
                        0 => (4, Type::INDEX),
                        1 => (3, Type::Scalar(ScalarType::U64)),
                        _ => panic!("unexpected bounds-check diagnostic"),
                    };
                    assert_eq!(local, expected_local);
                    assert_eq!(*ty, expected_type);
                    assert_eq!(
                        owner.source_semantic().types()
                            [original.locals()[local as usize].ty().index() as usize]
                            .shape(),
                        &SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                            signed: false,
                            bits: 64,
                        })
                    );
                    assert!(!values.contains(id));
                    assert!(root.source_slots.slots.iter().all(|slot| {
                        slot.instance.index() != 0
                            || slot.origin.identity.original_local() != Some(local)
                    }));
                    values.push(*id);
                    diagnostics.push(anchor);
                }
                assert_eq!(diagnostics.len(), 2);
                source.with_analysis_v18(budget, |scope| {
                    scope.with_sparse_and_memory_ssa_v1(|_, memory, budget| {
                        source.with_ranked_correspondence_v18(
                            memory.inventory(),
                            budget,
                            |relation, budget| {
                                scoped_raw_admission_v29::with_checked_source_memory_v29(
                                    relation,
                                    0,
                                    Some(memory),
                                    budget,
                                    |physical, budget| -> SourceOwnedResultV18<()> {
                                        let mut reads = Vec::new();
                                        physical.visit_effects(budget, |effect, _| {
                                            if let scoped_raw_admission_v29::
                                                PendingSourceMemoryEffectV29::FailureRead {
                                                    instance, anchor,
                                                } = effect
                                            {
                                                assert_eq!(instance.index(), 0);
                                                reads.push(anchor);
                                            }
                                            Ok(())
                                        })?;
                                        // Failure anchors describe original diagnostics. Both
                                        // operands are SSA values, not private memory reads.
                                        assert!(
                                            reads.is_empty(),
                                            "promoted diagnostic reads: {reads:?}"
                                        );
                                        visited.set(true);
                                        Ok(())
                                    },
                                )
                            },
                        )
                    })
                })
            })
        },
    );
    assert_eq!(budget.storage(), MODULE_FLOOR, "{result:?}");
    (result, budget.work(), budget.peak_storage(), visited.get())
}

#[test]
fn descriptor_failure_tail_keeps_original_coordinates_through_complete_memory_admission() {
    let (result, _, _, visited) = descriptor_failure_tail_run_v1766(MODULE_LIMIT, MODULE_LIMIT);
    result.unwrap();
    assert!(visited);
}

#[test]
fn descriptor_failure_tail_complete_admission_has_exact_and_one_short_resources() {
    let (result, work, storage, visited) =
        descriptor_failure_tail_run_v1766(MODULE_LIMIT, MODULE_LIMIT);
    result.unwrap();
    assert!(visited);
    let exact = descriptor_failure_tail_run_v1766(work, storage);
    exact.0.unwrap();
    assert!(exact.3);
    assert_eq!((exact.1, exact.2), (work, storage));
    for (work_limit, storage_limit, is_work) in
        [(work - 1, storage, true), (work, storage - 1, false)]
    {
        let short = descriptor_failure_tail_run_v1766(work_limit, storage_limit);
        let error = source_slot_tests::original_repeated_source_resource_v29(short.0.unwrap_err());
        match (is_work, error) {
            (true, ArgumentResourceV1::Work(error)) => {
                assert_eq!(error.actual(), work);
                assert_eq!(error.limit(), work - 1);
            }
            (false, ArgumentResourceV1::Storage(error)) => {
                assert_eq!(error.actual(), storage);
                assert_eq!(error.limit(), storage - 1);
            }
            other => panic!("exact original descriptor resource refusal: {other:?}"),
        }
    }
}
