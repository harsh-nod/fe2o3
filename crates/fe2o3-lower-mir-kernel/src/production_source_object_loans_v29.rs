// A selected backing is not a scalar-cell permit. These queries retain the
// original safe loan and admit only a whole primitive or flat aggregate referent.
include!("production_source_aggregate_object_loans_v29.rs");
#[derive(Debug)]
struct SourceObjectLoanV29 {
    loan: usize,
    cell: usize,
    original: SourceReferenceScalarCellV29,
    pointer_type: Type,
}

fn source_object_loan_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    loan: usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Option<SourceObjectLoanV29>, ProductionSemanticKirErrorV1> {
    budget.source_reference_owner_v29(plan)?;
    let result = (|| {
        source_reference_owned_prepay_v29::<Option<SourceObjectLoanV29>>(plan, budget)?;
        source_reference_owned_prepay_v29::<SourceObjectLoanV29>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<&SourceReferenceCellStrategyV29>>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<SourceReferenceCellStrategyV29>>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<&SourceReferenceScalarCellV29>>(plan, budget)?;
        source_reference_owned_prepay_v29::<SourceReferenceScalarCellV29>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<&SourceReferenceLoanV29>>(plan, budget)?;
        source_reference_owned_prepay_v29::<&SourceReferenceLoanV29>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<&SourceReferenceOriginV29>>(plan, budget)?;
        source_reference_owned_prepay_v29::<&SourceReferenceOriginV29>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<&SemanticTypeDeclV1>>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<&SemanticTypeShapeV1>>(plan, budget)?;
        budget.source_reference_charge_v29(plan, 18)?;
        let Some(SourceReferenceCellStrategyV29::Object(cell)) =
            plan.cells.strategies.get(loan).copied()
        else {
            return Ok(None);
        };
        let row = *plan
            .cells
            .rows
            .get(cell)
            .ok_or_else(scoped_object_error_v29)?;
        let record = plan.loans.get(loan).ok_or_else(scoped_object_error_v29)?;
        let origin = plan
            .origins
            .get(record.origin)
            .ok_or_else(scoped_object_error_v29)?;
        let types = plan.instances.owner().source_semantic().types();
        let Some(SemanticTypeShapeV1::Pointer(pointer)) = types
            .get(record.source_type.index() as usize)
            .map(|row| row.shape())
        else {
            return Err(scoped_object_error_v29());
        };
        if pointer.kind() != SemanticPointerKindV1::Reference
            || pointer.metadata() != SemanticPointerMetadataV1::None
            || !origin.projections.is_empty()
            || origin.ty != row.ty
            || (!matches!(
                types.get(row.ty.index() as usize).map(|row| row.shape()),
                Some(SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_))
            ) && !source_flat_aggregate_object_v29(types, row.ty, budget)?)
        {
            return Err(scoped_object_pending_v29());
        }
        // This independently rejoins the loan origin, local, generation and
        // selected schema, including mutable/shared pointer permission.
        let pointer_type = budget.source_reference_object_pointer_type_v29(plan, loan)?;
        let SourceBackingKindV29::Object(schema) = row.kind else {
            return Err(scoped_object_error_v29());
        };
        if !matches!(&pointer_type, Type::Pointer(pointer)
            if pointer.address_space == AddressSpace::Private
                && pointer.pointee.as_ref() == &Type::StorageObject(schema))
        {
            return Err(scoped_object_error_v29());
        }
        Ok(Some(SourceObjectLoanV29 {
            loan,
            cell,
            original: row,
            pointer_type,
        }))
    })();
    result.inspect_err(|error| source_reference_record_failure_v29(plan, error))
}

