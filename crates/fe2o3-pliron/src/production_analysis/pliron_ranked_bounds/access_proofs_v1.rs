struct AccessCheck<'a> {
    facts: &'a FactSet,
    fact_indices: &'a HashMap<LessThanFact, usize>,
    transport: Option<&'a BoundsEdgeTransportV1<'a>>,
    graph: &'a BoundsEdgeTransportV1<'a>,
    sparse_indices: &'a SparseIndexAnalysisV1,
    presburger: &'a PlironPresburgerAnalysisV1,
    findings: &'a mut Vec<RankedBoundsFindingV1>,
    budget: &'a mut RankedBoundsBudget,
}

fn verify_access(
    access: &RankedAccessOp,
    block: usize,
    operation: usize,
    check: &mut AccessCheck<'_>,
    observer: RankedBoundsObserverV1<'_, '_, '_>,
) -> Result<(), RankedBoundsFindingV1> {
    let context = check.graph.context;
    let view = access.view(context);
    let Some(view_type) = ranked_view_type(view, context) else {
        return push_finding(check.findings, check.budget, || {
            RankedBoundsFindingV1::StructuralVerificationFailed
        });
    };
    let view_type = view_type.deref(context);
    let Some(access_kind) = access.kind(context) else {
        return push_finding(check.findings, check.budget, || {
            RankedBoundsFindingV1::StructuralVerificationFailed
        });
    };
    for (dimension, index) in access.indices(context).into_iter().enumerate() {
        check.budget.work(1)?;
        let sparse_fact = check.sparse_indices.fact(index);
        if let Some(overflow) = sparse_fact.machine_overflow() {
            let (lhs, rhs) = overflow.operands();
            push_finding(check.findings, check.budget, || {
                RankedBoundsFindingV1::MachineIntegerOverflow {
                    block,
                    operation,
                    access: access_kind,
                    view: view.id(context).into(),
                    dimension,
                    invocation: overflow.invocation().to_vec(),
                    source_operation: overflow.operation(),
                    lhs,
                    rhs,
                    path_complete: block == 0,
                }
            })?;
            continue;
        }
        let index_expr = canonical_index_expr(index, context);
        let extent_expr = extent_expr(view, &view_type, dimension, context);
        let guarded = if let Some(transport) = check.transport {
            transport.proves(
                block,
                LessThanFact {
                    lhs: index_expr,
                    rhs: extent_expr,
                },
                check.budget,
            )?
        } else {
            bound_is_proven(index_expr, extent_expr, check.facts, check.fact_indices)
        };
        if guarded
            || remainder_bound_is_proven(index, extent_expr, context)
            || sparse_bound_is_proven(index, extent_expr, check.sparse_indices)
        {
            continue;
        }
        let static_extent = match extent_expr {
            IndexExpr::Constant(extent) => Some(extent),
            IndexExpr::Value(value) => check.sparse_indices.fact(value).constant_value(),
            IndexExpr::Dimension { .. } => None,
        };
        match (index_expr, extent_expr) {
            (IndexExpr::Constant(index), IndexExpr::Constant(extent)) => {
                push_finding(check.findings, check.budget, || {
                    RankedBoundsFindingV1::StaticOutOfBounds {
                        block,
                        operation,
                        access: access_kind,
                        view: view.id(context).into(),
                        dimension,
                        index,
                        extent,
                    }
                })?;
            }
            _ => {
                let presburger_map = check
                    .presburger
                    .map_for_facts(&[sparse_fact])
                    .inspect_err(|failure| observe_bounds_presburger_failure_v1(observer, failure))
                    .ok();
                let presburger_decision = static_extent.zip(presburger_map).map(|(extent, map)| {
                    let decision = map.find_out_of_bounds(&[extent]);
                    if let PresburgerRangeDecisionV1::Incomplete(failure) = &decision {
                        observe_bounds_presburger_failure_v1(observer, failure);
                    }
                    (extent, decision)
                });
                match presburger_decision {
                    Some((_, PresburgerRangeDecisionV1::Proved)) => continue,
                    Some((extent, PresburgerRangeDecisionV1::Counterexample { domain, range }))
                        if block == 0 =>
                    {
                        push_finding(check.findings, check.budget, || {
                            RankedBoundsFindingV1::PresburgerOutOfBounds {
                                block,
                                operation,
                                access: access_kind,
                                view: view.id(context).into(),
                                dimension,
                                invocation: domain,
                                index: range[0],
                                extent,
                            }
                        })?;
                    }
                    _ => {
                        if checked_domain_bound_is_proven_v1(
                            check.graph,
                            block,
                            index,
                            (view, dimension, extent_expr),
                            check.budget,
                        )? {
                            continue;
                        }
                        push_finding(check.findings, check.budget, || {
                            RankedBoundsFindingV1::UnprovedBound {
                                block,
                                operation,
                                access: access_kind,
                                view: view.id(context).into(),
                                dimension,
                                index: index_expr.describe(context),
                                extent: extent_expr.describe(context),
                            }
                        })?;
                    }
                }
            }
        }
    }
    Ok(())
}

