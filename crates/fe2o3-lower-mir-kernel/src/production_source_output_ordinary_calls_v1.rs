// Exact retained-call bindings on the already checked canonical graph. These
// rows never replace executable calls or establish value/control authority.

#[derive(Debug)]
struct SourceOutputOrdinaryFunctionV1 {
    input: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    source: Option<SemanticFunctionIdV1>,
    calls: std::ops::Range<usize>,
    returns: std::ops::Range<usize>,
    return_occurrences: usize,
    return_values: usize,
    refusal: Option<&'static str>,
}

#[derive(Debug)]
struct SourceOutputOrdinaryCallV1 {
    output: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    original: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    target: Option<usize>,
    defined: bool,
    arguments: std::ops::Range<usize>,
    results: std::ops::Range<usize>,
    source_bound: bool,
    typed: bool,
}

/// Inert typed argument locators; only a borrow of the enclosing exact-owner
/// view can qualify them. Copying a locator grants no graph or proof authority.
#[derive(Clone, Copy, Debug)]
pub struct ProductionSourceOutputOrdinaryArgumentV1 {
    /// Call operand use in the bound input graph.
    pub original_use: fe2o3_kernel_ir::CanonicalKirUseCoordinateV1,
    /// Corresponding Call operand use in the optimized graph.
    pub output_use: fe2o3_kernel_ir::CanonicalKirUseCoordinateV1,
    /// Optimized definition supplying the actual argument.
    pub actual: fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1,
    /// Matching parameter definition in the optimized callee.
    pub formal: fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1,
    /// Shared scalar type of the actual argument and formal parameter.
    pub scalar: fe2o3_kernel_ir::ScalarType,
}

/// Inert logical result-slot locators, not numerical result equivalence.
#[derive(Clone, Copy, Debug)]
pub struct ProductionSourceOutputOrdinaryResultV1 {
    /// Call result-slot definition in the bound input graph.
    pub original: fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1,
    /// Corresponding Call result-slot definition in the optimized graph.
    pub output: fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1,
    /// Scalar type of this logical result slot.
    pub scalar: fe2o3_kernel_ir::ScalarType,
}

/// Inert Return slot: ordinal, original use, O use, O definition and scalar.
/// An empty collection never establishes that a function returns.
#[derive(Clone, Copy, Debug)]
pub struct ProductionSourceOutputOrdinaryReturnV1 {
    /// Optimized callee block containing this Return.
    pub block: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1,
    /// Slot ordinal, input use, optimized use and definition, and scalar type.
    /// None represents one actual zero-result Return, not an absent return.
    pub value: Option<(
        u32,
        fe2o3_kernel_ir::CanonicalKirUseCoordinateV1,
        fe2o3_kernel_ir::CanonicalKirUseCoordinateV1,
        fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1,
        fe2o3_kernel_ir::ScalarType,
    )>,
}

#[derive(Clone, Copy, Debug)]
struct SourceOutputOrdinaryAliasV1 {
    original: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    owner: SemanticFunctionIdV1,
    function: SemanticFunctionIdV1,
    block: SemanticBlockIdV1,
    callee: SemanticFunctionIdV1,
    target: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
}

#[derive(Debug, Default)]
struct SourceOutputOrdinaryCallIndexV1 {
    functions: Vec<SourceOutputOrdinaryFunctionV1>,
    calls: Vec<SourceOutputOrdinaryCallV1>,
    arguments: Vec<ProductionSourceOutputOrdinaryArgumentV1>,
    results: Vec<ProductionSourceOutputOrdinaryResultV1>,
    returns: Vec<ProductionSourceOutputOrdinaryReturnV1>,
    aliases: Vec<SourceOutputOrdinaryAliasV1>,
}