fn source_object_loan_holder_type_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    site: SourceReferenceSiteV29,
    place: &SemanticPlaceV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Option<SemanticTypeIdV1>, ProductionSemanticKirErrorV1> {
    budget.source_reference_owner_v29(plan)?;
    let result = (|| {
        source_reference_owned_prepay_v29::<Option<SemanticTypeIdV1>>(plan, budget)?;
        source_reference_owned_prepay_v29::<
            Option<&production_call_instances_v1::ProductionCallInstanceV1<'_>>,
        >(plan, budget)?;
        source_reference_owned_prepay_v29::<
            &production_call_instances_v1::ProductionCallInstanceV1<'_>,
        >(plan, budget)?;
        source_reference_owned_prepay_v29::<
            Option<&fe2o3_mir_model::semantic_mir_v1::SemanticLocalDeclV1>,
        >(plan, budget)?;
        source_reference_owned_prepay_v29::<&fe2o3_mir_model::semantic_mir_v1::SemanticLocalDeclV1>(
            plan, budget,
        )?;
        source_reference_owned_prepay_v29::<Option<&SemanticTypeDeclV1>>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<&SemanticTypeShapeV1>>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<&SemanticTypeIdV1>>(plan, budget)?;
        source_reference_owned_prepay_v29::<&SemanticTypeIdV1>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<&SemanticProjectionV1>>(plan, budget)?;
        source_reference_owned_prepay_v29::<&SemanticProjectionV1>(plan, budget)?;
        source_reference_owned_prepay_v29::<std::slice::Iter<'_, SemanticProjectionV1>>(
            plan, budget,
        )?;
        budget.source_reference_charge_v29(
            plan,
            argument_sum_v1(&[8, argument_product_v1(place.projections().len(), 4)?])?,
        )?;
        if place.projections().is_empty() || place.projections().len() > 256 {
            return Ok(None);
        }
        let function = plan
            .instances
            .instance(site.instance)
            .ok_or_else(scoped_object_error_v29)?
            .declaration();
        let types = plan.instances.owner().source_semantic().types();
        let mut ty = function
            .locals()
            .get(place.local().index() as usize)
            .ok_or_else(scoped_object_error_v29)?
            .ty();
        for projection in &place.projections()[..place.projections().len() - 1] {
            let SemanticProjectionKindV1::Field(field) = projection.kind() else {
                return Ok(None);
            };
            let fields = match types.get(ty.index() as usize).map(|row| row.shape()) {
                Some(
                    SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields),
                ) => fields.fields(),
                _ => return Ok(None),
            };
            ty = *fields
                .get(field as usize)
                .ok_or_else(scoped_object_error_v29)?;
            if projection.result_type() != ty {
                return Err(scoped_object_error_v29());
            }
        }
        let terminal = place
            .projections()
            .last()
            .ok_or_else(scoped_object_error_v29)?;
        if terminal.kind() != SemanticProjectionKindV1::Dereference {
            return Ok(None);
        }
        let Some(SemanticTypeShapeV1::Pointer(pointer)) =
            types.get(ty.index() as usize).map(|row| row.shape())
        else {
            return Ok(None);
        };
        if pointer.kind() != SemanticPointerKindV1::Reference
            || pointer.metadata() != SemanticPointerMetadataV1::None
        {
            return Ok(None);
        }
        if pointer.pointee() != terminal.result_type() || place.ty() != terminal.result_type() {
            return Err(scoped_object_error_v29());
        }
        Ok(Some(ty))
    })();
    result.inspect_err(|error| source_reference_record_failure_v29(plan, error))
}

fn source_object_loan_access_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    site: SourceReferenceSiteV29,
    place: &SemanticPlaceV1,
    access: SourceReferenceAccessV29,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Option<SourceObjectLoanV29>, ProductionSemanticKirErrorV1> {
    budget.source_reference_owner_v29(plan)?;
    let result = (|| {
        source_reference_owned_prepay_v29::<Option<SourceObjectLoanV29>>(plan, budget)?;
        if plan.storage == SourceReferenceStorageV29::PromotedOnly {
            return Ok(None);
        }
        // A promoted descriptor/aggregate reborrow is not an Object read.
        // Preserve its original Borrow role for the existing fallback path.
        source_reference_owned_prepay_v29::<Option<&SemanticTypeDeclV1>>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<&SemanticTypeShapeV1>>(plan, budget)?;
        budget.source_reference_charge_v29(plan, 2)?;
        if !matches!(
            plan.instances.owner().source_semantic().types()
                .get(place.ty().index() as usize).map(SemanticTypeDeclV1::shape),
            Some(SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_))
        ) {
            return Ok(None);
        }
        let Some(holder) = source_object_loan_holder_type_v29(plan, site, place, budget)? else {
            return Ok(None);
        };
        if !matches!(
            access,
            SourceReferenceAccessV29::Read | SourceReferenceAccessV29::Write
        ) {
            return Err(scoped_object_pending_v29());
        }
        source_reference_owned_prepay_v29::<&SourceReferenceAccessRecordV29>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<&SourceReferenceLoanV29>>(plan, budget)?;
        source_reference_owned_prepay_v29::<&SourceReferenceLoanV29>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<&[usize]>>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<&usize>>(plan, budget)?;
        let source = source_reference_access_at_v29(plan, site, place, access, budget)?;
        let Some(loan) = source.loan else {
            return Ok(None);
        };
        let Some(checked) = source_object_loan_v29(plan, loan, budget)? else {
            return Ok(None);
        };
        budget.source_reference_charge_v29(plan, 12)?;
        let record = plan.loans.get(loan).ok_or_else(scoped_object_error_v29)?;
        if record.source_type != holder
            || source.ty != place.ty()
            || source.ty != checked.original.ty
            || source.instance != checked.original.instance
            || source.local != checked.original.local
            || source.generation != checked.original.generation
            || !source.projections.is_empty()
            || plan
                .access_loans
                .get(source.traversed.clone())
                .and_then(|rows| rows.last())
                != Some(&loan)
            || (access == SourceReferenceAccessV29::Write
                && (source.shared_path || record.kind != SemanticBorrowKindV1::Mutable))
        {
            return Err(scoped_object_error_v29());
        }
        Ok(Some(checked))
    })();
    result.inspect_err(|error| source_reference_record_failure_v29(plan, error))
}

