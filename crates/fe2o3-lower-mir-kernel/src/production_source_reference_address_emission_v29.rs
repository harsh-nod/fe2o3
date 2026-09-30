// An actual address producer receipt is a locator. Admission independently checks
// this sequence on the final graph, all uses, source recipes and storage history.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceRawFormationReceiptV29 {
    site: SourceReferenceSiteV29,
    source: usize,
    base: ValueId,
    result: ValueId,
    block: BlockId,
    first: usize,
    end: usize,
}

fn source_raw_physical_error_v29() -> ProductionSemanticKirErrorV1 {
    source_reference_error_v29(
        "source raw address differs from its actual formation or memory history",
    )
}

impl SourceReferenceEmissionV29<'_, '_> {
    fn check_raw_formation_source_v29(
        &self,
        site: SourceReferenceSiteV29,
        source: &SemanticPlaceV1,
        result_type: SemanticTypeIdV1,
        mutability: SemanticMutabilityV1,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.check(budget)?;
        let statement = site.statement.ok_or_else(source_raw_physical_error_v29)?;
        let original = self
            .plan
            .instances
            .instance(site.instance)
            .and_then(|row| row.declaration().blocks().get(site.block.index() as usize))
            .and_then(|block| block.statements().get(statement))
            .ok_or_else(source_raw_physical_error_v29)?;
        budget.source_reference_charge_v29(self.plan, 9)?;
        let SemanticStatementKindV1::Assign(assign) = original.kind() else {
            return Err(source_raw_physical_error_v29());
        };
        let SemanticRvalueKindV1::AddressOf {
            place,
            mutability: actual,
        } = assign.value().kind()
        else {
            return Err(source_raw_physical_error_v29());
        };
        if !std::ptr::eq(place, source)
            || *actual != mutability
            || assign.value().result_type() != result_type
        {
            return Err(source_raw_physical_error_v29());
        }
        let types = self.plan.instances.owner().source_semantic().types();
        let Some(SemanticTypeShapeV1::Pointer(pointer)) = types
            .get(result_type.index() as usize)
            .map(SemanticTypeDeclV1::shape)
        else {
            return Err(source_raw_physical_error_v29());
        };
        if pointer.kind() != SemanticPointerKindV1::Raw
            || pointer.metadata() != SemanticPointerMetadataV1::None
            || pointer.pointee() != source.ty()
            || pointer.mutability() != mutability
        {
            return Err(source_raw_physical_error_v29());
        }
        let start = (site.instance.index(), site.block.index(), statement, 0);
        let end = (
            site.instance.index(),
            site.block.index(),
            statement,
            u32::MAX,
        );
        charge_execution_cfg_lookup_v29(self.plan.raw_origin_sites.len(), budget)?;
        let mut count = 0usize;
        for (_, &index) in self.plan.raw_origin_sites.range(start..=end) {
            budget.source_reference_charge_v29(self.plan, 10)?;
            let row = self
                .plan
                .raw_origins
                .get(index)
                .ok_or_else(source_raw_physical_error_v29)?;
            if row.site != site
                || row.source != source as *const SemanticPlaceV1 as usize
                || row.pointer_type != result_type
                || row.formation != SourceReferenceRawFormationV29::AddressOf
                || row.mutable != (mutability == SemanticMutabilityV1::Mutable)
                || self.raw_formations.get(index).is_none()
            {
                return Err(source_raw_physical_error_v29());
            }
            count = argument_sum_v1(&[count, 1])?;
        }
        if count == 0 {
            return Err(source_raw_physical_error_v29());
        }
        Ok(())
    }

