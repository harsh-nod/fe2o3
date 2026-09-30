// Runtime extents are whole source descriptors, never fixed-array lengths or
// local initialized-byte evidence. These rows are original occurrence locators.
#[derive(Clone, Copy, Debug)]
struct SourceReferenceDescriptorV29 {
    instance: ProductionCallInstanceIdV1,
    occurrence: usize,
    holder_occurrence: usize,
    source: usize,
    projection: usize,
    pointer_type: SemanticTypeIdV1,
    slice_type: SemanticTypeIdV1,
    element: SemanticTypeIdV1,
    index_local: SemanticLocalIdV1,
    index_value: SsaValueV1,
    holder_value: SsaValueV1,
}

#[derive(Clone, Copy, Debug)]
struct SourceReferenceDescriptorGuardV29 {
    instance: ProductionCallInstanceIdV1,
    assertion: u32,
    condition: usize,
    length: usize,
    index: usize,
    comparison: usize,
    comparison_index: usize,
    comparison_length: usize,
    metadata: usize,
    holder: usize,
}

#[derive(Clone, Copy)]
struct CheckedSourceDescriptorGuardV29<'a> {
    condition: SsaValueV1,
    index: SsaValueV1,
    length: SsaValueV1,
    holder: SsaValueV1,
    source: &'a SemanticPlaceV1,
    fields: usize,
    pointer_type: SemanticTypeIdV1,
    element: SemanticTypeIdV1,
    success: SemanticBlockIdV1,
}

fn source_descriptor_whole_operand_v29(
    function: &SemanticFunctionDeclV1,
    site: ExecutionSiteV29,
    role: ExecutionOperandV29,
) -> Option<&SemanticPlaceV1> {
    match scoped_source_operand_v29(function, site, role)? {
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)
            if place.projections().is_empty() =>
        {
            Some(place)
        }
        _ => None,
    }
}

impl SourceReferenceDescriptorGuardV29 {
    fn check<'a>(
        &self,
        instances: &'a ExecutionInstancesV29<'_>,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<Option<CheckedSourceDescriptorGuardV29<'a>>, ProductionSemanticKirErrorV1> {
        budget.charge_work(14)?;
        let row = instances
            .instance(self.instance)
            .ok_or_else(source_descriptor_error_v29)?;
        let function = row.declaration();
        let occurrences = instances
            .occurrences(self.instance)
            .ok_or_else(source_descriptor_error_v29)?;
        let block = function
            .blocks()
            .get(self.assertion as usize)
            .ok_or_else(source_descriptor_error_v29)?;
        let SemanticTerminatorKindV1::Assert {
            expected: true,
            message: SemanticAssertMessageV1::BoundsCheck { .. },
            target,
            unwind,
            ..
        } = block.terminator().kind()
        else {
            return Err(source_descriptor_error_v29());
        };
        if matches!(unwind, SemanticUnwindActionV1::Cleanup(_)) {
            return Err(source_descriptor_error_v29());
        }
        let site = ExecutionSiteV29::Terminator {
            block: SsaBlockIdV1::new(self.assertion),
        };
        let condition = source_descriptor_use_v29(
            &occurrences,
            self.condition,
            site,
            ExecutionOperandV29::AssertCondition,
            budget,
        )?;
        let length = source_descriptor_use_v29(
            &occurrences,
            self.length,
            site,
            ExecutionOperandV29::AssertMessage(0),
            budget,
        )?;
        let index = source_descriptor_use_v29(
            &occurrences,
            self.index,
            site,
            ExecutionOperandV29::AssertMessage(1),
            budget,
        )?;
        for role in [
            ExecutionOperandV29::AssertCondition,
            ExecutionOperandV29::AssertMessage(0),
            ExecutionOperandV29::AssertMessage(1),
        ] {
            budget.charge_work(1)?;
            if source_descriptor_whole_operand_v29(function, site, role).is_none() {
                return Ok(None);
            }
        }
        let (comparison_site, comparison) = source_descriptor_assignment_v29(
            function,
            &occurrences,
            self.comparison,
            condition,
            budget,
        )?;
        if !matches!(
            comparison,
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::LessThan,
                ..
            }
        ) || source_descriptor_whole_operand_v29(
            function,
            comparison_site,
            ExecutionOperandV29::RvalueOperand(0),
        )
        .is_none()
            || source_descriptor_whole_operand_v29(
                function,
                comparison_site,
                ExecutionOperandV29::RvalueOperand(1),
            )
            .is_none()
            || source_descriptor_use_v29(
                &occurrences,
                self.comparison_index,
                comparison_site,
                ExecutionOperandV29::RvalueOperand(0),
                budget,
            )? != index
            || source_descriptor_use_v29(
                &occurrences,
                self.comparison_length,
                comparison_site,
                ExecutionOperandV29::RvalueOperand(1),
                budget,
            )? != length
        {
            return Ok(None);
        }
        let (metadata_site, metadata) = source_descriptor_assignment_v29(
            function,
            &occurrences,
            self.metadata,
            length,
            budget,
        )?;
        let (source, fields, role) = match metadata {
            SemanticRvalueKindV1::Unary {
                operation: SemanticUnaryOpV1::PointerMetadata,
                operand: SemanticOperandV1::Copy(source) | SemanticOperandV1::Move(source),
            } => (
                source,
                source.projections().len(),
                ExecutionOperandV29::RvalueOperand(0),
            ),
            SemanticRvalueKindV1::Length(source)
                if source.projections().last().map(|p| p.kind())
                    == Some(SemanticProjectionKindV1::Dereference) =>
            {
                (
                    source,
                    source.projections().len() - 1,
                    ExecutionOperandV29::RvaluePlace,
                )
            }
            _ => return Ok(None),
        };
        budget.charge_work(argument_sum_v1(&[fields, 5])?)?;
        if source.projections()[..fields]
            .iter()
            .any(|p| !matches!(p.kind(), SemanticProjectionKindV1::Field(_)))
        {
            return Ok(None);
        }
        let holder =
            source_descriptor_use_v29(&occurrences, self.holder, metadata_site, role, budget)?;
        if occurrences.events()[self.holder].event().variable().get() != source.local().index() {
            return Err(source_descriptor_error_v29());
        }
        let pointer_type = if fields == 0 {
            function.locals()[source.local().index() as usize].ty()
        } else {
            source.projections()[fields - 1].result_type()
        };
        let types = instances.owner().source_semantic().types();
        let Some(SemanticTypeShapeV1::Pointer(pointer)) = types
            .get(pointer_type.index() as usize)
            .map(SemanticTypeDeclV1::shape)
        else {
            return Ok(None);
        };
        let slice_type = pointer.pointee();
        let Some(SemanticTypeShapeV1::Slice { element }) = types
            .get(slice_type.index() as usize)
            .map(SemanticTypeDeclV1::shape)
        else {
            return Ok(None);
        };
        if pointer.kind() != SemanticPointerKindV1::Reference
            || pointer.metadata() != SemanticPointerMetadataV1::SliceLength
            || pointer.pointer_width_bits() != 64
            || !matches!(
                types
                    .get(element.index() as usize)
                    .map(SemanticTypeDeclV1::shape),
                Some(SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_))
            )
        {
            return Ok(None);
        }
        Ok(Some(CheckedSourceDescriptorGuardV29 {
            condition,
            index,
            length,
            holder,
            source,
            fields,
            pointer_type,
            element: *element,
            success: target.target(),
        }))
    }
}