/// Counts from one exact retained call's checked, memory-free helper closure.
/// This diagnostic is not a value, termination, convergence, ordering, purity
/// for speculation, or final attachment proof. The executable Call is unchanged.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionSourceOutputOrdinaryCallDiagnosticV1 {
    /// Semantic block containing the source Call terminator.
    pub source_block: SemanticBlockIdV1,
    /// Source function identified as the retained callee.
    pub callee: SemanticFunctionIdV1,
    /// Number of checked scalar argument bindings at this Call.
    pub arguments: usize,
    /// Number of checked scalar result slots at this Call.
    pub results: usize,
    /// Number of Return terminators in the callee, not a termination proof.
    pub return_occurrences: usize,
    /// Number of scalar value slots across the callee's Return terminators.
    pub return_values: usize,
}

/// Borrowed inert slots from this exact source/output view. Iterating payloads
/// is the caller's shared-ledger obligation. This is not a transferable proof.
pub struct ProductionSourceOutputOrdinaryCallBindingsV1<'a> {
    /// Source identifiers and counts for this retained Call.
    pub diagnostic: ProductionSourceOutputOrdinaryCallDiagnosticV1,
    /// Typed actual/formal argument bindings in logical argument order.
    pub arguments: &'a [ProductionSourceOutputOrdinaryArgumentV1],
    /// Typed Call results in logical result order.
    pub results: &'a [ProductionSourceOutputOrdinaryResultV1],
    /// Every callee Return slot, including explicit zero-result Returns.
    pub returns: &'a [ProductionSourceOutputOrdinaryReturnV1],
}

