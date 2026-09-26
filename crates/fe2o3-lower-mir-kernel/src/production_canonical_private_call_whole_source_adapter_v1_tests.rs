fn cpc_scope<T>(trace: &mut Trace, callback: impl FnOnce(&mut Trace)) {
    let scope = trace.enter(&[]);
    trace.reserve(size_of::<Cleanup<'_, '_>>());
    trace.reserve(size_of::<std::thread::Result<R<CsResultV1<T>>>>());
    trace.reserve(sum(&[
        size_of::<std::thread::Result<CsResultV1<T>>>(),
        size_of::<std::thread::Result<()>>(),
    ]));
    callback(trace);
    trace.leave(scope);
}

fn model_root_callable(owner: &ProductionPreRankedKirOwnerV1, trace: &mut Trace) {
    use fe2o3_mir_model::{SemanticDefinedCallableSummariesV1, semantic_mir_v1::*};
    // Both private representations are checked by the required model companion.
    type Direct = (bool, bool, Vec<usize>);
    fn backing<T>(count: usize, trace: &mut Trace) {
        trace.reserve(exact_capacity::<T>(count));
        trace.reserve(0);
    }
    fn zero_leaf(types: usize, trace: &mut Trace) {
        trace.work(types);
        backing::<u8>(types, trace);
        trace.work(1);
    }
    let semantic = owner.semantic_ssa().source_semantic();
    let [function] = semantic.functions() else {
        panic!("one semantic callable-root profile")
    };
    assert_eq!(
        semantic.callables(),
        &[SemanticCallableDeclV1::defined(
            SemanticFunctionIdV1::from_index(0),
        )]
    );
    assert!(function.abi().arguments().is_empty());
    assert!(function.abi().source_input_types().is_empty());
    assert!(function.abi().hidden_arguments().is_empty());
    assert!(!function.abi().c_variadic());
    assert!(matches!(
        function.abi().return_value().mode(),
        SemanticAbiPassModeV1::Ignore
    ));
    for ty in [
        function.abi().return_value().source_ty(),
        function.abi().source_output_type(),
    ] {
        assert!(matches!(
            semantic.types()[ty.index() as usize].shape(),
            SemanticTypeShapeV1::Unit
        ));
    }
    for local in function.locals() {
        assert!(matches!(
            semantic.types()[local.ty().index() as usize].shape(),
            SemanticTypeShapeV1::Unit | SemanticTypeShapeV1::Scalar(_)
        ));
    }
    trace.reserve(size_of::<SemanticDefinedCallableSummariesV1<'_>>());
    trace.work(0); // grow direct from an empty Vec.
    backing::<Direct>(1, trace);
    trace.work(1 + function.locals().len());
    trace.work(0); // Scalar ABI's actual empty argument/input rosters.
    units(trace, function.locals().len());
    trace.work(1); // Independent zero-sized ABI check still runs.
    zero_leaf(semantic.types().len(), trace);
    zero_leaf(semantic.types().len(), trace);
    for local in function.locals() {
        zero_leaf(semantic.types().len(), trace);
        if !matches!(
            semantic.types()[local.ty().index() as usize].shape(),
            SemanticTypeShapeV1::Unit
        ) {
            break;
        }
    }
    trace.work(function.blocks().len());
    for block in function.blocks() {
        assert!(matches!(
            block.terminator().kind(),
            SemanticTerminatorKindV1::Return
        ));
        trace.work(1);
        for statement in block.statements() {
            trace.work(1);
            match statement.kind() {
                SemanticStatementKindV1::Store(_) | SemanticStatementKindV1::Nop => {}
                SemanticStatementKindV1::Assign(assignment) => {
                    assert!(assignment.destination().projections().is_empty());
                    assert!(matches!(
                        assignment.value().kind(),
                        SemanticRvalueKindV1::Load(_)
                    ));
                    trace.work(1); // Closed scalar destination.
                    trace.work(1); // Load is nonempty, not an empty scalar rvalue.
                }
                _ => panic!("underived semantic callable-root statement"),
            }
        }
        trace.work(1);
    }
    trace.work(0); // grow callers from empty.
    backing::<Vec<usize>>(1, trace);
    trace.work(1); // Initialize caller rows.
    trace.work(1); // Complete empty direct-callee roster.
    trace.work(1);
    backing::<u8>(1, trace); // Actual private four-way decisions.
    backing::<usize>(1, trace);
    trace.work(1);
    trace.work(1);
    backing::<bool>(1, trace);
    let mut independent_queue = std::collections::VecDeque::<usize>::new();
    independent_queue.try_reserve_exact(1).unwrap();
    assert_eq!(
        independent_queue.capacity(),
        1,
        "pinned exact queue-capacity premise"
    );
    backing::<usize>(1, trace);
    units(trace, 3); // Seed one decision, visit pending root, close one decision.
}

fn root_call_effects(module: &fe2o3_kernel_ir::Module, trace: &mut Trace) -> usize {
    let graph = Graph::module(module);
    assert!(graph.callees.is_empty());
    shared_source::call_effects(module, trace)
}

pub(crate) fn whole_typed_source_component_v1(
    owner: &ProductionPreRankedKirOwnerV1,
    component: u8,
    floor: usize,
    work: usize,
    storage: usize,
) -> (usize, usize, usize, Option<usize>, Option<usize>) {
    shared_source::typed_component(owner, component, floor, work, storage)
}

pub(crate) fn execute_typed_source_component_v1(
    owner: &ProductionPreRankedKirOwnerV1,
    component: u8,
    budget: &mut ArgumentBudgetV1<'_>,
) -> R<()> {
    scoped(budget, |budget| {
        if component == 1 {
            use fe2o3_mir_model::{SemanticCallableDecisionV1, SemanticDefinedCallableSummariesV1};
            let semantic = owner.semantic_ssa().source_semantic();
            let summaries = SemanticDefinedCallableSummariesV1::new_metered(
                semantic.types(),
                semantic.functions(),
                semantic.callables(),
                SemanticAssertionLimitsV1::new(usize::MAX, usize::MAX),
                &mut CallableMeter(budget),
            )
            .map_err(callable_error)?;
            for ordinal in 0..semantic.functions().len() {
                assert_eq!(
                    summaries.decision(SemanticFunctionIdV1::from_index(ordinal as u32)),
                    Some(SemanticCallableDecisionV1::ExactEmptyDeterministicScalar)
                );
            }
        } else if component == 6 {
            owner
                .with_checked_canonical_ranked_source_v1(budget, |_, _| Ok(()))
                .map_err(|_| binding(None, "typed complete original source"))?;
        } else if component == 7 {
            owner
                .with_checked_canonical_ranked_source_v1(budget, |view, budget| {
                    Ok(cpc_with_source_v1(view, budget, |_, _, _, _| Ok(())))
                })
                .map_err(|_| binding(None, "typed original source"))?
                .map_err(|_| binding(None, "typed complete private-call source"))?;
        } else {
            use fe2o3_kernel_analysis::{
                CanonicalKirCallEffectDecisionV1, CanonicalKirCallEffectsV1,
                CanonicalKirInventoryV1,
            };
            let (inventory, receipt) = CanonicalKirInventoryV1::derive(owner.executable(), budget)
                .map_err(|_| binding(None, "typed component inventory"))?;
            budget.reserve_storage(receipt.retained_storage())?;
            if component == 0 {
                let (effects, receipt) = CanonicalKirCallEffectsV1::derive(&inventory, budget)
                    .map_err(Failure::CallEffects)?;
                budget.reserve_storage(receipt.retained_storage())?;
                for function in inventory.functions() {
                    assert_eq!(
                        effects
                            .decision(function.coordinate, budget)
                            .map_err(Failure::CallEffects)?,
                        CanonicalKirCallEffectDecisionV1::CompleteEmpty
                    );
                }
            } else if component == 2 {
                let calls = ProductionCanonicalCallsV1::build(owner, &inventory, budget)
                    .map_err(|_| binding(None, "typed component call correspondence"))?;
                assert_eq!((calls.groups.len(), calls.calls.len()), (4, 2));
            } else if component == 3 {
                use fe2o3_kernel_analysis::{
                    CanonicalKirPrivateMemoryLimitsV1, check_canonical_kir_private_memory_v1,
                };
                assert_eq!(component, 3);
                let (physical, receipt) = check_canonical_kir_private_memory_v1(
                    &inventory,
                    CanonicalKirPrivateMemoryLimitsV1 { max_cells: 8 },
                    budget,
                )
                .map_err(|_| binding(None, "typed component physical proof"))?;
                budget.reserve_storage(receipt.retained_storage())?;
                assert!(!physical.grants_authority());
                assert!(physical.is_for(&inventory));
            } else if component == 4 {
                use fe2o3_kernel_analysis::{CanonicalKirSparseLimitsV1, CanonicalKirSparseV1};
                let (report, receipt) = CanonicalKirSparseV1::derive(
                    &inventory,
                    CanonicalKirSparseLimitsV1::default(),
                    budget,
                )
                .map_err(|_| binding(None, "typed component sparse proof"))?;
                budget.reserve_storage(receipt.retained_storage())?;
                assert!(report.belongs_to(&inventory));
            } else {
                assert_eq!(component, 5);
                let calls = ProductionCanonicalCallsV1::build(owner, &inventory, budget)
                    .map_err(|_| binding(None, "typed argument original calls"))?;
                budget.reserve_storage(size_of::<CrArgumentRowsV1<'_>>())?;
                let rows = cr_build_arguments_v1(owner, &calls, budget)
                    .map_err(|_| binding(None, "typed argument construction"))?;
                cr_check_arguments_v1(owner, &calls, &rows, budget)
                    .map_err(|_| binding(None, "typed argument admission"))?;
                assert_eq!(
                    (
                        rows.rows.len(),
                        rows.paths.len(),
                        rows.associations.len(),
                        rows.frames.len()
                    ),
                    (4, 0, 4, 4)
                );
            }
        }
        Ok(())
    })
}

fn root_callable_rows(owner: &ProductionPreRankedKirOwnerV1, trace: &mut Trace) {
    let associations = owner.correspondence.lowered_functions.len();
    trace.reserve(size_of::<Vec<ProductionCanonicalAssertionCallableV1>>());
    trace.reserve(exact_capacity::<ProductionCanonicalAssertionCallableV1>(
        associations,
    ));
    trace.reserve(0);
    let scope = trace.enter(&[]);
    trace.reserve(size_of::<Cleanup<'_, '_>>());
    trace.reserve(size_of::<std::thread::Result<R<()>>>());
    if owner.executable().module().functions.len() == 1 {
        model_root_callable(owner, trace);
    } else {
        shared_source::typed_callable(owner, trace);
    }
    trace.work(3);
    let effects = shared_source::call_effects(owner.executable().module(), trace);
    trace.reserve(effects);
    for _ in owner.semantic_ssa().source_semantic().callables() {
        trace.work(1);
        trace.work(associations);
    }
    for _ in 0..associations {
        trace.work(4);
        trace.work(1);
    }
    trace.leave(scope);
}

pub(crate) fn whole_root_callable_component_v1(
    owner: &ProductionPreRankedKirOwnerV1,
    floor: usize,
    work_limit: usize,
    storage_limit: usize,
) -> (usize, usize, usize, Option<usize>, Option<usize>) {
    let mut trace = Trace::new(floor);
    source::root_source::<CsResultV1<()>>(owner, &mut trace, |trace| {
        cpc_scope::<()>(trace, |trace| root_callable_rows(owner, trace));
    });
    let p = trace.run(work_limit, storage_limit);
    (p.work, p.storage, p.peak, p.first_work, p.first_storage)
}

// SUT executor only, intentionally separate from every expected-value function.
// The public test subtree cannot name these private production component helpers.
// No receipt, owner, fact, observation or executable proof leaves this scope.
pub(crate) fn execute_root_callable_component_v1(
    owner: &ProductionPreRankedKirOwnerV1,
    budget: &mut ArgumentBudgetV1<'_>,
    observe: impl FnOnce(
        ProductionCanonicalAssertionCallKindV1,
        fe2o3_mir_model::SemanticCallableDecisionV1,
    ),
) -> CrResultV1<CsResultV1<()>> {
    owner.with_checked_canonical_ranked_source_v1(budget, |view, budget| {
        Ok(cpc_scope_v1(budget, |budget| {
            let rows = callable_rows(view.source, budget)?;
            assert_eq!(rows.len(), 1);
            observe(rows[0].kind(), rows[0].source_decision());
            Ok(())
        }))
    })
}

// The original adapter deliberately keeps the shared physical engine's legacy
// scratch paid through the source callback. No transferred native proof is
// substituted here. The root cell has an empty kill index; SharedTyped has no
// loads. No private SourceKillSite payload/layout is assumed for either shape.
fn root_private_reader_with(
    owner: &ProductionPreRankedKirOwnerV1,
    trace: &mut Trace,
    physical: bool,
) {
    use fe2o3_kernel_ir::OperationKind as Operation;
    use fe2o3_mir_model::semantic_mir_v1::*;
    fn scan_empty_kills(owner: &ProductionPreRankedKirOwnerV1, trace: &mut Trace) {
        for function in owner.semantic_ssa().source_semantic().functions() {
            trace.work(3);
            for block in function.blocks() {
                trace.work(3);
                for statement in block.statements() {
                    trace.work(3);
                    trace.work(2);
                    match statement.kind() {
                        SemanticStatementKindV1::Store(store) => {
                            trace.work(1);
                            assert!(!matches!(store.value(), SemanticOperandV1::Move(_)));
                        }
                        SemanticStatementKindV1::Assign(assignment) => assert!(matches!(
                            assignment.value().kind(),
                            SemanticRvalueKindV1::Load(_)
                        )),
                        SemanticStatementKindV1::Nop => {}
                        _ => panic!("closed empty source-kill roster"),
                    }
                }
            }
        }
    }
    let module = owner.executable().module();
    if physical {
        source::physical(module, trace);
    }
    let operations: Vec<_> = module
        .functions
        .iter()
        .flat_map(|f| &f.body.as_ref().unwrap().blocks)
        .flat_map(|b| &b.operations)
        .collect();
    let origins = shared_source::Metadata::source(owner).origins();
    trace.work(1); // Exact shared physical-proof inventory authentication.
    source::scratch::<Option<(SemanticFunctionIdV1, SemanticBlockIdV1, u32)>>(
        operations.len(),
        trace,
    );
    trace.work(operations.len());
    for _ in &operations {
        trace.work(2);
    }
    for _ in &owner.correspondence.lowered_functions {
        for index in 0..operations.len() {
            trace.work(2);
            trace.work(origins.iter().filter(|row| row.0 == index).count());
        }
        let mut kills_built = false;
        for operation in &operations {
            trace.work(4);
            if !matches!(operation.kind, Operation::Load { .. }) {
                continue;
            }
            trace.work(2);
            trace.work(3);
            trace.work(4); // Direct scalar source destination, no projection scan.
            if !kills_built {
                scan_empty_kills(owner, trace);
                source::scratch::<u8>(0, trace); // Empty Vec header; zero kill payload.
                scan_empty_kills(owner, trace);
                trace.work(1);
                kills_built = true;
            }
            trace.work(2);
            trace.work(5); // Empty kill-index interval: no binary-search iterations.
        }
    }
}

fn root_private_source(owner: &ProductionPreRankedKirOwnerV1, trace: &mut Trace) {
    root_private_source_with(owner, trace, |_| {});
}

fn root_private_source_with(
    owner: &ProductionPreRankedKirOwnerV1,
    trace: &mut Trace,
    callback: impl FnOnce(&mut Trace),
) {
    let continuation = |trace: &mut Trace| {
        native::empty_trap_shape::<CsResultV1<()>>(owner.executable().module(), trace, |trace| {
            cpc_scope::<()>(trace, |trace| {
                source::empty_assertion_coverage(owner, trace);
                source::root_private_profile(owner, trace);
                root_callable_rows(owner, trace);
                shared_source::private_calls(owner, trace);
                root_private_reader_with(owner, trace, true);
                callback(trace);
            });
        });
    };
    if owner.executable().module().functions.len() == 1 {
        source::root_source::<CsResultV1<()>>(owner, trace, continuation);
    } else {
        shared_source::original_source::<CsResultV1<()>>(owner, trace, continuation);
    }
}

// This deliberately ends at the first real fixed-point work debit, after the
// actual source continuation. It is a public-prepare accepted-prefix oracle,
// not successful prepare/history/consume totals. Callers must choose a work
// limit <= the returned completed-source prefix, so no unmodeled work executes.
pub(crate) fn whole_prepare_source_prefix_v1(
    owner: &ProductionPreRankedKirOwnerV1,
    floor: usize,
    work_limit: usize,
    storage_limit: usize,
) -> (usize, (usize, usize, usize, Option<usize>, Option<usize>)) {
    prepare_prefix(owner, floor, work_limit, storage_limit, false)
}

pub(crate) fn whole_prepare_import_prefix_v1(
    owner: &ProductionPreRankedKirOwnerV1,
    floor: usize,
    work_limit: usize,
    storage_limit: usize,
) -> (usize, (usize, usize, usize, Option<usize>, Option<usize>)) {
    prepare_prefix(owner, floor, work_limit, storage_limit, true)
}

fn prepare_prefix(
    owner: &ProductionPreRankedKirOwnerV1,
    floor: usize,
    work_limit: usize,
    storage_limit: usize,
    imported: bool,
) -> (usize, (usize, usize, usize, Option<usize>, Option<usize>)) {
    let mut trace = Trace::new(floor);
    trace.work(1);
    trace.work(2);
    let scope = trace.enter(&[]);
    trace.work(3);
    trace.reserve(size_of::<ProductionCanonicalScalarFixedPointOwnerV1>());
    root_private_source(owner, &mut trace);
    let history = trace.enter(&[]);
    trace.reserve(history::meter_header());
    let next = if imported {
        history::prepare_import_prefix(
            owner.executable().module(),
            owner.executable().canonical().canonical_bytes().len(),
            &mut trace,
        )
    } else {
        trace.work(1);
        1
    };
    trace.leave(history);
    trace.leave(scope);
    let prefix = trace.success().work.checked_sub(next).unwrap();
    let predicted = trace.run(work_limit, storage_limit);
    (
        prefix,
        (
            predicted.work,
            predicted.storage,
            predicted.peak,
            predicted.first_work,
            predicted.first_storage,
        ),
    )
}
