// Exact source-instance transfers consumed by the finite checked CFG worklist.

include!("production_source_reference_array_assignment_v29.rs");
include!("production_source_reference_static_assignment_v43.rs");
include!("production_source_direct_volatile_v29.rs");
include!("production_source_raw_volatile_v29.rs");
include!("production_source_ordered_issued_v29.rs");

impl SourceReferenceBuilderV29<'_, '_, '_> {
    fn function(
        &mut self,
        instance: ProductionCallInstanceIdV1,
        arguments: Option<&[usize]>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<SourceReferenceFunctionOutcomeV29, ProductionSemanticKirErrorV1> {
        budget.charge_work(3)?;
        if self.plan.instances.instance_reachable(instance) != Some(true) {
            return Err(source_reference_cfg_obligation_v29());
        }
        budget.charge_work(self.frames.len())?;
        if self.frames.iter().filter(|frame| frame.is_some()).count() >= 256 {
            return Err(source_reference_error_v29(
                "source reference call depth exceeds the checked bound",
            ));
        }
        let result = if self.visited.get(instance.index()) == Some(&true) {
            self.reuse_call_summary(instance, arguments, budget)?
        } else {
            self.evaluate_call_transfer(instance, arguments, budget)?
        };
        self.leave_selector_invocation(instance, result, budget)
    }

    fn evaluate_call_transfer(
        &mut self,
        instance: ProductionCallInstanceIdV1,
        arguments: Option<&[usize]>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<SourceReferenceFunctionOutcomeV29, ProductionSemanticKirErrorV1> {
        budget.reserve_storage(source_reference_call_transfer_headers_v29()?)?;
        budget.charge_work(1)?;
        if self
            .frames
            .get(instance.index())
            .is_none_or(Option::is_some)
        {
            return Err(source_reference_cfg_obligation_v29());
        }
        budget.reserve_storage(source_reference_cfg_call_headers_v29()?)?;
        let before = self.cfg_snapshot(budget)?;
        let previous_site = self.effect_site;
        let previous_ordinal = self.effect_ordinal;
        let summary_arguments = if let Some(arguments) = arguments {
            let mut copied = source_reference_scratch_v29(arguments.len(), budget)?;
            budget.charge_work(arguments.len())?;
            copied.extend_from_slice(arguments);
            Some(copied)
        } else {
            None
        };
        self.visited[instance.index()] = true;
        let instances = self.plan.instances;
        let row = instances
            .instance(instance)
            .ok_or_else(|| source_reference_error_v29("source reference instance is missing"))?;
        let function = row.declaration();
        let mut locals = source_reference_scratch_v29(function.locals().len(), budget)?;
        budget.charge_work(function.locals().len())?;
        locals.resize(function.locals().len(), SourceReferenceLocalV29::default());
        for (local, declaration) in function.locals().iter().enumerate() {
            budget.charge_work(1)?;
            let id = SemanticLocalIdV1::from_index(
                u32::try_from(local).map_err(|_| ArgumentResourceV1::Arithmetic)?,
            );
            self.retain_storage_activation(
                SourceReferenceStorageActivationV29 {
                    instance,
                    local: id,
                    generation: 0,
                    origin: SourceReferenceActivationOriginV29::Entry,
                },
                budget,
            )?;
            if declaration.role().is_entry_argument() {
                let node = if let Some(arguments) = arguments {
                    let selector = instances.parameter_source(instance, id, budget).map_err(|error| {
                        match error {
                            production_call_instances_v1::ProductionCallInstanceErrorV1::Resource(error) => error.into(),
                            _ => source_reference_error_v29("source reference parameter selector differs"),
                        }
                    })?;
                    let mut node = *arguments
                        .get(selector.source_argument as usize)
                        .ok_or_else(|| {
                            source_reference_error_v29("source reference argument is missing")
                        })?;
                    if let Some(field) = selector.tuple_field {
                        node = self.field(node, field as usize, budget)?;
                    }
                    if self.plan.nodes[node].ty != declaration.ty()
                        || selector.ty != declaration.ty()
                    {
                        return Err(source_reference_error_v29(
                            "source reference parameter type differs",
                        ));
                    }
                    node
                } else {
                    let SemanticLocalRoleV1::Argument(argument) = declaration.role() else {
                        return Err(source_reference_error_v29(
                            "source reference root argument is not direct",
                        ));
                    };
                    let node = self.node(
                        declaration.ty(),
                        SourceReferenceNodeKindV29::Plain(Some(SourceReferenceAnchorV29 {
                            argument,
                            ty: declaration.ty(),
                        })),
                        budget,
                    )?;
                    self.seed_descriptor_argument(node, argument, budget)?;
                    node
                };
                locals[local].node = Some(node);
            }
            locals[local].storage = self.initial_storage_snapshot(
                instance,
                id,
                declaration.role().is_entry_argument(),
                arguments.and(locals[local].node),
                budget,
            )?;
        }
        let entry = self.plan.states.len();
        emission_push_v1(&mut self.plan.states, locals, budget)?;
        self.retain_call_entry(instance, entry, budget)?;
        let result = self.run_cfg(instance, entry, budget);
        self.effect_site = previous_site;
        self.effect_ordinal = previous_ordinal;
        let result = result?;
        let after = match result {
            SourceReferenceFunctionOutcomeV29::Returned(_) => Some(self.cfg_snapshot(budget)?),
            SourceReferenceFunctionOutcomeV29::NoNormalReturn => {
                // Restore the analysis stack, not a normal or unwind output.
                // Keep the cached input immutable for later loop evaluations.
                let restored = self.cfg_clone(&before, budget)?;
                self.cfg_install(&restored, budget)?;
                None
            }
        };
        self.retain_call_summary(
            instance,
            SourceReferenceCallSummaryV29 {
                arguments: summary_arguments,
                before,
                after,
                result,
                reuse_count: 0,
                evaluations: 1,
            },
            budget,
        )?;
        Ok(result)
    }

    fn statement(
        &mut self,
        site: SourceReferenceSiteV29,
        statement: &SemanticStatementKindV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        match statement {
            SemanticStatementKindV1::Assign(assignment) => {
                if matches!(
                    assignment.value().kind(),
                    SemanticRvalueKindV1::CheckedBinary(_)
                ) {
                    return self.assign_constructed(
                        site,
                        SourceReferenceConstructionV29::CheckedBinary(assignment),
                        budget,
                    );
                }
                let source = self.plan.instances.owner().source_semantic();
                if source_reference_grid_leader_constant_v29(
                    source.types(),
                    source.callables(),
                    assignment,
                    budget,
                )? {
                    return self.assign_constructed(
                        site,
                        SourceReferenceConstructionV29::GridLeaderZero(assignment),
                        budget,
                    );
                }
                let node = self.rvalue(site, assignment.value(), budget)?;
                self.assign(site, assignment.destination(), node, budget)
            }
            SemanticStatementKindV1::StorageLive(local) => {
                #[cfg(test)]
                let raw_before = self.test_raw_restart_probe(site, *local);
                self.end_storage(site.instance, *local, budget)?;
                self.transfer_storage_lifetime(site.instance, *local, true, budget)?;
                let first = *self
                    .epochs
                    .get(site.instance.index())
                    .and_then(|blocks| blocks.get(site.block.index() as usize))
                    .ok_or_else(source_reference_cfg_obligation_v29)?;
                let generation = u32::try_from(argument_sum_v1(&[
                    first,
                    site.statement
                        .ok_or_else(source_reference_cfg_obligation_v29)?,
                ])?)
                .map_err(|_| ArgumentResourceV1::Arithmetic)?;
                self.retain_storage_activation(
                    SourceReferenceStorageActivationV29 {
                        instance: site.instance,
                        local: *local,
                        generation,
                        origin: SourceReferenceActivationOriginV29::StorageLive(site),
                    },
                    budget,
                )?;
                #[cfg(test)]
                self.test_raw_restart_observe(site, *local, generation, raw_before);
                let mut value = self.local(site.instance, *local)?;
                value.generation = generation;
                value.node = None;
                self.set_local(site.instance, *local, value)
            }
            SemanticStatementKindV1::StorageDead(local) => {
                self.end_storage(site.instance, *local, budget)
            }
            SemanticStatementKindV1::Store(store) => {
                if (store.volatility() != SemanticVolatilityV1::NonVolatile
                    || store.atomic().is_some())
                    && !self.plan.ordered_descriptor_effect(
                        site,
                        store.destination(),
                        ExecutionOperandV29::StoreDestination,
                        budget,
                    )?
                    && !self.plan.ordered_issued_effect(
                        site,
                        store.destination(),
                        ExecutionOperandV29::StoreDestination,
                        budget,
                    )?
                {
                    return Err(source_reference_error_v29(
                        "source reference ordered store requires checked addressable effects",
                    ));
                }
                let node = self.operand(site, store.value(), budget)?;
                self.assign(site, store.destination(), node, budget)
            }
            SemanticStatementKindV1::Deinitialize(place) => {
                let target = self.write_place(site, place, budget)?;
                let tracked = self.mutate_storage_place(
                    &target,
                    source_storage_v29::SourceStorageRootMutationV29::Deinitialize,
                    budget,
                )?;
                if !target.projections.is_empty() && !tracked {
                    return Err(source_reference_error_v29(
                        "projected reference deinitialization requires cell state",
                    ));
                }
                self.remove_storage_value(
                    site,
                    place,
                    SourceReferenceRemovalKindV29::Deinitialize,
                    &target,
                    budget,
                )?;
                self.invalidate_discriminant_values(
                    Some(target.instance),
                    Some(target.local),
                    None,
                    budget,
                )?;
                Ok(())
            }
            SemanticStatementKindV1::SetDiscriminant {
                place,
                variant_index,
            } => {
                let target = self.write_place(site, place, budget)?;
                self.plan
                    .enum_variant_fields(place.ty(), *variant_index, budget)?;
                if !self.mutate_storage_place(
                    &target,
                    source_storage_v29::SourceStorageRootMutationV29::SetDiscriminant(
                        *variant_index,
                    ),
                    budget,
                )? {
                    return Err(source_reference_enum_error_v29());
                }
                if self.local(target.instance, target.local)?.node.is_none() {
                    let node = self.plain(place.ty(), budget)?;
                    self.install_assigned_node(&target, node, budget)?;
                }
                // Retagging does not remove possible references in payload bytes.
                self.invalidate_discriminant_values(
                    Some(target.instance),
                    Some(target.local),
                    None,
                    budget,
                )?;
                Ok(())
            }
            SemanticStatementKindV1::AtomicRmw(atomic) => self.atomic_rmw_v41(site, atomic, budget),
            SemanticStatementKindV1::Nop => Ok(()),
            // No unexamined operation can silently certify a stable referent.
            _ => Err(source_reference_error_v29(
                "source reference statement effect is unsupported",
            )),
        }
    }

    fn rvalue(
        &mut self,
        site: SourceReferenceSiteV29,
        value: &fe2o3_mir_model::semantic_mir_v1::SemanticRvalueV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        match value.kind() {
            SemanticRvalueKindV1::Use(operand) => self.operand(site, operand, budget),
            SemanticRvalueKindV1::Borrow { kind, place } => {
                self.borrow(site, *kind, place, value.result_type(), budget)
            }
            SemanticRvalueKindV1::Aggregate(aggregate) => {
                let mut operands =
                    source_reference_scratch_v29(aggregate.operands().len(), budget)?;
                for operand in aggregate.operands() {
                    budget.charge_work(1)?;
                    operands.push(self.operand(site, operand, budget)?);
                }
                if let SemanticAggregateKindV1::EnumVariant(variant) = *aggregate.kind() {
                    return self.construct_enum_value(
                        value.result_type(),
                        variant,
                        &operands,
                        budget,
                    );
                }
                let first = self.plan.children.len();
                for &operand in &operands {
                    emission_push_v1(&mut self.plan.children, operand, budget)?;
                }
                self.node(
                    value.result_type(),
                    SourceReferenceNodeKindV29::Aggregate {
                        first,
                        count: operands.len(),
                    },
                    budget,
                )
            }
            SemanticRvalueKindV1::AddressOf { place, mutability } => {
                self.address_value(site, place, value.result_type(), *mutability, budget)
            }
            SemanticRvalueKindV1::Load(load) => {
                if (load.volatility() != SemanticVolatilityV1::NonVolatile
                    || load.atomic().is_some())
                    && !self.plan.ordered_descriptor_effect(
                        site,
                        load.source(),
                        ExecutionOperandV29::RvaluePlace,
                        budget,
                    )?
                    && !self.plan.ordered_issued_effect(
                        site,
                        load.source(),
                        ExecutionOperandV29::RvaluePlace,
                        budget,
                    )?
                {
                    if !load.source().projections().is_empty() {
                        check_source_raw_volatile_load_v29(
                            self.plan.instances,
                            site,
                            load,
                            value.result_type(),
                            budget,
                        )?;
                        let node = self.read_place(site, load.source(), budget)?;
                        self.plan
                            .check_resolved_raw_volatile_load_v29(site, load, budget)?;
                        return Ok(node);
                    }
                    check_source_direct_volatile_load_v29(
                        self.plan.instances,
                        site,
                        load,
                        value.result_type(),
                        budget,
                    )?;
                }
                self.read_place(site, load.source(), budget)
            }
            SemanticRvalueKindV1::Discriminant(place) => {
                self.observe_enum_discriminant(site, place, value.result_type(), budget)
            }
            SemanticRvalueKindV1::Length(place) => {
                let node = self.read_place(site, place, budget)?;
                self.observe_node_address(node, budget)?;
                self.plain(value.result_type(), budget)
            }
            SemanticRvalueKindV1::Cast { kind, operand } => {
                self.cast_address_value(site, *kind, operand, value.result_type(), budget)
            }
            other @ (SemanticRvalueKindV1::Unary { .. }
            | SemanticRvalueKindV1::Binary { .. }
            | SemanticRvalueKindV1::CheckedBinary(_)
            | SemanticRvalueKindV1::UncheckedBinary(_)) => {
                other.try_visit_operands(|operand| {
                    let node = self.operand(site, operand, budget)?;
                    self.observe_node_address(node, budget)
                })?;
                self.plain(value.result_type(), budget)
            }
        }
    }

    fn operand(
        &mut self,
        site: SourceReferenceSiteV29,
        operand: &SemanticOperandV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        budget.charge_work(1)?;
        match operand {
            SemanticOperandV1::Constant(constant) => self.plain(constant.ty(), budget),
            SemanticOperandV1::Copy(place) => self.read_place(site, place, budget),
            SemanticOperandV1::Move(place) => {
                let mut resolved = self.resolve_reference_place(
                    site,
                    place,
                    SourceReferenceAccessV29::Read,
                    budget,
                )?;
                if let Some(loan) = resolved.loan {
                    self.effect(loan, SourceReferenceEffectV29::ReadReferent, budget)?;
                }
                self.capture_atomic_root_pointer_v41(site, place, &mut resolved, budget)?;
                let node = self.attach_storage_value(&resolved, budget)?;
                let tracked = self.mutate_storage_place(
                    &resolved,
                    source_storage_v29::SourceStorageRootMutationV29::Deinitialize,
                    budget,
                )?;
                if !resolved.projections.is_empty()
                    && !tracked
                    && self.contains_loan(node, budget)?
                {
                    return Err(source_reference_error_v29(
                        "projected reference move requires exact partial-holder state",
                    ));
                }
                if resolved.projections.is_empty() || tracked {
                    self.remove_storage_value(
                        site,
                        place,
                        SourceReferenceRemovalKindV29::Move,
                        &resolved,
                        budget,
                    )?;
                }
                self.invalidate_discriminant_values(
                    Some(resolved.instance),
                    Some(resolved.local),
                    Some(node),
                    budget,
                )?
                .ok_or_else(source_reference_enum_error_v29)
            }
        }
    }

    fn field(
        &self,
        node: usize,
        field: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        budget.charge_work(2)?;
        let SourceReferenceNodeKindV29::Aggregate { first, count } = self.plan.nodes[node].kind
        else {
            return Err(source_reference_error_v29(
                "source reference field lacks an aggregate origin",
            ));
        };
        if field >= count {
            return Err(source_reference_error_v29(
                "source reference field is out of range",
            ));
        }
        self.plan
            .children
            .get(argument_sum_v1(&[first, field])?)
            .copied()
            .ok_or_else(|| {
                source_reference_error_v29("source reference aggregate children are missing")
            })
    }

    fn read_place(
        &mut self,
        site: SourceReferenceSiteV29,
        place: &SemanticPlaceV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        let mut resolved =
            self.resolve_reference_place(site, place, SourceReferenceAccessV29::Read, budget)?;
        if let Some(loan) = resolved.loan {
            self.effect(loan, SourceReferenceEffectV29::ReadReferent, budget)?;
        }
        self.capture_atomic_root_pointer_v41(site, place, &mut resolved, budget)?;
        self.attach_storage_value(&resolved, budget)
    }

    fn array_projection_node(
        &mut self,
        node: usize,
        projection: SemanticProjectionV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        budget.charge_work(5)?;
        let types = self.plan.instances.owner().source_semantic().types();
        let SemanticTypeShapeV1::Array { element, length } =
            types[self.plan.nodes[node].ty.index() as usize].shape()
        else {
            return Err(source_reference_error_v29(
                "source reference index requires an exact array",
            ));
        };
        let (element, length) = (*element, *length);
        let SourceReferenceNodeKindV29::Aggregate { count, .. } = self.plan.nodes[node].kind else {
            return Err(source_reference_error_v29(
                "source reference array has no represented elements",
            ));
        };
        if projection.result_type() != element || count as u64 != length {
            return Err(source_reference_error_v29(
                "source reference array element roster differs",
            ));
        }
        if let SemanticProjectionKindV1::ConstantIndex {
            offset,
            minimum_length,
            from_end,
        } = projection.kind()
        {
            let selected = if from_end {
                length.checked_sub(offset)
            } else {
                Some(offset)
            };
            if minimum_length > length || selected.is_none_or(|index| index >= length) {
                return Err(source_reference_error_v29(
                    "source reference constant index is outside its array",
                ));
            }
            return self.field(
                node,
                usize::try_from(selected.unwrap()).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                budget,
            );
        }
        if !matches!(projection.kind(), SemanticProjectionKindV1::Index(_)) || count == 0 {
            return Err(source_reference_error_v29(
                "source reference array projection differs",
            ));
        }
        // A runtime index is not guessed. Join every possible element; distinct
        // loan identities remain an explicit runtime-storage obligation.
        let mut joined = self.field(node, 0, budget)?;
        for field in 1..count {
            budget.charge_work(1)?;
            let next = self.field(node, field, budget)?;
            joined = self.merge_node(joined, next, budget)?;
        }
        Ok(joined)
    }

    fn resolve_reference_place(
        &mut self,
        site: SourceReferenceSiteV29,
        place: &SemanticPlaceV1,
        access: SourceReferenceAccessV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<SourceReferencePlaceV29, ProductionSemanticKirErrorV1> {
        self.resolve_reference_place_recorded(site, place, access, true, budget)
    }

    fn resolve_reference_place_recorded(
        &mut self,
        site: SourceReferenceSiteV29,
        place: &SemanticPlaceV1,
        access: SourceReferenceAccessV29,
        original_place: bool,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<SourceReferencePlaceV29, ProductionSemanticKirErrorV1> {
        budget.charge_work(1)?;
        budget.reserve_storage(std::mem::size_of::<SourceReferencePlaceV29>())?;
        let mut raw_path = self.plan.raw_source_path(site, place, access, budget)?;
        let local = self.local(site.instance, place.local())?;
        let constructing = local.node.is_none()
            && access == SourceReferenceAccessV29::Write
            && !place.projections().is_empty();
        let node = match local.node {
            Some(node) => node,
            None if access == SourceReferenceAccessV29::Address => {
                self.storage_live_at(site.instance, place.local(), budget)?;
                let ty = self
                    .plan
                    .instances
                    .instance(site.instance)
                    .ok_or(ArgumentResourceV1::Accounting)?
                    .declaration()
                    .locals()
                    .get(place.local().index() as usize)
                    .ok_or(ArgumentResourceV1::Accounting)?
                    .ty();
                // This is an absent location marker, not an initialized value.
                self.node(ty, SourceReferenceNodeKindV29::Absent, budget)?
            }
            None if access == SourceReferenceAccessV29::Write && place.projections().is_empty() => {
                self.plain(place.ty(), budget)?
            }
            None if constructing => {
                budget.charge_work(argument_sum_v1(&[place.projections().len(), 3])?)?;
                if self.storage_root.is_none()
                    || local.storage.is_none()
                    || place.projections().iter().any(|projection| {
                        projection.kind() == SemanticProjectionKindV1::Dereference
                    })
                {
                    return Err(source_reference_error_v29(
                        "projected construction needs original local storage, not an undefined pointer",
                    ));
                }
                self.storage_live_at(site.instance, place.local(), budget)?;
                let ty = self
                    .plan
                    .instances
                    .instance(site.instance)
                    .and_then(|instance| {
                        instance
                            .declaration()
                            .locals()
                            .get(place.local().index() as usize)
                    })
                    .ok_or(ArgumentResourceV1::Accounting)?
                    .ty();
                let node = self.plain(ty, budget)?;
                // This installs a representation shell only. The original C2
                // snapshot still marks every untouched component uninitialized.
                self.set_local(
                    site.instance,
                    place.local(),
                    SourceReferenceLocalV29 {
                        node: Some(node),
                        ..local
                    },
                )?;
                node
            }
            None => {
                return Err(source_reference_error_v29(
                    "source reference reads a dead or undefined holder",
                ));
            }
        };
        let mut resolved = SourceReferencePlaceV29 {
            instance: site.instance,
            local: place.local(),
            generation: local.generation,
            value: node,
            representation_root: node,
            node,
            projections: Vec::new(),
            selector_source: None,
            loan: None,
            shared_path: false,
            traversed: Vec::new(),
            anchor: match self.plan.nodes[node].kind {
                SourceReferenceNodeKindV29::Plain(anchor) => anchor,
                _ => None,
            },
        };
        for (projection_ordinal, projection) in place.projections().iter().enumerate() {
            budget.charge_work(2)?;
            let crossing_access = if raw_path
                .as_ref()
                .is_some_and(|path| projection_ordinal < path.last_dereference)
            {
                SourceReferenceAccessV29::Read
            } else {
                access
            };
            self.retain_descriptor_value(site, place, projection_ordinal, resolved.node, budget)?;
            if let SemanticProjectionKindV1::Index(index) = projection.kind() {
                if self.storage_root.is_some() {
                    let context = (site, place as *const SemanticPlaceV1 as usize);
                    let key = (
                        site.instance.index(),
                        site.block.index(),
                        site.statement,
                        context.1,
                        projection_ordinal,
                    );
                    charge_execution_cfg_lookup_v29(self.plan.selector_sites.len(), budget)?;
                    if self.plan.selector_sites.contains_key(&key) {
                        self.plan
                            .selector_for_path(Some(context), projection_ordinal, budget)?;
                        if matches!(access, SourceReferenceAccessV29::Borrow(_))
                            || resolved.loan.is_some()
                        {
                            return Err(source_reference_error_v29(
                                "source selected reference origin needs correlated index transport",
                            ));
                        }
                        resolved.selector_source = Some(context);
                    }
                }
                let ty = self
                    .plan
                    .instances
                    .instance(site.instance)
                    .ok_or(ArgumentResourceV1::Accounting)?
                    .declaration()
                    .locals()[index.index() as usize]
                    .ty();
                // This empty projection has no backing allocation. Prepay its
                // value, constructor return envelopes, and conversion headers.
                budget.charge_work(6)?;
                budget.reserve_storage(argument_sum_v1(&[
                    std::mem::size_of::<SemanticPlaceV1>(),
                    argument_product_v1(
                        2,
                        std::mem::size_of::<
                            Result<
                                SemanticPlaceV1,
                                fe2o3_mir_model::semantic_mir_v1::SemanticMirErrorV1,
                            >,
                        >(),
                    )?,
                    argument_product_v1(2, std::mem::size_of::<Vec<SemanticProjectionV1>>())?,
                    std::mem::size_of::<Box<[SemanticProjectionV1]>>(),
                ])?)?;
                let index_place = match SemanticPlaceV1::new(index, Vec::new(), ty) {
                    Ok(place) => place,
                    Err(_) => return Err(ArgumentResourceV1::Accounting.into()),
                };
                // The temporary whole-local place is only an evaluator input.
                // Retain the original parent projection/event, not its address.
                let index_value = self.resolve_reference_place_recorded(
                    site,
                    &index_place,
                    SourceReferenceAccessV29::Read,
                    false,
                    budget,
                )?;
                if let Some(loan) = index_value.loan {
                    self.effect(loan, SourceReferenceEffectV29::ReadReferent, budget)?;
                }
                self.attach_storage_value(&index_value, budget)?;
                self.retain_selector_generation(
                    site,
                    place,
                    projection_ordinal,
                    &index_value,
                    budget,
                )?;
            }
            if projection.kind() == SemanticProjectionKindV1::Dereference
                && self.plan.nodes[resolved.node].atomic_custody.is_some()
            {
                let fact = self
                    .plan
                    .atomic_custody_v41(resolved.node, budget)?
                    .ok_or(ArgumentResourceV1::Accounting)?;
                self.check_atomic_custody_v41(fact, budget)?;
                // Generic/RW is a physical carrier, never ordinary permission,
                // including before a view and through copied or moved aliases.
                return Err(source_atomic_view_error_v41());
            }
            match (projection.kind(), self.plan.nodes[resolved.node].kind) {
                (
                    SemanticProjectionKindV1::Downcast(variant),
                    SourceReferenceNodeKindV29::Plain(_),
                ) => {
                    if constructing
                        && !self.mutate_storage_place(
                            &resolved,
                            source_storage_v29::SourceStorageRootMutationV29::BeginVariant(variant),
                            budget,
                        )?
                    {
                        return Err(source_reference_enum_error_v29());
                    }
                    let source = self.expand_plain_enum(resolved.node, budget)?;
                    resolved.node = self.project_enum_value(source, *projection, budget)?;
                    resolved.anchor = None;
                    emission_push_v1(&mut resolved.projections, *projection, budget)?;
                }
                (
                    SemanticProjectionKindV1::Downcast(_),
                    SourceReferenceNodeKindV29::Enum { .. },
                )
                | (
                    SemanticProjectionKindV1::Field(_)
                    | SemanticProjectionKindV1::Downcast(_)
                    | SemanticProjectionKindV1::ConstantIndex { .. }
                    | SemanticProjectionKindV1::OpaqueCast
                    | SemanticProjectionKindV1::Subtype,
                    SourceReferenceNodeKindV29::EnumView(_),
                ) => {
                    resolved.node = self.project_enum_value(resolved.node, *projection, budget)?;
                    resolved.anchor = None;
                    emission_push_v1(&mut resolved.projections, *projection, budget)?;
                }
                (
                    SemanticProjectionKindV1::Dereference,
                    SourceReferenceNodeKindV29::Address(set),
                ) => {
                    let path = raw_path.as_ref().ok_or(ArgumentResourceV1::Accounting)?;
                    let holder = self.retain_raw_holder(&resolved, set, budget)?;
                    self.resolve_raw_target(
                        &mut resolved,
                        set,
                        projection.result_type(),
                        crossing_access,
                        budget,
                    )?;
                    self.retain_raw_access(
                        site,
                        place,
                        access,
                        path,
                        projection_ordinal,
                        set,
                        holder,
                        budget,
                    )?;
                    raw_path
                        .as_mut()
                        .ok_or(ArgumentResourceV1::Accounting)?
                        .crossed = true;
                }
                (SemanticProjectionKindV1::Field(_), SourceReferenceNodeKindV29::Absent)
                    if access == SourceReferenceAccessV29::Address =>
                {
                    resolved.node = self.node(
                        projection.result_type(),
                        SourceReferenceNodeKindV29::Absent,
                        budget,
                    )?;
                    resolved.anchor = None;
                    emission_push_v1(&mut resolved.projections, *projection, budget)?;
                }
                (
                    SemanticProjectionKindV1::Field(field),
                    SourceReferenceNodeKindV29::Aggregate { .. },
                ) => {
                    resolved.node = self.field(resolved.node, field as usize, budget)?;
                    resolved.anchor = None;
                    emission_push_v1(&mut resolved.projections, *projection, budget)?;
                }
                (
                    SemanticProjectionKindV1::ConstantIndex { .. }
                    | SemanticProjectionKindV1::Index(_),
                    SourceReferenceNodeKindV29::Aggregate { .. },
                ) => {
                    resolved.node =
                        self.array_projection_node(resolved.node, *projection, budget)?;
                    resolved.anchor = None;
                    emission_push_v1(&mut resolved.projections, *projection, budget)?;
                }
                (SemanticProjectionKindV1::Dereference, SourceReferenceNodeKindV29::Loan(loan)) => {
                    self.check_storage_read(&resolved, budget)?;
                    if self.plan.storage == SourceReferenceStorageV29::ScalarCells {
                        emission_push_v1(&mut resolved.traversed, loan, budget)?;
                    }
                    let holder_access = if crossing_access == SourceReferenceAccessV29::Write
                        || crossing_access
                            == SourceReferenceAccessV29::Borrow(SemanticBorrowKindV1::Mutable)
                    {
                        SourceReferenceAccessV29::Write
                    } else {
                        SourceReferenceAccessV29::Read
                    };
                    self.check_place_access(&resolved, holder_access, budget)?;
                    self.check_loan_use(loan, budget)?;
                    let shared = self.plan.loans[loan].kind != SemanticBorrowKindV1::Mutable;
                    if (shared || (raw_path.is_some() && resolved.shared_path))
                        && (crossing_access == SourceReferenceAccessV29::Write
                            || crossing_access
                                == SourceReferenceAccessV29::Borrow(SemanticBorrowKindV1::Mutable))
                    {
                        return Err(source_reference_error_v29(
                            "source reference traversal widens mutability",
                        ));
                    }
                    resolved.shared_path |= shared;
                    // Crossing another reference reads the intermediate holder,
                    // then resets the physical target to the inner loan's origin.
                    if let Some(previous) = resolved.loan {
                        self.effect(previous, SourceReferenceEffectV29::ReadReferent, budget)?;
                    }
                    let origin = &self.plan.origins[self.plan.loans[loan].origin];
                    resolved.instance = origin.instance;
                    resolved.local = origin.local;
                    resolved.generation = origin.generation;
                    resolved.value = origin.value;
                    resolved.anchor = origin.anchor;
                    budget.charge_work(2)?;
                    resolved.representation_root = self
                        .local(origin.instance, origin.local)?
                        .node
                        .ok_or_else(|| {
                            source_reference_error_v29(
                                "source reference referent has no current representation",
                            )
                        })?;
                    resolved.projections.clear();
                    resolved.selector_source = None;
                    for index in origin.projections.clone() {
                        emission_push_v1(
                            &mut resolved.projections,
                            self.plan.projections[index],
                            budget,
                        )?;
                    }
                    resolved.node = self.referent_node(loan, budget)?;
                    resolved.loan = Some(loan);
                }
                (_, SourceReferenceNodeKindV29::Plain(_)) => {
                    if projection.kind() == SemanticProjectionKindV1::Dereference
                        && raw_path.as_ref().is_some_and(|path| path.crossed)
                    {
                        return Err(source_reference_error_v29(
                            "source raw suffix lost its retained intermediate pointer",
                        ));
                    }
                    if projection.kind() == SemanticProjectionKindV1::Dereference
                        && access == SourceReferenceAccessV29::Write
                        && projection_ordinal + 2 == place.projections().len()
                    {
                        let key = (
                            site.instance.index(),
                            site.block.index(),
                            site.statement,
                            place as *const SemanticPlaceV1 as usize,
                            projection_ordinal + 1,
                        );
                        charge_execution_cfg_lookup_v29(self.plan.descriptor_sites.len(), budget)?;
                        if let Some(&descriptor) = self.plan.descriptor_sites.get(&key) {
                            self.plan.descriptors[descriptor].check(self.plan.instances, budget)?;
                            // The external write reads the descriptor holder, not
                            // the selected referent's local initialization state.
                            self.check_storage_read(&resolved, budget)?;
                        }
                    }
                    if projection.kind() == SemanticProjectionKindV1::Dereference
                        && (matches!(
                            access,
                            SourceReferenceAccessV29::Borrow(_) | SourceReferenceAccessV29::Address
                        ) || resolved.loan.is_some())
                    {
                        return Err(source_reference_error_v29(
                            "source reference dereference has no checked origin",
                        ));
                    }
                    if !matches!(projection.kind(), SemanticProjectionKindV1::Field(_))
                        && (matches!(access, SourceReferenceAccessV29::Borrow(_))
                            || resolved.loan.is_some())
                    {
                        return Err(source_reference_error_v29(
                            "source reference projection requires addressable place resolution",
                        ));
                    }
                    resolved.node = self.plain(projection.result_type(), budget)?;
                    resolved.anchor = None;
                    emission_push_v1(&mut resolved.projections, *projection, budget)?;
                }
                _ => {
                    return Err(source_reference_error_v29(
                        "source reference projection is unsupported",
                    ));
                }
            }
            if self.plan.nodes[resolved.node].ty != projection.result_type() {
                return Err(source_reference_error_v29(
                    "source reference projection type differs",
                ));
            }
        }
        if self.plan.nodes[resolved.node].ty != place.ty() {
            return Err(source_reference_error_v29(
                "source reference place type differs",
            ));
        }
        if access == SourceReferenceAccessV29::ReadDiscriminant {
            self.check_storage_discriminant(&resolved, budget)?;
        } else if !matches!(
            access,
            SourceReferenceAccessV29::Write | SourceReferenceAccessV29::Address
        ) {
            self.check_storage_read(&resolved, budget)?;
            if self.plan.nodes[resolved.node].kind == SourceReferenceNodeKindV29::Absent {
                return Err(source_reference_error_v29(
                    "source reference reads an absent holder leaf",
                ));
            }
        }
        if !matches!(
            access,
            SourceReferenceAccessV29::Borrow(_) | SourceReferenceAccessV29::Address
        ) {
            self.check_place_access(&resolved, access, budget)?;
        }
        if original_place && access == SourceReferenceAccessV29::Read {
            self.retain_descriptor_value(
                site,
                place,
                place.projections().len(),
                resolved.node,
                budget,
            )?;
        }
        if access == SourceReferenceAccessV29::Read && resolved.shared_path {
            for loan in self.node_loans(resolved.node, budget)? {
                if self.plan.loans[loan].kind == SemanticBorrowKindV1::Mutable {
                    return Err(source_reference_error_v29(
                        "shared source reference cannot extract mutable loan holder",
                    ));
                }
            }
        }
        if original_place {
            self.retain_reference_access(site, place, access, &resolved, budget)?;
        }
        Ok(resolved)
    }

    fn referent_node(
        &mut self,
        loan: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        let origin = &self.plan.origins[self.plan.loans[loan].origin];
        let range = origin.projections.clone();
        let ty = origin.ty;
        let node = self
            .local(origin.instance, origin.local)?
            .node
            .ok_or_else(|| {
                source_reference_error_v29("source reference referent has no live value")
            })?;
        self.current_projection_node(node, range, ty, budget)
    }

    fn assign(
        &mut self,
        site: SourceReferenceSiteV29,
        place: &SemanticPlaceV1,
        node: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(1)?;
        if self.plan.nodes[node].ty != place.ty() {
            return Err(source_reference_error_v29(
                "source reference assignment type differs",
            ));
        }
        let target = self.write_place(site, place, budget)?;
        self.retain_storage_representation(site, place, &target, node, budget)?;
        self.transfer_storage_value(&target, node, budget)?;
        self.install_assigned_node(&target, node, budget)?;
        self.invalidate_discriminant_values(
            Some(target.instance),
            Some(target.local),
            None,
            budget,
        )?;
        Ok(())
    }

    fn install_assigned_node(
        &mut self,
        target: &SourceReferencePlaceV29,
        node: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let mut local = self.local(target.instance, target.local)?;
        if target.projections.is_empty() {
            local.node = Some(node);
            self.set_local(target.instance, target.local, local)
        } else if target.projections.len() == 1
            && matches!(
                target.projections[0].kind(),
                SemanticProjectionKindV1::ConstantIndex { .. }
            )
            && self.install_plain_array_element_v29(target, node, budget)?
        {
            Ok(())
        } else if self.install_nested_static_components_v43(target, node, budget)? {
            Ok(())
        } else if target.loan.is_some()
            || self.contains_loan(node, budget)?
            || match local.node {
                Some(previous) => {
                    budget.charge_work(argument_product_v1(2, target.projections.len())?)?;
                    let static_path = target.projections.iter().all(|projection| {
                        matches!(
                            projection.kind(),
                            SemanticProjectionKindV1::Field(_)
                                | SemanticProjectionKindV1::Downcast(_)
                                | SemanticProjectionKindV1::ConstantIndex { .. }
                        )
                    });
                    self.contains_loan(previous, budget)?
                        || (static_path
                            && matches!(
                                self.plan.nodes[previous].kind,
                                SourceReferenceNodeKindV29::Aggregate { .. }
                                    | SourceReferenceNodeKindV29::Enum { .. }
                                    | SourceReferenceNodeKindV29::EnumView(_)
                            ))
                        || (target.projections.iter().all(|projection| {
                            matches!(projection.kind(), SemanticProjectionKindV1::Field(_))
                        }) && self
                            .ordinary_reference_field_count(previous, budget)?
                            .is_some())
                }
                None => false,
            }
        {
            self.replace_reference_fields(target, node, budget)
        } else {
            if let Some(previous) = local.node {
                if !target.projections.first().is_some_and(|projection| {
                    projection.kind() == SemanticProjectionKindV1::Dereference
                }) {
                    // A metadata-field write invalidates a whole-argument anchor.
                    local.node = Some(self.plain(self.plan.nodes[previous].ty, budget)?);
                    self.set_local(target.instance, target.local, local)?;
                }
            }
            Ok(())
        }
    }

    fn replace_reference_fields(
        &mut self,
        target: &SourceReferencePlaceV29,
        mut replacement: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let mut local = self.local(target.instance, target.local)?;
        let mut current = local.node.ok_or_else(|| {
            source_reference_error_v29("source reference assignment target is undefined")
        })?;
        budget.charge_work(argument_sum_v1(&[target.projections.len(), 3])?)?;
        if local.generation != target.generation || current != target.representation_root {
            return Err(source_reference_error_v29(
                "source reference assignment current representation changed",
            ));
        }
        if target
            .projections
            .iter()
            .any(|projection| matches!(projection.kind(), SemanticProjectionKindV1::Downcast(_)))
        {
            let changed = self.replace_enum_path(
                current,
                &target.projections,
                target.node,
                replacement,
                0,
                budget,
            )?;
            local.node = Some(changed);
            return self.set_local(target.instance, target.local, local);
        }
        let mut parents = source_reference_scratch_v29(target.projections.len(), budget)?;
        for (depth, projection) in target.projections.iter().enumerate() {
            budget.charge_work(1)?;
            let field = match projection.kind() {
                SemanticProjectionKindV1::Field(field) => field as usize,
                SemanticProjectionKindV1::ConstantIndex {
                    offset, from_end, ..
                } => {
                    self.array_projection_node(current, *projection, budget)?;
                    let types = self.plan.instances.owner().source_semantic().types();
                    let SemanticTypeShapeV1::Array { length, .. } =
                        types[self.plan.nodes[current].ty.index() as usize].shape()
                    else {
                        return Err(ArgumentResourceV1::Accounting.into());
                    };
                    usize::try_from(if from_end {
                        length
                            .checked_sub(offset)
                            .ok_or(ArgumentResourceV1::Accounting)?
                    } else {
                        offset
                    })
                    .map_err(|_| ArgumentResourceV1::Arithmetic)?
                }
                _ => {
                    return Err(source_reference_error_v29(
                        "projected reference assignment requires exact cell state",
                    ));
                }
            };
            parents.push((current, field));
            current = if matches!(
                self.plan.nodes[current].kind,
                SourceReferenceNodeKindV29::Plain(_)
            ) {
                let declared = self.ordinary_reference_field_type(current, field, budget)?;
                if declared != projection.result_type()
                    || !matches!(projection.kind(), SemanticProjectionKindV1::Field(_))
                {
                    return Err(source_reference_error_v29(
                        "source reference plain field path differs from its declaration",
                    ));
                }
                if depth + 1 == target.projections.len() {
                    let selected = self
                        .plan
                        .nodes
                        .get(target.node)
                        .ok_or(ArgumentResourceV1::Accounting)?;
                    if selected.ty != declared
                        || selected.kind != SourceReferenceNodeKindV29::Plain(None)
                        || selected.storage.is_some()
                        || selected.inactive.is_some()
                    {
                        return Err(source_reference_error_v29(
                            "source reference plain field lost its resolved leaf",
                        ));
                    }
                    target.node
                } else {
                    self.plain(declared, budget)?
                }
            } else {
                self.field(current, field, budget)?
            };
            if self.plan.nodes[current].ty != projection.result_type() {
                return Err(source_reference_error_v29(
                    "source reference assignment field type differs",
                ));
            }
        }
        if current != target.node || self.plan.nodes[current].ty != self.plan.nodes[replacement].ty
        {
            return Err(source_reference_error_v29(
                "source reference assignment target changed during resolution",
            ));
        }
        // Nodes are immutable snapshots shared by copies and CFG entries. Rebuild
        // only this physical holder path; live aliases resolve its current local.
        while let Some((parent, field)) = parents.pop() {
            budget.charge_work(1)?;
            let (first, count) = match self.plan.nodes[parent].kind {
                SourceReferenceNodeKindV29::Aggregate { first, count } => (Some(first), count),
                SourceReferenceNodeKindV29::Plain(_) => (
                    None,
                    self.ordinary_reference_field_count(parent, budget)?
                        .ok_or_else(|| {
                            source_reference_error_v29(
                                "projected reference assignment lacks an ordinary field roster",
                            )
                        })?,
                ),
                _ => {
                    return Err(source_reference_error_v29(
                        "projected reference assignment lacks represented aggregate state",
                    ));
                }
            };
            let start = self.plan.children.len();
            for index in 0..count {
                budget.charge_work(1)?;
                let child = if index == field {
                    replacement
                } else if let Some(first) = first {
                    self.plan.children[argument_sum_v1(&[first, index])?]
                } else {
                    let ty = self.ordinary_reference_field_type(parent, index, budget)?;
                    self.plain(ty, budget)?
                };
                emission_push_v1(&mut self.plan.children, child, budget)?;
            }
            replacement = self.node(
                self.plan.nodes[parent].ty,
                SourceReferenceNodeKindV29::Aggregate {
                    first: start,
                    count,
                },
                budget,
            )?;
        }
        local.node = Some(replacement);
        self.set_local(target.instance, target.local, local)
    }

    fn ordinary_reference_field_count(
        &self,
        node: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<usize>, ProductionSemanticKirErrorV1> {
        budget.charge_work(3)?;
        let node = self
            .plan
            .nodes
            .get(node)
            .ok_or(ArgumentResourceV1::Accounting)?;
        let ty = self
            .plan
            .instances
            .owner()
            .source_semantic()
            .types()
            .get(node.ty.index() as usize)
            .ok_or(ArgumentResourceV1::Accounting)?;
        Ok(match ty.shape() {
            SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields) => {
                Some(fields.fields().len())
            }
            _ => None,
        })
    }

    fn ordinary_reference_field_type(
        &self,
        node: usize,
        field: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<SemanticTypeIdV1, ProductionSemanticKirErrorV1> {
        budget.charge_work(4)?;
        let node = self
            .plan
            .nodes
            .get(node)
            .ok_or(ArgumentResourceV1::Accounting)?;
        let ty = self
            .plan
            .instances
            .owner()
            .source_semantic()
            .types()
            .get(node.ty.index() as usize)
            .ok_or(ArgumentResourceV1::Accounting)?;
        let (SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields)) =
            ty.shape()
        else {
            return Err(source_reference_error_v29(
                "source reference plain value has no ordinary field roster",
            ));
        };
        fields.fields().get(field).copied().ok_or_else(|| {
            source_reference_error_v29("source reference ordinary field is out of range")
        })
    }

    fn expand_plain_reference_fields(
        &mut self,
        original: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        let node = *self
            .plan
            .nodes
            .get(original)
            .ok_or(ArgumentResourceV1::Accounting)?;
        if !matches!(node.kind, SourceReferenceNodeKindV29::Plain(_)) || node.inactive.is_some() {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        let count = self
            .ordinary_reference_field_count(original, budget)?
            .ok_or_else(|| {
                source_reference_error_v29("source reference merge has no ordinary field roster")
            })?;
        let first = self.plan.children.len();
        for field in 0..count {
            let ty = self.ordinary_reference_field_type(original, field, budget)?;
            let child = self.plain(ty, budget)?;
            emission_push_v1(&mut self.plan.children, child, budget)?;
        }
        self.node(
            node.ty,
            SourceReferenceNodeKindV29::Aggregate { first, count },
            budget,
        )
    }

    fn terminator(
        &mut self,
        site: SourceReferenceSiteV29,
        terminator: &SemanticTerminatorKindV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<SourceReferenceTerminatorOutcomeV29, ProductionSemanticKirErrorV1> {
        match terminator {
            SemanticTerminatorKindV1::Call(call) => {
                if !matches!(call.unwind(), SemanticUnwindActionV1::Unreachable) {
                    return Err(source_reference_error_v29(
                        "source reference call unwind requires effect transport",
                    ));
                }
                let instances = self.plan.instances;
                let calls = instances.calls(site.instance).ok_or_else(|| {
                    source_reference_error_v29("source reference call instance roster is missing")
                })?;
                budget.charge_work(calls.len())?;
                let exact = calls
                    .iter()
                    .find(|row| row.occurrence().block == site.block)
                    .ok_or_else(|| {
                        source_reference_error_v29("source reference call is outside its instance")
                    })?;
                if !std::ptr::eq(exact.source(), call) {
                    return Err(source_reference_error_v29(
                        "source reference call source differs",
                    ));
                }
                if exact.child().is_none() && call.destination().is_some() {
                    self.assign_constructed(
                        site,
                        SourceReferenceConstructionV29::Intrinsic(exact),
                        budget,
                    )?;
                    return Ok(SourceReferenceTerminatorOutcomeV29::Continue);
                }
                let mut arguments = source_reference_scratch_v29(call.arguments().len(), budget)?;
                for (ordinal, operand) in call.arguments().iter().enumerate() {
                    let node = self.operand(site, operand, budget)?;
                    self.retain_boundary_value(
                        site,
                        SourceReferenceBoundaryRoleV29::Argument(
                            u32::try_from(ordinal).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                        ),
                        node,
                        budget,
                    )?;
                    arguments.push(node);
                }
                let outcome = if let Some(child) = exact.child() {
                    self.function(child, Some(&arguments), budget)?
                } else {
                    SourceReferenceFunctionOutcomeV29::Returned(self.intrinsic(
                        exact.callable(),
                        &arguments,
                        call,
                        budget,
                    )?)
                };
                if let SourceReferenceFunctionOutcomeV29::Returned(node) = outcome
                    && let Some(destination) = call.destination()
                {
                    self.assign(site, destination.place(), node, budget)?;
                }
                Ok(SourceReferenceTerminatorOutcomeV29::Continue)
            }
            SemanticTerminatorKindV1::Return => {
                let declaration = self
                    .plan
                    .instances
                    .instance(site.instance)
                    .ok_or_else(|| {
                        source_reference_error_v29("source reference return instance is missing")
                    })?
                    .declaration();
                budget.charge_work(declaration.locals().len())?;
                let local = declaration
                    .locals()
                    .iter()
                    .position(|local| local.role() == SemanticLocalRoleV1::Return)
                    .ok_or_else(|| {
                        source_reference_error_v29("source reference return local is missing")
                    })?;
                let return_local = SemanticLocalIdV1::from_index(
                    u32::try_from(local).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                );
                let value = self.local(site.instance, return_local)?.node;
                let node = if let Some(node) = value {
                    self.return_storage_value(site.instance, return_local, node, budget)?
                } else {
                    let ty = declaration.locals()[local].ty();
                    let types = self.plan.instances.owner().source_semantic().types();
                    let exact = &types[ty.index() as usize];
                    budget.charge_work(6)?;
                    if ty != declaration.abi().source_output_type()
                        || !matches!(exact.shape(), SemanticTypeShapeV1::Unit)
                        || exact.layout().size_bytes() != Some(0)
                        || exact.layout().is_uninhabited()
                    {
                        return Err(source_reference_error_v29(
                            "source reference return is undefined",
                        ));
                    }
                    require_ordinary_execution_representation_v29(exact)?;
                    // Ordinary Unit has one value and no bytes or authority.
                    // Rust MIR need not assign its return local explicitly.
                    self.plain(ty, budget)?
                };
                self.check_frame_exit(site.instance, budget)?;
                let node = self
                    .expire_addresses(site.instance, None, Some(node), budget)?
                    .ok_or(ArgumentResourceV1::Accounting)?;
                let node = self
                    .invalidate_discriminant_values(Some(site.instance), None, Some(node), budget)?
                    .ok_or(ArgumentResourceV1::Accounting)?;
                self.retain_reference_return(site, node, budget)?;
                self.retain_boundary_value(
                    site,
                    SourceReferenceBoundaryRoleV29::Return,
                    node,
                    budget,
                )?;
                Ok(SourceReferenceTerminatorOutcomeV29::Returned(node))
            }
            SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => {
                let node = self.operand(site, discriminant, budget)?;
                self.observe_node_address(node, budget)?;
                Ok(SourceReferenceTerminatorOutcomeV29::Switch(node))
            }
            SemanticTerminatorKindV1::Assert {
                condition,
                message,
                unwind,
                ..
            } => {
                if !matches!(unwind, SemanticUnwindActionV1::Unreachable) {
                    return Err(source_reference_error_v29(
                        "source reference assertion unwind requires effect transport",
                    ));
                }
                let condition = self.operand(site, condition, budget)?;
                self.observe_node_address(condition, budget)?;
                self.assertion_failure_effects(site, message, budget)?;
                Ok(SourceReferenceTerminatorOutcomeV29::Continue)
            }
            SemanticTerminatorKindV1::Abort | SemanticTerminatorKindV1::UnwindTerminate => {
                // The authenticated CFG has no successor on these paths. Scope
                // closure and the trap remain obligations of terminal replay.
                Ok(SourceReferenceTerminatorOutcomeV29::Continue)
            }
            SemanticTerminatorKindV1::Goto(_) | SemanticTerminatorKindV1::Unreachable => {
                Ok(SourceReferenceTerminatorOutcomeV29::Continue)
            }
            _ => Err(source_reference_error_v29(
                "source reference terminator effect is unsupported",
            )),
        }
    }

    fn merge_state(
        &mut self,
        into: usize,
        other: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let len = self.plan.states[into].len();
        if len != self.plan.states[other].len() {
            return Err(source_reference_error_v29(
                "source reference CFG local rosters differ",
            ));
        }
        for local in 0..len {
            budget.charge_work(1)?;
            let left = self.plan.states[into][local];
            let right = self.plan.states[other][local];
            if left.generation != right.generation {
                return Err(source_reference_error_v29(
                    "source reference CFG storage generations differ",
                ));
            }
            let node = match (left.node, right.node) {
                (Some(left), Some(right)) => Some(self.merge_node(left, right, budget)?),
                (None, None) => None,
                (Some(node), None) | (None, Some(node)) => {
                    if self.contains_loan(node, budget)? {
                        return Err(source_reference_error_v29(
                            "source reference CFG holder liveness differs",
                        ));
                    }
                    None
                }
            };
            self.plan.states[into][local].node = node;
        }
        Ok(())
    }

    fn merge_node(
        &mut self,
        left: usize,
        right: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        self.merge_node_at_depth(left, right, 0, budget)
    }

    fn merge_node_at_depth(
        &mut self,
        left: usize,
        right: usize,
        depth: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        budget.charge_work(2)?;
        if depth >= 256 {
            return Err(source_reference_error_v29(
                "source reference merge depth exceeds the checked bound",
            ));
        }
        if left == right {
            return Ok(left);
        }
        if self.nodes_equal(left, right, depth, budget)? {
            return Ok(left);
        }
        let a = self.plan.nodes[left];
        let b = self.plan.nodes[right];
        if a.ty != b.ty {
            return Err(source_reference_error_v29(
                "source reference CFG value types differ",
            ));
        }
        if a.atomic_custody != b.atomic_custody {
            return Err(source_atomic_view_error_v41());
        }
        match (a.kind, b.kind) {
            (
                SourceReferenceNodeKindV29::EnumView(left),
                SourceReferenceNodeKindV29::EnumView(right),
            ) => self.merge_enum_views(left, right, budget),
            (SourceReferenceNodeKindV29::Enum { .. }, SourceReferenceNodeKindV29::Plain(_)) => {
                let expanded = self.expand_plain_enum(right, budget)?;
                self.merge_enum_values(left, expanded, budget)
            }
            (SourceReferenceNodeKindV29::Plain(_), SourceReferenceNodeKindV29::Enum { .. }) => {
                let expanded = self.expand_plain_enum(left, budget)?;
                self.merge_enum_values(expanded, right, budget)
            }
            (
                SourceReferenceNodeKindV29::Discriminant(_),
                SourceReferenceNodeKindV29::Discriminant(_) | SourceReferenceNodeKindV29::Plain(_),
            )
            | (SourceReferenceNodeKindV29::Plain(_), SourceReferenceNodeKindV29::Discriminant(_)) => {
                self.plain(a.ty, budget)
            }
            (SourceReferenceNodeKindV29::Enum { .. }, SourceReferenceNodeKindV29::Enum { .. }) => {
                self.merge_enum_values(left, right, budget)
            }
            (
                SourceReferenceNodeKindV29::Plain(_),
                SourceReferenceNodeKindV29::Aggregate { .. },
            ) if self.ordinary_reference_field_count(left, budget)?.is_some() => {
                let expanded = self.expand_plain_reference_fields(left, budget)?;
                self.merge_node_at_depth(expanded, right, depth, budget)
            }
            (
                SourceReferenceNodeKindV29::Aggregate { .. },
                SourceReferenceNodeKindV29::Plain(_),
            ) if self
                .ordinary_reference_field_count(right, budget)?
                .is_some() =>
            {
                let expanded = self.expand_plain_reference_fields(right, budget)?;
                self.merge_node_at_depth(left, expanded, depth, budget)
            }
            (
                SourceReferenceNodeKindV29::Plain(_),
                SourceReferenceNodeKindV29::Aggregate { .. },
            ) => {
                let expanded = self.expand_plain_scalar_array_for_merge_v29(left, right, budget)?;
                self.merge_node_at_depth(expanded, right, depth, budget)
            }
            (
                SourceReferenceNodeKindV29::Aggregate { .. },
                SourceReferenceNodeKindV29::Plain(_),
            ) => {
                let expanded = self.expand_plain_scalar_array_for_merge_v29(right, left, budget)?;
                self.merge_node_at_depth(left, expanded, depth, budget)
            }
            (SourceReferenceNodeKindV29::Absent, SourceReferenceNodeKindV29::Absent) => {
                self.merge_inactive_nodes(left, right, budget)
            }
            (SourceReferenceNodeKindV29::Absent, _) if self.storage_root.is_some() => Ok(right),
            (_, SourceReferenceNodeKindV29::Absent) if self.storage_root.is_some() => Ok(left),
            (SourceReferenceNodeKindV29::Plain(a), SourceReferenceNodeKindV29::Plain(b)) => {
                let descriptor = self.merge_descriptor_facts(
                    self.plan.nodes[left],
                    self.plan.nodes[right],
                    budget,
                )?;
                let node = self.node(
                    self.plan.nodes[left].ty,
                    SourceReferenceNodeKindV29::Plain(if a == b { a } else { None }),
                    budget,
                )?;
                self.plan.nodes[node].descriptor = descriptor;
                self.plan.nodes[node].atomic_custody = self.plan.nodes[left].atomic_custody;
                Ok(node)
            }
            (SourceReferenceNodeKindV29::Address(a), SourceReferenceNodeKindV29::Address(b)) => {
                let a = *self
                    .plan
                    .raw_sets
                    .get(a)
                    .ok_or(ArgumentResourceV1::Accounting)?;
                let b = *self
                    .plan
                    .raw_sets
                    .get(b)
                    .ok_or(ArgumentResourceV1::Accounting)?;
                let set = self.raw_set(
                    SourceReferenceRawChoicesV29::Retained(a),
                    SourceReferenceRawChoicesV29::Retained(b),
                    false,
                    budget,
                )?;
                self.raw_node(self.plan.nodes[left].ty, set, budget)
            }
            (SourceReferenceNodeKindV29::Loan(a), SourceReferenceNodeKindV29::Loan(b))
                if a == b =>
            {
                Ok(left)
            }
            (
                SourceReferenceNodeKindV29::Aggregate { first: a, count },
                SourceReferenceNodeKindV29::Aggregate {
                    first: b,
                    count: other,
                },
            ) if count == other => {
                let mut children = source_reference_scratch_v29(count, budget)?;
                for field in 0..count {
                    let left = self.plan.children[argument_sum_v1(&[a, field])?];
                    let right = self.plan.children[argument_sum_v1(&[b, field])?];
                    children.push(self.merge_node_at_depth(left, right, depth + 1, budget)?);
                }
                let first = self.plan.children.len();
                for &child in &children {
                    emission_push_v1(&mut self.plan.children, child, budget)?;
                }
                self.node(
                    self.plan.nodes[left].ty,
                    SourceReferenceNodeKindV29::Aggregate { first, count },
                    budget,
                )
            }
            _ => Err(source_reference_error_v29(
                "source reference CFG merge changes loan identity",
            )),
        }
    }
}
