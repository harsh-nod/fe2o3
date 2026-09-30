// These receipts preserve an existing allocation-backed reference. They do not
// create local storage, a private referent snapshot, or allocation authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceExternalReferenceOriginV29 {
    Descriptor {
        instance: ProductionCallInstanceIdV1,
        descriptor: usize,
    },
    Issued {
        instance: ProductionCallInstanceIdV1,
        recipe: SourceIssuedSemanticRecipeV29,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceExternalReferenceBorrowV29 {
    site: SourceReferenceSiteV29,
    source: usize,
    kind: SemanticBorrowKindV1,
    ty: SemanticTypeIdV1,
    origin: SourceExternalReferenceOriginV29,
}

fn source_external_reference_error_v29() -> ProductionSemanticKirErrorV1 {
    source_reference_error_v29("external reference borrow differs from its original checked origin")
}

fn source_external_borrow_preserves_holder_v29(
    references: &SourceReferenceEmissionV29<'_, '_>,
    site: SourceReferenceSiteV29,
    statement: &SemanticStatementKindV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    references.check(budget)?;
    source_reference_owned_prepay_v29::<bool>(references.plan, budget)?;
    budget.source_reference_charge_v29(references.plan, 4)?;
    let SemanticStatementKindV1::Assign(assignment) = statement else {
        return Ok(false);
    };
    let SemanticRvalueKindV1::Borrow { kind, place } = assignment.value().kind() else {
        return Ok(false);
    };
    if !matches!(place.projections(), [projection]
        if projection.kind() == SemanticProjectionKindV1::Dereference)
    {
        return Ok(false);
    }
    let original = references
        .plan
        .instances
        .instance(site.instance)
        .ok_or_else(source_external_reference_error_v29)?
        .declaration();
    if site
        .statement
        .and_then(|index| {
            original
                .blocks()
                .get(site.block.index() as usize)
                .and_then(|block| block.statements().get(index))
        })
        .is_none_or(|actual| !std::ptr::eq(actual.kind(), statement))
    {
        return Err(source_external_reference_error_v29());
    }
    // A checked reborrow can mutate the pointee, never the pointer holder.
    // Recompute original issuer/descriptor evidence, including missing-row refusal.
    Ok(budget
        .source_external_reference_borrow_v29(
            references.plan,
            site,
            place,
            assignment.value().result_type(),
            *kind,
        )?
        .is_some())
}

// The original assignment, including its output type, selects transparent
// reborrows. A raw pointer, widened mutable borrow, or projected pointer cannot.
fn source_reference_pointer_alias_v29<'a>(
    types: &[SemanticTypeDeclV1],
    function: &'a SemanticFunctionDeclV1,
    site: ExecutionSiteV29,
    rvalue: &'a SemanticRvalueKindV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Option<(&'a SemanticPlaceV1, ExecutionOperandV29, bool)>, ProductionSemanticKirErrorV1>
{
    source_reference_emission_prepay_v29::<Option<(&SemanticPlaceV1, ExecutionOperandV29, bool)>>(
        budget,
    )?;
    budget.charge_work(8)?;
    match rvalue {
        SemanticRvalueKindV1::Use(
            SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place),
        ) => Ok(Some((place, ExecutionOperandV29::RvalueOperand(0), false))),
        SemanticRvalueKindV1::Borrow { kind, place } => {
            let Some(SemanticStatementKindV1::Assign(assignment)) =
                scoped_source_statement_v29(function, site)
            else {
                return Err(source_external_reference_error_v29());
            };
            let local = function
                .locals()
                .get(place.local().index() as usize)
                .ok_or_else(source_external_reference_error_v29)?;
            if !std::ptr::eq(assignment.value().kind(), rvalue)
                || assignment.value().result_type() != assignment.destination().ty()
                || assignment.destination().ty() != local.ty()
                || place.projections().len() != 1
                || place.projections()[0].kind() != SemanticProjectionKindV1::Dereference
                || place.projections()[0].result_type() != place.ty()
            {
                return Ok(None);
            }
            let Some(SemanticTypeShapeV1::Pointer(pointer)) = types
                .get(local.ty().index() as usize)
                .map(SemanticTypeDeclV1::shape)
            else {
                return Ok(None);
            };
            if pointer.kind() != SemanticPointerKindV1::Reference
                || pointer.metadata() != SemanticPointerMetadataV1::None
                || pointer.pointee() != place.ty()
                || !matches!(
                    (*kind, pointer.mutability()),
                    (
                        SemanticBorrowKindV1::Shared,
                        SemanticMutabilityV1::Immutable
                    ) | (SemanticBorrowKindV1::Mutable, SemanticMutabilityV1::Mutable)
                )
            {
                return Ok(None);
            }
            Ok(Some((place, ExecutionOperandV29::RvaluePlace, true)))
        }
        _ => Ok(None),
    }
}

