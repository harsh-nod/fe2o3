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
        check.budget.work(16)?;
        if publication_predicate_bounds_v1(access, dimension, index, context) {
            continue;
        }
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
        if view_type.shape().len() == 1
            && block != 0
            && check.graph.proves_relation(
                block,
                LessThanFact {
                    lhs: index_expr,
                    rhs: extent_expr,
                },
                true,
                check.budget,
            )?
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
                let presburger_map = check.presburger.map_for_facts(&[sparse_fact]).ok();
                let presburger_decision = static_extent.zip(presburger_map).map(|(extent, map)| {
                    let decision = map.find_out_of_bounds(&[extent]);
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

// This proves only the conditional read's physical bound. It establishes no
// read-from relation, initialization, role, or happens-before authority.
fn publication_predicate_bounds_v1(
    access: &RankedAccessOp,
    dimension: usize,
    index: Value,
    context: &Context,
) -> bool {
    if dimension != 0 || access.kind(context) != Some(dialect_kernel::AccessKindAttr::Read) {
        return false;
    }
    let Some(definition) = index.defining_op() else {
        return false;
    };
    let Some(guard) =
        Operation::get_op::<dialect_kernel::PublicationReadGuardOp>(definition, context)
    else {
        return false;
    };
    let Some(view_definition) = access.view(context).defining_op() else {
        return false;
    };
    let Some(view) = Operation::get_op::<RankedViewOp>(view_definition, context) else {
        return false;
    };
    let Some(view_type) = ranked_view_type(access.view(context), context) else {
        return false;
    };
    guard.result(context) == index
        && access.checked_success(context) == Some(guard.success(context))
        && view_type.deref(context).shape() == [dialect_kernel::DYNAMIC_EXTENT]
        && view.dynamic_extent(context, 0) == Some(guard.physical_extent(context))
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