fn source_output_ordinary_push_v1<T>(
    rows: &mut Vec<T>,
    row: T,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(), ProductionSourceOutputErrorV1> {
    // Existing reserve/push reconciles capacity before another fallible action.
    assert_origin_push_v1(rows, row, budget).map_err(ProductionSourceOutputErrorV1::SourceOrigin)
}

fn source_output_ordinary_payload_v1<T>(
    rows: &Vec<T>,
) -> Result<usize, ProductionSourceOutputErrorV1> {
    rows.capacity().checked_mul(std::mem::size_of::<T>()).ok_or(
        ProductionSourceOutputErrorV1::Resource(AssertOriginResourceV1::Arithmetic),
    )
}

impl SourceOutputOrdinaryCallIndexV1 {
    fn payload(&self) -> Result<usize, ProductionSourceOutputErrorV1> {
        let sizes = [
            source_output_ordinary_payload_v1(&self.functions)?,
            source_output_ordinary_payload_v1(&self.calls)?,
            source_output_ordinary_payload_v1(&self.arguments)?,
            source_output_ordinary_payload_v1(&self.results)?,
            source_output_ordinary_payload_v1(&self.returns)?,
            source_output_ordinary_payload_v1(&self.aliases)?,
        ];
        sizes.into_iter().try_fold(0usize, |sum, bytes| {
            sum.checked_add(bytes)
                .ok_or(ProductionSourceOutputErrorV1::Resource(
                    AssertOriginResourceV1::Arithmetic,
                ))
        })
    }
}

fn source_output_ordinary_function_alias_v1(
    source: &ProductionPreRankedKirOwnerV1,
    owner: SemanticFunctionIdV1,
    function: SemanticFunctionIdV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    let rows = &source.assert_origins().origins.functions;
    let found = assert_origin_find_v1(rows, budget, |row, budget| {
        budget.charge_work(2)?;
        Ok((row.owner, row.function).cmp(&(owner, function)))
    })
    .map_err(Error::SourceOrigin)?
    .ok_or(Error::Invalid("ordinary call source function alias absent"))?;
    budget.charge_work(1).map_err(Error::Resource)?;
    Ok(rows[found].canonical)
}

fn source_output_ordinary_type_v1(
    source: &ProductionPreRankedKirOwnerV1,
    ty: SemanticTypeIdV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<bool, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    let mut denied = None;
    let result = check_ordinary_helper_value_type_v1(
        source.semantic_ssa().source_semantic().types(),
        ty,
        &mut |amount| {
            budget.charge_work(amount).map_err(|error| {
                denied = Some(error);
                ProductionSemanticKirErrorV1::ResourceLimit {
                    resource: ProductionSemanticKirResourceV1::AnalysisWork,
                    actual: usize::MAX,
                    limit: 0,
                }
            })
        },
    );
    if let Some(error) = denied {
        return Err(Error::Resource(error));
    }
    match result {
        Ok(()) => Ok(true),
        Err(
            ProductionSemanticKirErrorV1::Unsupported { .. }
            | ProductionSemanticKirErrorV1::ScalarTypeUnavailable { .. },
        ) => Ok(false),
        Err(error) => Err(Error::SourceReplay(error)),
    }
}

fn source_output_ordinary_aliases_v1(
    source: &ProductionPreRankedKirOwnerV1,
    input: &fe2o3_kernel_analysis::CanonicalKirInventoryV1<'_>,
    aliases: &mut Vec<SourceOutputOrdinaryAliasV1>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    use fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 as Coordinate;
    let semantic = source.semantic_ssa().source_semantic();
    for span in source.correspondence.terminator_operation_spans() {
        budget.charge_work(5).map_err(Error::Resource)?;
        let function = semantic
            .functions()
            .get(span.semantic_function.index() as usize)
            .ok_or(Error::Invalid("ordinary call semantic function absent"))?;
        let block = function
            .blocks()
            .get(span.semantic_block.index() as usize)
            .ok_or(Error::Invalid("ordinary call semantic block absent"))?;
        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
            continue;
        };
        let Some(SemanticCallableDeclV1::Defined { function: callee }) =
            semantic.callables().get(call.callee().index() as usize)
        else {
            continue;
        };
        let caller = source_output_ordinary_function_alias_v1(
            source,
            span.correspondence_owner,
            span.semantic_function,
            budget,
        )?;
        let target = source_output_ordinary_function_alias_v1(
            source,
            span.correspondence_owner,
            *callee,
            budget,
        )?;
        let original_block = input
            .block_for_id(caller, span.kernel_ir_block, budget)
            .map_err(Error::Inventory)?
            .ok_or(Error::Invalid("ordinary call original block absent"))?;
        budget.charge_work(4).map_err(Error::Resource)?;
        let start = span.first_operation_ordinal as usize;
        let end = start
            .checked_add(span.operation_count as usize)
            .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
        let operations = original_block
            .block
            .operations
            .get(start..end)
            .ok_or(Error::Invalid(
                "ordinary call source operation span differs",
            ))?;
        let mut found = None;
        for (offset, operation) in operations.iter().enumerate() {
            budget.charge_work(2).map_err(Error::Resource)?;
            if let OperationKind::Call { callee: name, .. } = &operation.kind {
                let actual = input
                    .function_for_name(name.as_str(), budget)
                    .map_err(Error::Inventory)?;
                budget.charge_work(3).map_err(Error::Resource)?;
                if found.is_some() || actual.map(|row| row.coordinate) != Some(target) {
                    return Err(Error::Invalid(
                        "ordinary call source target or multiplicity differs",
                    ));
                }
                found = Some(Coordinate {
                    block: original_block.coordinate,
                    operation: u32::try_from(start + offset)
                        .map_err(|_| Error::Resource(AssertOriginResourceV1::Arithmetic))?,
                });
            }
        }
        budget.charge_work(1).map_err(Error::Resource)?;
        let original = found.ok_or(Error::Invalid(
            "ordinary source Call has no executable occurrence",
        ))?;
        source_output_ordinary_push_v1(
            aliases,
            SourceOutputOrdinaryAliasV1 {
                original,
                owner: span.correspondence_owner,
                function: span.semantic_function,
                block: span.semantic_block,
                callee: *callee,
                target,
            },
            budget,
        )?;
    }
    assert_origin_sort_v1(aliases, budget, |a, b, budget| {
        budget.charge_work(5)?;
        Ok((a.original, a.owner, a.function).cmp(&(b.original, b.owner, b.function)))
    })
    .map_err(Error::SourceOrigin)?;
    for pair in aliases.windows(2) {
        budget.charge_work(5).map_err(Error::Resource)?;
        if (pair[0].original, pair[0].owner, pair[0].function)
            == (pair[1].original, pair[1].owner, pair[1].function)
        {
            return Err(Error::Invalid("ordinary source Call alias duplicated"));
        }
    }
    Ok(())
}

