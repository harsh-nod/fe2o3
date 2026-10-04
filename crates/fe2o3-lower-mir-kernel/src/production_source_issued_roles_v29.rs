// These rows are private retained source recipes. Construction is completed by
// the original issuer/guard checker; immutable and optimized replay remain
// mandatory before any occurrence receives a descriptor source role.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingSourceIssuedSiteV29 {
    instance: ProductionCallInstanceIdV1,
    block: SemanticBlockIdV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingSourceIssuedIssuerV29 {
    instance: ProductionCallInstanceIdV1,
    block: SemanticBlockIdV1,
    definition: SsaValueV1,
    root_parameter: usize,
    root_input: ValueId,
    receiver: ValueId,
    index: ValueId,
    length: ValueId,
    present: ValueId,
    data: ValueId,
    pointer: ValueId,
    element: ScalarType,
    access: AccessMode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingSourceIssuedAccessV29 {
    instance: ProductionCallInstanceIdV1,
    anchor: usize,
    issuer_instance: ProductionCallInstanceIdV1,
    issuer: SsaValueV1,
    pointer: ValueId,
    access: MemoryAccess,
    writing: bool,
    guard_block: BlockId,
    guard_edge: usize,
}

struct PendingSourceIssuedRolesV29 {
    sources: Vec<PendingSourceIssuedSiteV29>,
    issuers: Vec<PendingSourceIssuedIssuerV29>,
    lengths: Vec<PendingSourceLengthV76>,
    accesses: Vec<PendingSourceIssuedAccessV29>,
    selected: Vec<PendingSourceSelectedAccessV30>,
}

impl PendingSourceIssuedRolesV29 {
    fn empty() -> Self {
        Self {
            sources: Vec::new(),
            issuers: Vec::new(),
            lengths: Vec::new(),
            accesses: Vec::new(),
            selected: Vec::new(),
        }
    }

    fn retained_storage(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ArgumentResourceV1> {
        let mut storage = argument_sum_v1(&[
            argument_product_v1(
                self.sources.capacity(),
                std::mem::size_of::<PendingSourceIssuedSiteV29>(),
            )?,
            argument_product_v1(
                self.issuers.capacity(),
                std::mem::size_of::<PendingSourceIssuedIssuerV29>(),
            )?,
            argument_product_v1(self.lengths.capacity(), size_of::<PendingSourceLengthV76>())?,
            argument_product_v1(
                self.accesses.capacity(),
                std::mem::size_of::<PendingSourceIssuedAccessV29>(),
            )?,
            argument_product_v1(
                self.selected.capacity(),
                std::mem::size_of::<PendingSourceSelectedAccessV30>(),
            )?,
        ])?;
        for row in &self.selected {
            budget.charge_work(6)?;
            storage = argument_sum_v1(&[storage, row.retained_storage()?])?;
        }
        Ok(storage)
    }
}

fn pending_issued_roles_match_v29(
    left: &PendingSourceIssuedRolesV29,
    right: &PendingSourceIssuedRolesV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    budget.charge_work(5)?;
    if left.sources.len() != right.sources.len()
        || left.issuers.len() != right.issuers.len()
        || left.lengths.len() != right.lengths.len()
        || left.accesses.len() != right.accesses.len()
        || left.selected.len() != right.selected.len()
    {
        return Ok(false);
    }
    budget.charge_work(argument_sum_v1(&[
        argument_product_v1(
            left.sources.len(),
            std::mem::size_of::<PendingSourceIssuedSiteV29>(),
        )?,
        argument_product_v1(
            left.issuers.len(),
            std::mem::size_of::<PendingSourceIssuedIssuerV29>(),
        )?,
        argument_product_v1(left.lengths.len(), size_of::<PendingSourceLengthV76>())?,
        argument_product_v1(
            left.accesses.len(),
            std::mem::size_of::<PendingSourceIssuedAccessV29>(),
        )?,
    ])?)?;
    for (a, b) in left.selected.iter().zip(&right.selected) {
        if !a.matches(b, budget)? {
            return Ok(false);
        }
    }
    Ok(left.sources == right.sources
        && left.issuers == right.issuers
        && left.lengths == right.lengths
        && left.accesses == right.accesses)
}

fn source_issued_census_query_headers_v29() -> Result<usize, ArgumentResourceV1> {
    fn h<T>() -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            std::mem::size_of::<T>(),
            argument_product_v1(
                2,
                std::mem::size_of::<Result<T, ProductionSemanticKirErrorV1>>(),
            )?,
        ])
    }
    argument_sum_v1(&[
        h::<usize>()?,
        h::<Option<usize>>()?,
        h::<bool>()?,
        h::<()>()?,
        h::<ProductionCallInstanceIdV1>()?,
        h::<Option<ProductionCallInstanceIdV1>>()?,
        h::<SemanticBlockIdV1>()?,
        h::<SsaValueV1>()?,
        h::<Option<SsaValueV1>>()?,
        h::<&production_call_instances_v1::ProductionCallInstanceV1<'_>>()?,
        h::<Option<&production_call_instances_v1::ProductionCallInstanceV1<'_>>>()?,
        h::<&SemanticFunctionDeclV1>()?,
        h::<Option<&fe2o3_mir_model::semantic_mir_v1::SemanticBasicBlockV1>>()?,
        h::<Option<&SemanticTerminatorKindV1>>()?,
        h::<Option<&SemanticCallableDeclV1>>()?,
        h::<&SemanticDirectCallV1>()?,
        h::<Option<&SemanticDirectCallV1>>()?,
        h::<Option<&SemanticOperandV1>>()?,
        h::<Option<&fe2o3_mir_model::semantic_mir_v1::SemanticCallDestinationV1>>()?,
        h::<&SemanticPlaceV1>()?,
        h::<Option<&SemanticPlaceV1>>()?,
        h::<&SemanticValueBindingV1>()?,
        h::<&SemanticSourceReferenceBindingV29>()?,
        h::<(&SemanticPlaceV1, &SemanticValueBindingV1)>()?,
        h::<&SourceReferenceLoanV29>()?,
        h::<Option<&SourceReferenceLoanV29>>()?,
        h::<ProductionSemanticSsaFunctionOccurrencesV1<'_>>()?,
        h::<Option<ProductionSemanticSsaFunctionOccurrencesV1<'_>>>()?,
        h::<&mut SourceIssuedOriginalV29<'_, '_, '_>>()?,
        h::<Option<&mut SourceIssuedOriginalV29<'_, '_, '_>>>()?,
        h::<(usize, SsaValueV1)>()?,
        h::<std::slice::Iter<'_, fe2o3_pliron::ProductionSemanticSsaEdgeDefinitionOccurrenceV1>>()?,
        h::<std::ops::Range<usize>>()?,
    ])
}

