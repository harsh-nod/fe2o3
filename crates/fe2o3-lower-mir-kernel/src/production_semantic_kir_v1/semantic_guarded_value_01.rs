#[derive(Default)]
struct KirScalarCorrelationAuthorityV1 {
    capabilities: BTreeMap<ValueId, GlobalCapabilityTypeV1>,
    loads: BTreeMap<SemanticAccessSiteV1, GlobalCapabilityTypeV1>,
}

struct KirScalarUseContextV1<'a> {
    consumer: FunctionOperationLocation,
    authority: &'a KirScalarCorrelationAuthorityV1,
}

fn source_global_scalar_type_v1(
    execution: SemanticKirExecutionInputV1<'_>,
    element: SemanticTypeIdV1,
    contract: SemanticCapabilityMemoryContractV1,
    provenance: SemanticKernelCapabilityProvenanceV1,
) -> Option<(GlobalCapabilityTypeV1, KernelContextSourceIdentityV1)> {
    let root = execution
        .source
        .functions()
        .get(execution.root.index() as usize)?;
    let entry = root.kernel_entry()?;
    if provenance.root() != execution.root
        || provenance.kernel_binding() != entry.kernel_binding_identity()
    {
        return None;
    }
    let context = KernelContextTypeV1::new(
        std::str::from_utf8(entry.export_symbol().as_bytes()).ok()?,
        *provenance.kernel_marker().as_bytes(),
        *provenance.target_brand().as_bytes(),
        *provenance.launch_brand().as_bytes(),
    );
    Some((
        lower_global_capability_type_v1(execution.source.types(), element, contract, &context)
            .ok()?,
        KernelContextSourceIdentityV1::new(
            *provenance.frontend_unit().as_bytes(),
            *execution
                .source
                .functions()
                .get(execution.source_body.index() as usize)?
                .identity()
                .as_bytes(),
            *entry.kernel_binding_identity().as_bytes(),
            *provenance.issuance().as_bytes(),
        ),
    ))
}

fn execution_intrinsic_at_v1(
    execution: SemanticKirExecutionInputV1<'_>,
    block: u32,
) -> Option<SemanticCompilerIntrinsicOperationV1> {
    let SemanticTerminatorKindV1::Call(call) = execution
        .function
        .blocks()
        .get(block as usize)?
        .terminator()
        .kind()
    else {
        return None;
    };
    let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = execution
        .source
        .callables()
        .get(call.callee().index() as usize)?
    else {
        return None;
    };
    Some(*operation)
}

