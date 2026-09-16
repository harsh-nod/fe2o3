/// Explicit limits of the unactivated ordinary Global occurrence bridge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionSourceOutputGlobalUnsupportedV1 {
    /// The old correlation engine selects the kernel entry, not another body.
    NonEntryFunction,
    /// This source span contains a call, including a lowered generated recipe.
    Call,
    /// This span contains Private memory, including compiler-owned storage.
    PrivateMemory,
    /// This span contains another memory, atomic, volatile or collective effect.
    OtherMemory,
    /// The unchanged pointer grammar cannot identify one external parameter.
    AllocationAncestry,
}

/// Checked occurrence placement, not a memory proof or attachment authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionSourceOutputGlobalAccessV1 {
    /// This source span needs a separate, explicit correlation rule.
    Unsupported(ProductionSourceOutputGlobalUnsupportedV1),
    /// A genuine original access is checked unreachable and absent from O.
    OmittedUnreachable {
        /// Exact original N access, independently known to be unreachable.
        original: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    },
    /// The exact physical O access descended from this source occurrence.
    Retained {
        /// Exact original N access; source aliases remain in the query key.
        original: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
        /// Actual O operation, never an original span ordinal interpreted in O.
        operation: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
        /// Original source argument, after exact N parameter/component lookup.
        source_argument: u32,
        /// Actual O function argument with a checked original N ancestor.
        parameter: fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1,
        /// Actual pointer use and the exact O definition it consumes.
        pointer: fe2o3_kernel_analysis::CanonicalKirOutputUseV1,
        /// Actual Store value use; absent for Load.
        value: Option<fe2o3_kernel_analysis::CanonicalKirOutputUseV1>,
        /// Actual Load result descended from the exact original result.
        result: Option<fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1>,
        /// Reachability is independent of physical retention.
        executable: bool,
    },
}

#[derive(Debug)]
struct SourceOutputGlobalSpanV1 {
    key: [u32; 5],
    first: usize,
    count: usize,
    unsupported: Option<ProductionSourceOutputGlobalUnsupportedV1>,
}

#[derive(Debug)]
struct SourceOutputGlobalRowV1 {
    placement: ProductionSourceOutputGlobalAccessV1,
}

fn source_output_global_key_v1(
    owner: SemanticFunctionIdV1,
    function: SemanticFunctionIdV1,
    block: u32,
    statement: Option<u32>,
) -> [u32; 5] {
    [
        owner.index(),
        function.index(),
        block,
        u32::from(statement.is_none()),
        statement.unwrap_or(0),
    ]
}

fn source_output_global_kind_v1(
    operation: &fe2o3_kernel_analysis::CanonicalKirOperationRefV1<'_>,
) -> Result<Option<(ValueId, Option<ValueId>)>, ProductionSourceOutputGlobalUnsupportedV1> {
    // The caller pays before invoking this fixed classification; no allocation
    // or memory_effects() vector is hidden in this path.
    use ProductionSourceOutputGlobalUnsupportedV1 as Unsupported;
    match &operation.operation.kind {
        OperationKind::Load { pointer, access }
            if access.address_space == AddressSpace::Global && !access.volatile =>
        {
            Ok(Some((*pointer, None)))
        }
        OperationKind::Store {
            pointer,
            value,
            access,
        } if access.address_space == AddressSpace::Global && !access.volatile => {
            Ok(Some((*pointer, Some(*value))))
        }
        OperationKind::Call { .. } => Err(Unsupported::Call),
        OperationKind::Load { access, .. } | OperationKind::Store { access, .. }
            if access.address_space == AddressSpace::Private =>
        {
            Err(Unsupported::PrivateMemory)
        }
        _ if !operation.effects.is_empty() => Err(Unsupported::OtherMemory),
        _ => Ok(None),
    }
}

#[derive(Clone, Copy)]
struct SourceOutputGlobalSpanInputV1 {
    owner: SemanticFunctionIdV1,
    function: SemanticFunctionIdV1,
    block: u32,
    statement: Option<u32>,
    physical_block: BlockId,
    first: u32,
    count: u32,
}