    fn claim_raw_formation_v29(
        &self,
        receipt: SourceRawFormationReceiptV29,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.check(budget)?;
        let site = receipt.site;
        let statement = site.statement.ok_or_else(source_raw_physical_error_v29)?;
        let start = (site.instance.index(), site.block.index(), statement, 0);
        let end = (
            site.instance.index(),
            site.block.index(),
            statement,
            u32::MAX,
        );
        charge_execution_cfg_lookup_v29(self.plan.raw_origin_sites.len(), budget)?;
        let mut count = 0usize;
        for (_, &index) in self.plan.raw_origin_sites.range(start..=end) {
            budget.source_reference_charge_v29(self.plan, 6)?;
            let row = self
                .plan
                .raw_origins
                .get(index)
                .ok_or_else(source_raw_physical_error_v29)?;
            let claim = self
                .raw_formations
                .get(index)
                .ok_or_else(source_raw_physical_error_v29)?;
            if row.site != site
                || row.source != receipt.source
                || claim.get().is_some_and(|old| old != receipt)
            {
                return Err(source_raw_physical_error_v29());
            }
            count = argument_sum_v1(&[count, 1])?;
        }
        if count == 0 {
            return Err(source_raw_physical_error_v29());
        }
        // Validate the full original-site group before publishing any receipt.
        budget.source_reference_charge_v29(self.plan, count)?;
        for (_, &index) in self.plan.raw_origin_sites.range(start..=end) {
            self.raw_formations[index].set(Some(receipt));
        }
        Ok(())
    }
}