fn source_descriptor_error_v29() -> ProductionSemanticKirErrorV1 {
    source_reference_error_v29(
        "source runtime slice descriptor/index/extent correspondence differs",
    )
}

fn source_descriptor_shape_v29(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    place: &SemanticPlaceV1,
    projection: usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<
    Option<(SemanticTypeIdV1, SemanticTypeIdV1, SemanticTypeIdV1)>,
    ProductionSemanticKirErrorV1,
> {
    budget.charge_work(argument_sum_v1(&[projection, 8])?)?;
    if projection == 0 || projection.checked_add(1) != Some(place.projections().len()) {
        return Ok(None);
    }
    let prefix = &place.projections()[..projection];
    if prefix.last().map(|step| step.kind()) != Some(SemanticProjectionKindV1::Dereference)
        || prefix[..projection - 1]
            .iter()
            .any(|step| !matches!(step.kind(), SemanticProjectionKindV1::Field(_)))
    {
        return Ok(None);
    }
    let pointer_type = if projection == 1 {
        function
            .locals()
            .get(place.local().index() as usize)
            .ok_or_else(source_descriptor_error_v29)?
            .ty()
    } else {
        prefix[projection - 2].result_type()
    };
    let slice_type = prefix[projection - 1].result_type();
    let Some(SemanticTypeShapeV1::Slice { element }) = types
        .get(slice_type.index() as usize)
        .map(SemanticTypeDeclV1::shape)
    else {
        return Ok(None);
    };
    let Some(SemanticTypeShapeV1::Pointer(pointer)) = types
        .get(pointer_type.index() as usize)
        .map(SemanticTypeDeclV1::shape)
    else {
        return Err(source_descriptor_error_v29());
    };
    if pointer.kind() != SemanticPointerKindV1::Reference
        || pointer.metadata() != SemanticPointerMetadataV1::SliceLength
        || pointer.pointee() != slice_type
        || pointer.pointer_width_bits() != 64
        || place.projections()[projection].result_type() != *element
        || place.ty() != *element
        || !matches!(
            types
                .get(element.index() as usize)
                .map(SemanticTypeDeclV1::shape),
            Some(SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_))
        )
    {
        return Err(source_descriptor_error_v29());
    }
    Ok(Some((pointer_type, slice_type, *element)))
}

fn source_descriptor_use_v29(
    occurrences: &ProductionSemanticSsaFunctionOccurrencesV1<'_>,
    index: usize,
    site: ExecutionSiteV29,
    role: ExecutionOperandV29,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<SsaValueV1, ProductionSemanticKirErrorV1> {
    budget.charge_work(6)?;
    let event = occurrences
        .events()
        .get(index)
        .ok_or_else(source_descriptor_error_v29)?;
    if !event.is_reachable()
        || !event.is_promoted()
        || event.site() != site
        || event.operand() != role
        || event.role() != ExecutionEventV29::BaseUse
    {
        return Err(source_descriptor_error_v29());
    }
    match event.resolved() {
        Some(SsaResolvedEventV1::Use { variable, value })
            if variable == event.event().variable() =>
        {
            Ok(value)
        }
        _ => Err(source_descriptor_error_v29()),
    }
}

impl SourceReferenceDescriptorV29 {
    fn check<'a>(
        &self,
        instances: &'a ExecutionInstancesV29<'_>,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<&'a SemanticPlaceV1, ProductionSemanticKirErrorV1> {
        budget.charge_work(10)?;
        let instance = instances
            .instance(self.instance)
            .ok_or_else(source_descriptor_error_v29)?;
        let occurrences = instances
            .occurrences(self.instance)
            .ok_or_else(source_descriptor_error_v29)?;
        let event = occurrences
            .events()
            .get(self.occurrence)
            .ok_or_else(source_descriptor_error_v29)?;
        let place = source_reference_selector_place_v29(
            instance.declaration(),
            event.site(),
            event.operand(),
        )
        .ok_or_else(source_descriptor_error_v29)?;
        if self.source != place as *const SemanticPlaceV1 as usize
            || !event.is_reachable()
            || !event.is_promoted()
            || event.role()
                != ExecutionEventV29::ProjectionIndexUse(
                    u32::try_from(self.projection).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                )
            || event.resolved()
                != Some(SsaResolvedEventV1::Use {
                    variable: fe2o3_mir_model::SsaVariableIdV1::new(self.index_local.index()),
                    value: self.index_value,
                })
            || place
                .projections()
                .get(self.projection)
                .map(|step| step.kind())
                != Some(SemanticProjectionKindV1::Index(self.index_local))
            || self.holder_occurrence >= self.occurrence
            || source_descriptor_use_v29(
                &occurrences,
                self.holder_occurrence,
                event.site(),
                event.operand(),
                budget,
            )? != self.holder_value
            || occurrences.events()[self.holder_occurrence]
                .event()
                .variable()
                .get()
                != place.local().index()
            || source_descriptor_shape_v29(
                instances.owner().source_semantic().types(),
                instance.declaration(),
                place,
                self.projection,
                budget,
            )? != Some((self.pointer_type, self.slice_type, self.element))
        {
            return Err(source_descriptor_error_v29());
        }
        Ok(place)
    }
}

struct SourceDescriptorInventoryV29 {
    sites: BTreeMap<(u32, Option<u32>), std::ops::Range<usize>>,
    definitions: BTreeMap<SsaValueV1, usize>,
}

impl SourceDescriptorInventoryV29 {
    fn build(
        occurrences: &ProductionSemanticSsaFunctionOccurrencesV1<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        budget.reserve_storage(argument_sum_v1(&[
            std::mem::size_of::<Self>(),
            std::mem::size_of::<Result<Self, ProductionSemanticKirErrorV1>>(),
        ])?)?;
        let mut inventory = Self {
            sites: BTreeMap::new(),
            definitions: BTreeMap::new(),
        };
        for (index, event) in occurrences.events().iter().enumerate() {
            budget.charge_work(3)?;
            let key = scoped_memory_site_key_v29(event.site());
            charge_execution_cfg_lookup_v29(inventory.sites.len(), budget)?;
            if let Some(range) = inventory.sites.get_mut(&key) {
                if range.end != index {
                    return Err(source_descriptor_error_v29());
                }
                range.end = argument_sum_v1(&[index, 1])?;
            } else {
                reserve_execution_cfg_map_entry_v29::<(u32, Option<u32>), std::ops::Range<usize>>(
                    inventory.sites.len(),
                    budget,
                )?;
                inventory
                    .sites
                    .insert(key, index..argument_sum_v1(&[index, 1])?);
            }
            if event.is_reachable()
                && let Some(SsaResolvedEventV1::Define { value, .. }) = event.resolved()
            {
                reserve_execution_cfg_map_entry_v29::<SsaValueV1, usize>(
                    inventory.definitions.len(),
                    budget,
                )?;
                if inventory.definitions.insert(value, index).is_some() {
                    return Err(source_descriptor_error_v29());
                }
            }
        }
        Ok(inventory)
    }