fn source_issued_call_v29<'a>(
    source: &'a fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1,
    function: &'a SemanticFunctionDeclV1,
    block: SemanticBlockIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Option<&'a SemanticDirectCallV1>, ProductionSemanticKirErrorV1> {
    budget.charge_work(4)?;
    let Some(SemanticTerminatorKindV1::Call(call)) = function
        .blocks()
        .get(block.index() as usize)
        .map(|block| block.terminator().kind())
    else {
        return Ok(None);
    };
    let Some(SemanticCallableDeclV1::CompilerIntrinsic { operation, .. }) =
        source.callables().get(call.callee().index() as usize)
    else {
        return Ok(None);
    };
    Ok(matches!(
        operation,
        SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut { .. }
            | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetDisjointMut { .. }
            | SemanticCompilerIntrinsicOperationV1::DisjointSliceLen { .. }
            | SemanticCompilerIntrinsicOperationV1::WriteOnlyDisjointSliceLen { .. }
    )
    .then_some(call))
}

fn source_issued_source_count_v29(
    instances: &ExecutionInstancesV29<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<usize, ProductionSemanticKirErrorV1> {
    let mut count = 0usize;
    for ordinal in 0..instances.instances().len() {
        budget.charge_work(2)?;
        let instance = instances
            .id_at(ordinal)
            .ok_or_else(source_issued_error_v29)?;
        if instances.instance_reachable(instance) != Some(true) {
            continue;
        }
        let function = instances
            .instance(instance)
            .ok_or_else(source_issued_error_v29)?
            .declaration();
        for block in 0..function.blocks().len() {
            budget.charge_work(2)?;
            let block = SemanticBlockIdV1::from_index(
                u32::try_from(block).map_err(|_| ArgumentResourceV1::Arithmetic)?,
            );
            if instances.block_reachable(instance, block) == Some(true)
                && source_issued_call_v29(
                    instances.owner().source_semantic(),
                    function,
                    block,
                    budget,
                )?
                .is_some()
            {
                count = count.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
            }
        }
    }
    Ok(count)
}

impl SourceIssuedOriginalV29<'_, '_, '_> {
    // This optional domain check does not authenticate an actual operation.
    // Unhandled source receiver families stay in the complete pending roster;
    // once selected, actual_issuer retains its existing strict equations.
    fn retained_receiver_candidate(
        &self,
        recipe: SourceIssuedRecipeV29,
        references: &SourceReferenceEmissionV29<'_, '_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        references.check(budget)?;
        budget.charge_work(6)?;
        let source = self.instances.owner().source_semantic();
        let function = self
            .instances
            .instance(self.instance)
            .ok_or_else(source_issued_error_v29)?
            .declaration();
        let Some(call) = source_issued_call_v29(source, function, recipe.block, budget)? else {
            return Err(source_issued_error_v29());
        };
        if !matches!(call.arguments().first(), Some(SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) if place.projections().is_empty())
        {
            return Ok(false);
        }
        let (_, binding) = self.direct_call_operand(call, recipe.block, 0, budget)?;
        let SemanticValueBindingV1::SourceReference(binding) = binding else {
            return Ok(false);
        };
        source_reference_validate_binding_v29(references.plan, binding, budget)?;
        let SourceReferenceBindingOriginV29::SingleLoan(loan) = binding.origin else {
            return Ok(false);
        };
        let loan = references
            .plan
            .loans
            .get(loan)
            .ok_or_else(source_issued_error_v29)?;
        Ok(matches!(
            loan.representation,
            SourceReferenceRepresentationV29::ExistingAllocationBinding(_)
        ))
    }
}