impl SemanticFunctionLoweringV1<'_, '_> {
    fn try_lower_source_address_v29(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        result_type: SemanticTypeIdV1,
        value: &SemanticRvalueKindV1,
        operations: &mut Vec<Operation>,
    ) -> Result<Option<SemanticValueBindingV1>, ProductionSemanticKirErrorV1> {
        let SemanticRvalueKindV1::AddressOf { place, mutability } = value else {
            return Ok(None);
        };
        let Some(cursor) = self.execution.as_ref() else {
            return Ok(None);
        };
        let references = cursor
            .references
            .ok_or_else(source_raw_physical_error_v29)?;
        let site = SourceReferenceSiteV29 {
            instance: cursor.instance,
            block,
            statement: statement.map(|s| s as usize),
        };
        self.with_emission_budget_v1(|_, budget| {
            references.check_raw_formation_source_v29(site, place, result_type, *mutability, budget)
        })?;
        // A projected address still consumes its promoted pointer holder, not
        // the pointee. Retained storage has no SSA holder use to invent.
        self.use_source_place_v29(block, statement, place)?;
        let formation_first = operations.len();
        if let Some((endpoint, base, _)) = self.source_object_leaf_address_v29(
            block,
            statement,
            place,
            SourceReferenceAccessV29::Address,
            operations,
        )? {
            let (expected, base_access) = self.with_emission_budget_v1(|this, budget| {
                source_reference_owned_prepay_v29::<(Type, AccessMode)>(references.plan, budget)?;
                let original = this
                    .function
                    .blocks()
                    .get(block.index() as usize)
                    .and_then(|block| {
                        statement.and_then(|statement| block.statements().get(statement as usize))
                    })
                    .ok_or(ArgumentResourceV1::Accounting)?;
                let SemanticStatementKindV1::Assign(assignment) = original.kind() else {
                    return Err(ArgumentResourceV1::Accounting.into());
                };
                let (ty, _) = source_reference_assignment_pointer_v29(
                    references.plan,
                    site,
                    assignment,
                    budget,
                )?
                .ok_or(ArgumentResourceV1::Accounting)?;
                let Type::Pointer(pointer) = &ty else {
                    return Err(ArgumentResourceV1::Accounting.into());
                };
                if pointer.pointee.as_ref() != &Type::StorageObject(endpoint.projected_schema)
                    || pointer.address_space != AddressSpace::Private
                    || pointer.access
                        != if *mutability == SemanticMutabilityV1::Mutable {
                            AccessMode::ReadWrite
                        } else {
                            AccessMode::ReadOnly
                        }
                {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                let key = source_reference_access_key_v29(
                    site,
                    assignment.destination(),
                    SourceReferenceAccessV29::Write,
                );
                let resolved = source_reference_access_at_v29(
                    references.plan,
                    site,
                    place,
                    SourceReferenceAccessV29::Address,
                    budget,
                )?;
                charge_execution_cfg_lookup_v29(
                    references.plan.representation_demand_sites.len(),
                    budget,
                )?;
                for &index in references
                    .plan
                    .representation_demand_sites
                    .get(&key)
                    .ok_or(ArgumentResourceV1::Accounting)?
                {
                    budget.source_reference_charge_v29(references.plan, 3)?;
                    let demand = references
                        .plan
                        .representation_demands
                        .get(index)
                        .ok_or(ArgumentResourceV1::Accounting)?;
                    let SourceReferenceNodeKindV29::Address(set) = references
                        .plan
                        .nodes
                        .get(demand.node)
                        .ok_or(ArgumentResourceV1::Accounting)?
                        .kind
                    else {
                        return Err(ArgumentResourceV1::Accounting.into());
                    };
                    let set = references
                        .plan
                        .raw_sets
                        .get(set)
                        .ok_or(ArgumentResourceV1::Accounting)?;
                    let end = argument_sum_v1(&[set.first, set.count])?;
                    for choice in references
                        .plan
                        .raw_choices
                        .get(set.first..end)
                        .ok_or(ArgumentResourceV1::Accounting)?
                    {
                        budget.source_reference_charge_v29(references.plan, 9)?;
                        let origin = references
                            .plan
                            .raw_origins
                            .get(choice.origin)
                            .ok_or(ArgumentResourceV1::Accounting)?;
                        if origin.site != site
                            || origin.source != place as *const SemanticPlaceV1 as usize
                            || origin.formation != SourceReferenceRawFormationV29::AddressOf
                            || resolved.instance != origin.instance
                            || resolved.local != origin.local
                            || resolved.loan != origin.parent
                            || resolved.ty != origin.ty
                        {
                            return Err(ArgumentResourceV1::Accounting.into());
                        }
                        if resolved.generation != origin.generation
                            && !source_reference_address_epoch_matches_v29(
                                references.plan,
                                site,
                                place,
                                &resolved,
                                choice.origin,
                                budget,
                            )?
                        {
                            return Err(ArgumentResourceV1::Accounting.into());
                        }
                    }
                }
                let base_access = match endpoint.object {
                    ScopedObjectIdentityV29::Local { .. } => AccessMode::ReadWrite,
                    ScopedObjectIdentityV29::Reference {
                        dereference_prefix, ..
                    } => {
                        let prefix = dereference_prefix
                            .checked_sub(1)
                            .ok_or(ArgumentResourceV1::Accounting)?
                            as usize;
                        let (source, _) = source_reference_place_pointer_v29(
                            references.plan,
                            site,
                            place,
                            prefix,
                            SourceReferenceAccessV29::Address,
                            budget,
                        )?
                        .ok_or(ArgumentResourceV1::Accounting)?;
                        let Type::Pointer(source) = source else {
                            return Err(ArgumentResourceV1::Accounting.into());
                        };
                        source.access
                    }
                    _ => return Err(ArgumentResourceV1::Accounting.into()),
                };
                if !matches!(base_access, AccessMode::ReadOnly | AccessMode::ReadWrite)
                    || (*mutability == SemanticMutabilityV1::Mutable
                        && base_access != AccessMode::ReadWrite)
                {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                Ok((ty, base_access))
            })?;
            let binding = if *mutability == SemanticMutabilityV1::Immutable
                && base_access == AccessMode::ReadWrite
            {
                let result = self.with_emission_budget_v1(|_, budget| {
                    emission_binding_clone_type_v1(&expected, budget)
                })?;
                self.emit(
                    operations,
                    result,
                    OperationKind::Cast {
                        kind: CastKind::RestrictPointerAccess,
                        value: base,
                        to: expected,
                    },
                )?
            } else {
                let condition = self.emit(
                    operations,
                    Type::Scalar(ScalarType::Bool),
                    OperationKind::Constant(Constant::Bool(true)),
                )?;
                let (condition, _) = condition
                    .value()
                    .map_err(|_| source_raw_physical_error_v29())?;
                self.emit(
                    operations,
                    expected,
                    OperationKind::Select {
                        condition,
                        true_value: base,
                        false_value: base,
                    },
                )?
            };
            let SemanticValueBindingV1::Value { id: result, .. } = &binding else {
                return Err(source_raw_physical_error_v29());
            };
            if *result == base {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            let physical_block = self.kernel_block_id_v1(block)?;
            self.with_emission_budget_v1(|_, budget| {
                references.claim_raw_formation_v29(
                    SourceRawFormationReceiptV29 {
                        site,
                        source: place as *const SemanticPlaceV1 as usize,
                        base,
                        result: *result,
                        block: physical_block,
                        first: formation_first,
                        end: operations.len(),
                    },
                    budget,
                )
            })?;
            return Ok(Some(binding));
        }
        if !place.projections().is_empty() {
            return Err(source_reference_error_v29(
                "source projected raw formation requires its exact holder address path",
            ));
        }
        let slot = self
            .legacy_retained_slot_v29(place.local())?
            .ok_or_else(source_raw_physical_error_v29)?;
        if slot.semantic_type != place.ty() || slot.storage.scalar_array()?.2.is_some() {
            return Err(source_raw_physical_error_v29());
        }
        let base = slot.pointer;
        let access = match mutability {
            SemanticMutabilityV1::Immutable => AccessMode::ReadOnly,
            SemanticMutabilityV1::Mutable => AccessMode::ReadWrite,
        };
        let first = operations.len();
        let (private, expected) = self.with_emission_budget_v1(|this, budget| {
            budget.source_reference_charge_v29(references.plan, 5)?;
            let SemanticTypeShapeV1::Pointer(source) =
                this.types[result_type.index() as usize].shape()
            else {
                return Err(source_raw_physical_error_v29());
            };
            let slot = lookup_legacy_retained_slot_v29(
                &this.retained_local_slots,
                place.local().index(),
                Some(budget),
            )?
            .ok_or_else(source_raw_physical_error_v29)?;
            let kernel_type = slot.storage.scalar_array()?.0;
            let private_element = emission_binding_clone_type_v1(kernel_type, budget)?;
            budget.reserve_storage(std::mem::size_of::<Type>())?;
            let private = Type::pointer(private_element, AddressSpace::Private, access);
            let exposed_element = emission_binding_clone_type_v1(kernel_type, budget)?;
            budget.reserve_storage(std::mem::size_of::<Type>())?;
            let expected = Type::pointer(
                exposed_element,
                source_address_space_v18(source.address_space())?,
                access,
            );
            Ok((private, expected))
        })?;
        let mut binding = if access == AccessMode::ReadOnly {
            let result = self.with_emission_budget_v1(|_, budget| {
                emission_binding_clone_type_v1(&private, budget)
            })?;
            self.emit(
                operations,
                result,
                OperationKind::Cast {
                    kind: CastKind::RestrictPointerAccess,
                    value: base,
                    to: private,
                },
            )?
        } else {
            SemanticValueBindingV1::Value {
                id: base,
                ty: private,
            }
        };
        let SemanticValueBindingV1::Value {
            id,
            ty: Type::Pointer(actual),
        } = &binding
        else {
            return Err(source_raw_physical_error_v29());
        };
        let id = *id;
        let Type::Pointer(target) = &expected else {
            return Err(source_raw_physical_error_v29());
        };
        if actual.pointee != target.pointee
            || actual.access != target.access
            || actual.address_space != AddressSpace::Private
        {
            return Err(source_raw_physical_error_v29());
        }
        match target.address_space {
            AddressSpace::Private => {
                if access == AccessMode::ReadWrite {
                    // A formed alias is not the implicit backing handle: a
                    // later StorageLive must expire the former independently.
                    let offset = self.emit_index_constant(operations, 0)?;
                    binding = self.emit(
                        operations,
                        expected,
                        OperationKind::GetElementPointer { base: id, offset },
                    )?;
                }
            }
            AddressSpace::Generic => {
                let result = self.with_emission_budget_v1(|_, budget| {
                    emission_binding_clone_type_v1(&expected, budget)
                })?;
                binding = self.emit(
                    operations,
                    result,
                    OperationKind::Cast {
                        kind: CastKind::PointerToGeneric,
                        value: id,
                        to: expected,
                    },
                )?;
            }
            _ => return Err(source_raw_physical_error_v29()),
        }
        let SemanticValueBindingV1::Value { id: result, .. } = &binding else {
            return Err(source_raw_physical_error_v29());
        };
        let result = *result;
        let physical_block = self.kernel_block_id_v1(block)?;
        self.with_emission_budget_v1(|_, budget| {
            references.claim_raw_formation_v29(
                SourceRawFormationReceiptV29 {
                    site,
                    source: place as *const SemanticPlaceV1 as usize,
                    base,
                    result,
                    block: physical_block,
                    first,
                    end: operations.len(),
                },
                budget,
            )
        })?;
        Ok(Some(binding))
    }
}