fn source_output_ordinary_body_v1(
    function: &fe2o3_kernel_analysis::CanonicalKirFunctionRefV1<'_>,
    output: &fe2o3_kernel_analysis::CanonicalKirInventoryV1<'_>,
    transition: &fe2o3_kernel_analysis::CheckedCanonicalKirTransitionV1<'_, '_, '_, '_>,
    returns: &mut Vec<ProductionSourceOutputOrdinaryReturnV1>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<Option<&'static str>, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    let mut refusal = None;
    budget.charge_work(4).map_err(Error::Resource)?;
    if function.function.role != fe2o3_kernel_ir::FunctionRole::InternalHelper
        || function.function.body.is_none()
        || !function.function.required_capabilities.is_empty()
        || !function.effects.is_empty()
    {
        refusal = Some("ordinary helper role, storage or capability is unsupported");
    }
    for ty in function
        .function
        .signature
        .parameters
        .iter()
        .chain(&function.function.signature.results)
    {
        budget.charge_work(1).map_err(Error::Resource)?;
        if ty.as_scalar().is_none() {
            refusal = Some("ordinary helper signature is not scalar transport");
        }
    }
    for definition in &output.definitions()[function.definitions.clone()] {
        budget.charge_work(1).map_err(Error::Resource)?;
        if definition.ty.as_scalar().is_none() {
            refusal = Some("ordinary helper contains a non-scalar definition");
        }
    }
    for operation in &output.operations()[function.operations.clone()] {
        budget.charge_work(2).map_err(Error::Resource)?;
        if !matches!(
            operation.operation.kind,
            OperationKind::Constant(_)
                | OperationKind::Intrinsic(_)
                | OperationKind::Unary { .. }
                | OperationKind::Binary { .. }
                | OperationKind::Compare { .. }
                | OperationKind::Select { .. }
                | OperationKind::Cast { .. }
                | OperationKind::Call { .. }
        ) {
            refusal = Some("ordinary helper contains an unsupported operation");
        }
    }
    // Every block, including unreachable blocks and loops, is checked. No CFG
    // edge is removed, no loop is executed/unrolled, and no return is presumed.
    for block in &output.blocks()[function.blocks.clone()] {
        budget.charge_work(2).map_err(Error::Resource)?;
        for edge in &output.edges()[block.edges.clone()] {
            budget.charge_work(2).map_err(Error::Resource)?;
            if edge.target.function != function.coordinate {
                return Err(Error::Invalid("ordinary helper edge leaves its function"));
            }
            for binding in &output.edge_arguments()[edge.bindings.clone()] {
                budget.charge_work(4).map_err(Error::Resource)?;
                let actual = &output.definitions()[binding.incoming_definition];
                let formal = &output.definitions()[binding.target_definition];
                if actual.ty.as_scalar().is_none() || actual.ty.as_scalar() != formal.ty.as_scalar()
                {
                    refusal = Some("ordinary helper edge argument is not exact scalar transport");
                }
            }
        }
        for use_ in &output.uses()[block.terminator_uses.clone()] {
            budget.charge_work(2).map_err(Error::Resource)?;
            if output.definitions()[use_.definition]
                .ty
                .as_scalar()
                .is_none()
            {
                refusal = Some("ordinary helper terminator has a non-scalar operand");
            }
        }
        if let Terminator::Return { values } = block.terminator {
            budget.charge_work(2).map_err(Error::Resource)?;
            if values.len() != function.function.signature.results.len()
                || values.len() != block.terminator_uses.len()
            {
                return Err(Error::Invalid("ordinary helper Return arity differs"));
            }
            if values.is_empty() {
                source_output_ordinary_push_v1(
                    returns,
                    ProductionSourceOutputOrdinaryReturnV1 {
                        block: block.coordinate,
                        value: None,
                    },
                    budget,
                )?;
            }
            for (slot, value) in values.iter().enumerate() {
                budget.charge_work(5).map_err(Error::Resource)?;
                let use_ = &output.uses()[block.terminator_uses.start + slot];
                let definition = &output.definitions()[use_.definition];
                let Some(scalar) = definition.ty.as_scalar() else {
                    continue;
                };
                if use_.value != *value
                    || Some(scalar) != function.function.signature.results[slot].as_scalar()
                {
                    return Err(Error::Invalid("ordinary helper Return slot differs"));
                }
                source_output_ordinary_push_v1(
                    returns,
                    ProductionSourceOutputOrdinaryReturnV1 {
                        block: block.coordinate,
                        value: Some((
                            u32::try_from(slot)
                                .map_err(|_| Error::Resource(AssertOriginResourceV1::Arithmetic))?,
                            transition.rows().uses[block.terminator_uses.start + slot].input,
                            use_.coordinate,
                            definition.coordinate,
                            scalar,
                        )),
                    },
                    budget,
                )?;
            }
        }
    }
    Ok(refusal)
}