    fn use_at(
        &self,
        occurrences: &ProductionSemanticSsaFunctionOccurrencesV1<'_>,
        site: ExecutionSiteV29,
        role: ExecutionOperandV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<usize>, ProductionSemanticKirErrorV1> {
        charge_execution_cfg_lookup_v29(self.sites.len(), budget)?;
        let Some(range) = self.sites.get(&scoped_memory_site_key_v29(site)) else {
            return Ok(None);
        };
        let mut found = None;
        for index in range.clone() {
            budget.charge_work(3)?;
            let event = &occurrences.events()[index];
            if event.operand() == role && event.role() == ExecutionEventV29::BaseUse {
                if found.replace(index).is_some() {
                    return Err(source_descriptor_error_v29());
                }
            }
        }
        Ok(found)
    }

    fn definition(
        &self,
        value: SsaValueV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<usize>, ProductionSemanticKirErrorV1> {
        charge_execution_cfg_lookup_v29(self.definitions.len(), budget)?;
        Ok(self.definitions.get(&value).copied())
    }
}

fn source_descriptor_assignment_v29<'a>(
    function: &'a SemanticFunctionDeclV1,
    occurrences: &ProductionSemanticSsaFunctionOccurrencesV1<'_>,
    definition: usize,
    value: SsaValueV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(ExecutionSiteV29, &'a SemanticRvalueKindV1), ProductionSemanticKirErrorV1> {
    budget.charge_work(5)?;
    let event = occurrences
        .events()
        .get(definition)
        .ok_or_else(source_descriptor_error_v29)?;
    let SemanticStatementKindV1::Assign(assignment) =
        scoped_source_statement_v29(function, event.site())
            .ok_or_else(source_descriptor_error_v29)?
    else {
        return Err(source_descriptor_error_v29());
    };
    if !event.is_reachable()
        || !event.is_promoted()
        || !assignment.destination().projections().is_empty()
        || event.resolved()
            != Some(SsaResolvedEventV1::Define {
                variable: fe2o3_mir_model::SsaVariableIdV1::new(
                    assignment.destination().local().index(),
                ),
                value,
            })
    {
        return Err(source_descriptor_error_v29());
    }
    Ok((event.site(), assignment.value().kind()))
}

impl SourceReferenceBuilderV29<'_, '_, '_> {
    fn collect_storage_descriptor(
        &mut self,
        instance: ProductionCallInstanceIdV1,
        occurrence: usize,
        source: &SemanticPlaceV1,
        projection: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let row = self
            .plan
            .instances
            .instance(instance)
            .ok_or_else(source_descriptor_error_v29)?;
        let Some((pointer_type, slice_type, element)) = source_descriptor_shape_v29(
            self.plan.instances.owner().source_semantic().types(),
            row.declaration(),
            source,
            projection,
            budget,
        )?
        else {
            return Ok(());
        };
        let occurrences = self
            .plan
            .instances
            .occurrences(instance)
            .ok_or_else(source_descriptor_error_v29)?;
        let event = &occurrences.events()[occurrence];
        let Some(SsaResolvedEventV1::Use { variable, value }) = event.resolved() else {
            return Err(source_descriptor_error_v29());
        };
        let mut holder = None;
        for index in (0..occurrence).rev() {
            budget.charge_work(3)?;
            let previous = &occurrences.events()[index];
            if previous.site() != event.site() || previous.operand() != event.operand() {
                break;
            }
            if previous.role() == ExecutionEventV29::BaseUse {
                holder = Some(index);
                break;
            }
        }
        let holder_occurrence = holder.ok_or_else(source_descriptor_error_v29)?;
        let descriptor = SourceReferenceDescriptorV29 {
            instance,
            occurrence,
            holder_occurrence,
            source: source as *const SemanticPlaceV1 as usize,
            projection,
            pointer_type,
            slice_type,
            element,
            index_local: SemanticLocalIdV1::from_index(variable.get()),
            index_value: value,
            holder_value: source_descriptor_use_v29(
                &occurrences,
                holder_occurrence,
                event.site(),
                event.operand(),
                budget,
            )?,
        };
        descriptor.check(self.plan.instances, budget)?;
        let key = source_reference_selector_site_v29(instance, event.site(), source, projection);
        reserve_execution_cfg_map_entry_v29::<SourceReferenceSelectorSiteV29, usize>(
            self.plan.descriptor_sites.len(),
            budget,
        )?;
        let index = self.plan.descriptors.len();
        if self.plan.descriptor_sites.insert(key, index).is_some() {
            return Err(source_descriptor_error_v29());
        }
        let holder_key = (instance.index(), holder_occurrence);
        charge_execution_cfg_lookup_v29(self.plan.descriptor_holders.len(), budget)?;
        if !self.plan.descriptor_holders.contains_key(&holder_key) {
            reserve_execution_cfg_map_entry_v29::<(usize, usize), ()>(
                self.plan.descriptor_holders.len(),
                budget,
            )?;
            self.plan.descriptor_holders.insert(holder_key, ());
        }
        emission_push_v1(&mut self.plan.descriptors, descriptor, budget)
    }

    fn collect_descriptor_guards(
        &mut self,
        instance: ProductionCallInstanceIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let row = self
            .plan
            .instances
            .instance(instance)
            .ok_or_else(source_descriptor_error_v29)?;
        let occurrences = self
            .plan
            .instances
            .occurrences(instance)
            .ok_or_else(source_descriptor_error_v29)?;
        let inventory = SourceDescriptorInventoryV29::build(&occurrences, budget)?;
        for (assertion, block) in row.declaration().blocks().iter().enumerate() {
            budget.charge_work(2)?;
            if !matches!(
                block.terminator().kind(),
                SemanticTerminatorKindV1::Assert {
                    expected: true,
                    message: SemanticAssertMessageV1::BoundsCheck { .. },
                    unwind: SemanticUnwindActionV1::Unreachable
                        | SemanticUnwindActionV1::Continue
                        | SemanticUnwindActionV1::Terminate,
                    ..
                }
            ) {
                continue;
            }
            let assertion = u32::try_from(assertion).map_err(|_| ArgumentResourceV1::Arithmetic)?;
            let site = ExecutionSiteV29::Terminator {
                block: SsaBlockIdV1::new(assertion),
            };
            if !row.ssa().plan().is_reachable(SsaBlockIdV1::new(assertion)) {
                continue;
            }
            let Some(condition) = inventory.use_at(
                &occurrences,
                site,
                ExecutionOperandV29::AssertCondition,
                budget,
            )?
            else {
                continue;
            };
            let Some(length) = inventory.use_at(
                &occurrences,
                site,
                ExecutionOperandV29::AssertMessage(0),
                budget,
            )?
            else {
                continue;
            };
            let Some(index) = inventory.use_at(
                &occurrences,
                site,
                ExecutionOperandV29::AssertMessage(1),
                budget,
            )?
            else {
                continue;
            };
            let condition_value = source_descriptor_use_v29(
                &occurrences,
                condition,
                site,
                ExecutionOperandV29::AssertCondition,
                budget,
            )?;
            let length_value = source_descriptor_use_v29(
                &occurrences,
                length,
                site,
                ExecutionOperandV29::AssertMessage(0),
                budget,
            )?;
            let Some(comparison) = inventory.definition(condition_value, budget)? else {
                continue;
            };
            let Some(metadata) = inventory.definition(length_value, budget)? else {
                continue;
            };
            let (comparison_site, comparison_value) = source_descriptor_assignment_v29(
                row.declaration(),
                &occurrences,
                comparison,
                condition_value,
                budget,
            )?;
            if !matches!(
                comparison_value,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::LessThan,
                    ..
                }
            ) {
                continue;
            }
            let Some(comparison_index) = inventory.use_at(
                &occurrences,
                comparison_site,
                ExecutionOperandV29::RvalueOperand(0),
                budget,
            )?
            else {
                continue;
            };
            let Some(comparison_length) = inventory.use_at(
                &occurrences,
                comparison_site,
                ExecutionOperandV29::RvalueOperand(1),
                budget,
            )?
            else {
                continue;
            };
            let (metadata_site, metadata_value) = source_descriptor_assignment_v29(
                row.declaration(),
                &occurrences,
                metadata,
                length_value,
                budget,
            )?;
            let role = match metadata_value {
                SemanticRvalueKindV1::Unary {
                    operation: SemanticUnaryOpV1::PointerMetadata,
                    ..
                } => ExecutionOperandV29::RvalueOperand(0),
                SemanticRvalueKindV1::Length(_) => ExecutionOperandV29::RvaluePlace,
                _ => continue,
            };
            let Some(holder) = inventory.use_at(&occurrences, metadata_site, role, budget)? else {
                continue;
            };
            let guard = SourceReferenceDescriptorGuardV29 {
                instance,
                assertion,
                condition,
                length,
                index,
                comparison,
                comparison_index,
                comparison_length,
                metadata,
                holder,
            };
            // Invalid candidate relations are not assertions about a slice.
            if guard.check(self.plan.instances, budget)?.is_none() {
                continue;
            }
            reserve_execution_cfg_map_entry_v29::<(usize, u32), usize>(
                self.plan.descriptor_guard_sites.len(),
                budget,
            )?;
            let next = self.plan.descriptor_guards.len();
            if self
                .plan
                .descriptor_guard_sites
                .insert((instance.index(), assertion), next)
                .is_some()
            {
                return Err(source_descriptor_error_v29());
            }
            emission_push_v1(&mut self.plan.descriptor_guards, guard, budget)?;
        }
        drop(inventory);
        Ok(())
    }
}

