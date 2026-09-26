#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DescriptorLocalFaultV18 { None, Pointer, Offset, Guard, Volatility }

thread_local! {
    static DESCRIPTOR_LOCAL_FAULT_V18: std::cell::Cell<DescriptorLocalFaultV18> = const { std::cell::Cell::new(DescriptorLocalFaultV18::None) };
    static DESCRIPTOR_LOCAL_OBJECT_V18: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static DESCRIPTOR_LOCAL_OBSERVED_V18: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static DESCRIPTOR_LOCAL_MUTATED_V18: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn descriptor_local_owner_v18(object: bool) -> ProductionSemanticSsaOwnerV1 {
    use fe2o3_mir_model::semantic_mir_v1::*;
    let original = descriptor_with_private_scalar_owner_v18();
    if !object { return original; }
    let semantic = original.source_semantic();
    let original = &semantic.functions()[0];
    let mut types = semantic.types().to_vec();
    let raw = declaration(&mut types, SemanticTypeLayoutV1::new_with_backend_repr(
        Some(8), 8, SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
            SemanticBackendPrimitiveV1::pointer(0, 8, 8),
            SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
        )), false,
    ).unwrap(), SemanticTypeShapeV1::Pointer(SemanticPointerTypeV1::new_with_kind(
        U32, SemanticPointerKindV1::Raw, SemanticMutabilityV1::Mutable, 0, 64,
        SemanticPointerMetadataV1::None,
    ).unwrap()), None);
    let mut locals = original.locals().to_vec();
    let value = locals.len() as u32 - 1;
    assert_eq!(locals[value as usize].ty(), U32);
    let pointer = locals.len() as u32;
    locals.push(local(226, raw, SemanticLocalRoleV1::Temporary));
    let result = locals.len() as u32;
    locals.push(local(227, U32, SemanticLocalRoleV1::Temporary));
    let mut statements = original.blocks()[1].statements().to_vec();
    statements.push(assign(place(pointer, raw), SemanticRvalueKindV1::AddressOf {
        place: place(value, U32), mutability: SemanticMutabilityV1::Mutable,
    }));
    statements.push(assign(place(result, U32), SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(pointer),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, U32).unwrap()], U32).unwrap(),
        SemanticVolatilityV1::NonVolatile, None,
    ))));
    let mut blocks = original.blocks().to_vec();
    blocks[1] = block(225, statements, SemanticTerminatorKindV1::Return);
    let root = function(200, original.role(), original.abi().clone(), locals, blocks)
        .with_kernel_entry(original.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new_with_callables(semantic.target(), types,
        vec![], vec![], vec![], vec![root], semantic.callables().to_vec(), semantic.roots().to_vec(),
    ).unwrap().admit_exact_v29(SemanticMirLimitsV1::default()).unwrap();
    ProductionSemanticSsaOwnerV1::try_new(ProductionSemanticMirOwnerV1::try_new(
        admitted, ProductionSemanticMirLimitsV1::default(),
    ).unwrap(), ProductionSemanticSsaLimitsV1::default()).unwrap()
}