fn source_output_ordinary_closure_v1(
    functions: &mut [SourceOutputOrdinaryFunctionV1],
    calls: &[SourceOutputOrdinaryCallV1],
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    let mut states = Vec::new();
    let mut pending = Vec::<(usize, usize)>::new();
    budget.charge_work(1).map_err(Error::Resource)?;
    budget
        .reserve_storage(std::mem::size_of_val(&states) + std::mem::size_of_val(&pending))
        .map_err(Error::Resource)?;
    for _ in &*functions {
        source_output_ordinary_push_v1(&mut states, 0u8, budget)?;
    }
    for root in 0..functions.len() {
        budget.charge_work(2).map_err(Error::Resource)?;
        if states[root] != 0 {
            continue;
        }
        states[root] = 1;
        source_output_ordinary_push_v1(&mut pending, (root, functions[root].calls.start), budget)?;
        loop {
            budget.charge_work(1).map_err(Error::Resource)?;
            let Some(&(function, cursor)) = pending.last() else {
                break;
            };
            budget.charge_work(3).map_err(Error::Resource)?;
            if cursor == functions[function].calls.end {
                states[function] = 2;
                pending.pop();
                continue;
            }
            let call = calls
                .get(cursor)
                .ok_or(Error::Invalid("ordinary closure call range differs"))?;
            let Some(target) = call.target else {
                functions[function].refusal =
                    Some("ordinary helper contains an unqualified external Call");
                pending.last_mut().unwrap().1 += 1;
                continue;
            };
            budget.charge_work(4).map_err(Error::Resource)?;
            if !call.source_bound || !call.typed || target >= functions.len() {
                functions[function].refusal =
                    Some("ordinary helper has an unbound or mistyped Call");
                pending.last_mut().unwrap().1 += 1;
            } else if states[target] == 0 {
                states[target] = 1;
                source_output_ordinary_push_v1(
                    &mut pending,
                    (target, functions[target].calls.start),
                    budget,
                )?;
            } else {
                if states[target] == 1 || functions[target].refusal.is_some() {
                    functions[function].refusal =
                        Some("ordinary helper closure is recursive or nonqualifying");
                }
                pending.last_mut().unwrap().1 += 1;
            }
        }
    }
    assert_origin_drop_v1(pending, budget).map_err(Error::SourceOrigin)?;
    assert_origin_drop_v1(states, budget).map_err(Error::SourceOrigin)?;
    budget
        .release_storage(
            std::mem::size_of::<Vec<u8>>() + std::mem::size_of::<Vec<(usize, usize)>>(),
        )
        .map_err(Error::Resource)
}