fn source_output_global_spans_v1(
    correspondence: &SemanticKirCorrespondenceV1,
) -> impl Iterator<Item = SourceOutputGlobalSpanInputV1> + '_ {
    correspondence
        .statement_operation_spans()
        .iter()
        .map(|span| SourceOutputGlobalSpanInputV1 {
            owner: span.correspondence_owner(),
            function: span.semantic_function(),
            block: span.semantic_block().index(),
            statement: Some(span.statement_ordinal()),
            physical_block: span.kernel_ir_block(),
            first: span.first_operation_ordinal(),
            count: span.operation_count(),
        })
        .chain(
            correspondence
                .terminator_operation_spans()
                .iter()
                .map(|span| SourceOutputGlobalSpanInputV1 {
                    owner: span.correspondence_owner(),
                    function: span.semantic_function(),
                    block: span.semantic_block().index(),
                    statement: None,
                    physical_block: span.kernel_ir_block(),
                    first: span.first_operation_ordinal(),
                    count: span.operation_count(),
                }),
        )
}

struct SourceOutputGlobalSpanFactsV1<'inventory, 'graph> {
    function: &'inventory fe2o3_kernel_analysis::CanonicalKirFunctionRefV1<'graph>,
    operations: &'inventory [fe2o3_kernel_analysis::CanonicalKirOperationRefV1<'graph>],
    accesses: usize,
    unsupported: Option<ProductionSourceOutputGlobalUnsupportedV1>,
}

fn source_output_global_span_facts_v1<'inventory, 'graph>(
    source: &ProductionPreRankedKirOwnerV1,
    input: &'inventory fe2o3_kernel_analysis::CanonicalKirInventoryV1<'graph>,
    span: SourceOutputGlobalSpanInputV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<SourceOutputGlobalSpanFactsV1<'inventory, 'graph>, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    use ProductionSourceOutputGlobalUnsupportedV1 as Unsupported;
    budget.charge_work(16).map_err(Error::Resource)?;
    let origins = source.assert_origins();
    let ordinal = assert_origin_find_v1(&origins.origins.functions, budget, |entry, budget| {
        budget.charge_work(2)?;
        Ok((entry.owner, entry.function).cmp(&(span.owner, span.function)))
    })
    .map_err(Error::SourceOrigin)?
    .ok_or(Error::Invalid("global source function alias absent"))?;
    budget.charge_work(4).map_err(Error::Resource)?;
    let alias = &origins.origins.functions[ordinal];
    let function = input
        .functions()
        .get(alias.canonical.0 as usize)
        .ok_or(Error::Invalid("global original function absent"))?;
    if function.coordinate != alias.canonical {
        return Err(Error::Invalid(
            "global original function coordinate changed",
        ));
    }
    // Source replay checks the correspondence role against the actual N role;
    // the same N/B coordinate witness preserves this declaration exactly.
    if function.function.role != fe2o3_kernel_ir::FunctionRole::KernelEntry {
        return Ok(SourceOutputGlobalSpanFactsV1 {
            function,
            operations: &[],
            accesses: 0,
            unsupported: Some(Unsupported::NonEntryFunction),
        });
    }
    budget.charge_work(4).map_err(Error::Resource)?;
    let semantic_function = source
        .semantic_ssa
        .source_semantic()
        .functions()
        .get(span.function.index() as usize)
        .ok_or(Error::Invalid("global source function absent"))?;
    let semantic_block = semantic_function
        .blocks()
        .get(span.block as usize)
        .ok_or(Error::Invalid("global source block absent"))?;
    let generated_call = span.statement.is_none()
        && matches!(
            semantic_block.terminator().kind(),
            SemanticTerminatorKindV1::Call(_)
                | SemanticTerminatorKindV1::TailCall(_)
                | SemanticTerminatorKindV1::Drop { .. }
        );
    let block = input
        .block_for_id(function.coordinate, span.physical_block, budget)
        .map_err(Error::Inventory)?
        .ok_or(Error::Invalid("global original block absent"))?;
    budget.charge_work(5).map_err(Error::Resource)?;
    let relative_end = (span.first as usize)
        .checked_add(span.count as usize)
        .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
    if relative_end > block.operations.len() {
        return Err(Error::Invalid("global original source span exceeds block"));
    }
    let start = block
        .operations
        .start
        .checked_add(span.first as usize)
        .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
    let end = block
        .operations
        .start
        .checked_add(relative_end)
        .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
    let operations = input
        .operations()
        .get(start..end)
        .ok_or(Error::Invalid("global original source span absent"))?;
    let mut unsupported = generated_call.then_some(Unsupported::Call);
    let mut accesses = 0_usize;
    for operation in operations {
        budget.charge_work(5).map_err(Error::Resource)?;
        match source_output_global_kind_v1(operation) {
            Ok(Some(_)) => {
                budget.charge_work(1).map_err(Error::Resource)?;
                accesses = accesses
                    .checked_add(1)
                    .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
            }
            Err(reason) if unsupported.is_none() => unsupported = Some(reason),
            _ => {}
        }
    }
    budget.charge_work(1).map_err(Error::Resource)?;
    Ok(SourceOutputGlobalSpanFactsV1 {
        function,
        operations,
        accesses: if unsupported.is_some() { 0 } else { accesses },
        unsupported,
    })
}