fn source_object_loan_holder_v29<'a>(
    plan: &SourceReferencePlanV29<'_, '_>,
    binding: &'a SemanticValueBindingV1,
    place: &SemanticPlaceV1,
    checked: &SourceObjectLoanV29,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<&'a SemanticSourceReferenceBindingV29, ProductionSemanticKirErrorV1> {
    source_safe_loan_holder_v29(plan, binding, place, checked.loan, &checked.pointer_type, budget)
}

fn source_safe_loan_holder_v29<'a>(
    plan: &SourceReferencePlanV29<'_, '_>,
    mut binding: &'a SemanticValueBindingV1,
    place: &SemanticPlaceV1,
    loan: usize,
    pointer_type: &Type,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<&'a SemanticSourceReferenceBindingV29, ProductionSemanticKirErrorV1> {
    budget.source_reference_owner_v29(plan)?;
    let result = (|| {
        source_reference_owned_prepay_v29::<&SemanticSourceReferenceBindingV29>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<&SemanticValueBindingV1>>(plan, budget)?;
        source_reference_owned_prepay_v29::<&SemanticValueBindingV1>(plan, budget)?;
        source_reference_owned_prepay_v29::<std::slice::Iter<'_, SemanticProjectionV1>>(
            plan, budget,
        )?;
        budget.source_reference_charge_v29(
            plan,
            argument_sum_v1(&[6, argument_product_v1(place.projections().len(), 3)?])?,
        )?;
        let fields = place
            .projections()
            .len()
            .checked_sub(1)
            .ok_or_else(scoped_object_error_v29)?;
        for projection in &place.projections()[..fields] {
            let (SemanticProjectionKindV1::Field(field), SemanticValueBindingV1::Aggregate(values)) =
                (projection.kind(), binding)
            else {
                return Err(scoped_object_pending_v29());
            };
            binding = values
                .get(field as usize)
                .ok_or_else(scoped_object_error_v29)?;
        }
        let SemanticValueBindingV1::SourceReference(binding) = binding else {
            return Err(scoped_object_error_v29());
        };
        source_reference_validate_binding_v29(plan, binding, budget)?;
        if binding.origin.single_loan()? != loan
            || !matches!(binding.values.as_slice(), [value]
                if &value.ty == pointer_type)
        {
            return Err(scoped_object_error_v29());
        }
        Ok(binding)
    })();
    result.inspect_err(|error| source_reference_record_failure_v29(plan, error))
}