fn descriptor_local_observer_v18(
    _: &ExecutionLifecycleSourceV29<'_>, _: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>], slots: &OwnedScopedSourceSlotsV29,
    _: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    assert!(!slots.slots.is_empty(), "genuine private memory must survive source emission");
    assert_eq!(slots.slots.iter().any(|slot| matches!(slot.representation,
        ScopedSlotRepresentationV29::Object { .. })), DESCRIPTOR_LOCAL_OBJECT_V18.get());
    DESCRIPTOR_LOCAL_OBSERVED_V18.set(DESCRIPTOR_LOCAL_OBSERVED_V18.get() + 1);
    let fault = DESCRIPTOR_LOCAL_FAULT_V18.get();
    if fault == DescriptorLocalFaultV18::None { return Ok(()); }
    let lowered = emitted.iter_mut().flatten().next().unwrap();
    let body = lowered.function.body.as_mut().unwrap();
    let (gep_block, gep_position, descriptor_pointer) = body.blocks.iter().find_map(|block| {
        block.operations.iter().enumerate().find_map(|(position, operation)| {
            matches!(operation.kind, OperationKind::GetElementPointer { .. })
                .then(|| operation.results.first()).flatten()
                .filter(|result| matches!(&result.ty, Type::Pointer(pointer)
                    if pointer.address_space == AddressSpace::Global))
                .map(|result| (block.id, position, result.id))
        })
    }).expect("captured original kernel slice has an actual Global descriptor GEP");
    match fault {
        DescriptorLocalFaultV18::Pointer => {
            assert!(!DESCRIPTOR_LOCAL_OBJECT_V18.get());
            let private = slots.slots[0].origin.pointer;
            let mut changed = None;
            for block in &mut body.blocks {
                for (position, operation) in block.operations.iter_mut().enumerate() {
                    if let OperationKind::Load { pointer, access } = &mut operation.kind
                        && *pointer == descriptor_pointer
                    {
                        *pointer = private;
                        access.address_space = AddressSpace::Private;
                        changed = Some((block.id, position));
                        break;
                    }
                }
                if changed.is_some() { break; }
            }
            let (block, position) = changed.unwrap();
            let mut anchors = 0;
            for row in &mut lowered.scoped_memory_anchors.as_mut().unwrap().rows {
                if row.block == block && row.position == position
                    && let ScopedMemoryAnchorKindV29::Access { pointer, .. } = &mut row.kind
                {
                    assert_eq!(*pointer, descriptor_pointer);
                    *pointer = private;
                    anchors += 1;
                }
            }
            assert_eq!(anchors, 1, "actual pointer and inert source payload move together");
        }
        DescriptorLocalFaultV18::Offset => {
            let extent = body.blocks.iter().flat_map(|block| &block.operations)
                .find(|operation| matches!(operation.kind, OperationKind::SliceLength { .. }))
                .unwrap().results[0].id;
            let operation = &mut body.blocks.iter_mut().find(|block| block.id == gep_block).unwrap()
                .operations[gep_position];
            let OperationKind::GetElementPointer { offset, .. } = &mut operation.kind else { unreachable!() };
            assert_ne!(*offset, extent);
            *offset = extent;
        }
        DescriptorLocalFaultV18::Guard => {
            let block = body.blocks.iter_mut().find(|block| matches!(block.terminator,
                Some(Terminator::ConditionalBranch { .. }))).unwrap();
            let Some(Terminator::ConditionalBranch { then_target, then_arguments, .. }) = block.terminator.take()
                else { unreachable!() };
            block.terminator = Some(Terminator::Branch { target: then_target, arguments: then_arguments });
        }
        DescriptorLocalFaultV18::Volatility => {
            let operation = body.blocks.iter_mut().flat_map(|block| &mut block.operations)
                .find(|operation| matches!(operation.kind, OperationKind::Load { pointer, .. }
                    if pointer == descriptor_pointer)).unwrap();
            let OperationKind::Load { access, .. } = &mut operation.kind else { unreachable!() };
            access.volatile = !access.volatile;
        }
        DescriptorLocalFaultV18::None => unreachable!(),
    }
    DESCRIPTOR_LOCAL_MUTATED_V18.set(DESCRIPTOR_LOCAL_MUTATED_V18.get() + 1);
    Ok(())
}

fn run_descriptor_local_v18(
    object: bool, profiled: bool, optimized: bool, fault: DescriptorLocalFaultV18,
    work_limit: usize, storage_limit: usize,
) -> (SourceOwnedResultV18<()>, usize, usize, bool) {
    struct Restore(Option<ScopedSlotObserverV29>);
    impl Drop for Restore { fn drop(&mut self) { SCOPED_SLOT_OBSERVER_V29.set(self.0); } }
    DESCRIPTOR_LOCAL_FAULT_V18.set(fault);
    DESCRIPTOR_LOCAL_OBJECT_V18.set(object);
    DESCRIPTOR_LOCAL_OBSERVED_V18.set(0);
    DESCRIPTOR_LOCAL_MUTATED_V18.set(0);
    let _restore = Restore(SCOPED_SLOT_OBSERVER_V29.replace(Some(descriptor_local_observer_v18)));
    let owner = descriptor_local_owner_v18(object);
    let semantic = owner.source_semantic();
    let entry = semantic.functions()[0].kernel_entry().unwrap();
    let launch = ProductionSourceLaunchRosterV1::try_new(semantic, &[
        ProductionSourceLaunchRootInputV1::new(
            std::str::from_utf8(entry.export_symbol().as_bytes()).unwrap(),
            *entry.kernel_binding_identity().as_bytes(),
            ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1])),
    ]).unwrap();
    let sha = *owner.source_semantic_sha256();
    let classes = vec![ProductionScopeCallableCandidateV29::Ordinary; semantic.callables().len()];
    let fixture = kernel_argument_abi_v18::tests::FixtureKernelAbiV18::new(&owner);
    let roots = fixture.roots();
    let input = ProductionExecutionSourceInputV29 { semantic_sha256: &sha, roots: &[], classes: &classes, events: &[] };
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let mut completed = false;
    let result = (|| -> SourceOwnedResultV18<()> {
        let prepared = if profiled {
            ProductionPendingScopedSourceOwnerV29::prepare_source_with_kernel_abi_budget_v18(
                owner, launch, input, ProductionKernelArgumentAbiInputV18 { roots: &roots },
                ProductionSemanticKirLimitsV1::default(), &mut budget,
            )
        } else {
            ProductionPendingScopedSourceOwnerV29::prepare_source_with_budget_v18(
                owner, launch, input, ProductionSemanticKirLimitsV1::default(), &mut budget,
            )
        }?;
        if optimized {
            with_production_optimized_consumer_v18(prepared, &mut budget, |original, output, budget| {
                assert!(original.source.root_row(0)?.source_slots.pending_memory.is_some());
                original.with_optimized_analysis_v18(output, budget, |analysis, budget| {
                        analysis.with_memory_versions(budget, |input_memory, output_memory, budget| {
                            scoped_raw_admission_v29::with_checked_optimized_source_memory_v18(
                                original, output, 0, input_memory, output_memory, budget, |physical, budget| {
                                    let mut accesses = 0;
                                    physical.visit_accesses(budget, |_, _, _, _, _| { accesses += 1; Ok(()) })?;
                                    assert!(accesses > 0);
                                    completed = true;
                                    Ok::<(), ProductionSourceOwnedViewErrorV18>(())
                                })
                        })
                })
            })
        } else {
            prepared.with_source_consumer_v18(&mut budget, |source, budget| {
                assert!(source.root_row(0)?.source_slots.pending_memory.is_some());
                source.with_analysis_v18(budget, |scope| scope.with_inventory_v1(|inventory, budget| {
                    source.with_ranked_correspondence_v18(inventory, budget, |original, budget| {
                        scoped_raw_admission_v29::with_checked_source_memory_v29(original, 0, None, budget,
                            |physical, budget| -> SourceOwnedResultV18<()> {
                                let mut accesses = 0;
                                physical.visit_effects(budget, |_, _| { accesses += 1; Ok(()) })?;
                                assert!(accesses > 0);
                                completed = true;
                                Ok(())
                            })
                    })
                }))
            })
        }
    })();
    assert_eq!(budget.storage(), MODULE_FLOOR, "{result:?}");
    (result, budget.work(), budget.peak_storage(), completed)
}

