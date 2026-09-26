// Original issuer recipes classify a source access, not an arbitrary physical
// pointer. The actual descriptor tail and guarded access are checked separately.
#[cfg(test)]
#[path = "production_source_issued_pointer_v29_tests.rs"]
mod source_issued_pointer_tests_v29;
#[cfg(test)]
#[path = "production_source_issued_pointer_source_v29_tests.rs"]
mod source_issued_pointer_source_tests_v29;
#[derive(Clone, Copy)]
enum SourceIssuedDefinitionV29 {
    Statement(usize),
    Edge(usize),
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum SourceIssuedFormV29 {
    Option,
    Pointer,
}

#[derive(Clone, Copy)]
struct SourceIssuedRecipeV29 {
    issuer: SsaValueV1,
    block: SemanticBlockIdV1,
    option_type: SemanticTypeIdV1,
    pointer_type: SemanticTypeIdV1,
    availability: SemanticOptionAvailabilityV1,
    element: ScalarType,
    access: AccessMode,
    present: ValueId,
    pointer: ValueId,
    form: SourceIssuedFormV29,
}

#[derive(Clone, Copy)]
enum SourceIssuedMemoV29 {
    Visiting,
    Complete(Option<SourceIssuedRecipeV29>),
}

#[derive(Clone, Copy)]
struct SourceIssuedLinkV29 {
    value: SsaValueV1,
    dependency: SsaValueV1,
    occurrence: usize,
}

struct SourceIssuedOriginalV29<'scope, 'owner, 'source> {
    instances: &'scope ExecutionInstancesV29<'owner>,
    instance: ProductionCallInstanceIdV1,
    source_index: &'scope SourceAddressSourceIndexV29<'source>,
    inventory: SourceDescriptorInventoryV29,
    definitions: BTreeMap<SsaValueV1, SourceIssuedDefinitionV29>,
    memo: BTreeMap<SsaValueV1, SourceIssuedMemoV29>,
    links: Vec<SourceIssuedLinkV29>,
    dominance: fe2o3_mir_model::SemanticOptionDominanceV1,
}

struct SourceIssuedMeterV29<'a, 'work>(&'a mut ArgumentBudgetV1<'work>);
impl fe2o3_mir_model::SemanticAssertionMeterV1 for SourceIssuedMeterV29<'_, '_> {
    type Error = ArgumentResourceV1;
    fn charge_work(&mut self, amount: usize) -> Result<(), Self::Error> {
        self.0.charge_work(amount)
    }
    fn reserve_storage(&mut self, amount: usize) -> Result<(), Self::Error> {
        self.0.reserve_storage(amount)
    }
}

fn source_issued_error_v29() -> ProductionSemanticKirErrorV1 {
    source_reference_error_v29("source issued pointer differs from its original issuer or actual guard")
}

fn source_issued_analysis_error_v29(
    error: fe2o3_mir_model::SemanticOptionDominanceMeteredErrorV18<ArgumentResourceV1>,
) -> ProductionSemanticKirErrorV1 {
    match error {
        fe2o3_mir_model::SemanticOptionDominanceMeteredErrorV18::Meter(error) => error.into(),
        fe2o3_mir_model::SemanticOptionDominanceMeteredErrorV18::Analysis(_) => source_issued_error_v29(),
    }
}