fn checked_kir_scalar_authority_v1(
    execution: Option<SemanticKirExecutionInputV1<'_>>,
    correspondence: &SemanticKirCorrespondenceV1,
    kir: &KirCorrelationIndexV1<'_>,
    sites: &BTreeMap<(FunctionOperationLocation, u32), SemanticAccessSiteV1>,
    budget: &mut UnsupportedIndexCorrelationBudgetV1,
) -> Option<KirScalarCorrelationAuthorityV1> {
    let mut authority = KirScalarCorrelationAuthorityV1::default();
    let Some(execution) = execution else {
        return Some(authority);
    };
    for (&value, operation) in &kir.definitions {
        budget.charge()?;
        let OperationKind::GlobalCapabilityBind(bind) = operation.kind else {
            continue;
        };
        let location = *kir.definition_locations.get(&value)?;
        let mut source_block = None;
        for span in correspondence.terminator_operation_spans() {
            budget.charge()?;
            if span.correspondence_owner() == execution.root
                && span.semantic_function() == execution.source_body
                && operation_span_contains_v1(
                    span.kernel_ir_block(),
                    span.first_operation_ordinal(),
                    span.operation_count(),
                    location,
                )
            {
                if source_block
                    .replace(span.semantic_block().index())
                    .is_some()
                {
                    return None;
                }
            }
        }
        let (element, contract, provenance) =
            match execution_intrinsic_at_v1(execution, source_block?)? {
                SemanticCompilerIntrinsicOperationV1::CapabilityGlobalBindReadOnly {
                    element,
                    contract,
                    provenance,
                    ..
                }
                | SemanticCompilerIntrinsicOperationV1::CapabilityGlobalBindExclusiveReadWrite {
                    element,
                    contract,
                    provenance,
                    ..
                }
                | SemanticCompilerIntrinsicOperationV1::CapabilityGlobalBindDisjointWrite {
                    element,
                    contract,
                    provenance,
                    ..
                } => (element, contract, provenance),
                _ => return None,
            };
        let (expected, source) =
            source_global_scalar_type_v1(execution, element, contract, provenance)?;
        let [result] = operation.results.as_slice() else {
            return None;
        };
        let context = unique_kir_ssa_origin_v1(kir, bind.context, budget)?;
        let issuance = kir.definitions.get(&context)?;
        if result.id != value
            || result.ty != Type::GlobalCapability(expected.clone())
            || !matches!(issuance.kind, OperationKind::KernelContextIssue(issue) if issue.source() == source)
            || issuance.results.as_slice()
                != [ValueDef::new(
                    context,
                    Type::KernelContext(expected.context().clone()),
                )]
        {
            return None;
        }
        authority.capabilities.insert(value, expected);
    }
    for (&(location, ordinal), &site) in sites {
        budget.charge()?;
        if ordinal != 0
            || site.statement.is_some()
            || site.ordinal != 0
            || !matches!(
                kir.operations.get(&location)?.kind,
                OperationKind::GuardedLoad { .. }
            )
        {
            continue;
        }
        let (element, contract, provenance) = match execution_intrinsic_at_v1(execution, site.block)
        {
            Some(
                SemanticCompilerIntrinsicOperationV1::CapabilityGlobalLoad {
                    element,
                    contract,
                    provenance,
                    ..
                }
                | SemanticCompilerIntrinsicOperationV1::CapabilityGlobalExclusiveLoad {
                    element,
                    contract,
                    provenance,
                    ..
                },
            ) => (element, contract, provenance),
            _ => continue,
        };
        let (expected, _) = source_global_scalar_type_v1(execution, element, contract, provenance)?;
        if authority.loads.insert(site, expected).is_some() {
            return None;
        }
    }
    Some(authority)
}

fn checked_global_index_operand_v1(
    kir: &KirCorrelationIndexV1<'_>,
    authority: &KirScalarCorrelationAuthorityV1,
    value: ValueId,
    budget: &mut UnsupportedIndexCorrelationBudgetV1,
) -> Option<ValueId> {
    budget.charge()?;
    let value = unique_kir_ssa_origin_v1(kir, value, budget)?;
    let operation = kir.definitions.get(&value)?;
    let OperationKind::GlobalCapabilityIndex(projected) = operation.kind else {
        return None;
    };
    let capability = unique_kir_ssa_origin_v1(kir, projected.capability, budget)?;
    let expected = authority.capabilities.get(&capability)?;
    let mapping = match expected.role() {
        GlobalCapabilityRoleV1::ReadOnly | GlobalCapabilityRoleV1::ExclusiveReadWrite => None,
        GlobalCapabilityRoleV1::DisjointWrite(mapping) => Some(mapping),
    };
    (projected.index_space == mapping
        && operation.results.as_slice() == [ValueDef::new(value, Type::INDEX)])
    .then_some(projected.index)
}