fn source_external_descriptor_origin_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    instance: ProductionCallInstanceIdV1,
    site: ExecutionSiteV29,
    source: &SemanticPlaceV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Option<SourceExternalReferenceOriginV29>, ProductionSemanticKirErrorV1> {
    budget.charge_work(4)?;
    if source.projections().len() != 2
        || source.projections()[0].kind() != SemanticProjectionKindV1::Dereference
        || !matches!(
            source.projections()[1].kind(),
            SemanticProjectionKindV1::Index(_)
        )
    {
        return Ok(None);
    }
    let Some((descriptor, row)) = plan.descriptor_at(instance, site, source, 1, budget)? else {
        return Ok(None);
    };
    row.check(plan.instances, budget)?;
    if row.element != source.ty() {
        return Err(source_external_reference_error_v29());
    }
    // This fact was derived by the common C1 descriptor transfer from the
    // authenticated root ABI, including exact helper argument edges.
    plan.descriptor_value_space(
        instance,
        row.holder_occurrence,
        source,
        0,
        row.pointer_type,
        budget,
    )?;
    Ok(Some(SourceExternalReferenceOriginV29::Descriptor {
        instance,
        descriptor,
    }))
}

fn source_external_reference_origin_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    site: SourceReferenceSiteV29,
    source: &SemanticPlaceV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Option<SourceExternalReferenceOriginV29>, ProductionSemanticKirErrorV1> {
    let statement = site
        .statement
        .ok_or_else(source_external_reference_error_v29)?;
    source_external_reference_origin_from_use_v29(
        plan,
        site.instance,
        execution_site_v29(
            site.block,
            Some(u32::try_from(statement).map_err(|_| ArgumentResourceV1::Arithmetic)?),
        ),
        ExecutionOperandV29::RvaluePlace,
        source,
        None,
        budget,
    )
}