impl<'scope, 'owner, 'source> SourceIssuedOriginalV29<'scope, 'owner, 'source> {
    fn new(
        instances: &'scope ExecutionInstancesV29<'owner>,
        instance: ProductionCallInstanceIdV1,
        source_index: &'scope SourceAddressSourceIndexV29<'source>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        budget.reserve_storage(argument_sum_v1(&[
            std::mem::size_of::<Self>(),
            std::mem::size_of::<Result<Self, ProductionSemanticKirErrorV1>>(),
            std::mem::size_of::<SourceIssuedMeterV29<'_, '_>>(),
        ])?)?;
        let original = instances.instance(instance).ok_or_else(source_issued_error_v29)?.declaration();
        let occurrences = instances.occurrences(instance).ok_or_else(source_issued_error_v29)?;
        let inventory = SourceDescriptorInventoryV29::build(&occurrences, budget)?;
        let mut definitions = BTreeMap::new();
        for (&value, &index) in &inventory.definitions {
            budget.charge_work(1)?;
            reserve_execution_cfg_map_entry_v29::<SsaValueV1, SourceIssuedDefinitionV29>(definitions.len(), budget)?;
            definitions.insert(value, SourceIssuedDefinitionV29::Statement(index));
        }
        for (index, edge) in occurrences.edge_definitions().iter().enumerate() {
            budget.charge_work(3)?;
            if !edge.is_reachable() || !edge.is_promoted() { continue; }
            let value = edge.value().ok_or_else(source_issued_error_v29)?;
            reserve_execution_cfg_map_entry_v29::<SsaValueV1, SourceIssuedDefinitionV29>(definitions.len(), budget)?;
            if definitions.insert(value, SourceIssuedDefinitionV29::Edge(index)).is_some() {
                return Err(source_issued_error_v29());
            }
        }
        let (producers, producer_storage) = fe2o3_mir_model::semantic_option_producers_with_meter_v18(
            original, instances.owner().source_semantic().callables(), &mut SourceIssuedMeterV29(budget),
        ).map_err(source_issued_analysis_error_v29)?;
        let (dominance, _) = fe2o3_mir_model::SemanticOptionDominanceV1::analyze_with_meter_v18(
            original, &producers, &mut SourceIssuedMeterV29(budget),
        ).map_err(source_issued_analysis_error_v29)?;
        drop(producers);
        budget.release_storage(producer_storage)?;
        let links = emission_vec_v1(definitions.len(), budget)?;
        Ok(Self { instances, instance, source_index, inventory, definitions, memo: BTreeMap::new(), links, dominance })
    }

    fn archived<'a>(&'a self, value: SsaValueV1, budget: &mut ArgumentBudgetV1<'_>)
        -> Result<&'a SemanticValueBindingV1, ProductionSemanticKirErrorV1>
    {
        self.source_index.sidecar(self.instance, budget)?.execution_observation.as_ref()
            .ok_or_else(source_issued_error_v29)?
            .lookup_original_v29(self.instances, self.instance, value, budget)
    }

