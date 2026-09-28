// Source identities never contain physical ValueIds. The immutable replay mode
// prevents a source-only memo from being reused as an emitted archive proof.
#[derive(Clone, Copy)]
enum SourceIssuedDefinitionV29 {
    Statement(usize),
    Edge(usize),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceIssuedFormV29 {
    Option,
    Pointer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceIssuedSemanticRecipeV29 {
    issuer: SsaValueV1,
    block: SemanticBlockIdV1,
    option_type: SemanticTypeIdV1,
    pointer_type: SemanticTypeIdV1,
    availability: SemanticOptionAvailabilityV1,
    element: ScalarType,
    access: AccessMode,
    form: SourceIssuedFormV29,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceIssuedOriginalEffectV29 {
    recipe: SourceIssuedSemanticRecipeV29,
    writing: bool,
    volatile: bool,
}

#[derive(Clone, Copy)]
enum SourceIssuedMemoV29 {
    Visiting,
    Complete(Option<SourceIssuedSemanticRecipeV29>),
}

#[derive(Clone, Copy)]
struct SourceIssuedLinkV29 {
    value: SsaValueV1,
    dependency: SsaValueV1,
    occurrence: usize,
}

enum SourceIssuedReplayModeV29<'scope, 'source> {
    SourceOnly,
    Emitted(&'scope SourceAddressSourceIndexV29<'source>),
}

struct SourceIssuedSemanticV29<'scope, 'owner, 'source> {
    instances: &'scope ExecutionInstancesV29<'owner>,
    instance: ProductionCallInstanceIdV1,
    mode: SourceIssuedReplayModeV29<'scope, 'source>,
    inventory: SourceDescriptorInventoryV29,
    definitions: BTreeMap<SsaValueV1, SourceIssuedDefinitionV29>,
    memo: BTreeMap<SsaValueV1, SourceIssuedMemoV29>,
    links: Vec<SourceIssuedLinkV29>,
    dominance: fe2o3_mir_model::SemanticOptionDominanceV1,
    slot: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    floor: usize,
    owned: usize,
    failure: std::cell::Cell<Option<ArgumentResourceV1>>,
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

fn source_issued_analysis_error_v29(
    error: fe2o3_mir_model::SemanticOptionDominanceMeteredErrorV18<ArgumentResourceV1>,
) -> ProductionSemanticKirErrorV1 {
    match error {
        fe2o3_mir_model::SemanticOptionDominanceMeteredErrorV18::Meter(error) => error.into(),
        fe2o3_mir_model::SemanticOptionDominanceMeteredErrorV18::Analysis(_) => {
            source_issued_error_v29()
        }
    }
}

impl<'scope, 'owner, 'source> SourceIssuedSemanticV29<'scope, 'owner, 'source> {
    fn new(
        instances: &'scope ExecutionInstancesV29<'owner>,
        instance: ProductionCallInstanceIdV1,
        mode: SourceIssuedReplayModeV29<'scope, 'source>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        let floor = budget.storage();
        budget.reserve_storage(argument_sum_v1(&[
            std::mem::size_of::<Self>(),
            std::mem::size_of::<Result<Self, ProductionSemanticKirErrorV1>>(),
            std::mem::size_of::<SourceIssuedMeterV29<'_, '_>>(),
            source_issued_semantic_query_headers_v29()?,
        ])?)?;
        budget.charge_work(8)?;
        let original = instances
            .instance(instance)
            .ok_or_else(source_issued_error_v29)?
            .declaration();
        let occurrences = instances
            .occurrences(instance)
            .ok_or_else(source_issued_error_v29)?;
        let inventory = SourceDescriptorInventoryV29::build(&occurrences, budget)?;
        let mut definitions = BTreeMap::new();
        for (&value, &index) in &inventory.definitions {
            budget.charge_work(1)?;
            reserve_execution_cfg_map_entry_v29::<SsaValueV1, SourceIssuedDefinitionV29>(
                definitions.len(),
                budget,
            )?;
            definitions.insert(value, SourceIssuedDefinitionV29::Statement(index));
        }
        for (index, edge) in occurrences.edge_definitions().iter().enumerate() {
            budget.charge_work(3)?;
            if !edge.is_reachable() || !edge.is_promoted() {
                continue;
            }
            let value = edge.value().ok_or_else(source_issued_error_v29)?;
            reserve_execution_cfg_map_entry_v29::<SsaValueV1, SourceIssuedDefinitionV29>(
                definitions.len(),
                budget,
            )?;
            if definitions
                .insert(value, SourceIssuedDefinitionV29::Edge(index))
                .is_some()
            {
                return Err(source_issued_error_v29());
            }
        }
        let (producers, producer_storage) =
            fe2o3_mir_model::semantic_option_producers_with_meter_v18(
                original,
                instances.owner().source_semantic().callables(),
                &mut SourceIssuedMeterV29(budget),
            )
            .map_err(source_issued_analysis_error_v29)?;
        let (dominance, _) = fe2o3_mir_model::SemanticOptionDominanceV1::analyze_with_meter_v18(
            original,
            &producers,
            &mut SourceIssuedMeterV29(budget),
        )
        .map_err(source_issued_analysis_error_v29)?;
        drop(producers);
        budget.release_storage(producer_storage)?;
        let links = emission_vec_v1(definitions.len(), budget)?;
        let owned = budget
            .storage()
            .checked_sub(floor)
            .ok_or(ArgumentResourceV1::Accounting)?;
        Ok(Self {
            instances,
            instance,
            mode,
            inventory,
            definitions,
            memo: BTreeMap::new(),
            links,
            dominance,
            slot: std::ptr::from_ref(budget) as usize,
            ledger: budget.work_ledger_identity_v1(),
            floor,
            owned,
            failure: std::cell::Cell::new(None),
        })
    }

    fn check(&self, budget: &ArgumentBudgetV1<'_>) -> Result<(), ProductionSemanticKirErrorV1> {
        if let Some(error) = self.failure.get() {
            return Err(error.into());
        }
        let required = match argument_sum_v1(&[self.floor, self.owned]) {
            Ok(required) => required,
            Err(error) => return self.retain(Err(error.into())),
        };
        if self.slot != std::ptr::from_ref(budget) as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < required
        {
            self.failure.set(Some(ArgumentResourceV1::Accounting));
            return Err(ArgumentResourceV1::Accounting.into());
        }
        Ok(())
    }

    fn retain<T>(
        &self,
        result: Result<T, ProductionSemanticKirErrorV1>,
    ) -> Result<T, ProductionSemanticKirErrorV1> {
        if self.failure.get().is_none() {
            if let Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error)) =
                &result
            {
                self.failure.set(Some(*error));
            }
        }
        match self.failure.get() {
            Some(error) => Err(error.into()),
            None => result,
        }
    }

    fn use_value(
        &self,
        site: ExecutionSiteV29,
        role: ExecutionOperandV29,
        place: &SemanticPlaceV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<SsaValueV1, ProductionSemanticKirErrorV1> {
        self.retain((|| {
            self.check(budget)?;
            let function = self
                .instances
                .instance(self.instance)
                .ok_or_else(source_issued_error_v29)?
                .declaration();
            // Query callers must supply the immutable operand, not a same-shaped
            // place copied from another source occurrence.
            budget.charge_work(2)?;
            let exact = match scoped_source_operand_v29(function, site, role) {
                Some(SemanticOperandV1::Copy(original) | SemanticOperandV1::Move(original)) => {
                    Some(original)
                }
                _ => scoped_source_place_v29(function, site, role),
            };
            if !exact.is_some_and(|original| std::ptr::eq(original, place)) {
                return Err(source_issued_error_v29());
            }
            let occurrences = self
                .instances
                .occurrences(self.instance)
                .ok_or_else(source_issued_error_v29)?;
            let index = self
                .inventory
                .use_at(&occurrences, site, role, budget)?
                .ok_or_else(source_issued_error_v29)?;
            budget.charge_work(2)?;
            if occurrences.events()[index].event().variable().get() != place.local().index() {
                return Err(source_issued_error_v29());
            }
            source_descriptor_use_v29(&occurrences, index, site, role, budget)
        })())
    }

    fn replay_archive(
        &self,
        value: SsaValueV1,
        recipe: SourceIssuedSemanticRecipeV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.check(budget)?;
        match self.mode {
            SourceIssuedReplayModeV29::SourceOnly => Ok(()),
            SourceIssuedReplayModeV29::Emitted(index) => {
                let physical = source_issued_archive_recipe_v29(
                    self.instances,
                    self.instance,
                    index,
                    recipe,
                    budget,
                )?;
                check_source_issued_archive_v29(
                    self.instances,
                    self.instance,
                    index,
                    value,
                    physical,
                    budget,
                )
            }
        }
    }

    // This is an original effect selector, not an access permit. Callers still
    // owe C2 state, receiver ownership, actual tail, payload and guard replay.
    fn effect(
        &mut self,
        site: ExecutionSiteV29,
        role: ExecutionOperandV29,
        place: &SemanticPlaceV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<SourceIssuedOriginalEffectV29>, ProductionSemanticKirErrorV1> {
        let result = self.effect_inner(site, role, place, budget);
        self.retain(result)
    }

    fn effect_inner(
        &mut self,
        site: ExecutionSiteV29,
        role: ExecutionOperandV29,
        place: &SemanticPlaceV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<SourceIssuedOriginalEffectV29>, ProductionSemanticKirErrorV1> {
        self.check(budget)?;
        budget.charge_work(16)?;
        let function = self
            .instances
            .instance(self.instance)
            .ok_or_else(source_issued_error_v29)?
            .declaration();
        let (writing, volatile) = match (scoped_source_statement_v29(function, site), role) {
            (
                Some(SemanticStatementKindV1::Assign(assignment)),
                ExecutionOperandV29::RvaluePlace,
            ) => {
                let SemanticRvalueKindV1::Load(load) = assignment.value().kind() else {
                    return Ok(None);
                };
                if !std::ptr::eq(load.source(), place)
                    || assignment.value().result_type() != place.ty()
                {
                    return Err(source_issued_error_v29());
                }
                if load.atomic().is_some() {
                    return Ok(None);
                }
                (false, load.volatility() == SemanticVolatilityV1::Volatile)
            }
            (
                Some(SemanticStatementKindV1::Store(store)),
                ExecutionOperandV29::StoreDestination,
            ) => {
                if !std::ptr::eq(store.destination(), place)
                    || semantic_operand_type(store.value()) != place.ty()
                {
                    return Err(source_issued_error_v29());
                }
                if store.atomic().is_some() {
                    return Ok(None);
                }
                (true, store.volatility() == SemanticVolatilityV1::Volatile)
            }
            _ => return Ok(None),
        };
        if place.projections().len() != 1
            || place.projections()[0].kind() != SemanticProjectionKindV1::Dereference
        {
            return Ok(None);
        }
        let definition = self.use_value(site, role, place, budget)?;
        let Some(recipe) = self.resolve(definition, budget)? else {
            return Ok(None);
        };
        budget.charge_work(5)?;
        if recipe.form != SourceIssuedFormV29::Pointer
            || function
                .locals()
                .get(place.local().index() as usize)
                .map(|local| local.ty())
                != Some(recipe.pointer_type)
            || lower_scalar_type(self.instances.owner().source_semantic().types(), place.ty())?
                != Type::Scalar(recipe.element)
            || (writing && recipe.access != AccessMode::ReadWrite)
        {
            return Err(source_issued_error_v29());
        }
        Ok(Some(SourceIssuedOriginalEffectV29 {
            recipe,
            writing,
            volatile,
        }))
    }

    fn issuer(
        &self,
        ordinal: usize,
        value: SsaValueV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<SourceIssuedSemanticRecipeV29>, ProductionSemanticKirErrorV1> {
        budget.charge_work(15)?;
        let source = self.instances.owner().source_semantic();
        let function = self
            .instances
            .instance(self.instance)
            .ok_or_else(source_issued_error_v29)?
            .declaration();
        let occurrences = self
            .instances
            .occurrences(self.instance)
            .ok_or_else(source_issued_error_v29)?;
        let edge = occurrences
            .edge_definitions()
            .get(ordinal)
            .ok_or_else(source_issued_error_v29)?;
        if !edge.is_reachable()
            || !edge.is_promoted()
            || edge.value() != Some(value)
            || edge.edge().ordinal() != 0
            || edge.ordinal() != 0
        {
            return Err(source_issued_error_v29());
        }
        let block = SemanticBlockIdV1::from_index(edge.edge().source().get());
        let Some(SemanticTerminatorKindV1::Call(call)) = function
            .blocks()
            .get(block.index() as usize)
            .map(|block| block.terminator().kind())
        else {
            return Err(source_issued_error_v29());
        };
        let Some(SemanticCallableDeclV1::CompilerIntrinsic { operation, .. }) =
            source.callables().get(call.callee().index() as usize)
        else {
            return Ok(None);
        };
        if !matches!(
            operation,
            SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut { .. }
                | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetDisjointMut { .. }
        ) {
            return Ok(None);
        }
        let destination = call
            .destination()
            .ok_or_else(source_issued_error_v29)?
            .place();
        if !destination.projections().is_empty()
            || destination.local().index() != edge.variable().get()
        {
            return Err(source_issued_error_v29());
        }
        let option_type = destination.ty();
        let pointer_type = option_payload_type_v1(source.types(), option_type)
            .ok_or_else(source_issued_error_v29)?;
        let availability = self
            .dominance
            .availability(destination.local())
            .ok_or_else(source_issued_error_v29)?;
        budget.charge_work(argument_sum_v1(&[
            argument_product_v1(source.callables().len(), 4)?,
            32,
        ])?)?;
        let Some(SemanticPromotedBindingV1::OptionPointer {
            element,
            address_space: AddressSpace::Global,
            access,
            ..
        }) = optional_pointer_binding_v1(
            source.types(),
            source.callables(),
            operation,
            pointer_type,
            availability,
        )
        else {
            return Err(source_issued_error_v29());
        };
        let recipe = SourceIssuedSemanticRecipeV29 {
            issuer: value,
            block,
            option_type,
            pointer_type,
            availability,
            element,
            access,
            form: SourceIssuedFormV29::Option,
        };
        self.replay_archive(value, recipe, budget)?;
        Ok(Some(recipe))
    }

    fn complete(
        &mut self,
        value: SsaValueV1,
        recipe: Option<SourceIssuedSemanticRecipeV29>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        charge_execution_cfg_lookup_v29(self.memo.len(), budget)?;
        let Some(row @ SourceIssuedMemoV29::Visiting) = self.memo.get_mut(&value) else {
            return Err(ArgumentResourceV1::Accounting.into());
        };
        *row = SourceIssuedMemoV29::Complete(recipe);
        Ok(())
    }

    fn resolve(
        &mut self,
        requested: SsaValueV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<SourceIssuedSemanticRecipeV29>, ProductionSemanticKirErrorV1> {
        self.check(budget)?;
        let before = budget.storage();
        let result = self.resolve_inner(requested, budget);
        let result = self.retain(result);
        // Failed partial construction still owns its paid map nodes. It does
        // not publish a recipe or permit a successful replay of Visiting rows.
        let owned = budget
            .storage()
            .checked_sub(before)
            .ok_or(ArgumentResourceV1::Accounting)
            .and_then(|extra| argument_sum_v1(&[self.owned, extra]));
        match owned {
            Ok(owned) => self.owned = owned,
            Err(error) => return self.retain(Err(error.into())),
        }
        self.retain(result)
    }

    fn resolve_inner(
        &mut self,
        requested: SsaValueV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<SourceIssuedSemanticRecipeV29>, ProductionSemanticKirErrorV1> {
        if !self.links.is_empty() {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        let mut value = requested;
        let mut resolved;
        loop {
            charge_execution_cfg_lookup_v29(self.memo.len(), budget)?;
            match self.memo.get(&value).copied() {
                Some(SourceIssuedMemoV29::Complete(result)) => {
                    resolved = result;
                    break;
                }
                Some(SourceIssuedMemoV29::Visiting) => return Err(source_issued_error_v29()),
                None => {}
            }
            reserve_execution_cfg_map_entry_v29::<SsaValueV1, SourceIssuedMemoV29>(
                self.memo.len(),
                budget,
            )?;
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
            let function = self
                .instances
                .instance(self.instance)
                .ok_or_else(source_issued_error_v29)?
                .declaration();
            let occurrences = self
                .instances
                .occurrences(self.instance)
                .ok_or_else(source_issued_error_v29)?;
            let (site, rvalue) =
                source_descriptor_assignment_v29(function, &occurrences, statement, value, budget)?;
            let SemanticRvalueKindV1::Use(
                SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place),
            ) = rvalue
            else {
                resolved = None;
                self.complete(value, None, budget)?;
                break;
            };
            let dependency =
                self.use_value(site, ExecutionOperandV29::RvalueOperand(0), place, budget)?;
            budget.charge_work(2)?;
            if self.links.len() == self.links.capacity() {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            self.links.push(SourceIssuedLinkV29 {
                value,
                dependency,
                occurrence: statement,
            });
            value = dependency;
        }
        while let Some(link) = self.links.pop() {
            budget.charge_work(3)?;
            if let Some(mut recipe) = resolved {
                self.replay_archive(link.dependency, recipe, budget)?;
                let source = self.instances.owner().source_semantic();
                let function = self
                    .instances
                    .instance(self.instance)
                    .ok_or_else(source_issued_error_v29)?
                    .declaration();
                let occurrences = self
                    .instances
                    .occurrences(self.instance)
                    .ok_or_else(source_issued_error_v29)?;
                let (site, rvalue) = source_descriptor_assignment_v29(
                    function,
                    &occurrences,
                    link.occurrence,
                    link.value,
                    budget,
                )?;
                let SemanticRvalueKindV1::Use(
                    SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place),
                ) = rvalue
                else {
                    return Err(source_issued_error_v29());
                };
                let local_type = function
                    .locals()
                    .get(place.local().index() as usize)
                    .ok_or_else(source_issued_error_v29)?
                    .ty();
                if place.projections().is_empty() {
                    let expected = match recipe.form {
                        SourceIssuedFormV29::Option => recipe.option_type,
                        SourceIssuedFormV29::Pointer => recipe.pointer_type,
                    };
                    if local_type != expected || place.ty() != expected {
                        return Err(source_issued_error_v29());
                    }
                } else if recipe.form == SourceIssuedFormV29::Option
                    && local_type == recipe.option_type
                    && exact_option_payload_projection_v1(
                        source.types(),
                        local_type,
                        place.projections(),
                        place.ty(),
                    ) == Some(recipe.pointer_type)
                    && self.dominance.allows(
                        recipe.availability,
                        SemanticBlockIdV1::from_index(scoped_memory_site_key_v29(site).0),
                    )
                {
                    recipe.form = SourceIssuedFormV29::Pointer;
                } else {
                    return Err(source_issued_error_v29());
                }
                self.replay_archive(link.value, recipe, budget)?;
                resolved = Some(recipe);
            }
            self.complete(link.value, resolved, budget)?;
        }
        Ok(resolved)
    }
}

fn source_issued_semantic_query_headers_v29() -> Result<usize, ArgumentResourceV1> {
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
        h::<SourceIssuedReplayModeV29<'_, '_>>()?,
        h::<&SourceIssuedReplayModeV29<'_, '_>>()?,
        h::<&SourceAddressSourceIndexV29<'_>>()?,
        h::<std::cell::Cell<Option<ArgumentResourceV1>>>()?,
        h::<Option<ArgumentResourceV1>>()?,
        h::<ArgumentResourceV1>()?,
        h::<&ProductionSemanticKirErrorV1>()?,
        h::<&ArgumentResourceV1>()?,
        h::<SourceIssuedSemanticRecipeV29>()?,
        h::<Option<SourceIssuedSemanticRecipeV29>>()?,
        h::<SourceIssuedOriginalEffectV29>()?,
        h::<Option<SourceIssuedOriginalEffectV29>>()?,
        h::<SourceIssuedRecipeV29>()?,
        h::<&SourceIssuedRecipeV29>()?,
        h::<Option<SourceIssuedRecipeV29>>()?,
        // Additional return slots for the resource-retaining query wrappers.
        argument_product_v1(
            2,
            argument_sum_v1(&[
                std::mem::size_of::<Result<(), ProductionSemanticKirErrorV1>>(),
                std::mem::size_of::<Result<SsaValueV1, ProductionSemanticKirErrorV1>>(),
                std::mem::size_of::<
                    Result<Option<SourceIssuedSemanticRecipeV29>, ProductionSemanticKirErrorV1>,
                >(),
                std::mem::size_of::<
                    Result<Option<SourceIssuedOriginalEffectV29>, ProductionSemanticKirErrorV1>,
                >(),
                std::mem::size_of::<
                    Result<Option<SourceIssuedRecipeV29>, ProductionSemanticKirErrorV1>,
                >(),
                std::mem::size_of::<Result<&SemanticValueBindingV1, ProductionSemanticKirErrorV1>>(
                ),
            ])?,
        )?,
        std::mem::size_of::<Result<usize, ArgumentResourceV1>>(),
        h::<SourceIssuedMemoV29>()?,
        h::<Option<SourceIssuedMemoV29>>()?,
        h::<Option<&SourceIssuedMemoV29>>()?,
        h::<Option<&mut SourceIssuedMemoV29>>()?,
        h::<&mut SourceIssuedMemoV29>()?,
        h::<SourceIssuedDefinitionV29>()?,
        h::<Option<SourceIssuedDefinitionV29>>()?,
        h::<Option<&SourceIssuedDefinitionV29>>()?,
        h::<SourceIssuedLinkV29>()?,
        h::<Option<SourceIssuedLinkV29>>()?,
        h::<SsaValueV1>()?,
        h::<Option<SsaValueV1>>()?,
        h::<usize>()?,
        h::<Option<usize>>()?,
        h::<&SemanticValueBindingV1>()?,
        h::<&SemanticValueBindingV1>()?,
        h::<&ValueId>()?,
        h::<&ValueId>()?,
        h::<&SemanticOptionAvailabilityV1>()?,
        h::<&PendingInstanceSidecarsV29>()?,
        h::<&ExecutionArchiveV29>()?,
        h::<Option<&ExecutionArchiveV29>>()?,
        h::<&SemanticPlaceV1>()?,
        h::<Option<&SemanticPlaceV1>>()?,
        argument_product_v1(3, h::<&SemanticFunctionDeclV1>()?)?,
        h::<&Type>()?,
        h::<&fe2o3_kernel_ir::PointerType>()?,
        h::<Type>()?,
        h::<&production_call_instances_v1::ProductionCallInstanceV1<'_>>()?,
        h::<Option<&production_call_instances_v1::ProductionCallInstanceV1<'_>>>()?,
        argument_product_v1(2, h::<ProductionSemanticSsaFunctionOccurrencesV1<'_>>()?)?,
        h::<Option<ProductionSemanticSsaFunctionOccurrencesV1<'_>>>()?,
        h::<&fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1>()?,
        h::<&SemanticDirectCallV1>()?,
        h::<Option<&SemanticCallableDeclV1>>()?,
        h::<Option<&fe2o3_mir_model::semantic_mir_v1::SemanticCallDestinationV1>>()?,
        h::<Option<&fe2o3_mir_model::semantic_mir_v1::SemanticBasicBlockV1>>()?,
        h::<Option<&SemanticTerminatorKindV1>>()?,
        h::<Option<&fe2o3_mir_model::semantic_mir_v1::SemanticLocalDeclV1>>()?,
        h::<&fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1>()?,
        h::<&fe2o3_mir_model::semantic_mir_v1::SemanticMemoryLoadV1>()?,
        h::<&fe2o3_mir_model::semantic_mir_v1::SemanticMemoryStoreV1>()?,
        h::<Option<&SemanticStatementKindV1>>()?,
        h::<Option<&SemanticOperandV1>>()?,
        h::<&SemanticStatementKindV1>()?,
        h::<&SemanticRvalueKindV1>()?,
        h::<&SemanticCompilerIntrinsicOperationV1>()?,
        h::<(ExecutionSiteV29, &SemanticRvalueKindV1)>()?,
        h::<(bool, bool)>()?,
        h::<std::collections::btree_map::Iter<'_, SsaValueV1, usize>>()?,
        h::<Option<(&SsaValueV1, &usize)>>()?,
        h::<(&SsaValueV1, &usize)>()?,
        h::<&[fe2o3_pliron::ProductionSemanticSsaEventOccurrenceV1]>()?,
        h::<std::slice::Iter<'_, fe2o3_pliron::ProductionSemanticSsaEventOccurrenceV1>>()?,
        h::<
            std::iter::Enumerate<
                std::slice::Iter<'_, fe2o3_pliron::ProductionSemanticSsaEventOccurrenceV1>,
            >,
        >()?,
        h::<Option<(usize, &fe2o3_pliron::ProductionSemanticSsaEventOccurrenceV1)>>()?,
        h::<(usize, &fe2o3_pliron::ProductionSemanticSsaEventOccurrenceV1)>()?,
        h::<&fe2o3_pliron::ProductionSemanticSsaEventOccurrenceV1>()?,
        h::<Option<&fe2o3_pliron::ProductionSemanticSsaEventOccurrenceV1>>()?,
        h::<std::ops::Range<usize>>()?,
        h::<&std::ops::Range<usize>>()?,
        h::<&mut std::ops::Range<usize>>()?,
        h::<Option<&std::ops::Range<usize>>>()?,
        h::<Option<&mut std::ops::Range<usize>>>()?,
        h::<(u32, Option<u32>)>()?,
        h::<SsaResolvedEventV1>()?,
        h::<Option<SsaResolvedEventV1>>()?,
        h::<std::slice::Iter<'_, fe2o3_pliron::ProductionSemanticSsaEdgeDefinitionOccurrenceV1>>()?,
        h::<
            std::iter::Enumerate<
                std::slice::Iter<'_, fe2o3_pliron::ProductionSemanticSsaEdgeDefinitionOccurrenceV1>,
            >,
        >()?,
        h::<
            Option<(
                usize,
                &fe2o3_pliron::ProductionSemanticSsaEdgeDefinitionOccurrenceV1,
            )>,
        >()?,
        h::<(
            usize,
            &fe2o3_pliron::ProductionSemanticSsaEdgeDefinitionOccurrenceV1,
        )>()?,
        h::<&fe2o3_pliron::ProductionSemanticSsaEdgeDefinitionOccurrenceV1>()?,
        h::<Option<&fe2o3_pliron::ProductionSemanticSsaEdgeDefinitionOccurrenceV1>>()?,
        h::<Vec<fe2o3_mir_model::SemanticOptionProducerV1>>()?,
        h::<(Vec<fe2o3_mir_model::SemanticOptionProducerV1>, usize)>()?,
        h::<(fe2o3_mir_model::SemanticOptionDominanceV1, usize)>()?,
        std::mem::size_of::<
            Result<
                (Vec<fe2o3_mir_model::SemanticOptionProducerV1>, usize),
                fe2o3_mir_model::SemanticOptionDominanceMeteredErrorV18<ArgumentResourceV1>,
            >,
        >(),
        std::mem::size_of::<
            Result<
                (fe2o3_mir_model::SemanticOptionDominanceV1, usize),
                fe2o3_mir_model::SemanticOptionDominanceMeteredErrorV18<ArgumentResourceV1>,
            >,
        >(),
        h::<Option<SemanticTypeIdV1>>()?,
        h::<Option<SemanticOptionAvailabilityV1>>()?,
        h::<Option<SemanticPromotedBindingV1>>()?,
        h::<bool>()?,
        h::<()>()?,
    ])
}