impl SemanticFunctionLoweringV1<'_, '_> {
    fn source_reference_object_address_v29(
        &mut self,
        site: SourceReferenceSiteV29,
        statement: Option<u32>,
        place: &SemanticPlaceV1,
        loan: usize,
        operations: &mut Vec<Operation>,
    ) -> Result<Option<SemanticValueBindingV1>, ProductionSemanticKirErrorV1> {
        let checked = self.with_emission_budget_v1(|this, budget| {
            let references = this
                .execution
                .as_ref()
                .and_then(|cursor| cursor.references)
                .ok_or_else(scoped_object_error_v29)?;
            references.check(budget)?;
            source_reference_owned_prepay_v29::<Option<SemanticValueBindingV1>>(
                references.plan,
                budget,
            )?;
            source_reference_owned_prepay_v29::<(ScopedObjectEndpointV29, ValueId, MemoryAccess)>(
                references.plan,
                budget,
            )?;
            source_object_loan_v29(references.plan, loan, budget)
        })?;
        let Some(checked) = checked else {
            return Ok(None);
        };
        if !place.projections().is_empty() {
            return self.source_aggregate_object_reborrow_v29(site, place, &checked, operations);
        }
        if checked.original.instance != site.instance
            || checked.original.local != place.local()
            || checked.original.ty != place.ty()
        {
            return Err(scoped_object_pending_v29());
        }
        // The exact original Borrow access below already passed C2's storage
        // read check. Legacy scalar initialized bits do not describe typed
        // same-block stores; final typed history still proves the actual value.
        let kind = match &checked.pointer_type {
            Type::Pointer(pointer) if pointer.access == AccessMode::ReadOnly => {
                SemanticBorrowKindV1::Shared
            }
            Type::Pointer(pointer) if pointer.access == AccessMode::ReadWrite => {
                SemanticBorrowKindV1::Mutable
            }
            _ => return Err(scoped_object_error_v29()),
        };
        let (endpoint, address, _) = self
            .source_object_local_endpoint_v29(
                site.block,
                statement,
                place,
                SourceReferenceAccessV29::Borrow(kind),
            )?
            .ok_or_else(scoped_object_error_v29)?;
        if !matches!(endpoint.object, ScopedObjectIdentityV29::Local { instance, local, generation }
            if instance == checked.original.instance && local == checked.original.local && generation == checked.original.generation)
            || checked.original.kind != SourceBackingKindV29::Object(endpoint.root_schema)
        {
            return Err(scoped_object_error_v29());
        }
        if kind == SemanticBorrowKindV1::Shared {
            let ty = self.with_emission_budget_v1(|_, budget| {
                execution_cfg_clone_type_v29(&checked.pointer_type, budget)
            })?;
            self.emit(
                operations,
                ty,
                OperationKind::Cast {
                    kind: CastKind::RestrictPointerAccess,
                    value: address,
                    to: checked.pointer_type,
                },
            )
            .map(Some)
        } else {
            Ok(Some(SemanticValueBindingV1::Value {
                id: address,
                ty: checked.pointer_type,
            }))
        }
    }

    fn source_object_loan_endpoint_v29(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        place: &SemanticPlaceV1,
        access: SourceReferenceAccessV29,
    ) -> Result<
        Option<(ScopedObjectEndpointV29, ValueId, MemoryAccess)>,
        ProductionSemanticKirErrorV1,
    > {
        self.with_emission_budget_v1(|this, budget| {
            let Some(cursor) = this.execution.as_ref() else {
                return Ok(None);
            };
            let Some(references) = cursor.references else {
                return Ok(None);
            };
            references.check(budget)?;
            let plan = references.plan;
            source_reference_owned_prepay_v29::<
                Option<(ScopedObjectEndpointV29, ValueId, MemoryAccess)>,
            >(plan, budget)?;
            source_reference_owned_prepay_v29::<Option<&Option<SemanticValueBindingV1>>>(
                plan, budget,
            )?;
            source_reference_owned_prepay_v29::<Option<&SemanticValueBindingV1>>(plan, budget)?;
            source_reference_owned_prepay_v29::<&SemanticValueBindingV1>(plan, budget)?;
            source_reference_owned_prepay_v29::<SourceReferenceSiteV29>(plan, budget)?;
            source_reference_owned_prepay_v29::<Option<ScopedMemoryFrameV29>>(plan, budget)?;
            source_reference_owned_prepay_v29::<ScopedMemoryFrameV29>(plan, budget)?;
            source_reference_owned_prepay_v29::<Option<&SemanticPlaceV1>>(plan, budget)?;
            let site = SourceReferenceSiteV29 {
                instance: cursor.instance,
                block,
                statement: statement.map(|value| value as usize),
            };
            // This optional producer only owns thin primitive private loans.
            // Ordinary issued Value pointers retain their separate source proof.
            source_reference_owned_prepay_v29::<Option<&SemanticTypeDeclV1>>(plan, budget)?;
            source_reference_owned_prepay_v29::<Option<&SemanticTypeShapeV1>>(plan, budget)?;
            budget.source_reference_charge_v29(plan, 2)?;
            if plan.storage == SourceReferenceStorageV29::PromotedOnly
                || !matches!(plan.instances.owner().source_semantic().types()
                    .get(place.ty().index() as usize).map(SemanticTypeDeclV1::shape),
                    Some(SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_)))
                || source_object_loan_holder_type_v29(plan, site, place, budget)?.is_none()
            {
                return Ok(None);
            }
            let frame = this
                .scoped_memory
                .as_ref()
                .and_then(|row| row.frame)
                .ok_or_else(scoped_object_error_v29)?;
            let Some(ScopedMemoryRoleV29::Operand(role)) = frame.role else {
                return Err(scoped_object_pending_v29());
            };
            if frame.site != execution_site_v29(block, statement)
                || !scoped_object_original_place_v29(this.function, frame.site, role)
                    .is_some_and(|original| std::ptr::eq(original, place))
            {
                return Err(scoped_object_error_v29());
            }
            let value = this
                .locals
                .get(place.local().index() as usize)
                .and_then(Option::as_ref)
                .ok_or_else(scoped_object_pending_v29)?;
            if !source_object_private_holder_family_v29(plan, value, place, budget)? {
                return Ok(None);
            }
            let Some(checked) = source_object_loan_access_v29(plan, site, place, access, budget)?
            else {
                return Ok(None);
            };
            let binding = source_object_loan_holder_v29(plan, value, place, &checked, budget)?;
            let pointer = binding.values[0].id;
            let SourceBackingKindV29::Object(schema) = checked.original.kind else {
                return Err(scoped_object_error_v29());
            };
            let mut path = source_reference_owned_vec_v29(plan, place.projections().len(), budget)?;
            for &projection in place.projections() {
                budget.source_reference_charge_v29(plan, 1)?;
                path.push(ScopedObjectComponentV29::Original {
                    projection,
                    selector: None,
                });
            }
            source_reference_owned_prepay_v29::<ScopedObjectPathV29>(plan, budget)?;
            let path = this
                .scoped_memory
                .as_mut()
                .ok_or_else(scoped_object_error_v29)?
                .anchors
                .append_object_path(&path, budget)?;
            let prefix = u32::try_from(place.projections().len())
                .map_err(|_| ArgumentResourceV1::Arithmetic)?;
            Ok(Some((
                ScopedObjectEndpointV29 {
                    object: ScopedObjectIdentityV29::Reference {
                        instance: cursor.instance,
                        site: frame.site,
                        role,
                        dereference_prefix: prefix,
                    },
                    source: ScopedObjectSourceV29::Place {
                        site: frame.site,
                        role,
                        local: place.local(),
                        prefix,
                    },
                    root_type: place.ty(),
                    projected_type: place.ty(),
                    root_schema: schema,
                    projected_schema: schema,
                    source_path: path,
                    path: ScopedObjectPathV29 { first: 0, count: 0 },
                },
                pointer,
                MemoryAccess::new(AddressSpace::Private, 1),
            )))
        })
    }
}

