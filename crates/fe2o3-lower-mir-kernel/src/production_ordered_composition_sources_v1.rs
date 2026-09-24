/// Actual source marker definition corresponding to one canonical operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrderedCompositionSourceDefinitionV1 {
    key: fe2o3_kernel_ir::OrderedProgramDefinitionKeyV1,
    function: SemanticFunctionIdV1,
    block: SemanticBlockIdV1,
}
impl OrderedCompositionSourceDefinitionV1 {
    /// Owner-local canonical definition key.
    pub const fn key(self) -> fe2o3_kernel_ir::OrderedProgramDefinitionKeyV1 {
        self.key
    }
    /// Actual retained semantic function, never an ordinal guessed from Rust locals.
    pub const fn semantic_function(self) -> SemanticFunctionIdV1 {
        self.function
    }
    /// Actual semantic block joined through retained lowering correspondence.
    pub const fn semantic_block(self) -> SemanticBlockIdV1 {
        self.block
    }
}
/// One root-qualified execution occurrence of a retained marker definition.
/// Repeated helper calls have distinct instance coordinates but one definition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrderedCompositionSourceOccurrenceV1 {
    key: fe2o3_kernel_ir::OrderedProgramOccurrenceKeyV1,
    definition: fe2o3_kernel_ir::OrderedProgramDefinitionKeyV1,
    occurrence: production_call_instance_ids_v1::ProductionCallOccurrenceV1,
}
impl OrderedCompositionSourceOccurrenceV1 {
    /// Owner-local canonical execution-occurrence locator.
    pub const fn key(self) -> fe2o3_kernel_ir::OrderedProgramOccurrenceKeyV1 {
        self.key
    }
    /// Shared canonical marker definition, not an expanded executable copy.
    pub const fn definition(self) -> fe2o3_kernel_ir::OrderedProgramDefinitionKeyV1 {
        self.definition
    }
    /// Existing source call-instance coordinate; it is not transferable authority.
    pub const fn call_instance_ordinal(self) -> usize {
        self.occurrence.caller.index()
    }
    /// Actual marker block in that source instance.
    pub const fn semantic_block(self) -> SemanticBlockIdV1 {
        self.occurrence.block
    }
}
#[derive(Debug, Eq, PartialEq)]
struct OrderedCompositionSourceRowsV1 {
    definitions: [Option<OrderedCompositionSourceDefinitionV1>; 8],
    occurrences: [Option<OrderedCompositionSourceOccurrenceV1>; 8],
}
impl From<production_call_instances_v1::ProductionCallInstanceErrorV1>
    for ProductionOrderedCompositionErrorV1
{
    fn from(error: production_call_instances_v1::ProductionCallInstanceErrorV1) -> Self {
        match error {
            production_call_instances_v1::ProductionCallInstanceErrorV1::Resource(e) => e.into(),
            _ => ProductionSemanticKirErrorV1::CorrespondenceMismatch.into(),
        }
    }
}
fn bind_ordered_composition_sources_v1(
    semantic_ssa: &ProductionSemanticSsaOwnerV1,
    composition: &fe2o3_kernel_ir::VerifiedOrderedProgramCompositionV1,
    correspondence: &SemanticKirCorrespondenceV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<OrderedCompositionSourceRowsV1, ProductionOrderedCompositionErrorV1> {
    let mismatch = || ProductionSemanticKirErrorV1::CorrespondenceMismatch;
    let semantic = semantic_ssa.source_semantic();
    let module = composition.canonical().module();
    let root = *semantic.roots().first().ok_or_else(mismatch)?;
    budget.charge_work(16)?;
    let mut rows = OrderedCompositionSourceRowsV1 {
        definitions: [None; 8],
        occurrences: [None; 8],
    };
    for definition in composition.definitions() {
        budget.charge_work(
            correspondence.lowered_functions.len()
                + correspondence.terminator_operation_spans.len(),
        )?;
        let site = definition.site();
        let function = module
            .functions
            .get(site.function_ordinal() as usize)
            .ok_or_else(mismatch)?;
        let source = correspondence
            .lowered_functions
            .iter()
            .find(|r| r.kernel_ir_function == function.id)
            .filter(|r| r.correspondence_owner == root)
            .ok_or_else(mismatch)?;
        let mut spans = correspondence
            .terminator_operation_spans
            .iter()
            .filter(|r| {
                r.correspondence_owner == root
                    && r.semantic_function == source.semantic_function
                    && r.kernel_ir_block == site.block()
                    && r.first_operation_ordinal <= site.operation_ordinal()
                    && r.first_operation_ordinal
                        .checked_add(r.operation_count)
                        .is_some_and(|end| site.operation_ordinal() < end)
            });
        let span = spans.next().ok_or_else(mismatch)?;
        if spans.next().is_some() {
            return Err(mismatch().into());
        }
        let source_function = semantic
            .functions()
            .get(source.semantic_function.index() as usize)
            .ok_or_else(mismatch)?;
        let source_block = source_function
            .blocks()
            .get(span.semantic_block.index() as usize)
            .ok_or_else(mismatch)?;
        let SemanticTerminatorKindV1::Call(call) = source_block.terminator().kind() else {
            return Err(mismatch().into());
        };
        if ordered_composition_call_kind_v1(semantic, call) != 1 {
            return Err(mismatch().into());
        }
        let source_identity = call.ordered_program_source_v32().ok_or_else(mismatch)?;
        let operation = composition
            .definition_operation(composition.canonical().identity(), definition.key(), budget)
            .map_err(ordered_composition_structural_error_v1)?;
        let OperationKind::Gfx942OrderedProgram(program) = &operation.kind else {
            return Err(mismatch().into());
        };
        if program.source()
            != fe2o3_kernel_ir::AssemblySourceIdentity::new(
                source_identity.frontend_unit(),
                *source_identity.function().as_bytes(),
                source_identity.contract(),
                source_identity.statement(),
            )
        {
            return Err(mismatch().into());
        }
        let row = OrderedCompositionSourceDefinitionV1 {
            key: definition.key(),
            function: source.semantic_function,
            block: span.semantic_block,
        };
        if rows
            .definitions
            .iter()
            .flatten()
            .any(|prior| prior.function == row.function && prior.block == row.block)
        {
            return Err(mismatch().into());
        }
        let slot = rows
            .definitions
            .get_mut(definition.key().ordinal() as usize)
            .ok_or_else(mismatch)?;
        if slot.replace(row).is_some() {
            return Err(mismatch().into());
        }
    }
    production_call_instances_v1::with_production_call_instances_v1(
        semantic_ssa,
        root,
        budget,
        |plan, budget| {
            budget.charge_work(plan.instances().len())?;
            if plan.instances().len() != composition.calls().len() + 1 {
                return Err(mismatch().into());
            }
            for occurrence in composition.occurrences() {
                budget.charge_work(16)?;
                let definition = rows
                    .definitions
                    .get(occurrence.definition().ordinal() as usize)
                    .and_then(Option::as_ref)
                    .ok_or_else(mismatch)?;
                let instance = if let Some(incoming) = occurrence.incoming_call() {
                    let call = composition
                        .calls()
                        .get(incoming.ordinal() as usize)
                        .ok_or_else(mismatch)?;
                    let site = call.site();
                    let function = module
                        .functions
                        .get(site.function_ordinal() as usize)
                        .ok_or_else(mismatch)?;
                    budget.charge_work(
                        correspondence.lowered_functions.len()
                            + correspondence.terminator_operation_spans.len(),
                    )?;
                    let caller = correspondence
                        .lowered_functions
                        .iter()
                        .find(|r| r.kernel_ir_function == function.id)
                        .ok_or_else(mismatch)?;
                    if caller.semantic_function != root || caller.correspondence_owner != root {
                        return Err(mismatch().into());
                    }
                    let span = correspondence
                        .terminator_operation_spans
                        .iter()
                        .find(|r| {
                            r.correspondence_owner == root
                                && r.semantic_function == root
                                && r.kernel_ir_block == site.block()
                        })
                        .ok_or_else(mismatch)?;
                    let calls = plan.calls(plan.root()).ok_or_else(mismatch)?;
                    budget.charge_work(calls.len())?;
                    let actual = calls
                        .iter()
                        .find(|r| r.occurrence().block == span.semantic_block)
                        .ok_or_else(mismatch)?;
                    actual.child().ok_or_else(mismatch)?
                } else {
                    plan.root()
                };
                if plan.instance(instance).ok_or_else(mismatch)?.function() != definition.function {
                    return Err(mismatch().into());
                }
                let actual_calls = plan.calls(instance).ok_or_else(mismatch)?;
                budget.charge_work(actual_calls.len())?;
                let actual = actual_calls
                    .iter()
                    .find(|r| r.occurrence().block == definition.block)
                    .ok_or_else(mismatch)?;
                if actual.child().is_some()
                    || ordered_composition_call_kind_v1(semantic, actual.source()) != 1
                {
                    return Err(mismatch().into());
                }
                let row = OrderedCompositionSourceOccurrenceV1 {
                    key: occurrence.key(),
                    definition: definition.key,
                    occurrence: actual.occurrence(),
                };
                if rows
                    .occurrences
                    .iter()
                    .flatten()
                    .any(|prior| prior.occurrence == row.occurrence)
                {
                    return Err(mismatch().into());
                }
                let slot = rows
                    .occurrences
                    .get_mut(occurrence.key().ordinal() as usize)
                    .ok_or_else(mismatch)?;
                if slot.replace(row).is_some() {
                    return Err(mismatch().into());
                }
            }
            let mut actual_markers = 0usize;
            for index in 0..plan.instances().len() {
                let id = plan.id_at(index).ok_or_else(mismatch)?;
                let calls = plan.calls(id).ok_or_else(mismatch)?;
                budget.charge_work(calls.len())?;
                actual_markers = argument_sum_v1(&[
                    actual_markers,
                    calls
                        .iter()
                        .filter(|r| ordered_composition_call_kind_v1(semantic, r.source()) == 1)
                        .count(),
                ])?;
            }
            if actual_markers != composition.occurrences().len() {
                return Err(mismatch().into());
            }
            Ok::<(), ProductionOrderedCompositionErrorV1>(())
        },
    )?;
    Ok(rows)
}
