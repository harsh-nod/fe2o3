use super::*;

#[derive(Clone, Copy)]
enum SinkFault {
    None,
    MissingConsumer,
    WrongSource,
    MissingRoute,
    SwappedChild,
    FinishProbes,
}

fn run_sink(
    shape: Shape,
    fault: SinkFault,
    work_limit: usize,
    storage_limit: usize,
) -> (
    Result<Vec<LoweredFunctionResultV1>, ProductionSemanticKirErrorV1>,
    usize,
    usize,
) {
    let mut owner = owner(shape);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    let result =
        (|| {
            budget.reserve_storage(FLOOR)?;
            let captured = owner
                .try_capture_occurrences_with_budget_v1(&mut budget)
                .map_err(|error| match error {
                    fe2o3_pliron::ProductionSemanticSsaOccurrenceErrorV1::Resource(error) => {
                        error.into()
                    }
                    _ => execution_call_error_v29(),
                })?;
            budget.reserve_storage(captured.retained_storage())?;
            with_production_call_instances_v1(&owner, ROOT, &mut budget, |instances, budget| {
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(
                with_execution_call_scope_v29(budget, |scope, budget| {
                    let semantic = owner.source_semantic();
                    let first = instances.calls(instances.root()).unwrap()[0]
                        .child()
                        .unwrap();
                    let second = instances.calls(instances.root()).unwrap()[1]
                        .child()
                        .unwrap();
                    let mut sink = ExecutionDefinedCallSinkV29::new(scope, instances, budget)?;
                    if matches!(fault, SinkFault::FinishProbes) {
                        let storage = budget.storage();
                        assert!(sink.finish(budget).is_err());
                        assert_eq!(budget.storage(), storage);
                        assert_eq!(sink.routes.len(), 2);
                        assert!(sink.routes.iter().all(|row| row.target.is_some()));
                        let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(10_000_000);
                        let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, 10_000_000);
                        assert!(sink.finish(&mut foreign).is_err());
                        assert_eq!(foreign.storage(), 0);
                        assert_eq!(foreign_work.work(), 0);
                    }
                    match fault {
                        SinkFault::WrongSource => sink.source.semantic[0] ^= 1,
                        SinkFault::MissingRoute => sink.routes[0].target = None,
                        SinkFault::SwappedChild => sink.routes[0].child = second,
                        _ => {}
                    }
                    let signatures = BTreeMap::from([(
                        HELPER,
                        execution_function_signature_v29(instances, first, budget)?,
                    )]);
                    let root = LoweredFunctionPlanV1 {
                        correspondence_owner: ROOT,
                        semantic_function: ROOT,
                        kernel_ir_function: FunctionId::new("sink_root"),
                        role: SemanticKirFunctionRoleV1::KernelEntry,
                        parameter_declarations: vec![],
                        parameter_types: vec![shape.caller_type(); 2],
                        parameter_values: vec![ValueId(20), ValueId(21)],
                        call_arguments: vec![],
                        parameter_local_bindings: vec![],
                        parameter_component_bindings: vec![],
                        ignored_parameter_bindings: vec![],
                        result_types: vec![],
                    };
                    let mut private = PrivateArrayLazyBudgetV1::new(1, 1024);
                    let parent = with_execution_availability_v29(
                        instances,
                        instances.root(),
                        budget,
                        |mut cursor, budget| {
                            let seed = |id, nominal| {
                                SemanticValueBindingV1::Aggregate(vec![
                                    SemanticValueBindingV1::Execution(
                                        SemanticExecutionBindingV29::context(
                                            semantic.types(),
                                            CONTEXT,
                                            ProductionCallOccurrenceV1 {
                                                caller: instances.root(),
                                                block: SemanticBlockIdV1::from_index(0),
                                            },
                                            ValueId(nominal),
                                        )
                                        .unwrap(),
                                    ),
                                    SemanticValueBindingV1::Value {
                                        id: ValueId(id),
                                        ty: shape.caller_type(),
                                    },
                                ])
                            };
                            cursor.entry_seeds = vec![(1, seed(20, 90)), (2, seed(21, 91))];
                            lower_one_semantic_function_with_calls_v29(
                                semantic,
                                &root,
                                owner.plan_for_function(ROOT).unwrap(),
                                &BTreeMap::new(),
                                &signatures,
                                Some([64, 1, 1]),
                                BTreeSet::new(),
                                1,
                                false,
                                1024,
                                None,
                                &mut private,
                                None,
                                budget,
                                SemanticEmissionPlacementV1 {
                                    first_block: 0,
                                    first_value: 200,
                                },
                                Some(cursor),
                                if matches!(fault, SinkFault::MissingConsumer) {
                                    None
                                } else {
                                    Some(&mut sink)
                                },
                                None,
                            )
                        },
                    )?;
                    assert_eq!(sink.pending.len(), 2);
                    if matches!(fault, SinkFault::FinishProbes) {
                        let storage = budget.storage();
                        assert!(sink.finish(budget).is_err());
                        assert_eq!(budget.storage(), storage);
                        assert_eq!(sink.pending.len(), 2);
                    }
                    assert!(sink.routes.iter().all(|route| route.target.is_none()));
                    let calls: Vec<_> = parent
                        .function
                        .body
                        .as_ref()
                        .unwrap()
                        .blocks
                        .iter()
                        .flat_map(|block| &block.operations)
                        .filter_map(|operation| match &operation.kind {
                            OperationKind::Call { callee, arguments } => {
                                Some((callee.clone(), arguments.clone()))
                            }
                            _ => None,
                        })
                        .collect();
                    assert_eq!(calls.len(), 2);
                    assert_ne!(calls[0].0, calls[1].0);
                    let mut next = parent.next_value;
                    assert!(next >= 200);
                    let mut emitted = vec![parent];
                    while let Some(pending) = sink.pop_pending() {
                        let child = pending.child;
                        let plan = execution_instance_plan_v29(
                            instances,
                            child,
                            pending.kernel_ir_function,
                            SemanticEmissionPlacementV1 {
                                first_block: 17,
                                first_value: next,
                            },
                            budget,
                        )?;
                        let (arguments, parameters) = prepare_execution_parameters_v29(
                            instances,
                            child,
                            pending.arguments,
                            &plan,
                            budget,
                        )?;
                        let call = calls
                            .iter()
                            .filter(|(id, _)| *id == plan.kernel_ir_function)
                            .collect::<Vec<_>>();
                        assert_eq!(call.len(), 1);
                        assert_eq!(call[0].1, arguments);
                        let result = with_execution_availability_v29(
                            instances,
                            child,
                            budget,
                            |cursor, budget| {
                                lower_one_semantic_function_with_calls_v29(
                                    semantic,
                                    &plan,
                                    owner.plan_for_function(HELPER).unwrap(),
                                    &BTreeMap::new(),
                                    &signatures,
                                    None,
                                    BTreeSet::new(),
                                    1,
                                    false,
                                    1024,
                                    None,
                                    &mut private,
                                    None,
                                    budget,
                                    SemanticEmissionPlacementV1 {
                                        first_block: 17,
                                        first_value: next,
                                    },
                                    Some(cursor.with_call_parameters_v29(parameters)?),
                                    Some(&mut sink),
                                    None,
                                )
                            },
                        )?;
                        assert_eq!(result.source_call_instance, Some(child));
                        let destination = if shape.expanded() {
                            4
                        } else if shape.rust_call() {
                            3
                        } else {
                            2
                        };
                        let expected = SemanticExecutionBindingV29::context(
                            semantic.types(), CONTEXT,
                            ProductionCallOccurrenceV1 {
                                caller: instances.root(), block: SemanticBlockIdV1::from_index(0),
                            },
                            ValueId(if child == first { 90 } else { 91 }),
                        ).unwrap();
                        assert!(matches!(
                            &result.execution_observation.as_ref().unwrap().locals[destination],
                            Some(SemanticValueBindingV1::Execution(actual)) if actual == &expected
                        ));
                        assert_eq!(
                            result.function.body.as_ref().unwrap().parameters,
                            [ValueId(next)]
                        );
                        assert_eq!(result.next_value, next + 3);
                        next = result.next_value;
                        emitted.push(result);
                    }
                    let before = budget.storage();
                    let backings = sink.routes.capacity()
                        * std::mem::size_of::<ExecutionDefinedCallRouteV29>()
                        + sink.pending.capacity()
                            * std::mem::size_of::<PendingExecutionDefinedCallV29<'_>>();
                    sink.finish(budget)?;
                    assert_eq!(budget.storage(), before - backings);
                    Ok(emitted)
                }),
            )
        })
        .map_err(|error| match error {
            production_call_instances_v1::ProductionCallInstanceErrorV1::Resource(error) => {
                error.into()
            }
            _ => execution_call_error_v29(),
        })?
        })();
    let peak = budget.peak_storage();
    (result, work.work(), peak)
}