// This is only an original effect selector. The descriptor occurrence, SSA
// holder/index, actual access, and dominating bounds guard are separate proofs.
fn source_descriptor_volatile_source_v29(
    function: &SemanticFunctionDeclV1,
    site: ExecutionSiteV29,
    source: &SemanticPlaceV1,
    role: ExecutionOperandV29,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    budget.charge_work(10)?;
    Ok(match (scoped_source_statement_v29(function, site), role) {
        (Some(SemanticStatementKindV1::Assign(assignment)), ExecutionOperandV29::RvaluePlace) => {
            matches!(assignment.value().kind(), SemanticRvalueKindV1::Load(load)
                if std::ptr::eq(load.source(), source)
                    && assignment.value().result_type() == source.ty()
                    && load.volatility() == SemanticVolatilityV1::Volatile
                    && load.atomic().is_none())
        }
        (Some(SemanticStatementKindV1::Store(store)), ExecutionOperandV29::StoreDestination) => {
            std::ptr::eq(store.destination(), source)
                && store.volatility() == SemanticVolatilityV1::Volatile
                && store.atomic().is_none()
        }
        _ => false,
    })
}

impl SourceReferencePlanV29<'_, '_> {
    fn ordered_descriptor_effect(
        &self,
        site: SourceReferenceSiteV29,
        source: &SemanticPlaceV1,
        role: ExecutionOperandV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        budget.source_reference_owner_v29(self)?;
        let result = (|| {
            budget.source_reference_charge_v29(self, 5)?;
            let Some(statement) = site.statement else {
                return Ok(false);
            };
            let statement = u32::try_from(statement).map_err(|_| ArgumentResourceV1::Arithmetic)?;
            let execution = execution_site_v29(site.block, Some(statement));
            let function = self
                .instances
                .instance(site.instance)
                .ok_or_else(source_descriptor_error_v29)?
                .declaration();
            if !source_descriptor_volatile_source_v29(function, execution, source, role, budget)? {
                return Ok(false);
            }
            let Some(projection) = source.projections().len().checked_sub(1) else {
                return Ok(false);
            };
            let Some((_, row)) =
                self.descriptor_at(site.instance, execution, source, projection, budget)?
            else {
                return Ok(false);
            };
            let occurrences = self
                .instances
                .occurrences(site.instance)
                .ok_or_else(source_descriptor_error_v29)?;
            let event = occurrences
                .events()
                .get(row.occurrence)
                .ok_or_else(source_descriptor_error_v29)?;
            if event.site() != execution || event.operand() != role {
                return Err(source_descriptor_error_v29());
            }
            Ok(true)
        })();
        if let Err(error) = &result {
            source_reference_record_failure_v29(self, error);
        }
        result
    }