#[allow(clippy::too_many_arguments)]
fn source_external_reference_origin_from_use_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    mut instance: ProductionCallInstanceIdV1,
    mut execution: ExecutionSiteV29,
    mut role: ExecutionOperandV29,
    source: &SemanticPlaceV1,
    source_index: Option<&SourceAddressSourceIndexV29<'_>>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Option<SourceExternalReferenceOriginV29>, ProductionSemanticKirErrorV1> {
    with_canonical_call_scratch_v1(budget, |budget| {
        budget.reserve_storage(argument_sum_v1(&[
            std::mem::size_of::<SourceReferenceSiteV29>(),
            std::mem::size_of::<SourceExternalReferenceOriginV29>(),
            std::mem::size_of::<Option<SourceExternalReferenceOriginV29>>(),
            std::mem::size_of::<Option<&SourceAddressSourceIndexV29<'_>>>(),
            argument_product_v1(
                3,
                std::mem::size_of::<
                    Result<Option<SourceExternalReferenceOriginV29>, ProductionSemanticKirErrorV1>,
                >(),
            )?,
            std::mem::size_of::<(
                ProductionCallInstanceIdV1,
                ExecutionSiteV29,
                ExecutionOperandV29,
                &SemanticPlaceV1,
            )>(),
        ])?)?;
        if let Some(origin) =
            source_external_descriptor_origin_v29(plan, instance, execution, source, budget)?
        {
            return Ok(Some(origin));
        }
        let mut operand = source;
        loop {
            let mut original = SourceIssuedSemanticV29::new(
                plan.instances,
                instance,
                SourceIssuedReplayModeV29::SourceOnly,
                budget,
            )?;
            let mut value = original.use_value(execution, role, operand, budget)?;
            let function = plan
                .instances
                .instance(instance)
                .ok_or_else(source_external_reference_error_v29)?
                .declaration();
            let occurrences = plan
                .instances
                .occurrences(instance)
                .ok_or_else(source_external_reference_error_v29)?;
            let mut entry = None;
            // Definitions are immutable and finite. A join without one exact
            // origin remains a refusal, not an arbitrary selected predecessor.
            for _ in 0..=original.definitions.len() {
                if let Some(recipe) = original.resolve(value, budget)? {
                    if recipe.form != SourceIssuedFormV29::Pointer {
                        return Ok(None);
                    }
                    return Ok(Some(SourceExternalReferenceOriginV29::Issued {
                        instance,
                        recipe,
                    }));
                }
                charge_execution_cfg_lookup_v29(original.definitions.len(), budget)?;
                match original.definitions.get(&value).copied() {
                    Some(SourceIssuedDefinitionV29::Statement(definition)) => {
                        let (at, rvalue) = source_descriptor_assignment_v29(
                            function,
                            &occurrences,
                            definition,
                            value,
                            budget,
                        )?;
                        if let SemanticRvalueKindV1::Borrow {
                            place,
                            kind: SemanticBorrowKindV1::Shared,
                        } = rvalue
                        {
                            if let Some(origin) = source_external_descriptor_origin_v29(
                                plan, instance, at, place, budget,
                            )? {
                                return Ok(Some(origin));
                            }
                        }
                        let Some((place, next_role, reborrow)) =
                            source_reference_pointer_alias_v29(
                                plan.instances.owner().source_semantic().types(),
                                function,
                                at,
                                rvalue,
                                budget,
                            )?
                        else {
                            return Ok(None);
                        };
                        if !reborrow && !place.projections().is_empty() {
                            return Ok(None);
                        }
                        value = original.use_value(at, next_role, place, budget)?;
                    }
                    Some(SourceIssuedDefinitionV29::Edge(_)) => return Ok(None),
                    None => {
                        entry = original.entry_dependency_v26(value, budget)?;
                        break;
                    }
                }
            }
            let Some(local) = entry else {
                return Ok(None);
            };
            let Some(incoming) = plan.instances.incoming(instance) else {
                return Ok(None);
            };
            let occurrence = incoming.occurrence();
            if incoming.child() != Some(instance) || occurrence.caller.index() >= instance.index() {
                return Err(source_external_reference_error_v29());
            }
            let selector = plan
                .instances
                .parameter_source(instance, local, budget)
                .map_err(|error| match error {
                    production_call_instances_v1::ProductionCallInstanceErrorV1::Resource(
                        error,
                    ) => error.into(),
                    _ => source_external_reference_error_v29(),
                })?;
            if selector.tuple_field.is_some() {
                return Ok(None);
            }
            let argument = incoming
                .source()
                .arguments()
                .get(selector.source_argument as usize)
                .ok_or_else(source_external_reference_error_v29)?;
            let (SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) = argument else {
                return Ok(None);
            };
            if !std::ptr::eq(selector.operand, argument)
                || !place.projections().is_empty()
                || place.ty() != selector.ty
            {
                return Err(source_external_reference_error_v29());
            }
            if let Some(index) = source_index {
                check_source_reference_parameter_transport_v29(
                    index,
                    instance,
                    local,
                    selector.source_argument,
                    selector.ty,
                    budget,
                )?;
            }
            instance = occurrence.caller;
            execution = ExecutionSiteV29::Terminator {
                block: SsaBlockIdV1::new(occurrence.block.index()),
            };
            role = ExecutionOperandV29::CallArgument(selector.source_argument);
            operand = place;
        }
    })
}