fn checked_guarded_scalar_load_v1(
    function: &Function,
    kir: &KirCorrelationIndexV1<'_>,
    use_context: &KirScalarUseContextV1<'_>,
    location: FunctionOperationLocation,
    site: SemanticAccessSiteV1,
    budget: &mut UnsupportedIndexCorrelationBudgetV1,
) -> Option<()> {
    budget.charge()?;
    let expected = use_context.authority.loads.get(&site)?;
    let operation = kir.operations.get(&location)?;
    let OperationKind::GuardedLoad {
        pointer,
        predicate,
        fallback,
        access,
    } = operation.kind
    else {
        return None;
    };
    let [loaded] = operation.results.as_slice() else {
        return None;
    };
    if loaded.ty != *expected.element()
        || access.address_space != AddressSpace::Global
        || !access.volatile
        || access.alignment != strided_read_scalar_alignment_v1(expected.element())?
    {
        return None;
    }
    let earlier = |value| {
        let producer = kir.definition_locations.get(&value)?;
        (producer.block == location.block && producer.operation_index < location.operation_index)
            .then(|| kir.definitions.get(&value).copied())
            .flatten()
    };
    let OperationKind::GetElementPointer { base, offset } = earlier(pointer)?.kind else {
        return None;
    };
    let OperationKind::SliceData { slice } = earlier(base)?.kind else {
        return None;
    };
    let capability = unique_kir_ssa_origin_v1(kir, slice, budget)?;
    if use_context.authority.capabilities.get(&capability)? != expected {
        return None;
    }
    let OperationKind::Select {
        condition,
        true_value: projected,
        false_value: zero,
    } = earlier(offset)?.kind
    else {
        return None;
    };
    if condition != predicate
        || !matches!(
            earlier(zero)?.kind,
            OperationKind::Constant(Constant::Index(0))
        )
    {
        return None;
    }
    let OperationKind::GlobalCapabilityIndex(index) = earlier(projected)?.kind else {
        return None;
    };
    if index.capability != slice {
        return None;
    }
    checked_global_index_operand_v1(kir, use_context.authority, projected, budget)?;
    let OperationKind::Compare {
        predicate: ComparePredicate::LessThan,
        lhs,
        rhs,
    } = earlier(predicate)?.kind
    else {
        return None;
    };
    if lhs != projected
        || !matches!(earlier(rhs)?.kind, OperationKind::SliceLength { slice: length_slice } if length_slice == slice)
    {
        return None;
    }
    let OperationKind::Constant(ref actual_fallback) = earlier(fallback)?.kind else {
        return None;
    };
    if *actual_fallback != volatile_load_zero_constant_v1(expected.element())? {
        return None;
    }
    prove_guarded_value_at_consumer_v1(function, kir, use_context, location, predicate, budget)
}

// Unknown values overapproximate paths. Only exact Boolean structure and typed
// index/extent identities may rule out a path on which the load used its fallback.
fn guard_index_origin_v1(
    function: &Function,
    kir: &KirCorrelationIndexV1<'_>,
    authority: &KirScalarCorrelationAuthorityV1,
    mut value: ValueId,
    budget: &mut UnsupportedIndexCorrelationBudgetV1,
) -> Option<(ValueId, Option<GlobalDisjointIndexContractV1>)> {
    let mut seen = BTreeSet::new();
    let mut mapping = None;
    loop {
        budget.charge()?;
        value = unique_kir_ssa_origin_v1(kir, value, budget)?;
        if !seen.insert(value) {
            return None;
        }
        match kir.definitions.get(&value).map(|operation| &operation.kind) {
            Some(OperationKind::GlobalCapabilityIndex(projected)) => {
                if mapping.is_some() {
                    return None;
                }
                mapping = projected.index_space;
                value = checked_global_index_operand_v1(kir, authority, value, budget)?;
            }
            Some(OperationKind::Cast {
                kind,
                value: operand,
                ..
            }) if matches!((kir_value_scalar_v1(function, kir, *operand)?, kir_value_scalar_v1(function, kir, value)?),
                    (ProductionSemanticScalarTypeV2::Integer { signed: false, bits: from },
                     ProductionSemanticScalarTypeV2::Integer { signed: false, bits: to })
                    if (*kind == CastKind::ZeroExtend && from < to)
                        || (*kind == CastKind::Bitcast && from == to)) =>
            {
                value = *operand
            }
            _ => return Some((value, mapping)),
        }
    }
}