    fn descriptor_at(
        &self,
        instance: ProductionCallInstanceIdV1,
        site: ExecutionSiteV29,
        source: &SemanticPlaceV1,
        projection: usize,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<Option<(usize, SourceReferenceDescriptorV29)>, ProductionSemanticKirErrorV1> {
        budget.source_reference_owner_v29(self)?;
        budget.source_reference_charge_v29(
            self,
            self.descriptor_sites.len().checked_ilog2().unwrap_or(0) as usize + 3,
        )?;
        let key = source_reference_selector_site_v29(instance, site, source, projection);
        let Some(&index) = self.descriptor_sites.get(&key) else {
            return Ok(None);
        };
        let row = *self
            .descriptors
            .get(index)
            .ok_or_else(source_descriptor_error_v29)?;
        if !std::ptr::eq(row.check(self.instances, budget)?, source) {
            return Err(source_descriptor_error_v29());
        }
        Ok(Some((index, row)))
    }

    fn descriptor_holder_event(
        &self,
        instance: ProductionCallInstanceIdV1,
        occurrence: usize,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        budget.source_reference_owner_v29(self)?;
        budget.source_reference_charge_v29(
            self,
            self.descriptor_holders.len().checked_ilog2().unwrap_or(0) as usize + 2,
        )?;
        Ok(self
            .descriptor_holders
            .contains_key(&(instance.index(), occurrence)))
    }
}

#[derive(Clone, Copy, Debug)]
struct SourceReferenceDescriptorUseV29 {
    original: ValueId,
    scalar: ScalarType,
    slice: ValueId,
    producer: SourceReferenceSelectorProducerV29,
}

#[derive(Clone, Copy, Debug)]
struct SourceReferenceDescriptorGuardUseV29 {
    condition: ValueId,
    index: ValueId,
    index_scalar: ScalarType,
    length: ValueId,
    length_scalar: ScalarType,
    slice: ValueId,
    block: BlockId,
    success: BlockId,
    failure: BlockId,
}

fn source_descriptor_binding_v29<'a>(
    mut binding: &'a SemanticValueBindingV1,
    source: &SemanticPlaceV1,
    fields: usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(ValueId, &'a Type), ProductionSemanticKirErrorV1> {
    budget.charge_work(argument_sum_v1(&[fields, 2])?)?;
    for projection in source
        .projections()
        .get(..fields)
        .ok_or_else(source_descriptor_error_v29)?
    {
        let SemanticProjectionKindV1::Field(field) = projection.kind() else {
            return Err(source_descriptor_error_v29());
        };
        binding = match binding {
            SemanticValueBindingV1::Aggregate(values) => values
                .get(field as usize)
                .ok_or_else(source_descriptor_error_v29)?,
            _ if field == 0 => binding,
            _ => return Err(source_descriptor_error_v29()),
        };
    }
    match binding {
        SemanticValueBindingV1::Value { id, ty } => Ok((*id, ty)),
        _ => Err(source_descriptor_error_v29()),
    }
}

fn check_source_descriptor_type_v29(
    types: &[SemanticTypeDeclV1],
    pointer: SemanticTypeIdV1,
    element: SemanticTypeIdV1,
    representation: AddressSpace,
    physical: &Type,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(8)?;
    let Some(SemanticTypeShapeV1::Pointer(pointer)) = types
        .get(pointer.index() as usize)
        .map(SemanticTypeDeclV1::shape)
    else {
        return Err(source_descriptor_error_v29());
    };
    let Type::Slice(slice) = physical else {
        return Err(source_descriptor_error_v29());
    };
    let access = match pointer.mutability() {
        SemanticMutabilityV1::Immutable => AccessMode::ReadOnly,
        SemanticMutabilityV1::Mutable => AccessMode::ReadWrite,
    };
    if slice.address_space != representation
        || slice.access != access
        || *slice.element != lower_scalar_type(types, element)?
    {
        return Err(source_descriptor_error_v29());
    }
    Ok(())
}

impl SemanticFunctionLoweringV1<'_, '_> {
    fn has_original_source_descriptor_v29(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        source: &SemanticPlaceV1,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        if !source.projections().last().is_some_and(|projection| {
            matches!(projection.kind(), SemanticProjectionKindV1::Index(_))
        }) || self
            .execution
            .as_ref()
            .and_then(|cursor| cursor.references)
            .is_none()
        {
            return Ok(false);
        }
        self.with_emission_budget_v1(|this, budget| {
            let cursor = this
                .execution
                .as_ref()
                .ok_or_else(source_descriptor_error_v29)?;
            let references = cursor.references.ok_or_else(source_descriptor_error_v29)?;
            references.check(budget)?;
            // This only selects the existing address builder. Its begin/finish
            // checks still bind and consume the original descriptor occurrence.
            Ok(references
                .plan
                .descriptor_at(
                    cursor.instance,
                    execution_site_v29(block, statement),
                    source,
                    source.projections().len() - 1,
                    budget,
                )?
                .is_some())
        })
    }

    fn begin_source_descriptor_v29(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        source: &SemanticPlaceV1,
        projection: usize,
        slice: ValueId,
        original: ValueId,
        scalar: ScalarType,
    ) -> Result<Option<usize>, ProductionSemanticKirErrorV1> {
        if self
            .execution
            .as_ref()
            .and_then(|cursor| cursor.references)
            .is_none()
        {
            return Ok(None);
        }
        self.with_emission_budget_v1(|this, budget| {
            let cursor = this.execution.as_mut().ok_or_else(source_descriptor_error_v29)?;
            let references = cursor.references.ok_or_else(source_descriptor_error_v29)?;
            let Some((index, row)) = references.plan.descriptor_at(cursor.instance, execution_site_v29(block, statement), source, projection, budget)? else { return Ok(None); };
            references.check(budget)?;
            source_reference_owned_prepay_v29::<Option<usize>>(references.plan, budget)?;
            source_reference_owned_prepay_v29::<SourceReferenceDescriptorUseV29>(references.plan, budget)?;
            source_reference_owned_prepay_v29::<SourceReferenceSelectorProducerV29>(references.plan, budget)?;
            if !matches!(scalar, ScalarType::U64 | ScalarType::Index) { return Err(source_descriptor_error_v29()); }
            charge_execution_cfg_lookup_v29(this.semantic_ssa_bindings.len(), budget)?;
            let archived_index = this.semantic_ssa_bindings.get(&row.index_value).ok_or_else(source_descriptor_error_v29)?;
            let SemanticValueBindingV1::Value { id, ty } = archived_index else { return Err(source_descriptor_error_v29()); };
            if *id != original || *ty != Type::Scalar(scalar) { return Err(source_descriptor_error_v29()); }
            let held_index = this.locals.get(row.index_local.index() as usize).and_then(Option::as_ref).ok_or_else(source_descriptor_error_v29)?;
            if !matches!(held_index, SemanticValueBindingV1::Value { id, ty } if *id == original && *ty == Type::Scalar(scalar)) { return Err(source_descriptor_error_v29()); }
            charge_execution_cfg_lookup_v29(this.semantic_ssa_bindings.len(), budget)?;
            let archived_holder = this.semantic_ssa_bindings.get(&row.holder_value).ok_or_else(source_descriptor_error_v29)?;
            let archived = source_descriptor_binding_v29(archived_holder, source, projection - 1, budget)?;
            let held_holder = this.locals.get(source.local().index() as usize).and_then(Option::as_ref).ok_or_else(source_descriptor_error_v29)?;
            let held = source_descriptor_binding_v29(held_holder, source, projection - 1, budget)?;
            if archived != held || archived.0 != slice { return Err(source_descriptor_error_v29()); }
            let representation = references.plan.descriptor_value_space(cursor.instance,
                row.holder_occurrence, source, projection - 1, row.pointer_type, budget)?;
            check_source_descriptor_type_v29(this.types, row.pointer_type, row.element, representation, archived.1, budget)?;
            let event = &cursor.occurrences.events()[row.occurrence];
            if cursor.instance != row.instance || cursor.block != Some(execution_event_block_v29(event.site())) { return Err(source_descriptor_error_v29()); }
            let claim = references.descriptors.get(index).ok_or_else(source_descriptor_error_v29)?;
            if claim.get().is_some() { return Err(source_descriptor_error_v29()); }
            if cursor.claimed.get(row.holder_occurrence) == Some(&true) {
                cursor.claim_events(&[row.occurrence], budget)?;
            } else {
                cursor.claim_events(&[row.holder_occurrence, row.occurrence], budget)?;
            }
            claim.set(Some(SourceReferenceDescriptorUseV29 { original, scalar, slice, producer: SourceReferenceSelectorProducerV29::Pending }));
            Ok(Some(index))
        })
    }

    fn finish_source_descriptor_v29(
        &mut self,
        descriptor: Option<usize>,
        producer: SourceReferenceSelectorProducerV29,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let Some(index) = descriptor else {
            return Ok(());
        };
        self.with_emission_budget_v1(|this, budget| {
            let references = this
                .execution
                .as_ref()
                .and_then(|cursor| cursor.references)
                .ok_or_else(source_descriptor_error_v29)?;
            references.check(budget)?;
            budget.source_reference_charge_v29(references.plan, 4)?;
            let cell = references
                .descriptors
                .get(index)
                .ok_or_else(source_descriptor_error_v29)?;
            let mut claim = cell.get().ok_or_else(source_descriptor_error_v29)?;
            if !matches!(claim.producer, SourceReferenceSelectorProducerV29::Pending)
                || !matches!(producer, SourceReferenceSelectorProducerV29::Address { .. })
            {
                return Err(source_descriptor_error_v29());
            }
            claim.producer = producer;
            cell.set(Some(claim));
            Ok(())
        })
    }

