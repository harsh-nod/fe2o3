const MODULE_FLOOR: usize = 23;
const MODULE_LIMIT: usize = 20_000_000;

fn check_scoped_module(
    owner: &PendingScopedModuleV29,
    source: &ExecutionLifecycleSourceV29<'_>,
    kind: ModuleFixture,
) {
    let module = owner.graph.module();
    let names = if matches!(kind, ModuleFixture::Ordinary) {
        vec!["z_ordinary", "a_ordinary"]
    } else {
        vec!["z_ordinary", "lifecycle_fixture", "a_ordinary"]
    };
    assert_eq!(module.kernels.len(), names.len());
    assert_eq!(module.functions.len(), names.len() + 1);
    assert_eq!(owner.roots.len(), names.len());
    assert!(owner.ledger == source.ledger);
    assert!(owner.graph_storage.retained_storage() > 0);
    assert!(owner.retained_storage > owner.graph_storage.retained_storage());
    let mut events = [0; 3];
    let mut routes = 0;
    for (ordinal, ((kernel, function), root)) in module
        .kernels
        .iter()
        .zip(&module.functions)
        .zip(&owner.roots)
        .enumerate()
    {
        assert_eq!(kernel.id.as_str(), names[ordinal]);
        assert_eq!(kernel.entry, function.id);
        assert_eq!(root.function_ordinal, ordinal);
        assert_eq!(
            root.coordinates.root,
            source.launch.roots()[ordinal].selected_root()
        );
        assert_eq!(root.coordinates.ssa, source.owner.identity());
        assert_eq!(
            root.sidecars.rows.len(),
            root.coordinates.sources.rows.len()
        );
        assert_eq!(root.source_slots.instances.len(), root.sidecars.rows.len());
        assert_eq!(root.coordinates.sources.rows[0].instance.index(), 0);
        assert!(root.slot_relocation.is_some());
        assert!(root.inherited_emission_storage >= root.inherited_assembly_storage);
        assert_eq!(
            kernel.domain,
            LaunchDomain::D1 {
                x: if ordinal == 0 {
                    LaunchExtent::Static(64)
                } else {
                    LaunchExtent::Dynamic
                },
            }
        );
        let is_context = !matches!(kind, ModuleFixture::Ordinary) && ordinal == 1;
        assert_eq!(root.requires_context_issue, is_context);
        assert_eq!(root.insertions.len(), if is_context { 3 } else { 0 });
        check_module_physical_payload(root, function, is_context);
        for sidecar in &root.sidecars.rows {
            assert!(sidecar.diagnostic_declarations.is_empty());
            assert!(sidecar.float_declarations.is_empty());
            assert!(
                sidecar
                    .operation_capabilities
                    .is_subset(&module.required_capabilities)
            );
            assert!(sidecar.instance_assert_origins.is_some());
            assert!(sidecar.lifecycle_events.is_some());
            assert!(sidecar.scoped_memory_anchors.is_some());
        }
        for route in &root.declarations {
            assert!(route.instance < root.sidecars.rows.len());
            assert_eq!(route.kind, ScopedDeclarationKindV29::Diagnostic);
            assert_eq!(route.function_ordinal, names.len());
            assert_eq!(route.id, module.functions[route.function_ordinal].id);
            routes += 1;
        }
        for block in &function.body.as_ref().unwrap().blocks {
            for operation in &block.operations {
                if let OperationKind::Execution(operation) = &operation.kind {
                    use fe2o3_kernel_ir::ExecutionOperationV15 as E;
                    let index = match operation {
                        E::ContextIssue => 0,
                        E::WorkgroupDerive { .. } => 1,
                        E::ScopeEnd { .. } => 2,
                        _ => panic!("unexpected execution operation"),
                    };
                    assert!(is_context);
                    events[index] += 1;
                }
            }
        }
    }
    assert_eq!(
        routes, 2,
        "both original diagnostic declaration uses survive deduplication"
    );
    assert_eq!(
        events,
        if matches!(kind, ModuleFixture::Ordinary) {
            [0; 3]
        } else {
            [1; 3]
        }
    );
    if matches!(kind, ModuleFixture::Array) {
        assert_eq!(owner.roots[0].private_payload.occupied, 0);
        assert!(owner.roots[1].private_payload.occupied > 0);
        assert_eq!(
            owner.roots[1].private_payload.occupied,
            owner.roots[2].private_payload.occupied
        );
        assert_eq!(
            owner.roots[1].private_payload.capacity,
            owner.roots[2].private_payload.capacity
        );
    }
}