fn guard_extent_origin_v1(
    kir: &KirCorrelationIndexV1<'_>,
    authority: &KirScalarCorrelationAuthorityV1,
    value: ValueId,
    budget: &mut UnsupportedIndexCorrelationBudgetV1,
) -> Option<ValueId> {
    let value = unique_kir_ssa_origin_v1(kir, value, budget)?;
    let OperationKind::SliceLength { slice } = kir.definitions.get(&value)?.kind else {
        return None;
    };
    let slice = unique_kir_ssa_origin_v1(kir, slice, budget)?;
    if let Some(OperationKind::GlobalCapabilityBind(bind)) =
        kir.definitions.get(&slice).map(|operation| &operation.kind)
    {
        authority.capabilities.get(&slice)?;
        unique_kir_ssa_origin_v1(kir, bind.physical, budget)
    } else {
        Some(slice)
    }
}

fn same_guard_bound_v1(
    function: &Function,
    kir: &KirCorrelationIndexV1<'_>,
    authority: &KirScalarCorrelationAuthorityV1,
    left: ValueId,
    right: ValueId,
    budget: &mut UnsupportedIndexCorrelationBudgetV1,
) -> Option<bool> {
    if left == right {
        return Some(true);
    }
    let bounds = |value| match kir.definitions.get(&value)?.kind {
        OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs,
            rhs,
        } => Some((lhs, rhs)),
        _ => None,
    };
    let (Some((li, le)), Some((ri, re))) = (bounds(left), bounds(right)) else {
        return Some(false);
    };
    let (Some(li), Some(ri)) = (
        guard_index_origin_v1(function, kir, authority, li, budget),
        guard_index_origin_v1(function, kir, authority, ri, budget),
    ) else {
        return Some(false);
    };
    let (Some(le), Some(re)) = (
        guard_extent_origin_v1(kir, authority, le, budget),
        guard_extent_origin_v1(kir, authority, re, budget),
    ) else {
        return Some(false);
    };
    Some(li == ri && le == re)
}

type KirKnownScalarV1 = Option<(ProductionSemanticScalarTypeV2, u64)>;