    fn record_source_descriptor_guard_v29(
        &mut self,
        original_block: SemanticBlockIdV1,
        terminator: &Terminator,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if self
            .execution
            .as_ref()
            .and_then(|cursor| cursor.references)
            .is_none_or(|references| references.plan.descriptor_guards.is_empty())
        {
            return Ok(());
        }
        self.with_emission_budget_v1(|this, budget| {
            let cursor = this
                .execution
                .as_ref()
                .ok_or_else(source_descriptor_error_v29)?;
            let references = cursor.references.ok_or_else(source_descriptor_error_v29)?;
            references.check(budget)?;
            charge_execution_cfg_lookup_v29(references.plan.descriptor_guard_sites.len(), budget)?;
            let Some(&index) = references
                .plan
                .descriptor_guard_sites
                .get(&(cursor.instance.index(), original_block.index()))
            else {
                return Ok(());
            };
            let source = references.plan.descriptor_guards[index]
                .check(references.plan.instances, budget)?
                .ok_or_else(source_descriptor_error_v29)?;
            let Terminator::ConditionalBranch {
                condition,
                then_target,
                else_target,
                ..
            } = terminator
            else {
                // A folded assertion cannot be used as an emitted runtime guard.
                return Ok(());
            };
            source_reference_owned_prepay_v29::<SourceReferenceDescriptorGuardUseV29>(
                references.plan,
                budget,
            )?;
            let scalar = |value, budget: &mut dyn SemanticEmissionBudgetV1| {
                charge_execution_cfg_lookup_v29(this.semantic_ssa_bindings.len(), budget)?;
                match this.semantic_ssa_bindings.get(&value) {
                    Some(SemanticValueBindingV1::Value {
                        id,
                        ty: Type::Scalar(scalar),
                    }) => Ok((*id, *scalar)),
                    _ => Err(source_descriptor_error_v29()),
                }
            };
            let checked_condition = scalar(source.condition, budget)?;
            let (original_index, index_scalar) = scalar(source.index, budget)?;
            let (length, length_scalar) = scalar(source.length, budget)?;
            if checked_condition != (*condition, ScalarType::Bool)
                || !matches!(index_scalar, ScalarType::Index | ScalarType::U64)
                || !matches!(length_scalar, ScalarType::Index | ScalarType::U64)
                || *then_target != this.kernel_block_id_v1(source.success)?
            {
                return Err(source_descriptor_error_v29());
            }
            charge_execution_cfg_lookup_v29(this.semantic_ssa_bindings.len(), budget)?;
            let archived = this
                .semantic_ssa_bindings
                .get(&source.holder)
                .ok_or_else(source_descriptor_error_v29)?;
            let (slice, ty) =
                source_descriptor_binding_v29(archived, source.source, source.fields, budget)?;
            let representation = references.plan.descriptor_value_space(
                cursor.instance,
                references.plan.descriptor_guards[index].holder,
                source.source,
                source.fields,
                source.pointer_type,
                budget,
            )?;
            check_source_descriptor_type_v29(
                this.types,
                source.pointer_type,
                source.element,
                representation,
                ty,
                budget,
            )?;
            let cell = references
                .descriptor_guards
                .get(index)
                .ok_or_else(source_descriptor_error_v29)?;
            if cell.get().is_some() {
                return Err(source_descriptor_error_v29());
            }
            cell.set(Some(SourceReferenceDescriptorGuardUseV29 {
                condition: *condition,
                index: original_index,
                index_scalar,
                length,
                length_scalar,
                slice,
                block: this.kernel_block_id_v1(original_block)?,
                success: *then_target,
                failure: *else_target,
            }));
            Ok(())
        })
    }
}

#[derive(Clone, Copy)]
struct SourceDescriptorCfgQueryV29 {
    instance: usize,
    guard: BlockId,
    access: BlockId,
    checked: bool,
}

impl SourceReferenceEmissionV29<'_, '_> {
    fn check_descriptor_guard(
        &self,
        row: SourceReferenceDescriptorGuardV29,
        used: SourceReferenceDescriptorGuardUseV29,
        lowered: &LoweredFunctionResultV1,
        definitions: &BTreeMap<(usize, ValueId), SourceSelectorDefinitionV29<'_>>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<SsaValueV1, ProductionSemanticKirErrorV1> {
        let source = row
            .check(self.plan.instances, budget)?
            .ok_or_else(source_descriptor_error_v29)?;
        budget.charge_work(argument_product_v1(lowered.blocks.len(), 3)?)?;
        if lowered
            .blocks
            .iter()
            .filter(|block| {
                block.semantic_block.index() == row.assertion && block.kernel_ir_block == used.block
            })
            .count()
            != 1
        {
            return Err(source_descriptor_error_v29());
        }
        let body = lowered
            .function
            .body
            .as_ref()
            .ok_or_else(source_descriptor_error_v29)?;
        budget.charge_work(body.blocks.len())?;
        let block = body
            .blocks
            .iter()
            .find(|block| block.id == used.block)
            .ok_or_else(source_descriptor_error_v29)?;
        if !matches!(block.terminator.as_ref(), Some(Terminator::ConditionalBranch {
            condition, then_target, else_target, ..
        }) if *condition == used.condition && *then_target == used.success && *else_target == used.failure)
            || used.success == used.failure
        {
            return Err(source_descriptor_error_v29());
        }
        let condition =
            self.selector_definition(definitions, row.instance.index(), used.condition, budget)?;
        if *condition.0 != Type::BOOL {
            return Err(source_descriptor_error_v29());
        }
        let Some(OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs,
            rhs,
        }) = condition.1.map(|op| &op.kind)
        else {
            return Err(source_descriptor_error_v29());
        };
        let (block, operation) = condition.2.ok_or_else(source_descriptor_error_v29)?;
        self.check_source_occurrence_span(
            row.instance,
            row.comparison,
            lowered,
            block,
            operation,
            budget,
        )?;
        let index =
            self.selector_definition(definitions, row.instance.index(), used.index, budget)?;
        let length =
            self.selector_definition(definitions, row.instance.index(), used.length, budget)?;
        if *index.0 != Type::Scalar(used.index_scalar)
            || *length.0 != Type::Scalar(used.length_scalar)
            || !matches!(used.index_scalar, ScalarType::Index | ScalarType::U64)
            || !matches!(used.length_scalar, ScalarType::Index | ScalarType::U64)
        {
            return Err(source_descriptor_error_v29());
        }
        if used.index_scalar == ScalarType::U64 && used.length_scalar == ScalarType::U64 {
            if *lhs != used.index || *rhs != used.length {
                return Err(source_descriptor_error_v29());
            }
        } else {
            self.check_source_index_normalization(
                row.instance.index(),
                used.index,
                used.index_scalar,
                None,
                *lhs,
                definitions,
                block,
                operation,
                budget,
            )?;
            self.check_source_index_normalization(
                row.instance.index(),
                used.length,
                used.length_scalar,
                None,
                *rhs,
                definitions,
                block,
                operation,
                budget,
            )?;
        }
        let mut definition = length;
        if used.length_scalar == ScalarType::U64 {
            let Some(OperationKind::Cast {
                kind: CastKind::Bitcast,
                value,
                to: Type::Scalar(ScalarType::U64),
            }) = definition.1.map(|op| &op.kind)
            else {
                return Err(source_descriptor_error_v29());
            };
            let (block, operation) = definition.2.ok_or_else(source_descriptor_error_v29)?;
            self.check_source_occurrence_span(
                row.instance,
                row.metadata,
                lowered,
                block,
                operation,
                budget,
            )?;
            definition =
                self.selector_definition(definitions, row.instance.index(), *value, budget)?;
        }
        if *definition.0 != Type::INDEX
            || !matches!(definition.1.map(|op| &op.kind), Some(OperationKind::SliceLength { slice }) if *slice == used.slice)
        {
            return Err(source_descriptor_error_v29());
        }
        let (block, operation) = definition.2.ok_or_else(source_descriptor_error_v29)?;
        self.check_source_occurrence_span(
            row.instance,
            row.metadata,
            lowered,
            block,
            operation,
            budget,
        )?;
        let descriptor =
            self.selector_definition(definitions, row.instance.index(), used.slice, budget)?;
        let representation = self.plan.descriptor_value_space(
            row.instance,
            row.holder,
            source.source,
            source.fields,
            source.pointer_type,
            budget,
        )?;
        check_source_descriptor_type_v29(
            self.plan.instances.owner().source_semantic().types(),
            source.pointer_type,
            source.element,
            representation,
            descriptor.0,
            budget,
        )?;
        Ok(source.index)
    }