fn check_module_physical_payload(
    root: &ScopedModuleRootV29,
    function: &Function,
    is_context: bool,
) {
    let body = function.body.as_ref().unwrap();
    let entry = &body.blocks[0];
    let mut allocations = BTreeMap::new();
    for block in &body.blocks {
        for operation in &block.operations {
            if let OperationKind::Alloca { count, .. } = &operation.kind {
                assert_eq!(block.id, entry.id);
                assert!(
                    allocations
                        .insert(operation.results[0].id, *count)
                        .is_none()
                );
            }
        }
    }
    assert_eq!(allocations.len(), root.source_slots.slots.len());
    for slot in &root.source_slots.slots {
        assert_eq!(
            allocations.get(&slot.origin.pointer),
            Some(&slot.count.map(|row| row.0))
        );
        if let Some((value, _)) = slot.count {
            let operation = entry
                .operations
                .iter()
                .find(|operation| {
                    operation
                        .results
                        .first()
                        .is_some_and(|result| result.id == value)
                })
                .unwrap();
            assert_eq!(
                operation.kind,
                OperationKind::Constant(Constant::Index(slot.length))
            );
        }
    }
    if !is_context {
        let sidecar = &root.sidecars.rows[0];
        assert_eq!(
            sidecar
                .instance_assert_origins
                .as_ref()
                .unwrap()
                .records
                .len(),
            1
        );
        assert_eq!(sidecar.synthetic_operation_spans.len(), 1);
        let failure = &sidecar.synthetic_operation_spans[0];
        assert_eq!(
            failure.rule,
            SemanticKirSyntheticOperationRuleV1::RuntimeAssertFailureTrap
        );
        let block = body
            .blocks
            .iter()
            .find(|block| block.id == failure.kernel_ir_block)
            .unwrap();
        assert_eq!(block.terminator, Some(Terminator::Unreachable));
        assert_eq!(
            block.operations,
            vec![AmdGpuDiagnosticOperation::Trap.operation(None)]
        );
    }
}

fn module_probe(
    kind: ModuleFixture,
    work_limit: usize,
    storage_limit: usize,
) -> (
    Result<(), ScopedModuleErrorV29>,
    usize,
    usize,
    Option<usize>,
    Option<usize>,
) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let (result, peak, failed_storage) = {
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let result = with_module_fixture(kind, &mut budget, |source, budget| {
            let floor = budget.storage();
            let result = admit_pending_scoped_module_v29(
                source,
                ProductionSemanticKirLimitsV1::default(),
                budget,
            );
            let outcome = match result {
                Ok(owner) => {
                    assert_eq!(budget.storage(), floor + owner.retained_storage);
                    check_scoped_module(&owner, source, kind);
                    let retained = owner.retained_storage;
                    drop(owner);
                    budget.release_storage(retained).unwrap();
                    Ok(())
                }
                Err(error) => Err(error),
            };
            assert_eq!(budget.storage(), floor);
            outcome
        })
        .and_then(|result| result);
        assert_eq!(budget.storage(), MODULE_FLOOR);
        (result, budget.peak_storage(), budget.failed_storage())
    };
    (
        result,
        work.work(),
        peak,
        work.failed_work(),
        failed_storage,
    )
}

#[test]
fn complete_module_preserves_ordinary_and_context_roots_and_shared_declarations() {
    for kind in [
        ModuleFixture::Mixed,
        ModuleFixture::Array,
        ModuleFixture::Ordinary,
    ] {
        module_probe(kind, MODULE_LIMIT, MODULE_LIMIT)
            .0
            .unwrap_or_else(|error| panic!("{kind:?}: {error:?}"));
    }
}

#[test]
fn complete_module_keeps_live_scope_trap_refusal() {
    let error = module_probe(ModuleFixture::LiveAssertion, MODULE_LIMIT, MODULE_LIMIT)
        .0
        .unwrap_err();
    let ScopedModuleErrorV29::Canonical(
        fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV15::Verification(errors),
    ) = error
    else {
        panic!("expected actual V15 verification refusal: {error:?}");
    };
    assert!(errors.diagnostics().iter().any(|diagnostic| {
        diagnostic.code == fe2o3_kernel_ir::DiagnosticCode::InvalidSemanticOperation
            && diagnostic
                .location
                .function
                .as_ref()
                .is_some_and(|id| id.as_str() == "lifecycle_fixture")
            && diagnostic.location.block.is_some()
            && diagnostic.location.operation.is_some()
    }));
}