impl SourceIssuedAccessesV29<'_, '_, '_> {
    // Read-only descriptor references use the existing actual-pointer census
    // and transport solver. This classification cannot issue a write permit.
    fn descriptor_reference_access_v29(
        &mut self,
        references: &SourceReferenceEmissionV29<'_, '_>,
        instance: ProductionCallInstanceIdV1,
        row: &ScopedMemoryAnchorV29,
        place: &SemanticPlaceV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        self.check(budget)?;
        references.plan.check_owner(self.instances, budget)?;
        source_reference_owned_prepay_v29::<bool>(references.plan, budget)?;
        budget.charge_work(14)?;
        let frame = row.source.ok_or_else(source_external_reference_error_v29)?;
        let Some(ScopedMemoryRoleV29::Operand(role)) = frame.role else {
            return Err(source_external_reference_error_v29());
        };
        let source = self.instances.owner().source_semantic();
        let function = self
            .instances
            .instance(instance)
            .ok_or_else(source_external_reference_error_v29)?
            .declaration();
        let local = function
            .locals()
            .get(place.local().index() as usize)
            .ok_or_else(source_external_reference_error_v29)?;
        if place.projections().len() != 1
            || place.projections()[0].kind() != SemanticProjectionKindV1::Dereference
            || !matches!(source.types().get(local.ty().index() as usize).map(SemanticTypeDeclV1::shape),
                Some(SemanticTypeShapeV1::Pointer(pointer))
                    if pointer.kind() == SemanticPointerKindV1::Reference
                        && pointer.metadata() == SemanticPointerMetadataV1::None
                        && pointer.mutability() == SemanticMutabilityV1::Immutable
                        && pointer.pointee() == place.ty())
        {
            return Ok(false);
        }
        let Some(SourceExternalReferenceOriginV29::Descriptor {
            instance: producer_instance,
            descriptor,
        }) = source_external_reference_origin_from_use_v29(
            references.plan,
            instance,
            frame.site,
            role,
            place,
            Some(self.source_index),
            budget,
        )?
        else {
            return Ok(false);
        };
        source_reference_owned_prepay_v29::<SourceAddressValueAccessV29>(references.plan, budget)?;
        source_reference_owned_prepay_v29::<[SourceIssuedPointerTransportV26; 1]>(
            references.plan,
            budget,
        )?;
        source_reference_owned_prepay_v29::<Option<SourceReferenceDescriptorUseV29>>(
            references.plan,
            budget,
        )?;
        let original_descriptor = references
            .plan
            .descriptors
            .get(descriptor)
            .ok_or_else(source_descriptor_error_v29)?;
        original_descriptor.check(self.instances, budget)?;
        if original_descriptor.instance != producer_instance
            || original_descriptor.element != place.ty()
        {
            return Err(source_descriptor_error_v29());
        }
        let claim = references
            .descriptors
            .get(descriptor)
            .and_then(std::cell::Cell::get)
            .ok_or_else(source_descriptor_error_v29)?;
        let SourceReferenceSelectorProducerV29::Address {
            base,
            offset,
            pointer,
            block,
            operation,
        } = claim.producer
        else {
            return Err(source_descriptor_error_v29());
        };
        let producer =
            self.source_index
                .emitted
                .operation(producer_instance, block, operation, budget)?;
        let (
            OperationKind::GetElementPointer {
                base: actual_base,
                offset: actual_offset,
            },
            [result],
        ) = (&producer.kind, producer.results.as_slice())
        else {
            return Err(source_descriptor_error_v29());
        };
        let Type::Scalar(element) = lower_scalar_type(source.types(), place.ty())? else {
            return Err(source_descriptor_error_v29());
        };
        if *actual_base != base
            || *actual_offset != offset
            || result.id != pointer
            || source_issued_pointer_shape_v26(&result.ty)
                != Some((element, AddressSpace::Global, AccessMode::ReadOnly))
        {
            return Err(source_descriptor_error_v29());
        }
        self.source_index
            .frame_gap(instance, frame, row.block, row.position, budget)?;
        let actual_operation =
            self.source_index
                .emitted
                .operation(instance, row.block, row.position, budget)?;
        let occurrences = self
            .instances
            .occurrences(instance)
            .ok_or_else(source_descriptor_error_v29)?;
        check_scoped_payload_v29(function, &occurrences, row, actual_operation, budget)?;
        let access = source_address_value_access_v29(actual_operation)?
            .ok_or_else(source_descriptor_error_v29)?;
        if access.writing
            || access.object
            || access.access.alignment == 0
            || u64::from(access.access.alignment)
                > source.types()[place.ty().index() as usize]
                    .layout()
                    .alignment_bytes()
            || *self.actual.value(access.value, budget)?.ty != Type::Scalar(element)
            || !matches!(row.kind, ScopedMemoryAnchorKindV29::Access { pointer: expected, .. } if expected == access.pointer)
            || !source_issued_memory_pointer_v26(
                &self.actual,
                access.pointer,
                access.access,
                false,
                element,
                budget,
            )?
            || !matches!(source_issued_pointer_shape_v26(self.actual.value(access.pointer, budget)?.ty),
                Some((actual_element, AddressSpace::Global | AddressSpace::Generic, AccessMode::ReadOnly))
                    if actual_element == element)
        {
            return Err(source_descriptor_error_v29());
        }
        let original = self.original_v26(instance, budget)?;
        let value = original.use_value(frame.site, role, place, budget)?;
        if !matches!(original.archived(value, budget)?,
            SemanticValueBindingV1::Value { id, ty: Type::Pointer(_) } if *id == access.pointer)
        {
            return Err(source_descriptor_error_v29());
        }
        charge_execution_cfg_lookup_v29(self.originals.len(), budget)?;
        let original = self
            .originals
            .get(&instance.index())
            .ok_or_else(source_descriptor_error_v29)?;
        check_source_issued_payload_v29(original, row, actual_operation, &self.actual, budget)?;
        check_source_issued_pointer_transports_v26(
            &self.source_index.pending.function,
            &self.actual,
            &[SourceIssuedPointerTransportV26 {
                pointer: access.pointer,
                issuer: pointer,
            }],
            budget,
        )?;
        Ok(true)
    }
}