    fn check_descriptor_access(
        &self,
        descriptor_ordinal: usize,
        row: SourceReferenceDescriptorV29,
        used: SourceReferenceDescriptorUseV29,
        lowered: &LoweredFunctionResultV1,
        definitions: &BTreeMap<(usize, ValueId), SourceSelectorDefinitionV29<'_>>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<BlockId, ProductionSemanticKirErrorV1> {
        let source = row.check(self.plan.instances, budget)?;
        let SourceReferenceSelectorProducerV29::Address {
            base,
            offset,
            pointer,
            block,
            operation,
        } = used.producer
        else {
            return Err(source_descriptor_error_v29());
        };
        self.check_source_occurrence_span(
            row.instance,
            row.occurrence,
            lowered,
            block,
            operation,
            budget,
        )?;
        self.check_source_index_normalization(
            row.instance.index(),
            used.original,
            used.scalar,
            None,
            offset,
            definitions,
            block,
            operation,
            budget,
        )?;
        let descriptor =
            self.selector_definition(definitions, row.instance.index(), used.slice, budget)?;
        let representation = self.plan.descriptor_value_space(
            row.instance,
            row.holder_occurrence,
            source,
            row.projection - 1,
            row.pointer_type,
            budget,
        )?;
        check_source_descriptor_type_v29(
            self.plan.instances.owner().source_semantic().types(),
            row.pointer_type,
            row.element,
            representation,
            descriptor.0,
            budget,
        )?;
        let Type::Slice(slice) = descriptor.0 else {
            return Err(source_descriptor_error_v29());
        };
        let data = self.selector_definition(definitions, row.instance.index(), base, budget)?;
        let address =
            self.selector_definition(definitions, row.instance.index(), pointer, budget)?;
        if !matches!(data.1.map(|op| &op.kind), Some(OperationKind::SliceData { slice }) if *slice == used.slice)
            || data
                .2
                .is_none_or(|(owner, ordinal)| owner != block || ordinal >= operation)
            || address.2 != Some((block, operation))
            || !matches!(address.1.map(|op| &op.kind), Some(OperationKind::GetElementPointer { base: actual, offset: index }) if *actual == base && *index == offset)
            || data.0 != address.0
        {
            return Err(source_descriptor_error_v29());
        }
        let Type::Pointer(physical) = address.0 else {
            return Err(source_descriptor_error_v29());
        };
        if physical.address_space != slice.address_space
            || physical.access != slice.access
            || physical.pointee != slice.element
        {
            return Err(source_descriptor_error_v29());
        }
        let occurrences = self
            .plan
            .instances
            .occurrences(row.instance)
            .ok_or_else(source_descriptor_error_v29)?;
        let event = &occurrences.events()[row.occurrence];
        let source = self
            .plan
            .instances
            .instance(row.instance)
            .ok_or_else(source_descriptor_error_v29)?
            .declaration();
        let write = matches!(
            event.operand(),
            ExecutionOperandV29::Destination | ExecutionOperandV29::StoreDestination
        );
        let anchors = lowered
            .scoped_memory_anchors
            .as_ref()
            .ok_or_else(source_descriptor_error_v29)?;
        if anchors.subject.instance != row.instance
            || anchors.subject.ledger != budget.work_ledger_identity_v1()
            || anchors.subject.source
                != ExecutionCallSourceV29::from_instances(self.plan.instances, budget)?
        {
            return Err(source_descriptor_error_v29());
        }
        let volatile = match (
            scoped_source_statement_v29(source, event.site()),
            event.operand(),
        ) {
            (
                Some(SemanticStatementKindV1::Assign(assignment)),
                ExecutionOperandV29::RvaluePlace,
            ) => match assignment.value().kind() {
                SemanticRvalueKindV1::Load(load) if load.atomic().is_none() => {
                    load.volatility() == SemanticVolatilityV1::Volatile
                }
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place,
                } => {
                    source_reference_owned_prepay_v29::<SourceReferenceSiteV29>(self.plan, budget)?;
                    let ExecutionSiteV29::Statement {
                        block: source_block,
                        statement,
                    } = event.site()
                    else {
                        return Err(source_descriptor_error_v29());
                    };
                    let site = SourceReferenceSiteV29 {
                        instance: row.instance,
                        block: SemanticBlockIdV1::from_index(source_block.get()),
                        statement: Some(statement as usize),
                    };
                    let receipt = source_external_reference_borrow_record_v29(
                        self.plan,
                        site,
                        place,
                        assignment.value().result_type(),
                        SemanticBorrowKindV1::Shared,
                        budget,
                    )?
                    .ok_or_else(source_descriptor_error_v29)?;
                    if place as *const SemanticPlaceV1 as usize != row.source
                        || self.plan.external_borrows[receipt].origin
                            != (SourceExternalReferenceOriginV29::Descriptor {
                                instance: row.instance,
                                descriptor: descriptor_ordinal,
                            })
                        || !self
                            .external_borrows
                            .get(receipt)
                            .is_some_and(std::cell::Cell::get)
                    {
                        return Err(source_descriptor_error_v29());
                    }
                    // This occurrence forms the checked address, not a load.
                    // The caller still checks that its original bounds success
                    // edge dominates this GEP; C2 checks each eventual access.
                    return Ok(block);
                }
                _ => return Err(source_descriptor_error_v29()),
            },
            (
                Some(SemanticStatementKindV1::Assign(_)),
                ExecutionOperandV29::Destination | ExecutionOperandV29::RvalueOperand(_),
            ) => false,
            (
                Some(SemanticStatementKindV1::Store(store)),
                ExecutionOperandV29::StoreDestination,
            ) if store.atomic().is_none() => store.volatility() == SemanticVolatilityV1::Volatile,
            _ => return Err(source_descriptor_error_v29()),
        };
        let body = lowered
            .function
            .body
            .as_ref()
            .ok_or_else(source_descriptor_error_v29)?;
        budget.charge_work(body.blocks.len())?;
        let body_block = body
            .blocks
            .iter()
            .find(|candidate| candidate.id == block)
            .ok_or_else(source_descriptor_error_v29)?;
        let mut matches = 0;
        for anchor in &anchors.rows {
            budget.charge_work(7)?;
            if !matches!(anchor.kind, ScopedMemoryAnchorKindV29::Access { pointer: actual, .. } if actual == pointer)
            {
                continue;
            }
            if anchor.block != block
                || anchor.position <= operation
                || anchor.source
                    != Some(ScopedMemoryFrameV29::operand(
                        event.site(),
                        Some(event.operand()),
                    ))
            {
                return Err(source_descriptor_error_v29());
            }
            self.check_source_occurrence_span(
                row.instance,
                row.occurrence,
                lowered,
                block,
                anchor.position,
                budget,
            )?;
            let access = body_block
                .operations
                .get(anchor.position)
                .ok_or_else(source_descriptor_error_v29)?;
            let memory = match (&access.kind, write) {
                (
                    OperationKind::Load {
                        pointer: actual,
                        access: memory,
                    },
                    false,
                ) if *actual == pointer => {
                    if access.results.len() != 1 || access.results[0].ty != *slice.element {
                        return Err(source_descriptor_error_v29());
                    }
                    memory
                }
                (
                    OperationKind::Store {
                        pointer: actual,
                        value,
                        access: memory,
                    },
                    true,
                ) if *actual == pointer => {
                    if slice.access != AccessMode::ReadWrite
                        || !access.results.is_empty()
                        || *self
                            .selector_definition(definitions, row.instance.index(), *value, budget)?
                            .0
                            != *slice.element
                    {
                        return Err(source_descriptor_error_v29());
                    }
                    memory
                }
                _ => return Err(source_descriptor_error_v29()),
            };
            if memory.address_space != slice.address_space
                || memory.volatile != volatile
                || u64::from(memory.alignment)
                    != self.plan.instances.owner().source_semantic().types()
                        [row.element.index() as usize]
                        .layout()
                        .alignment_bytes()
            {
                return Err(source_descriptor_error_v29());
            }
            matches = argument_sum_v1(&[matches, 1])?;
        }
        if matches != 1 {
            return Err(source_descriptor_error_v29());
        }
        Ok(block)
    }

    fn check_source_descriptors(
        &self,
        emitted: &[Option<LoweredFunctionResultV1>],
        definitions: &BTreeMap<(usize, ValueId), SourceSelectorDefinitionV29<'_>>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.plan.descriptors.is_empty() {
            return Ok(());
        }
        type GuardKey = (usize, SsaValueV1, ValueId);
        let before = budget.storage();
        source_reference_owned_prepay_v29::<
            BTreeMap<GuardKey, Vec<SourceReferenceDescriptorGuardUseV29>>,
        >(self.plan, budget)?;
        let mut guards: BTreeMap<GuardKey, Vec<SourceReferenceDescriptorGuardUseV29>> =
            BTreeMap::new();
        for (ordinal, row) in self.plan.descriptor_guards.iter().enumerate() {
            budget.charge_work(2)?;
            let Some(used) = self
                .descriptor_guards
                .get(ordinal)
                .ok_or_else(source_descriptor_error_v29)?
                .get()
            else {
                continue;
            };
            let lowered = emitted
                .get(row.instance.index())
                .and_then(Option::as_ref)
                .ok_or_else(source_descriptor_error_v29)?;
            let index = self.check_descriptor_guard(*row, used, lowered, definitions, budget)?;
            let key = (row.instance.index(), index, used.slice);
            charge_execution_cfg_lookup_v29(guards.len(), budget)?;
            if !guards.contains_key(&key) {
                reserve_execution_cfg_map_entry_v29::<
                    GuardKey,
                    Vec<SourceReferenceDescriptorGuardUseV29>,
                >(guards.len(), budget)?;
                guards.insert(key, Vec::new());
            }
            charge_execution_cfg_lookup_v29(guards.len(), budget)?;
            emission_push_v1(
                guards
                    .get_mut(&key)
                    .ok_or_else(source_descriptor_error_v29)?,
                used,
                budget,
            )?;
        }
        source_reference_owned_prepay_v29::<Vec<SourceDescriptorCfgQueryV29>>(self.plan, budget)?;
        let mut queries = Vec::new();
        let mut ranges = emission_vec_v1(self.plan.descriptors.len(), budget)?;
        for (ordinal, row) in self.plan.descriptors.iter().enumerate() {
            budget.charge_work(3)?;
            let used = self
                .descriptors
                .get(ordinal)
                .and_then(std::cell::Cell::get)
                .ok_or_else(source_descriptor_error_v29)?;
            let lowered = emitted
                .get(row.instance.index())
                .and_then(Option::as_ref)
                .ok_or_else(source_descriptor_error_v29)?;
            let access =
                self.check_descriptor_access(ordinal, *row, used, lowered, definitions, budget)?;
            charge_execution_cfg_lookup_v29(guards.len(), budget)?;
            let candidates = guards
                .get(&(row.instance.index(), row.index_value, used.slice))
                .ok_or_else(source_descriptor_error_v29)?;
            let first = queries.len();
            for guard in candidates {
                budget.charge_work(3)?;
                if guard.index != used.original || guard.index_scalar != used.scalar {
                    continue;
                }
                emission_push_v1(
                    &mut queries,
                    SourceDescriptorCfgQueryV29 {
                        instance: row.instance.index(),
                        guard: guard.block,
                        access,
                        checked: false,
                    },
                    budget,
                )?;
            }
            ranges.push(first..queries.len());
        }
        let mut first = 0;
        while first < queries.len() {
            let instance = queries[first].instance;
            let mut end = first;
            while end < queries.len() && queries[end].instance == instance {
                budget.charge_work(1)?;
                end += 1;
            }
            let lowered = emitted
                .get(instance)
                .and_then(Option::as_ref)
                .ok_or_else(source_descriptor_error_v29)?;
            fe2o3_kernel_ir::with_function_control_flow_v1(
                &lowered.function,
                Default::default(),
                budget,
                |view| {
                    for query in &mut queries[first..end] {
                        query.checked =
                            view.success_edge_dominates(query.guard, 0, query.access)?;
                    }
                    Ok(())
                },
            )
            .map_err(|error| match error {
                fe2o3_kernel_ir::FunctionControlFlowScopeErrorV1::Resource(error) => error.into(),
                _ => source_descriptor_error_v29(),
            })?;
            first = end;
        }
        for range in &ranges {
            budget.charge_work(range.len())?;
            if !queries[range.clone()].iter().any(|query| query.checked) {
                return Err(source_descriptor_error_v29());
            }
        }
        let owned = budget
            .storage()
            .checked_sub(before)
            .ok_or(ArgumentResourceV1::Accounting)?;
        drop((guards, queries, ranges));
        budget.release_storage(owned)?;
        Ok(())
    }
}