fn source_output_global_rows_v1(
    source: &ProductionPreRankedKirOwnerV1,
    transition: &fe2o3_kernel_analysis::CheckedCanonicalKirTransitionV1<'_, '_, '_, '_>,
    control: &fe2o3_kernel_analysis::CheckedCanonicalKirControlIndexV1<'_, '_, '_>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<
    (
        Vec<SourceOutputGlobalSpanV1>,
        Vec<SourceOutputGlobalRowV1>,
        usize,
        usize,
    ),
    ProductionSourceOutputErrorV1,
> {
    use ProductionSourceOutputErrorV1 as Error;
    budget.charge_work(3).map_err(Error::Resource)?;
    let correspondence = &source.correspondence;
    let span_count = correspondence
        .statement_operation_spans()
        .len()
        .checked_add(correspondence.terminator_operation_spans().len())
        .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
    let mut row_limit = 0_usize;
    for span in source_output_global_spans_v1(correspondence) {
        let facts = source_output_global_span_facts_v1(source, control.input(), span, budget)?;
        budget.charge_work(1).map_err(Error::Resource)?;
        row_limit = row_limit
            .checked_add(facts.accesses)
            .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
    }
    budget.charge_work(5).map_err(Error::Resource)?;
    let bytes = span_count
        .checked_mul(std::mem::size_of::<SourceOutputGlobalSpanV1>())
        .and_then(|n| {
            row_limit
                .checked_mul(std::mem::size_of::<SourceOutputGlobalRowV1>())
                .and_then(|m| n.checked_add(m))
        })
        .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
    let header = std::mem::size_of::<Vec<SourceOutputGlobalSpanV1>>()
        .checked_add(std::mem::size_of::<Vec<SourceOutputGlobalRowV1>>())
        .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
    let live = bytes
        .checked_add(header)
        .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
    budget.reserve_storage(live).map_err(Error::Resource)?;
    let mut spans = Vec::new();
    let mut rows = Vec::new();
    let mut allocation_scratch = None;
    budget.charge_work(1).map_err(Error::Resource)?;
    spans
        .try_reserve_exact(span_count)
        .map_err(|_| Error::Resource(AssertOriginResourceV1::Allocation))?;
    budget.charge_work(1).map_err(Error::Resource)?;
    rows.try_reserve_exact(row_limit)
        .map_err(|_| Error::Resource(AssertOriginResourceV1::Allocation))?;
    for span in source_output_global_spans_v1(correspondence) {
        budget.charge_work(3).map_err(Error::Resource)?;
        let key =
            source_output_global_key_v1(span.owner, span.function, span.block, span.statement);
        let facts = source_output_global_span_facts_v1(source, control.input(), span, budget)?;
        let first = rows.len();
        budget.charge_work(1).map_err(Error::Resource)?;
        if facts.unsupported.is_none() {
            for operation in facts.operations {
                budget.charge_work(5).map_err(Error::Resource)?;
                if let Ok(Some((pointer, value))) = source_output_global_kind_v1(operation) {
                    let placement = source_output_global_placement_v1(
                        source,
                        transition,
                        control,
                        span.owner,
                        span.function,
                        facts.function,
                        operation,
                        pointer,
                        value,
                        &mut allocation_scratch,
                        budget,
                    )?;
                    budget.charge_work(2).map_err(Error::Resource)?;
                    if rows.len() >= row_limit {
                        return Err(Error::Invalid("global access row bound"));
                    }
                    rows.push(SourceOutputGlobalRowV1 { placement });
                }
            }
        }
        budget.charge_work(4).map_err(Error::Resource)?;
        let count = rows
            .len()
            .checked_sub(first)
            .ok_or(Error::Resource(AssertOriginResourceV1::Accounting))?;
        if spans.len() >= span_count || count != facts.accesses {
            return Err(Error::Invalid("global source span bound"));
        }
        spans.push(SourceOutputGlobalSpanV1 {
            key,
            first,
            count,
            unsupported: facts.unsupported,
        });
    }
    budget.charge_work(2).map_err(Error::Resource)?;
    if rows.len() != row_limit || spans.len() != span_count {
        return Err(Error::Invalid("global census differs from retained rows"));
    }
    assert_origin_sort_v1(&mut spans, budget, |a, b, budget| {
        budget.charge_work(5)?;
        Ok(a.key.cmp(&b.key))
    })
    .map_err(Error::SourceOrigin)?;
    for pair in spans.windows(2) {
        budget.charge_work(6).map_err(Error::Resource)?;
        if pair[0].key == pair[1].key {
            return Err(Error::Invalid("duplicate global source span"));
        }
    }
    if let Some(scratch) = allocation_scratch {
        let storage = scratch.storage;
        drop(scratch);
        budget.release_storage(storage).map_err(Error::Resource)?;
    }
    Ok((spans, rows, bytes, header))
}

impl ProductionSourceOutputOccurrencesV1<'_, '_> {
    /// Resolves only an exact materialized source span/access ordinal. This
    /// initial bridge refuses mixed, private, generated and call-bearing spans;
    /// callers must not interpret that refusal or an absent key as a discharge.
    #[allow(clippy::too_many_arguments)]
    pub fn global_access(
        &self,
        owner: SemanticFunctionIdV1,
        function: SemanticFunctionIdV1,
        block: u32,
        statement: Option<u32>,
        ordinal: u32,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<ProductionSourceOutputGlobalAccessV1, ProductionSourceOutputErrorV1> {
        use ProductionSourceOutputErrorV1 as Error;
        // B's separate receipt remains a caller precondition, as in B0/C.
        budget.charge_work(4).map_err(Error::Resource)?;
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
        budget.charge_work(1).map_err(Error::Resource)?;
        let key = source_output_global_key_v1(owner, function, block, statement);
        let found = assert_origin_find_v1(&self.global_spans, budget, |span, budget| {
            budget.charge_work(5)?;
            Ok(span.key.cmp(&key))
        })
        .map_err(Error::SourceOrigin)?
        .ok_or(Error::Invalid("global source span is absent"))?;
        budget.charge_work(2).map_err(Error::Resource)?;
        let span = &self.global_spans[found];
        if let Some(reason) = span.unsupported {
            return Ok(ProductionSourceOutputGlobalAccessV1::Unsupported(reason));
        }
        budget.charge_work(3).map_err(Error::Resource)?;
        if ordinal as usize >= span.count {
            return Err(Error::Invalid("global source access ordinal is absent"));
        }
        let index = span
            .first
            .checked_add(ordinal as usize)
            .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
        let row = self
            .global_accesses
            .get(index)
            .ok_or(Error::Invalid("global access row absent"))?;
        Ok(row.placement)
    }
}

fn source_output_global_descendant_v1(
    transition: &fe2o3_kernel_analysis::CheckedCanonicalKirTransitionV1<'_, '_, '_, '_>,
    input_ordinal: usize,
    original: fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1,
    output: fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    budget.charge_work(5).map_err(Error::Resource)?;
    let row = transition
        .rows()
        .definitions
        .get(input_ordinal)
        .ok_or(Error::Invalid("global original definition row absent"))?;
    if row.input != original {
        return Err(Error::Invalid(
            "global original definition coordinate changed",
        ));
    }
    let start = row.outputs.start as usize;
    let end = start
        .checked_add(row.outputs.len as usize)
        .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
    let outputs = transition
        .rows()
        .definition_outputs
        .get(start..end)
        .ok_or(Error::Invalid("global definition descendant range changed"))?;
    // The checked transition's structure pass rejects unordered or duplicate
    // descendants. Reuse that sorted range without another retained index.
    assert_origin_find_v1(outputs, budget, |descendant, budget| {
        budget.charge_work(5)?;
        Ok(descendant.output.cmp(&output))
    })
    .map_err(Error::SourceOrigin)?
    .ok_or(Error::Invalid(
        "global output definition lacks its original ancestor",
    ))?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn source_output_global_parameter_v1(
    source: &ProductionPreRankedKirOwnerV1,
    transition: &fe2o3_kernel_analysis::CheckedCanonicalKirTransitionV1<'_, '_, '_, '_>,
    owner: SemanticFunctionIdV1,
    semantic_function: SemanticFunctionIdV1,
    original_function: &fe2o3_kernel_analysis::CanonicalKirFunctionRefV1<'_>,
    output_function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    argument: u32,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(u32, fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1), ProductionSourceOutputErrorV1>
{
    use ProductionSourceOutputErrorV1 as Error;
    use fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::FunctionArgument;
    budget.charge_work(4).map_err(Error::Resource)?;
    let original = FunctionArgument {
        function: original_function.coordinate,
        argument,
    };
    let output = FunctionArgument {
        function: output_function,
        argument,
    };
    let ordinal = original_function
        .definitions
        .start
        .checked_add(argument as usize)
        .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
    source_output_global_descendant_v1(transition, ordinal, original, output, budget)?;
    let (original_value, _) = source_output_definition_v1(source.executable(), original, budget)?;
    let _ = source_output_definition_v1(transition.output().owner(), output, budget)?;
    // Bound the unchanged source-only helper's three scans and comparisons
    // before entering it, including the final source-local lookup.
    // This is not a post-hoc credit or an O ValueId used in a source table.
    budget.charge_work(8).map_err(Error::Resource)?;
    let correspondence = &source.correspondence;
    let steps = correspondence
        .lowered_functions()
        .len()
        .checked_mul(6)
        .and_then(|n| {
            correspondence
                .parameter_bindings()
                .len()
                .checked_mul(10)
                .and_then(|m| n.checked_add(m))
        })
        .and_then(|n| {
            correspondence
                .parameter_component_bindings()
                .len()
                .checked_mul(10)
                .and_then(|m| n.checked_add(m))
        })
        .and_then(|n| n.checked_add(8))
        .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
    budget.charge_work(steps).map_err(Error::Resource)?;
    let function = source
        .semantic_ssa
        .source_semantic()
        .functions()
        .get(semantic_function.index() as usize)
        .ok_or(Error::Invalid("global source function absent"))?;
    let argument = semantic_source_argument_for_kir_parameter_v1(
        correspondence,
        owner,
        function,
        original_value,
    )
    .ok_or(Error::Invalid(
        "global original parameter lacks one source argument",
    ))?;
    Ok((argument, output))
}

#[allow(clippy::too_many_arguments)]
fn source_output_global_placement_v1(
    source: &ProductionPreRankedKirOwnerV1,
    transition: &fe2o3_kernel_analysis::CheckedCanonicalKirTransitionV1<'_, '_, '_, '_>,
    control: &fe2o3_kernel_analysis::CheckedCanonicalKirControlIndexV1<'_, '_, '_>,
    owner: SemanticFunctionIdV1,
    semantic_function: SemanticFunctionIdV1,
    function: &fe2o3_kernel_analysis::CanonicalKirFunctionRefV1<'_>,
    original: &fe2o3_kernel_analysis::CanonicalKirOperationRefV1<'_>,
    pointer: ValueId,
    value: Option<ValueId>,
    scratch: &mut Option<SourceOutputAllocationScratchV1>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<ProductionSourceOutputGlobalAccessV1, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    use ProductionSourceOutputGlobalAccessV1 as Placement;
    use fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::Result as ResultCoordinate;
    let SourceOutputMemoryOperandsV1::Retained {
        operation: output_coordinate,
        pointer: pointer_use,
        value: value_use,
        executable,
    } = source_output_memory_operands_v1(control, original, value, budget)?
    else {
        return Ok(Placement::OmittedUnreachable {
            original: original.coordinate,
        });
    };
    budget.charge_work(1).map_err(Error::Resource)?;
    if scratch.is_none() {
        *scratch = Some(source_output_allocation_scratch_v1(
            control.input(),
            budget,
        )?);
    }
    let Some(argument) = source_output_allocation_parameter_v1(
        control.input(),
        function.coordinate,
        pointer,
        scratch
            .as_mut()
            .ok_or(Error::Invalid("global allocation scratch absent"))?,
        budget,
    )?
    else {
        return Ok(Placement::Unsupported(
            ProductionSourceOutputGlobalUnsupportedV1::AllocationAncestry,
        ));
    };
    let (source_argument, parameter) = source_output_global_parameter_v1(
        source,
        transition,
        owner,
        semantic_function,
        function,
        output_coordinate.block.function,
        argument,
        budget,
    )?;
    budget.charge_work(1).map_err(Error::Resource)?;
    let result = if value.is_none() {
        let original_result = ResultCoordinate {
            operation: original.coordinate,
            result: 0,
        };
        let output_result = ResultCoordinate {
            operation: output_coordinate,
            result: 0,
        };
        source_output_global_descendant_v1(
            transition,
            original.results.start,
            original_result,
            output_result,
            budget,
        )?;
        let _ = source_output_definition_v1(control.output().owner(), output_result, budget)?;
        Some(output_result)
    } else {
        None
    };
    Ok(Placement::Retained {
        original: original.coordinate,
        operation: output_coordinate,
        source_argument,
        parameter,
        pointer: pointer_use,
        value: value_use,
        result,
        executable,
    })
}