fn source_output_ordinary_calls_v1(
    source: &ProductionPreRankedKirOwnerV1,
    transition: &fe2o3_kernel_analysis::CheckedCanonicalKirTransitionV1<'_, '_, '_, '_>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(SourceOutputOrdinaryCallIndexV1, usize, usize), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    use fe2o3_kernel_ir::{
        CanonicalKirDefinitionCoordinateV1 as Definition, CanonicalKirOperationOriginV1 as Origin,
    };
    let input = transition.input();
    let output = transition.output();
    let header = std::mem::size_of::<SourceOutputOrdinaryCallIndexV1>();
    budget.charge_work(1).map_err(Error::Resource)?;
    budget.reserve_storage(header).map_err(Error::Resource)?;
    let mut index = SourceOutputOrdinaryCallIndexV1::default();
    source_output_ordinary_aliases_v1(source, input, &mut index.aliases, budget)?;
    let semantic = source.semantic_ssa().source_semantic();
    // Function rosters are dense, but checked transition associations, not raw
    // ordinal equality, bind B functions to O functions.
    for function in output.functions() {
        budget.charge_work(4).map_err(Error::Resource)?;
        let original = transition.rows().functions[function.coordinate.0 as usize].input;
        source_output_ordinary_push_v1(
            &mut index.functions,
            SourceOutputOrdinaryFunctionV1 {
                input: original,
                source: None,
                calls: function.calls.clone(),
                returns: 0..0,
                return_occurrences: 0,
                return_values: 0,
                refusal: Some("ordinary helper lacks a source alias"),
            },
            budget,
        )?;
    }
    for alias in source.correspondence.lowered_functions.iter() {
        budget.charge_work(2).map_err(Error::Resource)?;
        let function = output
            .function_for_name(alias.kernel_ir_function.as_str(), budget)
            .map_err(Error::Inventory)?
            .ok_or(Error::Invalid("ordinary helper exact function absent"))?;
        let row = &mut index.functions[function.coordinate.0 as usize];
        budget.charge_work(4).map_err(Error::Resource)?;
        if row.source.is_some_and(|old| old != alias.semantic_function) {
            return Err(Error::Invalid("ordinary helper function aliases disagree"));
        }
        row.source = Some(alias.semantic_function);
    }
    for function in output.functions() {
        budget.charge_work(3).map_err(Error::Resource)?;
        let row = &mut index.functions[function.coordinate.0 as usize];
        row.returns.start = index.returns.len();
        row.refusal = source_output_ordinary_body_v1(
            function,
            output,
            transition,
            &mut index.returns,
            budget,
        )?;
        row.returns.end = index.returns.len();
        let mut previous = None;
        for return_ in &index.returns[row.returns.clone()] {
            budget.charge_work(3).map_err(Error::Resource)?;
            if previous != Some(return_.block) {
                row.return_occurrences += 1;
                previous = Some(return_.block);
            }
            if return_.value.is_some() {
                row.return_values += 1;
            }
        }
        let source_function = row
            .source
            .and_then(|id| semantic.functions().get(id.index() as usize));
        if let Some(source_function) = source_function {
            if !source_output_ordinary_type_v1(
                source,
                source_function.abi().source_output_type(),
                budget,
            )? {
                row.refusal = Some("ordinary helper source result shape is unsupported");
            }
            for ty in source_function.abi().source_input_types() {
                budget.charge_work(2).map_err(Error::Resource)?;
                if !matches!(
                    semantic.types()[ty.index() as usize].shape(),
                    SemanticTypeShapeV1::Scalar(_)
                ) {
                    row.refusal = Some("ordinary helper source argument is not scalar");
                }
            }
        } else {
            row.refusal = Some("ordinary helper source function absent");
        }
    }
    for call in output.calls() {
        budget.charge_work(4).map_err(Error::Resource)?;
        let operation_index = assert_origin_find_v1(output.operations(), budget, |row, budget| {
            budget.charge_work(3)?;
            Ok(row.coordinate.cmp(&call.coordinate))
        })
        .map_err(Error::SourceOrigin)?
        .ok_or(Error::Invalid("ordinary output Call absent"))?;
        let operation = &output.operations()[operation_index];
        let Origin::Retained(original) = transition.rows().operations[operation_index].origin
        else {
            return Err(Error::Invalid(
                "output Call is not a checked retained occurrence",
            ));
        };
        let target = call.target.map(|coordinate| coordinate.0 as usize);
        let mut source_bound = false;
        {
            let expected = target.map(|target| index.functions[target].input);
            // A lower-bound search visits each alias only once after locating
            // the original Call; shared roots never merge their source aliases.
            let mut low = 0usize;
            let mut high = index.aliases.len();
            while low < high {
                budget.charge_work(4).map_err(Error::Resource)?;
                let middle = low + (high - low) / 2;
                if index.aliases[middle].original < original {
                    low = middle + 1;
                } else {
                    high = middle;
                }
            }
            loop {
                budget.charge_work(1).map_err(Error::Resource)?;
                let Some(alias) = index.aliases.get(low) else {
                    break;
                };
                budget.charge_work(2).map_err(Error::Resource)?;
                if alias.original != original {
                    break;
                }
                if Some(alias.target) != expected {
                    return Err(Error::Invalid("ordinary retained callee source differs"));
                }
                source_bound = true;
                low += 1;
            }
        }
        let argument_start = index.arguments.len();
        let result_start = index.results.len();
        let mut typed = target.is_some();
        if let Some(target) = target {
            let callee = &output.functions()[target];
            budget.charge_work(3).map_err(Error::Resource)?;
            if operation.operands.len() != callee.function.signature.parameters.len()
                || operation.results.len() != callee.function.signature.results.len()
            {
                return Err(Error::Invalid(
                    "ordinary retained Call signature arity differs",
                ));
            }
            for (slot, use_index) in operation.operands.clone().enumerate() {
                budget.charge_work(7).map_err(Error::Resource)?;
                let use_ = &output.uses()[use_index];
                let actual = &output.definitions()[use_.definition];
                let formal = &output.definitions()[callee.definitions.start + slot];
                let Some(scalar) = actual.ty.as_scalar() else {
                    typed = false;
                    continue;
                };
                if Some(scalar) != formal.ty.as_scalar() {
                    return Err(Error::Invalid(
                        "ordinary retained Call argument type differs",
                    ));
                }
                source_output_ordinary_push_v1(
                    &mut index.arguments,
                    ProductionSourceOutputOrdinaryArgumentV1 {
                        original_use: transition.rows().uses[use_index].input,
                        output_use: use_.coordinate,
                        actual: actual.coordinate,
                        formal: formal.coordinate,
                        scalar,
                    },
                    budget,
                )?;
            }
            for (slot, definition_index) in operation.results.clone().enumerate() {
                budget.charge_work(6).map_err(Error::Resource)?;
                let definition = &output.definitions()[definition_index];
                let Some(scalar) = definition.ty.as_scalar() else {
                    typed = false;
                    continue;
                };
                if Some(scalar) != callee.function.signature.results[slot].as_scalar() {
                    return Err(Error::Invalid("ordinary retained Call result type differs"));
                }
                source_output_ordinary_push_v1(
                    &mut index.results,
                    ProductionSourceOutputOrdinaryResultV1 {
                        original: Definition::Result {
                            operation: original,
                            result: u32::try_from(slot)
                                .map_err(|_| Error::Resource(AssertOriginResourceV1::Arithmetic))?,
                        },
                        output: definition.coordinate,
                        scalar,
                    },
                    budget,
                )?;
            }
        }
        budget.charge_work(2).map_err(Error::Resource)?;
        let defined = source_bound
            || target.is_some_and(|target| {
                output.functions()[target].function.role
                    != fe2o3_kernel_ir::FunctionRole::ExternalImport
            });
        source_output_ordinary_push_v1(
            &mut index.calls,
            SourceOutputOrdinaryCallV1 {
                output: call.coordinate,
                original,
                target,
                defined,
                arguments: argument_start..index.arguments.len(),
                results: result_start..index.results.len(),
                source_bound,
                typed,
            },
            budget,
        )?;
    }
    source_output_ordinary_closure_v1(&mut index.functions, &index.calls, budget)?;
    budget.charge_work(12).map_err(Error::Resource)?;
    let payload = index.payload()?;
    Ok((index, payload, header))
}