fn remainder_bound_is_proven(index: Value, extent: IndexExpr, context: &Context) -> bool {
    let IndexExpr::Constant(extent) = extent else {
        return false;
    };
    let Some(definition) = index.defining_op() else {
        return false;
    };
    let operation = Operation::get_op_dyn(definition, context);
    let Some(remainder) = operation.downcast_ref::<IndexBinaryOp>() else {
        return false;
    };
    if remainder.kind(context) != Some(IndexBinaryKindAttr::Remainder) {
        return false;
    }
    let Some(modulus_definition) = remainder.rhs(context).defining_op() else {
        return false;
    };
    let modulus = Operation::get_op_dyn(modulus_definition, context)
        .downcast_ref::<IndexConstantOp>()
        .and_then(|constant| constant.value(context));
    modulus.is_some_and(|modulus| modulus != 0 && modulus <= extent)
}

fn sparse_bound_is_proven(
    index: Value,
    extent: IndexExpr,
    sparse_indices: &SparseIndexAnalysisV1,
) -> bool {
    let Some(index_maximum) = sparse_indices
        .fact(index)
        .maximum(sparse_indices.launch_extents())
    else {
        return false;
    };
    let extent = match extent {
        IndexExpr::Constant(extent) => Some(extent),
        IndexExpr::Value(value) => sparse_indices.fact(value).constant_value(),
        IndexExpr::Dimension { .. } => None,
    };
    extent.is_some_and(|extent| index_maximum < extent)
}

fn sparse_index_failure(failure: SparseIndexFailureV1) -> RankedBoundsFindingV1 {
    let detail = match failure {
        SparseIndexFailureV1::ResourceLimit {
            resource,
            limit,
            actual,
        } => format!("{resource} count {actual} exceeds {limit}"),
        SparseIndexFailureV1::InconsistentLaunchExtent {
            dimension,
            first,
            second,
        } => format!(
            "invocation dimension {dimension} has inconsistent launch extents {first} and {second}"
        ),
        SparseIndexFailureV1::MalformedControlFlow { detail } => detail.to_owned(),
    };
    RankedBoundsFindingV1::SparseIndexAnalysisFailed { detail }
}

fn bound_is_proven(
    index: IndexExpr,
    extent: IndexExpr,
    facts: &FactSet,
    fact_indices: &HashMap<LessThanFact, usize>,
) -> bool {
    match (index, extent) {
        (IndexExpr::Constant(index), IndexExpr::Constant(extent)) => index < extent,
        _ => fact_indices
            .get(&LessThanFact {
                lhs: index,
                rhs: extent,
            })
            .is_some_and(|fact| facts.contains(*fact)),
    }
}

fn extent_expr(
    view: Value,
    view_type: &RankedViewType,
    dimension: usize,
    context: &Context,
) -> IndexExpr {
    let extent = view_type.shape()[dimension];
    if extent == dialect_kernel::DYNAMIC_EXTENT {
        if let Some(definition) = view.defining_op() {
            let definition = Operation::get_op_dyn(definition, context);
            if let Some(view_op) = definition.downcast_ref::<RankedViewOp>()
                && let Some(runtime_extent) = view_op.dynamic_extent(context, dimension)
            {
                return canonical_runtime_extent(runtime_extent, context);
            }
        }
        IndexExpr::Dimension { view, dimension }
    } else {
        IndexExpr::Constant(extent)
    }
}