impl SourceReferenceBuilderV29<'_, '_, '_> {
    fn external_reference_borrow_v29(
        &mut self,
        site: SourceReferenceSiteV29,
        kind: SemanticBorrowKindV1,
        source: &SemanticPlaceV1,
        ty: SemanticTypeIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<usize>, ProductionSemanticKirErrorV1> {
        budget.charge_work(8)?;
        let Some(node) = self.local(site.instance, source.local())?.node else {
            return Ok(None);
        };
        if !matches!(
            self.plan.nodes[node].kind,
            SourceReferenceNodeKindV29::Plain(_)
        ) || source.projections().is_empty()
            || source.projections()[0].kind() != SemanticProjectionKindV1::Dereference
        {
            return Ok(None);
        }
        let types = self.plan.instances.owner().source_semantic().types();
        let Some(SemanticTypeShapeV1::Pointer(pointer)) = types
            .get(ty.index() as usize)
            .map(SemanticTypeDeclV1::shape)
        else {
            return Ok(None);
        };
        if pointer.kind() != SemanticPointerKindV1::Reference
            || pointer.metadata() != SemanticPointerMetadataV1::None
            || pointer.pointee() != source.ty()
            || !matches!(
                (kind, pointer.mutability()),
                (
                    SemanticBorrowKindV1::Shared,
                    SemanticMutabilityV1::Immutable
                ) | (SemanticBorrowKindV1::Mutable, SemanticMutabilityV1::Mutable)
            )
        {
            return Ok(None);
        }
        let direct_descriptor =
            source.projections().len() == 2 && kind == SemanticBorrowKindV1::Shared;
        if !direct_descriptor && (source.projections().len() != 1 || self.plan.nodes[node].ty != ty)
        {
            return Ok(None);
        }
        // Read the holder through the common transfer once, retaining its
        // descriptor fact. No dereferenced scalar is adopted as a snapshot.
        self.resolve_reference_place(site, source, SourceReferenceAccessV29::Read, budget)?;
        let Some(origin) = source_external_reference_origin_v29(&self.plan, site, source, budget)?
        else {
            return Ok(None);
        };
        let row = SourceExternalReferenceBorrowV29 {
            site,
            source: source as *const SemanticPlaceV1 as usize,
            kind,
            ty,
            origin,
        };
        budget.charge_work(self.plan.external_borrows.len())?;
        if let Some(previous) = self
            .plan
            .external_borrows
            .iter()
            .find(|row| row.site == site)
        {
            if *previous != row {
                return Err(source_external_reference_error_v29());
            }
        } else {
            emission_push_v1(&mut self.plan.external_borrows, row, budget)?;
        }
        self.plain(ty, budget).map(Some)
    }
}