#[test]
fn scoped_sink_emits_distinct_call_instances_and_prepared_children() {
    for shape in [
        Shape::Tuple,
        Shape::Struct,
        Shape::RustCall,
        Shape::PackedRustCall,
        Shape::IndexTuple,
        Shape::IndexRustCall,
    ] {
        let result = run_sink(shape, SinkFault::None, 10_000_000, 10_000_000)
            .0
            .unwrap_or_else(|error| panic!("{shape:?}: {error:?}"));
        assert_eq!(result.len(), 3);
        let mut ids = BTreeSet::new();
        for emitted in &result {
            let body = emitted.function.body.as_ref().unwrap();
            for id in body
                .parameters
                .iter()
                .copied()
                .chain(body.blocks.iter().flat_map(|block| {
                    block
                        .parameters
                        .iter()
                        .chain(
                            block
                                .operations
                                .iter()
                                .flat_map(|operation| &operation.results),
                        )
                        .map(|value| value.id)
                }))
            {
                assert!(ids.insert(id), "overlapping emitted value identity {id:?}");
                assert!(id.0 < emitted.next_value);
            }
        }
        for child in &result[1..] {
            assert_eq!(child.function.signature.parameters, [shape.physical()]);
            assert_eq!(child.function.signature.results, [shape.physical()]);
            let parameter = child.function.body.as_ref().unwrap().parameters[0];
            let binaries: Vec<_> = child
                .function
                .body
                .as_ref()
                .unwrap()
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter(|operation| matches!(operation.kind, OperationKind::Binary { .. }))
                .collect();
            assert_eq!(binaries.len(), 1);
            assert!(matches!(binaries[0].kind, OperationKind::Binary {
                op: BinaryOp::BitXor, lhs, ..
            } if lhs == parameter));
        }
    }
}