fn canonical_runtime_extent(value: Value, context: &Context) -> IndexExpr {
    if let Some(operation) = value.defining_op() {
        let operation = Operation::get_op_dyn(operation, context);
        if let Some(constant) = operation.downcast_ref::<IndexConstantOp>()
            && let Some(value) = constant.value(context)
        {
            return IndexExpr::Constant(value);
        }
    }
    IndexExpr::Value(value)
}

#[cfg(test)]
mod observed_presburger_tests {
    use super::*;
    use crate::production_analysis::pliron_pipeline::invocation_receipt_v1::{
        InvocationReceiptFailureV1 as ReceiptFailure, InvocationReceiptV1 as Receipt,
    };
    use fe2o3_kernel_analysis::{
        MAX_PRESBURGER_WORK_UNITS_V1 as CAP, PresburgerFailureV1 as Failure,
    };
    use pliron::{builtin::types::FunctionType, op::Op};

    type Query = Result<PresburgerRangeDecisionV1, Failure>;

    fn fixture(launch: u64, extent: u64) -> (Context, FuncOp, Value) {
        let mut context = Context::new();
        dialect_kernel::register_dialect(
            &mut context,
            &pliron::dialect::DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
        )
        .unwrap();
        let signature = FunctionType::get(&context, vec![], vec![]);
        let function = FuncOp::new(
            &mut context,
            "observed_bounds".try_into().unwrap(),
            signature,
        );
        let entry = function.get_entry_block(&context);
        let ty = RankedViewType::new(&context, 32, false, vec![extent]).unwrap();
        let view = RankedViewOp::new(&mut context, ty, vec![]).unwrap();
        let lane = InvocationIndexOp::new(&mut context, 0, launch);
        let index = lane.result(&context);
        let value = view.result(&context);
        let access =
            RankedAccessOp::new(&mut context, AccessKindAttr::Read, value, vec![index]).unwrap();
        let ret = ReturnOp::new(&mut context);
        for op in [
            view.get_operation(),
            lane.get_operation(),
            access.get_operation(),
            ret.get_operation(),
        ] {
            op.insert_at_back(entry, &context);
        }
        (context, function, index)
    }

    fn run(launch: u64, extent: u64) -> (Query, RankedBoundsReportV1, Receipt<'static>) {
        let (context, function, index) = fixture(launch, extent);
        let mut baseline = PlironAnalysisManagerV1::new(&function);
        baseline.prepare_sparse_indices(&context, &function);
        baseline.prepare_presburger(&context, &function);
        let fact = baseline.sparse_indices().unwrap().fact(index);
        let query = baseline
            .presburger()
            .unwrap()
            .map_for_facts(&[fact])
            .map(|map| map.find_out_of_bounds(&[extent]));
        let ordinary =
            run_pliron_ranked_bounds_check_with_analyses_v1(&context, &function, &mut baseline);
        let mut manager = PlironAnalysisManagerV1::new(&function);
        let mut receipt = Receipt::new(
            Default::default(),
            ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
        )
        .unwrap();
        let phase = receipt
            .phase(ProductionAnalysisResourcePhaseV1::MemoryBounds, 0)
            .unwrap();
        let observed = run_pliron_ranked_bounds_check_with_observation_v1(
            &context,
            &function,
            &mut manager,
            Some(&phase.observer(&Ok)),
        );
        drop(phase);
        assert_eq!(observed.findings(), ordinary.findings());
        // Solver classification only: no phase admission or owner transfer.
        assert_eq!(receipt.snapshot().committed, Default::default());
        (query, observed, receipt)
    }