#[test]
fn complete_module_obeys_exact_and_one_short_resources() {
    for kind in [
        ModuleFixture::Mixed,
        ModuleFixture::Array,
        ModuleFixture::Ordinary,
    ] {
        let (result, work, peak, _, _) = module_probe(kind, MODULE_LIMIT, MODULE_LIMIT);
        result.unwrap();
        assert!(module_probe(kind, work, peak).0.is_ok());
        for (work_limit, storage_limit, is_work) in
            [(work - 1, peak, true), (work, peak - 1, false)]
        {
            let (result, _, _, denied_work, denied_storage) =
                module_probe(kind, work_limit, storage_limit);
            let error = result.unwrap_err();
            let resource = match error {
                ScopedModuleErrorV29::Source(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error),
                )
                | ScopedModuleErrorV29::Source(ProductionSemanticKirErrorV1::AssertOrigin(
                    SemanticKirAssertOriginErrorV1::Resource(error),
                ))
                | ScopedModuleErrorV29::Canonical(
                    fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV15::Resource(error),
                )
                | ScopedModuleErrorV29::Canonical(
                    fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV15::Decode(
                        fe2o3_kernel_ir::KernelIrDecodeError::Resource(error),
                    ),
                ) => error,
                other => panic!("expected typed resource refusal: {other:?}"),
            };
            match resource {
                ArgumentResourceV1::Work(_) => {
                    assert!(is_work);
                    assert!(denied_work.is_some());
                }
                ArgumentResourceV1::Storage(_) => {
                    assert!(!is_work);
                    assert!(denied_storage.is_some());
                }
                other => panic!("wrong resource boundary: {other:?}"),
            }
        }
    }
}

#[test]
fn complete_module_rejects_root_and_declaration_substitution() {
    for fault in 0..11 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        with_module_fixture(ModuleFixture::Mixed, &mut budget, |source, budget| {
            let floor = budget.storage();
            let mut emitted =
                scoped_module_roots_v29(source, ProductionSemanticKirLimitsV1::default(), budget)
                    .unwrap();
            match fault {
                0 => {
                    emitted.pop();
                }
                1 => emitted.swap(0, 2),
                2 => emitted[2].root.kernel.id = emitted[0].root.kernel.id.clone(),
                3 => {
                    let entry = emitted[0].root.kernel.entry.clone();
                    emitted[2].root.kernel.entry = entry.clone();
                    emitted[2].root.pending.function.id = entry;
                }
                4 => {
                    let different = module_fixture_owner(ModuleFixture::Array).identity();
                    assert_ne!(different, source.owner.identity());
                    emitted[2].root.pending.coordinates.ssa = different;
                }
                _ => {
                    let root_id = emitted[0].root.pending.function.id.clone();
                    let body = emitted[0].root.pending.function.body.clone();
                    let map = &mut emitted[2].root.pending.sidecars.rows[0].diagnostic_declarations;
                    let (mut key, mut declaration) = map.pop_first().unwrap();
                    match fault {
                        5 => declaration
                            .signature
                            .parameters
                            .push(Type::Scalar(ScalarType::U32)),
                        6 => {
                            declaration
                                .required_capabilities
                                .insert(fe2o3_kernel_ir::TargetCapability::Int64);
                        }
                        7 => declaration.role = fe2o3_kernel_ir::FunctionRole::InternalHelper,
                        8 => key = FunctionId::new("different-key"),
                        9 => {
                            key = root_id.clone();
                            declaration.id = root_id;
                        }
                        10 => declaration.body = body,
                        _ => unreachable!(),
                    }
                    map.insert(key, declaration);
                }
            }
            let result = scoped_module_candidate_v29(
                source,
                emitted,
                ProductionSemanticKirLimitsV1::default(),
                budget,
            );
            assert!(
                matches!(
                    result,
                    Err(ProductionSemanticKirErrorV1::Unsupported { .. })
                ),
                "fault {fault}"
            );
            drop(result);
            budget.release_storage(budget.storage() - floor).unwrap();
            assert_eq!(budget.storage(), floor);
        })
        .unwrap();
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn complete_module_cannot_cross_ledgers_or_ignore_aggregate_limits() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    with_module_fixture(ModuleFixture::Mixed, &mut budget, |source, budget| {
        let floor = budget.storage();
        let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, MODULE_LIMIT);
        assert!(matches!(admit_pending_scoped_module_v29(source, ProductionSemanticKirLimitsV1::default(), &mut foreign),
            Err(ScopedModuleErrorV29::Source(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Accounting)))));
        let emitted = scoped_module_roots_v29(source, ProductionSemanticKirLimitsV1::default(), budget).unwrap();
        assert!(matches!(scoped_module_candidate_v29(source, emitted, ProductionSemanticKirLimitsV1::default(), &mut foreign),
            Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Accounting))));
        assert_eq!(foreign.storage(), 0);
        assert_eq!(foreign.work(), 0);
        budget.release_storage(budget.storage() - floor).unwrap();
        for resource in [ProductionSemanticKirResourceV1::Blocks, ProductionSemanticKirResourceV1::Operations, ProductionSemanticKirResourceV1::Statements] {
            let emitted = scoped_module_roots_v29(source, ProductionSemanticKirLimitsV1::default(), budget).unwrap();
            let counts: Vec<_> = emitted.iter().map(|row| {
                let blocks = &row.root.pending.function.body.as_ref().unwrap().blocks;
                if resource == ProductionSemanticKirResourceV1::Blocks { blocks.len() }
                else if resource == ProductionSemanticKirResourceV1::Operations { blocks.iter().map(|block| block.operations.len()).sum() }
                else {
                    row.root.pending.coordinates.sources.rows.iter().map(|instance| {
                        source.owner.source_semantic().functions()[instance.function.index() as usize]
                            .blocks().iter().map(|block| block.statements().len()).sum::<usize>()
                    }).sum()
                }
            }).collect();
            let maximum = *counts.iter().max().unwrap();
            assert!(counts.iter().sum::<usize>() > maximum);
            let mut limits = ProductionSemanticKirLimitsV1::default();
            if resource == ProductionSemanticKirResourceV1::Blocks { limits.max_blocks = maximum; }
            else if resource == ProductionSemanticKirResourceV1::Operations { limits.max_operations = maximum; }
            else { limits.max_statements = maximum; }
            let result = scoped_module_candidate_v29(source, emitted, limits, budget);
            assert!(matches!(result, Err(ProductionSemanticKirErrorV1::ResourceLimit { resource: found, .. }) if found == resource));
            drop(result);
            budget.release_storage(budget.storage() - floor).unwrap();
        }
    }).unwrap();
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