fn source_external_reference_borrow_record_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    site: SourceReferenceSiteV29,
    source: &SemanticPlaceV1,
    ty: SemanticTypeIdV1,
    kind: SemanticBorrowKindV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Option<usize>, ProductionSemanticKirErrorV1> {
    source_external_reference_borrow_rows_v29(
        plan,
        &plan.external_borrows,
        site,
        source,
        ty,
        kind,
        budget,
    )
}

#[allow(clippy::too_many_arguments)]
fn source_external_reference_borrow_rows_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    rows: &[SourceExternalReferenceBorrowV29],
    site: SourceReferenceSiteV29,
    source: &SemanticPlaceV1,
    ty: SemanticTypeIdV1,
    kind: SemanticBorrowKindV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Option<usize>, ProductionSemanticKirErrorV1> {
    plan.check_owner(plan.instances, budget)?;
    source_reference_owned_prepay_v29::<Option<usize>>(plan, budget)?;
    source_reference_owned_prepay_v29::<SourceExternalReferenceBorrowV29>(plan, budget)?;
    budget.charge_work(rows.len())?;
    let mut matching = rows.iter().enumerate().filter(|(_, row)| row.site == site);
    let retained = matching.next();
    if matching.next().is_some() {
        return Err(source_external_reference_error_v29());
    }
    budget.charge_work(plan.loans.len())?;
    if plan.loans.iter().any(|loan| loan.site == site) {
        // A retained local loan uses the existing checked loan reader. It may
        // be non-promoted and therefore has no external SSA value to invent.
        return if retained.is_none() {
            Ok(None)
        } else {
            Err(source_external_reference_error_v29())
        };
    }
    let types = plan.instances.owner().source_semantic().types();
    let original = plan
        .instances
        .instance(site.instance)
        .ok_or_else(source_external_reference_error_v29)?
        .declaration();
    let original_holder_type = original
        .locals()
        .get(source.local().index() as usize)
        .ok_or_else(source_external_reference_error_v29)?
        .ty();
    let reference = types
        .get(ty.index() as usize)
        .map(SemanticTypeDeclV1::shape);
    let candidate = matches!(reference, Some(SemanticTypeShapeV1::Pointer(pointer))
        if pointer.kind() == SemanticPointerKindV1::Reference
            && pointer.metadata() == SemanticPointerMetadataV1::None
            && pointer.pointee() == source.ty()
            && matches!((kind, pointer.mutability()),
                (SemanticBorrowKindV1::Shared, SemanticMutabilityV1::Immutable)
                | (SemanticBorrowKindV1::Mutable, SemanticMutabilityV1::Mutable)))
        && source
            .projections()
            .first()
            .is_some_and(|p| p.kind() == SemanticProjectionKindV1::Dereference)
        && ((source.projections().len() == 1 && original_holder_type == ty)
            || (source.projections().len() == 2 && kind == SemanticBorrowKindV1::Shared));
    if !candidate {
        return if retained.is_none() {
            Ok(None)
        } else {
            Err(source_external_reference_error_v29())
        };
    }
    let exact = plan
        .instances
        .borrow_at(
            site.instance,
            site.block,
            site.statement
                .ok_or_else(source_external_reference_error_v29)?,
            budget,
        )
        .map_err(|error| match error {
            production_call_instances_v1::ProductionCallInstanceErrorV1::Resource(error) => {
                error.into()
            }
            _ => source_external_reference_error_v29(),
        })?;
    if !std::ptr::eq(exact.source, source) || exact.kind != kind || exact.destination.ty() != ty {
        return Err(source_external_reference_error_v29());
    }
    let origin = source_external_reference_origin_v29(plan, site, source, budget)?;
    match (retained, origin) {
        (None, None) => Ok(None),
        (Some((index, row)), Some(origin))
            if *row
                == (SourceExternalReferenceBorrowV29 {
                    site,
                    source: source as *const SemanticPlaceV1 as usize,
                    kind,
                    ty,
                    origin,
                }) =>
        {
            Ok(Some(index))
        }
        _ => Err(source_external_reference_error_v29()),
    }
}