#[allow(clippy::too_many_arguments)]
fn scalar_with_false_guard_v1(
    function: &Function,
    kir: &KirCorrelationIndexV1<'_>,
    authority: &KirScalarCorrelationAuthorityV1,
    value: ValueId,
    false_guard: ValueId,
    depth: usize,
    visiting: &mut BTreeSet<ValueId>,
    budget: &mut UnsupportedIndexCorrelationBudgetV1,
) -> Option<KirKnownScalarV1> {
    budget.charge()?;
    if depth > MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 {
        return None;
    }
    let value = unique_kir_ssa_origin_v1(kir, value, budget)?;
    if !visiting.insert(value) {
        return None;
    }
    let boolean = |value| Some((ProductionSemanticScalarTypeV2::Bool, u64::from(value)));
    let mut inner = || -> Option<KirKnownScalarV1> {
        if same_guard_bound_v1(function, kir, authority, value, false_guard, budget)? {
            return Some(boolean(false));
        }
        let Some(operation) = kir.definitions.get(&value) else {
            return Some(None);
        };
        let recurse = |operand,
                       visiting: &mut BTreeSet<ValueId>,
                       budget: &mut UnsupportedIndexCorrelationBudgetV1| {
            scalar_with_false_guard_v1(
                function,
                kir,
                authority,
                operand,
                false_guard,
                depth + 1,
                visiting,
                budget,
            )
        };
        Some(match &operation.kind {
            OperationKind::Constant(constant) => normalize_kir_constant_v1(constant),
            OperationKind::Unary {
                op: UnaryOp::Not,
                operand,
            } => match recurse(*operand, visiting, budget)? {
                Some((ProductionSemanticScalarTypeV2::Bool, value)) => boolean(value == 0),
                _ => None,
            },
            OperationKind::Binary {
                op: BinaryOp::BitAnd | BinaryOp::BitOr,
                lhs,
                rhs,
            } => {
                let lhs = recurse(*lhs, visiting, budget)?;
                let rhs = recurse(*rhs, visiting, budget)?;
                let and = matches!(
                    operation.kind,
                    OperationKind::Binary {
                        op: BinaryOp::BitAnd,
                        ..
                    }
                );
                let decisive = if and { 0 } else { 1 };
                if lhs == Some((ProductionSemanticScalarTypeV2::Bool, decisive))
                    || rhs == Some((ProductionSemanticScalarTypeV2::Bool, decisive))
                {
                    boolean(!and)
                } else if let (
                    Some((ProductionSemanticScalarTypeV2::Bool, lhs)),
                    Some((ProductionSemanticScalarTypeV2::Bool, rhs)),
                ) = (lhs, rhs)
                {
                    boolean(if and {
                        lhs != 0 && rhs != 0
                    } else {
                        lhs != 0 || rhs != 0
                    })
                } else {
                    None
                }
            }
            OperationKind::Compare {
                predicate: ComparePredicate::Equal | ComparePredicate::NotEqual,
                lhs,
                rhs,
            } => {
                match (
                    recurse(*lhs, visiting, budget)?,
                    recurse(*rhs, visiting, budget)?,
                ) {
                    (Some((lt, lhs)), Some((rt, rhs)))
                        if lt == rt
                            && matches!(
                                lt,
                                ProductionSemanticScalarTypeV2::Bool
                                    | ProductionSemanticScalarTypeV2::Integer { .. }
                            ) =>
                    {
                        boolean(
                            (lhs == rhs)
                                == matches!(
                                    operation.kind,
                                    OperationKind::Compare {
                                        predicate: ComparePredicate::Equal,
                                        ..
                                    }
                                ),
                        )
                    }
                    _ => None,
                }
            }
            OperationKind::Select {
                condition,
                true_value,
                false_value,
            } => match recurse(*condition, visiting, budget)? {
                Some((ProductionSemanticScalarTypeV2::Bool, condition)) => recurse(
                    if condition == 0 {
                        *false_value
                    } else {
                        *true_value
                    },
                    visiting,
                    budget,
                )?,
                _ => {
                    let left = recurse(*true_value, visiting, budget)?;
                    let right = recurse(*false_value, visiting, budget)?;
                    if left == right { left } else { None }
                }
            },
            _ => None,
        })
    };
    let result = inner();
    visiting.remove(&value);
    result
}