fn panic_on_last_module_root(
    _source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    _emitted: &mut [Option<LoweredFunctionResultV1>],
    _slots: &OwnedScopedSourceSlotsV29,
    _budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    if instances.instances()[0].function().index() == 4 {
        panic!("late module-root unwind");
    }
    Ok(())
}

#[test]
fn complete_module_restores_its_floor_after_late_root_panic() {
    let previous = SCOPED_SLOT_OBSERVER_V29.replace(Some(panic_on_last_module_root));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_module_fixture(ModuleFixture::Mixed, &mut budget, |source, budget| {
            admit_pending_scoped_module_v29(
                source,
                ProductionSemanticKirLimitsV1::default(),
                budget,
            )
        })
    }));
    SCOPED_SLOT_OBSERVER_V29.set(previous);
    let panic = match result {
        Err(panic) => panic,
        Ok(_) => panic!("expected late module-root unwind"),
    };
    assert_eq!(
        panic.downcast_ref::<&str>(),
        Some(&"late module-root unwind")
    );
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn capability_union_preserves_long_names_and_obeys_exact_resources() {
    use fe2o3_kernel_ir::TargetCapability;
    let capability = |index| TargetCapability::Extension {
        namespace: "shared.namespace.".repeat(32),
        name: format!("{}{index:02}", "shared.prefix.".repeat(32)),
    };
    let first: BTreeSet<_> = (0..12).map(capability).collect();
    let second: BTreeSet<_> = (6..18).map(capability).collect();
    let expected: BTreeSet<_> = (0..18).map(capability).collect();
    let probe = |work_limit, storage_limit| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let (result, peak, denied_storage) = {
            let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
            budget.reserve_storage(MODULE_FLOOR).unwrap();
            let mut output = BTreeSet::new();
            let result = (|| {
                scoped_module_capabilities_v29(&mut output, &first, &mut budget)?;
                scoped_module_capabilities_v29(&mut output, &second, &mut budget)?;
                assert_eq!(output, expected);
                Ok::<_, ProductionSemanticKirErrorV1>(())
            })();
            drop(output);
            budget
                .release_storage(budget.storage() - MODULE_FLOOR)
                .unwrap();
            assert_eq!(budget.storage(), MODULE_FLOOR);
            (result, budget.peak_storage(), budget.failed_storage())
        };
        (
            result,
            work.work(),
            peak,
            work.failed_work(),
            denied_storage,
        )
    };
    let (result, work, peak, _, _) = probe(MODULE_LIMIT, MODULE_LIMIT);
    result.unwrap();
    assert!(probe(work, peak).0.is_ok());
    for (work_limit, storage_limit, is_work) in [(work - 1, peak, true), (work, peak - 1, false)] {
        let (result, _, _, denied_work, denied_storage) = probe(work_limit, storage_limit);
        match result.unwrap_err() {
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Work(_),
            ) => {
                assert!(is_work);
                assert!(denied_work.is_some());
            }
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Storage(_),
            ) => {
                assert!(!is_work);
                assert!(denied_storage.is_some());
            }
            other => panic!("expected exact capability resource refusal: {other:?}"),
        }
    }
    assert_eq!(first, (0..12).map(capability).collect());
    assert_eq!(second, (6..18).map(capability).collect());
}