#[test]
fn scoped_sink_rejects_missing_consumer_and_source_route_substitution() {
    for fault in [
        SinkFault::MissingConsumer,
        SinkFault::WrongSource,
        SinkFault::MissingRoute,
        SinkFault::SwappedChild,
    ] {
        assert!(matches!(
            run_sink(Shape::Tuple, fault, 10_000_000, 10_000_000).0,
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "execution call parameters differ from their source instance",
                ..
            })
        ));
    }
}

#[test]
fn scoped_sink_obeys_exact_shared_work_and_storage_limits() {
    let (result, work, storage) =
        run_sink(Shape::RustCall, SinkFault::None, 10_000_000, 10_000_000);
    result.unwrap();
    run_sink(Shape::RustCall, SinkFault::None, work, storage)
        .0
        .unwrap();
    assert!(
        run_sink(Shape::RustCall, SinkFault::None, work - 1, storage)
            .0
            .is_err()
    );
    assert!(matches!(
        run_sink(Shape::RustCall, SinkFault::None, work, storage - 1).0,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Storage(_)
            )
        )
    ));
}

#[test]
fn scoped_sink_rejected_finish_retains_storage_and_owned_queue() {
    run_sink(
        Shape::Tuple,
        SinkFault::FinishProbes,
        10_000_000,
        10_000_000,
    )
    .0
    .unwrap();
}