    #[test]
    fn real_inventory_limit_is_observed_before_report_conversion() {
        use crate::production_analysis::pliron_function_inventory::MAX_PLIRON_FUNCTION_INVENTORY_OPERATIONS_V1;
        let (mut context, function, _) = fixture(8, 4);
        let entry = function.get_entry_block(&context);
        let limit = MAX_PLIRON_FUNCTION_INVENTORY_OPERATIONS_V1;
        // Four existing operations plus these constants exceed the cap by one.
        for _ in 0..limit - 3 {
            IndexConstantOp::new(&mut context, 0)
                .get_operation()
                .insert_at_front(entry, &context);
        }
        let mut receipt = Receipt::new(
            Default::default(),
            ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
        )
        .unwrap();
        let phase = receipt
            .phase(ProductionAnalysisResourcePhaseV1::MemoryBounds, 0)
            .unwrap();
        let mut manager = PlironAnalysisManagerV1::new(&function);
        let report = run_pliron_ranked_bounds_check_with_observation_v1(
            &context,
            &function,
            &mut manager,
            Some(&phase.observer(&Ok)),
        );
        assert!(matches!(report.findings(),
            [RankedBoundsFindingV1::ResourceLimitExceeded { resource: "operation", limit: cap, actual }]
            if *cap == limit && *actual == limit + 1));
        drop(phase);
        let error = ranked_bounds_resource_error_v1("operation");
        assert_eq!(receipt.snapshot().first_denial, Some(error));
        assert!(!receipt.snapshot().caught_panic);
        assert_eq!(receipt.complete(), Err(ReceiptFailure::Denied(error)));
    }

    #[test]
    fn dynamic_launch_unsupported_is_not_denial() {
        let (query, report, receipt) = run(0, 4);
        assert!(matches!(query, Err(Failure::Unsupported { .. })));
        assert!(matches!(
            report.findings(),
            [RankedBoundsFindingV1::UnprovedBound { .. }]
        ));
        assert_eq!(receipt.complete(), Ok(Default::default()));
    }

    #[test]
    fn actual_counterexample_is_not_denial() {
        let (query, report, receipt) = run(8, 4);
        assert_eq!(
            query,
            Ok(PresburgerRangeDecisionV1::Counterexample {
                domain: vec![4],
                range: vec![4],
            })
        );
        assert!(matches!(report.findings(),
            [RankedBoundsFindingV1::PresburgerOutOfBounds {
                block: 0, operation: 2, index: 4, extent: 4, invocation, ..
            }] if invocation == &[4]));
        assert_eq!(receipt.complete(), Ok(Default::default()));
    }

    #[test]
    fn real_solver_cap_survives_unproved_fallback() {
        let (query, report, receipt) = run((CAP + 1) as u64, CAP as u64);
        assert_eq!(
            query,
            Ok(PresburgerRangeDecisionV1::Incomplete(
                Failure::ResourceLimit {
                    limit: CAP,
                    actual: CAP + 1,
                }
            ))
        );
        assert!(matches!(
            report.findings(),
            [RankedBoundsFindingV1::UnprovedBound {
                block: 0,
                operation: 2,
                ..
            }]
        ));
        let error = ranked_bounds_resource_error_v1("Presburger query work limit");
        assert_eq!(receipt.snapshot().first_denial, Some(error));
        assert!(!receipt.snapshot().caught_panic);
        assert_eq!(receipt.complete(), Err(ReceiptFailure::Denied(error)));
    }
}

fn canonical_index_expr(value: Value, context: &Context) -> IndexExpr {
    let Some(operation) = value.defining_op() else {
        return IndexExpr::Value(value);
    };
    let operation = Operation::get_op_dyn(operation, context);
    if let Some(constant) = operation.downcast_ref::<IndexConstantOp>()
        && let Some(value) = constant.value(context)
    {
        return IndexExpr::Constant(value);
    }
    if let Some(dimension) = operation.downcast_ref::<DimensionOp>()
        && let Some(dimension_index) = dimension.dimension(context)
        && let Ok(dimension_index) = usize::try_from(dimension_index)
    {
        let view = dimension.view(context);
        if let Some(view_type) = ranked_view_type(view, context) {
            let view_type: TypedHandle<RankedViewType> = view_type;
            let view_type = view_type.deref(context);
            if view_type.shape().get(dimension_index).is_some() {
                return extent_expr(view, &view_type, dimension_index, context);
            }
        }
    }
    IndexExpr::Value(value)
}