    fn use_value(&self, site: ExecutionSiteV29, role: ExecutionOperandV29, place: &SemanticPlaceV1,
        budget: &mut ArgumentBudgetV1<'_>) -> Result<SsaValueV1, ProductionSemanticKirErrorV1>
    {
        let occurrences = self.instances.occurrences(self.instance).ok_or_else(source_issued_error_v29)?;
        let index = self.inventory.use_at(&occurrences, site, role, budget)?.ok_or_else(source_issued_error_v29)?;
        budget.charge_work(2)?;
        if occurrences.events()[index].event().variable().get() != place.local().index() {
            return Err(source_issued_error_v29());
        }
        source_descriptor_use_v29(&occurrences, index, site, role, budget)
    }

    fn check_archive(&self, value: SsaValueV1, recipe: SourceIssuedRecipeV29,
        budget: &mut ArgumentBudgetV1<'_>) -> Result<(), ProductionSemanticKirErrorV1>
    {
        budget.charge_work(8)?;
        let expected = |ty: &Type| matches!(ty, Type::Pointer(pointer)
            if *pointer.pointee == Type::Scalar(recipe.element)
                && pointer.address_space == AddressSpace::Global && pointer.access == recipe.access);
        match (recipe.form, self.archived(value, budget)?) {
            (SourceIssuedFormV29::Option, SemanticValueBindingV1::OptionPointer {
                present, pointer, pointer_ty, availability,
            }) if *present == recipe.present && *pointer == recipe.pointer
                && *availability == recipe.availability && expected(pointer_ty) => Ok(()),
            (SourceIssuedFormV29::Pointer, SemanticValueBindingV1::Value { id, ty })
                if *id == recipe.pointer && expected(ty) => Ok(()),
            _ => Err(source_issued_error_v29()),
        }
    }

    fn issuer(&self, ordinal: usize, value: SsaValueV1, budget: &mut ArgumentBudgetV1<'_>)
        -> Result<Option<SourceIssuedRecipeV29>, ProductionSemanticKirErrorV1>
    {
        budget.charge_work(15)?;
        let source = self.instances.owner().source_semantic();
        let function = self.instances.instance(self.instance).ok_or_else(source_issued_error_v29)?.declaration();
        let occurrences = self.instances.occurrences(self.instance).ok_or_else(source_issued_error_v29)?;
        let edge = occurrences.edge_definitions().get(ordinal).ok_or_else(source_issued_error_v29)?;
        if !edge.is_reachable() || !edge.is_promoted() || edge.value() != Some(value)
            || edge.edge().ordinal() != 0 || edge.ordinal() != 0 {
            return Err(source_issued_error_v29());
        }
        let block = SemanticBlockIdV1::from_index(edge.edge().source().get());
        let Some(SemanticTerminatorKindV1::Call(call)) = function.blocks().get(block.index() as usize)
            .map(|block| block.terminator().kind()) else { return Err(source_issued_error_v29()); };
        let Some(SemanticCallableDeclV1::CompilerIntrinsic { operation, .. }) = source.callables()
            .get(call.callee().index() as usize) else { return Ok(None); };
        if !matches!(operation, SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut { .. }
            | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetDisjointMut { .. }) { return Ok(None); }
        let destination = call.destination().ok_or_else(source_issued_error_v29)?.place();
        if !destination.projections().is_empty() || destination.local().index() != edge.variable().get() {
            return Err(source_issued_error_v29());
        }
        let option_type = destination.ty();
        let pointer_type = option_payload_type_v1(source.types(), option_type).ok_or_else(source_issued_error_v29)?;
        let availability = self.dominance.availability(destination.local()).ok_or_else(source_issued_error_v29)?;
        budget.charge_work(argument_sum_v1(&[argument_product_v1(source.callables().len(), 4)?, 32])?)?;
        let Some(SemanticPromotedBindingV1::OptionPointer { element, address_space: AddressSpace::Global, access, .. }) =
            optional_pointer_binding_v1(source.types(), source.callables(), operation, pointer_type, availability)
        else { return Err(source_issued_error_v29()); };
        let SemanticValueBindingV1::OptionPointer { present, pointer, .. } = self.archived(value, budget)?
            else { return Err(source_issued_error_v29()); };
        let recipe = SourceIssuedRecipeV29 { issuer: value, block, option_type, pointer_type, availability,
            element, access, present: *present, pointer: *pointer, form: SourceIssuedFormV29::Option };
        self.check_archive(value, recipe, budget)?;
        Ok(Some(recipe))
    }

    // Each original definition enters this iterative memo once. A cyclic phi or
    // an unsupported defining operation never becomes an invented issuer.
    fn complete(&mut self, value: SsaValueV1, recipe: Option<SourceIssuedRecipeV29>,
        budget: &mut ArgumentBudgetV1<'_>) -> Result<(), ProductionSemanticKirErrorV1>
    {
        charge_execution_cfg_lookup_v29(self.memo.len(), budget)?;
        let Some(row @ SourceIssuedMemoV29::Visiting) = self.memo.get_mut(&value)
            else { return Err(ArgumentResourceV1::Accounting.into()); };
        *row = SourceIssuedMemoV29::Complete(recipe);
        Ok(())
    }

    fn resolve(&mut self, requested: SsaValueV1, budget: &mut ArgumentBudgetV1<'_>)
        -> Result<Option<SourceIssuedRecipeV29>, ProductionSemanticKirErrorV1>
    {
        if !self.links.is_empty() { return Err(ArgumentResourceV1::Accounting.into()); }
        let mut value = requested;
        let mut resolved;
        loop {
            charge_execution_cfg_lookup_v29(self.memo.len(), budget)?;
            match self.memo.get(&value).copied() {
                Some(SourceIssuedMemoV29::Complete(result)) => { resolved = result; break; }
                Some(SourceIssuedMemoV29::Visiting) => return Err(source_issued_error_v29()),
                None => {}
            }
            reserve_execution_cfg_map_entry_v29::<SsaValueV1, SourceIssuedMemoV29>(self.memo.len(), budget)?;
            self.memo.insert(value, SourceIssuedMemoV29::Visiting);
            charge_execution_cfg_lookup_v29(self.definitions.len(), budget)?;
            let definition = self.definitions.get(&value).copied();
            let statement = match definition {
                Some(SourceIssuedDefinitionV29::Edge(index)) => {
                    resolved = self.issuer(index, value, budget)?;
                    self.complete(value, resolved, budget)?;
                    break;
                }
                Some(SourceIssuedDefinitionV29::Statement(index)) => index,
                None => {
                    resolved = None;
                    self.complete(value, None, budget)?;
                    break;
                }
            };
            let function = self.instances.instance(self.instance).ok_or_else(source_issued_error_v29)?.declaration();
            let occurrences = self.instances.occurrences(self.instance).ok_or_else(source_issued_error_v29)?;
            let (site, rvalue) = source_descriptor_assignment_v29(function, &occurrences, statement, value, budget)?;
            let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) = rvalue
                else {
                    resolved = None;
                    self.complete(value, None, budget)?;
                    break;
                };
            let dependency = self.use_value(site, ExecutionOperandV29::RvalueOperand(0), place, budget)?;
            budget.charge_work(2)?;
            if self.links.len() == self.links.capacity() { return Err(ArgumentResourceV1::Accounting.into()); }
            self.links.push(SourceIssuedLinkV29 { value, dependency, occurrence: statement });
            value = dependency;
        }
        while let Some(link) = self.links.pop() {
            budget.charge_work(3)?;
            if let Some(mut recipe) = resolved {
                self.check_archive(link.dependency, recipe, budget)?;
                let source = self.instances.owner().source_semantic();
                let function = self.instances.instance(self.instance).ok_or_else(source_issued_error_v29)?.declaration();
                let occurrences = self.instances.occurrences(self.instance).ok_or_else(source_issued_error_v29)?;
                let (site, rvalue) = source_descriptor_assignment_v29(function, &occurrences, link.occurrence, link.value, budget)?;
                let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) = rvalue
                    else { return Err(source_issued_error_v29()); };
                let local_type = function.locals().get(place.local().index() as usize)
                    .ok_or_else(source_issued_error_v29)?.ty();
                if place.projections().is_empty() {
                    let expected = match recipe.form {
                        SourceIssuedFormV29::Option => recipe.option_type,
                        SourceIssuedFormV29::Pointer => recipe.pointer_type,
                    };
                    if local_type != expected || place.ty() != expected { return Err(source_issued_error_v29()); }
                } else if recipe.form == SourceIssuedFormV29::Option
                    && local_type == recipe.option_type
                    && exact_option_payload_projection_v1(source.types(), local_type, place.projections(), place.ty())
                        == Some(recipe.pointer_type)
                    && self.dominance.allows(recipe.availability,
                        SemanticBlockIdV1::from_index(scoped_memory_site_key_v29(site).0)) {
                    recipe.form = SourceIssuedFormV29::Pointer;
                } else { return Err(source_issued_error_v29()); }
                self.check_archive(link.value, recipe, budget)?;
                resolved = Some(recipe);
            }
            self.complete(link.value, resolved, budget)?;
        }
        Ok(resolved)
    }
}