impl SemanticFunctionLoweringV1<'_, '_> {
    fn try_lower_source_external_reference_v29(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        result_type: SemanticTypeIdV1,
        value: &SemanticRvalueKindV1,
        operations: &mut Vec<Operation>,
    ) -> Result<Option<SemanticValueBindingV1>, ProductionSemanticKirErrorV1> {
        let SemanticRvalueKindV1::Borrow { kind, place } = value else {
            return Ok(None);
        };
        let Some((references, instance)) = self.execution.as_ref().and_then(|cursor| {
            cursor
                .references
                .map(|references| (references, cursor.instance))
        }) else {
            return Ok(None);
        };
        let site = SourceReferenceSiteV29 {
            instance,
            block,
            statement: statement.map(|x| x as usize),
        };
        let Some(index) = self.with_emission_budget_v1(|this, budget| {
            references.check(budget)?;
            let function = references
                .plan
                .instances
                .instance(instance)
                .ok_or_else(source_external_reference_error_v29)?
                .declaration();
            if !std::ptr::eq(function, this.function)
                || !std::ptr::eq(
                    this.types,
                    references.plan.instances.owner().source_semantic().types(),
                )
                || !std::ptr::eq(
                    this.callables,
                    references
                        .plan
                        .instances
                        .owner()
                        .source_semantic()
                        .callables(),
                )
            {
                return Err(source_external_reference_error_v29());
            }
            budget.source_external_reference_borrow_v29(
                references.plan,
                site,
                place,
                result_type,
                *kind,
            )
        })?
        else {
            return Ok(None);
        };
        let binding = if place.projections().len() == 2 {
            // The existing descriptor builder checks the original holder/index
            // archives and records the actual address producer and guard use.
            self.lower_indexed_place_address(block, statement, place, operations)?
        } else {
            self.with_emission_budget_v1(|this, budget| {
                let cursor = this
                    .execution
                    .as_mut()
                    .ok_or_else(source_external_reference_error_v29)?;
                let definition = cursor.use_place(
                    execution_site_v29(block, statement),
                    ExecutionOperandV29::RvaluePlace,
                    place,
                    false,
                    budget,
                )?;
                check_source_use_archive_v29(
                    cursor,
                    &this.control_flow_ssa.cfg_carriers,
                    &this.locals,
                    &this.semantic_ssa_bindings,
                    execution_site_v29(block, statement),
                    ExecutionOperandV29::RvaluePlace,
                    place,
                    definition,
                    budget,
                )
            })?;
            self.resolve_place(block, statement, place, operations)?
        };
        self.with_emission_budget_v1(|this, budget| {
            references.check(budget)?;
            budget.source_reference_charge_v29(references.plan, 8)?;
            let SemanticValueBindingV1::Value {
                ty: Type::Pointer(pointer),
                ..
            } = &binding
            else {
                return Err(source_external_reference_error_v29());
            };
            let scalar = lower_scalar_type(this.types, place.ty())?;
            if *pointer.pointee != scalar
                || !matches!(
                    pointer.address_space,
                    AddressSpace::Global | AddressSpace::Generic
                )
                || !matches!(
                    (*kind, pointer.access),
                    (SemanticBorrowKindV1::Shared, AccessMode::ReadOnly)
                        | (SemanticBorrowKindV1::Mutable, AccessMode::ReadWrite)
                )
            {
                return Err(source_external_reference_error_v29());
            }
            let claimed = references
                .external_borrows
                .get(index)
                .ok_or_else(source_external_reference_error_v29)?;
            if claimed.replace(true) {
                return Err(source_external_reference_error_v29());
            }
            Ok(Some(binding))
        })
    }
}