fn prove_guarded_value_at_consumer_v1(
    function: &Function,
    kir: &KirCorrelationIndexV1<'_>,
    use_context: &KirScalarUseContextV1<'_>,
    load: FunctionOperationLocation,
    predicate: ValueId,
    budget: &mut UnsupportedIndexCorrelationBudgetV1,
) -> Option<()> {
    let body = function.body.as_ref()?;
    let entry = body.blocks.first()?.id;
    let consumer = use_context.consumer;
    let operation = kir.operations.get(&consumer)?;
    kir_written_value_v1(operation)?;
    let mut blocks = BTreeMap::new();
    let mut predecessors = BTreeMap::<BlockId, BTreeSet<BlockId>>::new();
    for block in &body.blocks {
        budget.charge()?;
        blocks.insert(block.id, block);
        for target in block.terminator.as_ref()?.successors() {
            budget.charge()?;
            predecessors.entry(target).or_default().insert(block.id);
        }
    }
    let mut ancestors = BTreeSet::new();
    let mut pending = vec![consumer.block];
    while let Some(block) = pending.pop() {
        budget.charge()?;
        if ancestors.insert(block) {
            pending.extend(predecessors.get(&block).into_iter().flatten().copied());
        }
    }
    if !ancestors.contains(&entry) {
        return None;
    }
    let mut reachable = BTreeSet::new();
    let mut pending = vec![entry];
    while let Some(block) = pending.pop() {
        budget.charge()?;
        if ancestors.contains(&block) && reachable.insert(block) {
            pending.extend(blocks.get(&block)?.terminator.as_ref()?.successors());
        }
    }
    if !reachable.contains(&load.block)
        || (load.block == consumer.block && load.operation_index >= consumer.operation_index)
    {
        return None;
    }
    // Reject cyclic corridors: a static SSA origin alone cannot distinguish
    // a loop-carried previous load from the current iteration's predicate.
    let mut indegree = BTreeMap::new();
    for &block in &reachable {
        budget.charge()?;
        indegree.insert(
            block,
            predecessors
                .get(&block)
                .into_iter()
                .flatten()
                .filter(|source| reachable.contains(source))
                .count(),
        );
    }
    let mut ready = indegree
        .iter()
        .filter_map(|(&block, &degree)| (degree == 0).then_some(block))
        .collect::<BTreeSet<_>>();
    let mut order = vec![];
    while let Some(block) = ready.pop_first() {
        budget.charge()?;
        order.push(block);
        for target in blocks
            .get(&block)?
            .terminator
            .as_ref()?
            .successors()
            .into_iter()
            .collect::<BTreeSet<_>>()
        {
            budget.charge()?;
            if let Some(degree) = indegree.get_mut(&target) {
                *degree = degree.checked_sub(1)?;
                if *degree == 0 {
                    ready.insert(target);
                }
            }
        }
    }
    if order.len() != reachable.len() {
        return None;
    }
    let mut without_load = BTreeSet::from([entry]);
    let mut when_false = BTreeSet::from([entry]);
    for block in order {
        budget.charge()?;
        let terminator = blocks.get(&block)?.terminator.as_ref()?;
        if block == consumer.block {
            if load.block != consumer.block && without_load.contains(&block) {
                return None;
            }
            if !when_false.contains(&block) {
                return Some(());
            }
            let OperationKind::GuardedStore {
                predicate: store_guard,
                ..
            } = operation.kind
            else {
                return None;
            };
            return (scalar_with_false_guard_v1(
                function,
                kir,
                use_context.authority,
                store_guard,
                predicate,
                0,
                &mut BTreeSet::new(),
                budget,
            )? == Some((ProductionSemanticScalarTypeV2::Bool, 0)))
            .then_some(());
        }
        if block != load.block && without_load.contains(&block) {
            without_load.extend(terminator.successors());
        }
        if !when_false.contains(&block) {
            continue;
        }
        let mut evaluate = |value| {
            scalar_with_false_guard_v1(
                function,
                kir,
                use_context.authority,
                value,
                predicate,
                0,
                &mut BTreeSet::new(),
                budget,
            )
        };
        let successors = match terminator {
            Terminator::ConditionalBranch {
                condition,
                then_target,
                else_target,
                ..
            } => match evaluate(*condition)? {
                Some((ProductionSemanticScalarTypeV2::Bool, value)) => vec![if value == 0 {
                    *else_target
                } else {
                    *then_target
                }],
                _ => terminator.successors(),
            },
            Terminator::Switch {
                selector,
                cases,
                default_target,
                ..
            } => match evaluate(*selector)? {
                Some((ProductionSemanticScalarTypeV2::Integer { .. }, value)) => vec![
                    cases
                        .iter()
                        .find(|case| case.value == value)
                        .map_or(*default_target, |case| case.target),
                ],
                _ => terminator.successors(),
            },
            Terminator::IntegerSwitch {
                selector,
                cases,
                default_target,
                ..
            } => match evaluate(*selector)? {
                Some(value) => {
                    let mut selected = *default_target;
                    for case in cases {
                        budget.charge()?;
                        let constant = normalize_kir_constant_v1(&case.value)?;
                        if constant == value {
                            selected = case.target;
                            break;
                        }
                    }
                    vec![selected]
                }
                _ => terminator.successors(),
            },
            _ => terminator.successors(),
        };
        when_false.extend(successors);
    }
    None
}