// Candidate classification only: false grants no memory/alias authority. Once
// a private reference is selected, the caller still requires its exact access.
fn source_object_private_holder_family_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    mut value: &SemanticValueBindingV1,
    place: &SemanticPlaceV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    budget.source_reference_owner_v29(plan)?;
    let result = (|| {
        source_reference_owned_prepay_v29::<bool>(plan, budget)?;
        source_reference_owned_prepay_v29::<&SemanticValueBindingV1>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<&SemanticValueBindingV1>>(plan, budget)?;
        source_reference_owned_prepay_v29::<std::slice::Iter<'_, SemanticProjectionV1>>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<&SemanticProjectionV1>>(plan, budget)?;
        source_reference_owned_prepay_v29::<&SemanticProjectionV1>(plan, budget)?;
        source_reference_owned_prepay_v29::<&[SemanticProjectionV1]>(plan, budget)?;
        source_reference_owned_prepay_v29::<&Vec<SemanticValueBindingV1>>(plan, budget)?;
        source_reference_owned_prepay_v29::<&SemanticSourceReferenceBindingV29>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<usize>>(plan, budget)?;
        budget.source_reference_charge_v29(plan, 4)?;
        let Some(fields) = place.projections().len().checked_sub(1).filter(|fields| *fields < 256)
        else { return Err(scoped_object_pending_v29()); };
        if place.projections()[fields].kind() != SemanticProjectionKindV1::Dereference {
            return Err(scoped_object_pending_v29());
        }
        for projection in &place.projections()[..fields] {
            budget.source_reference_charge_v29(plan, 3)?;
            let (SemanticProjectionKindV1::Field(field), SemanticValueBindingV1::Aggregate(values)) =
                (projection.kind(), value)
            else { return Err(scoped_object_pending_v29()); };
            value = values.get(field as usize).ok_or_else(scoped_object_error_v29)?;
        }
        match value {
            SemanticValueBindingV1::Value { .. } => Ok(false),
            SemanticValueBindingV1::SourceReference(binding) => {
                source_reference_validate_binding_v29(plan, binding, budget)?;
                Ok(true)
            }
            _ => Err(scoped_object_pending_v29()),
        }
    })();
    result.inspect_err(|error| source_reference_record_failure_v29(plan, error))
}