impl SourceIssuedAccessesV29<'_, '_, '_> {
    fn census(
        &mut self,
        references: &SourceReferenceEmissionV29<'_, '_>,
        expected: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.check(budget)?;
        references.plan.check_owner(self.instances, budget)?;
        let before = budget.storage();
        budget.reserve_storage(argument_sum_v1(&[
            source_length_call_headers_v76()?,
            std::mem::size_of::<SourceIssuedOriginalV29<'_, '_, '_>>(),
            std::mem::size_of::<Result<SourceIssuedOriginalV29<'_, '_, '_>, ProductionSemanticKirErrorV1>>(),
            std::mem::size_of::<SourceIssuedRecipeV29>(),
            std::mem::size_of::<Option<SourceIssuedRecipeV29>>(),
            std::mem::size_of::<Result<Option<SourceIssuedRecipeV29>, ProductionSemanticKirErrorV1>>(),
            std::mem::size_of::<SourceIssuedRootTransportV29>(),
            std::mem::size_of::<PendingSourceIssuedIssuerV29>(),
            std::mem::size_of::<(SourceIssuedRootTransportV29, PendingSourceIssuedIssuerV29)>(),
            std::mem::size_of::<Result<(SourceIssuedRootTransportV29, PendingSourceIssuedIssuerV29), ProductionSemanticKirErrorV1>>(),
            std::mem::size_of::<PendingSourceIssuedSiteV29>(),
            std::mem::size_of::<Result<(), ProductionSemanticKirErrorV1>>(),
        ])?)?;
        if !self.retained.sources.is_empty() {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        self.retained.sources = emission_vec_v1(expected, budget)?;
        for ordinal in 0..self.instances.instances().len() {
            budget.charge_work(2)?;
            let instance = self
                .instances
                .id_at(ordinal)
                .ok_or_else(source_issued_error_v29)?;
            if self.instances.instance_reachable(instance) != Some(true) {
                continue;
            }
            let function = self
                .instances
                .instance(instance)
                .ok_or_else(source_issued_error_v29)?
                .declaration();
            for block in 0..function.blocks().len() {
                budget.charge_work(2)?;
                let block = SemanticBlockIdV1::from_index(
                    u32::try_from(block).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                );
                if self.instances.block_reachable(instance, block) != Some(true) {
                    continue;
                }
                let Some(call) = source_issued_call_v29(
                    self.instances.owner().source_semantic(),
                    function,
                    block,
                    budget,
                )?
                else {
                    continue;
                };
                if self.retained.sources.len() >= expected {
                    return Err(source_issued_error_v29());
                }
                self.retained
                    .sources
                    .push(PendingSourceIssuedSiteV29 { instance, block });
                let Some(destination) = call.destination().map(|destination| destination.place())
                else {
                    continue;
                };
                if !destination.projections().is_empty() {
                    continue;
                }
                charge_execution_cfg_lookup_v29(self.originals.len(), budget)?;
                if !self.originals.contains_key(&instance.index()) {
                    let original = SourceIssuedOriginalV29::new(
                        self.instances,
                        instance,
                        self.source_index,
                        budget,
                    )?;
                    reserve_execution_cfg_map_entry_v29::<
                        usize,
                        SourceIssuedOriginalV29<'_, '_, '_>,
                    >(self.originals.len(), budget)?;
                    self.originals.insert(instance.index(), original);
                }
                charge_execution_cfg_lookup_v29(self.originals.len(), budget)?;
                let original = self
                    .originals
                    .get_mut(&instance.index())
                    .ok_or_else(source_issued_error_v29)?;
                let occurrences = self
                    .instances
                    .occurrences(instance)
                    .ok_or_else(source_issued_error_v29)?;
                let mut definition = None;
                for edge in occurrences.edge_definitions() {
                    budget.charge_work(5)?;
                    if edge.is_reachable()
                        && edge.is_promoted()
                        && edge.edge().source().get() == block.index()
                        && edge.edge().ordinal() == 0
                        && edge.ordinal() == 0
                        && edge.variable().get() == destination.local().index()
                    {
                        if definition
                            .replace(edge.value().ok_or_else(source_issued_error_v29)?)
                            .is_some()
                        {
                            return Err(source_issued_error_v29());
                        }
                    }
                }
                if source_length_call_v76(self.instances.owner().source_semantic(), call).is_some()
                {
                    let (transport, retained) = original.actual_length_v76(
                        block,
                        definition,
                        references,
                        &self.actual,
                        budget,
                    )?;
                    emission_push_v1(&mut self.transports, transport, budget)?;
                    emission_push_v1(&mut self.retained.lengths, retained, budget)?;
                    continue;
                }
                let Some(definition) = definition else {
                    continue;
                };
                let recipe = original
                    .resolve(definition, budget)?
                    .ok_or_else(source_issued_error_v29)?;
                if recipe.issuer != definition
                    || recipe.block != block
                    || recipe.form != SourceIssuedFormV29::Option
                {
                    return Err(source_issued_error_v29());
                }
                let key = (instance.index(), definition);
                charge_execution_cfg_lookup_v29(self.issuers.len(), budget)?;
                if self.issuers.contains(&key) {
                    continue;
                }
                if !original.retained_receiver_candidate(recipe, references, budget)? {
                    continue;
                }
                let (transport, retained) =
                    original.actual_issuer(recipe, references, &self.actual, budget)?;
                emission_push_v1(&mut self.transports, transport, budget)?;
                emission_push_v1(&mut self.retained.issuers, retained, budget)?;
                reserve_execution_cfg_map_entry_v29::<(usize, SsaValueV1), ()>(
                    self.issuers.len(),
                    budget,
                )?;
                self.issuers.insert(key);
            }
        }
        if self.retained.sources.len() != expected {
            return Err(source_issued_error_v29());
        }
        self.owned = argument_sum_v1(&[
            self.owned,
            budget
                .storage()
                .checked_sub(before)
                .ok_or(ArgumentResourceV1::Accounting)?,
        ])?;
        Ok(())
    }
}

include!("production_source_length_calls_v76.rs");