#[test]
fn original_descriptor_and_private_scalar_or_object_finish_same_candidate_and_optimizer() {
    for object in [false, true] {
        for optimized in [false, true] {
            let (result, _, _, completed) = run_descriptor_local_v18(object, true, optimized,
                DescriptorLocalFaultV18::None, OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
            assert!(result.is_ok() && completed, "object={object}, optimized={optimized}: {result:?}");
            assert!(DESCRIPTOR_LOCAL_OBSERVED_V18.get() > 0);
            assert_eq!(DESCRIPTOR_LOCAL_MUTATED_V18.get(), 0);
        }
    }
}

#[test]
fn original_descriptor_private_memory_full_scope_has_exact_and_short_boundaries() {
    for object in [false, true] {
        let run = |work, storage| run_descriptor_local_v18(object, true, false,
            DescriptorLocalFaultV18::None, work, storage);
        let (positive, work, peak, completed) = run(OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
        assert!(positive.is_ok() && completed, "{positive:?}");
        let (exact, _, _, completed) = run(work, peak);
        assert!(exact.is_ok() && completed, "{exact:?}");
        for (work, storage, is_work) in [(work - 1, peak, true), (work, peak - 1, false)] {
            let (refused, _, _, consumer_visited) = run(work, storage);
            // A final postflight can exhaust the budget after the physical
            // consumer ran. The complete transaction must still refuse.
            let error = source_slot_tests::original_repeated_source_resource_v29(
                refused.expect_err("a short budget must refuse the whole transaction"),
            );
            match (is_work, error) {
                (true, ArgumentResourceV1::Work(limit)) => {
                    assert_eq!(limit.limit(), work, "consumer_visited={consumer_visited}");
                    assert!(limit.actual() > work, "{limit:?}");
                }
                (false, ArgumentResourceV1::Storage(limit)) => {
                    assert_eq!(limit.limit(), storage, "consumer_visited={consumer_visited}");
                    assert!(limit.actual() > storage, "{limit:?}");
                }
                (_, error) => panic!("wrong resource refusal: {error:?}; consumer_visited={consumer_visited}"),
            }
        }
    }
}

#[test]
fn original_descriptor_private_memory_keeps_source_pointer_offset_guard_and_effect_checks() {
    for fault in [DescriptorLocalFaultV18::Pointer, DescriptorLocalFaultV18::Offset,
        DescriptorLocalFaultV18::Guard, DescriptorLocalFaultV18::Volatility]
    {
        let (positive, _, _, completed) = run_descriptor_local_v18(false, true, false,
            DescriptorLocalFaultV18::None, OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
        assert!(positive.is_ok() && completed, "{positive:?}");
        let (refused, _, _, completed) = run_descriptor_local_v18(false, true, false,
            fault, OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
        assert!(!completed);
        assert_eq!(DESCRIPTOR_LOCAL_MUTATED_V18.get(), 1);
        assert!(matches!(refused, Err(ProductionSourceOwnedViewErrorV18::Source(
            ProductionPendingScopedSourceErrorV29::Source(ProductionSemanticKirErrorV1::Unsupported { detail, .. })))
            if matches!(detail, "source runtime slice descriptor/index/extent correspondence differs"
                | "scoped memory anchors differ from their source instance"
                | "execution call parameters differ from their source instance")), "{fault:?}: {refused:?}");
    }
}