impl ProductionSourceOutputOccurrencesV1<'_, '_> {
    /// Inspects a source-qualified retained defined Call on this view's exact O.
    /// None is only a non-defined Call; a recognized but unsafe/missing Defined
    /// binding is an error and cannot fall through to the runtime-trap rule.
    pub fn retained_ordinary_call_v1(
        &self,
        owner: SemanticFunctionIdV1,
        function: SemanticFunctionIdV1,
        coordinate: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<Option<ProductionSourceOutputOrdinaryCallDiagnosticV1>, ProductionSourceOutputErrorV1>
    {
        self.retained_ordinary_call_bindings_v1(owner, function, coordinate, budget)
            .map(|bindings| bindings.map(|bindings| bindings.diagnostic))
    }

    /// Borrows the same checked slots; no additional graph or expression is
    /// built. Payload traversal must remain on this caller's canonical ledger.
    pub fn retained_ordinary_call_bindings_v1(
        &self,
        owner: SemanticFunctionIdV1,
        function: SemanticFunctionIdV1,
        coordinate: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<
        Option<ProductionSourceOutputOrdinaryCallBindingsV1<'_>>,
        ProductionSourceOutputErrorV1,
    > {
        use ProductionSourceOutputErrorV1 as Error;
        budget.charge_work(6).map_err(Error::Resource)?;
        let minimum = self
            .source
            .executable_storage()
            .retained_storage()
            .checked_add(self.source.assert_origin_storage().payload_storage())
            .and_then(|n| n.checked_add(self.checked_output.storage().retained_storage()))
            .and_then(|n| n.checked_add(self.storage.retained_storage()))
            .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
        if budget.storage() < minimum {
            return Err(Error::Resource(AssertOriginResourceV1::Accounting));
        }
        let ordinal = assert_origin_find_v1(&self.ordinary_calls.calls, budget, |row, budget| {
            budget.charge_work(3)?;
            Ok(row.output.cmp(&coordinate))
        })
        .map_err(Error::SourceOrigin)?
        .ok_or(Error::Invalid("ordinary output Call coordinate absent"))?;
        budget.charge_work(4).map_err(Error::Resource)?;
        let call = &self.ordinary_calls.calls[ordinal];
        if !call.defined {
            return Ok(None);
        }
        let Some(target) = call.target else {
            return Err(Error::Invalid("defined output Call has no resolved target"));
        };
        if !call.source_bound || !call.typed {
            return Err(Error::Invalid(
                "defined output Call lacks ordinary source/typed bindings",
            ));
        }
        let callee = &self.ordinary_calls.functions[target];
        budget.charge_work(2).map_err(Error::Resource)?;
        if let Some(refusal) = callee.refusal {
            return Err(Error::Invalid(refusal));
        }
        let alias = assert_origin_find_v1(&self.ordinary_calls.aliases, budget, |row, budget| {
            budget.charge_work(5)?;
            Ok((row.original, row.owner, row.function).cmp(&(call.original, owner, function)))
        })
        .map_err(Error::SourceOrigin)?
        .ok_or(Error::Invalid("ordinary Call source alias differs"))?;
        let source = self.ordinary_calls.aliases[alias];
        budget.charge_work(5).map_err(Error::Resource)?;
        let diagnostic = ProductionSourceOutputOrdinaryCallDiagnosticV1 {
            source_block: source.block,
            callee: source.callee,
            arguments: call.arguments.len(),
            results: call.results.len(),
            return_occurrences: callee.return_occurrences,
            return_values: callee.return_values,
        };
        Ok(Some(ProductionSourceOutputOrdinaryCallBindingsV1 {
            diagnostic,
            arguments: &self.ordinary_calls.arguments[call.arguments.clone()],
            results: &self.ordinary_calls.results[call.results.clone()],
            returns: &self.ordinary_calls.returns[callee.returns.clone()],
        }))
    }
}

include!("production_semantic_kir_v1/tests/production_source_output_ordinary_calls_v1_tests.rs");
