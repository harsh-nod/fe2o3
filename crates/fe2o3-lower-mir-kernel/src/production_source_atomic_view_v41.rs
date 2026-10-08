// Private original-plan custody for a V41 atomic view. These records never
// authorize an ordinary dereference or manufacture an allocation capability.
// Every record descends from an existing original ExclusiveOwner argument loan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceAtomicPointerCustodyV41 {
    parent: usize,
    instance: ProductionCallInstanceIdV1,
    local: SemanticLocalIdV1,
    generation: u32,
    anchor: SourceReferenceAnchorV29,
    field: u32,
    scalar: SemanticTypeIdV1,
    view: Option<SourceAtomicStoragePathV41>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceAtomicViewOperationV41 {
    Cast,
    SharedBorrow,
    AddressOf,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceAtomicViewFormationV41 {
    site: SourceReferenceSiteV29,
    source: usize,
    operation: SourceAtomicViewOperationV41,
    input_type: SemanticTypeIdV1,
    output_type: SemanticTypeIdV1,
    input: usize,
    output: usize,
}

fn source_atomic_view_error_v41() -> ProductionSemanticKirErrorV1 {
    source_reference_error_v29(
        "atomic view requires its exact original owner, field chain and current loan",
    )
}

fn source_atomic_view_pointer_v41(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
) -> Option<&fe2o3_mir_model::semantic_mir_v1::SemanticPointerTypeV1> {
    let declaration = types.get(ty.index() as usize)?;
    let SemanticTypeShapeV1::Pointer(pointer) = declaration.shape() else {
        return None;
    };
    (declaration.layout().size_bytes() == Some(8)
        && declaration.layout().alignment_bytes() == 8
        && pointer.pointer_width_bits() == 64
        && pointer.address_space() == 0
        && pointer.metadata() == SemanticPointerMetadataV1::None
        && matches!(
            pointer.kind(),
            SemanticPointerKindV1::Raw | SemanticPointerKindV1::Reference
        ))
    .then_some(pointer)
}

impl SourceReferencePlanV29<'_, '_> {
    // Conservative until an explicit atomic-view lifetime discharge exists:
    // another raw alias of the same original owner may not perform ordinary
    // memory access after any checked view formation, even after a CFG split.
    fn atomic_origin_was_viewed_v41(
        &self,
        fact: SourceAtomicPointerCustodyV41,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        source_reference_emission_prepay_v29::<SourceAtomicPointerCustodyV41>(budget)?;
        for prior in &self.atomic_custody {
            budget.charge_work(6)?;
            if prior.view.is_some()
                && prior.instance == fact.instance
                && prior.local == fact.local
                && prior.generation == fact.generation
                && prior.anchor == fact.anchor
                && prior.scalar == fact.scalar
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
    fn atomic_custody_v41(
        &self,
        node: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<SourceAtomicPointerCustodyV41>, ProductionSemanticKirErrorV1> {
        source_reference_emission_prepay_v29::<Option<SourceAtomicPointerCustodyV41>>(budget)?;
        budget.charge_work(3)?;
        let row = self.nodes.get(node).ok_or(ArgumentResourceV1::Accounting)?;
        row.atomic_custody
            .map(|index| {
                self.atomic_custody
                    .get(index)
                    .copied()
                    .ok_or_else(source_atomic_view_error_v41)
            })
            .transpose()
    }

    // The record is private and owner-bound; no caller-supplied boolean, layout,
    // same-shaped clone, or marker by itself is a valid formation receipt.
    fn atomic_view_formation_v41(
        &self,
        instance: ProductionCallInstanceIdV1,
        site: ExecutionSiteV29,
        value: &fe2o3_mir_model::semantic_mir_v1::SemanticRvalueV1,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<Option<SourceAtomicViewFormationV41>, ProductionSemanticKirErrorV1> {
        let result = (|| {
            budget.source_reference_owner_v29(self)?;
            source_reference_emission_prepay_v29::<Option<SourceAtomicViewFormationV41>>(budget)?;
            source_reference_emission_prepay_v29::<
                Result<Option<SourceAtomicViewFormationV41>, ProductionSemanticKirErrorV1>,
            >(budget)?;
            let ExecutionSiteV29::Statement { block, statement } = site else {
                return Err(source_atomic_view_error_v41());
            };
            let source_site = SourceReferenceSiteV29 {
                instance,
                block: SemanticBlockIdV1::from_index(block.get()),
                statement: Some(statement as usize),
            };
            let function = self
                .instances
                .instance(instance)
                .ok_or(ArgumentResourceV1::Accounting)?
                .declaration();
            let Some(SemanticStatementKindV1::Assign(original)) =
                scoped_source_statement_v29(function, site)
            else {
                return Err(source_atomic_view_error_v41());
            };
            if !std::ptr::eq(original.value(), value) {
                return Err(source_atomic_view_error_v41());
            }
            let (source, operation) = match value.kind() {
                SemanticRvalueKindV1::Cast { operand, .. } => (
                    operand as *const SemanticOperandV1 as usize,
                    SourceAtomicViewOperationV41::Cast,
                ),
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place,
                } => (
                    place as *const SemanticPlaceV1 as usize,
                    SourceAtomicViewOperationV41::SharedBorrow,
                ),
                SemanticRvalueKindV1::AddressOf { place, .. } => (
                    place as *const SemanticPlaceV1 as usize,
                    SourceAtomicViewOperationV41::AddressOf,
                ),
                _ => return Ok(None),
            };
            let mut observed = None;
            for row in &self.atomic_formations {
                budget.charge_work(4)?;
                if row.site != source_site {
                    continue;
                }
                if row.source != source
                    || row.operation != operation
                    || row.output_type != value.result_type()
                    || observed.replace(*row).is_some()
                {
                    return Err(source_atomic_view_error_v41());
                }
                let input = self
                    .atomic_custody
                    .get(row.input)
                    .ok_or(ArgumentResourceV1::Accounting)?;
                let output = self
                    .atomic_custody
                    .get(row.output)
                    .ok_or(ArgumentResourceV1::Accounting)?;
                if input.parent != output.parent
                    || input.anchor != output.anchor
                    || input.instance != output.instance
                    || input.local != output.local
                    || input.generation != output.generation
                    || output.view.is_none()
                {
                    return Err(source_atomic_view_error_v41());
                }
                if row.operation == SourceAtomicViewOperationV41::Cast {
                    source_reference_emission_prepay_v29::<Option<SourceAtomicStoragePathV41>>(
                        budget,
                    )?;
                    source_reference_emission_prepay_v29::<(
                        &fe2o3_mir_model::semantic_mir_v1::SemanticPointerTypeV1,
                        &fe2o3_mir_model::semantic_mir_v1::SemanticPointerTypeV1,
                    )>(budget)?;
                    if let Some(input_path) = input.view {
                        budget.charge_work(8)?;
                        let SemanticRvalueKindV1::Cast {
                            kind: SemanticCastKindV1::Pointer,
                            operand,
                        } = value.kind()
                        else {
                            return Err(source_atomic_view_error_v41());
                        };
                        let types = self.instances.owner().source_semantic().types();
                        let from = source_atomic_view_pointer_v41(types, row.input_type)
                            .ok_or_else(source_atomic_view_error_v41)?;
                        let to = source_atomic_view_pointer_v41(types, row.output_type)
                            .ok_or_else(source_atomic_view_error_v41)?;
                        let Some(advanced) = source_atomic_cast_descendant_v41(
                            types,
                            input_path,
                            to.pointee(),
                            budget,
                        )?
                        else {
                            return Err(source_atomic_view_error_v41());
                        };
                        if operand.ty() != row.input_type
                            || from.pointee() != input_path.current_type()
                            || to.kind() != SemanticPointerKindV1::Raw
                            || output.view != Some(advanced)
                        {
                            return Err(source_atomic_view_error_v41());
                        }
                    }
                }
            }
            Ok(observed)
        })();
        result.inspect_err(|error| source_reference_record_failure_v29(self, error))
    }
}

impl SourceReferenceBuilderV29<'_, '_, '_> {
    fn retain_atomic_custody_v41(
        &mut self,
        fact: SourceAtomicPointerCustodyV41,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        source_reference_emission_prepay_v29::<(SourceAtomicPointerCustodyV41, usize)>(budget)?;
        for (index, prior) in self.plan.atomic_custody.iter().enumerate() {
            budget.charge_work(1)?;
            if *prior == fact {
                return Ok(index);
            }
        }
        let index = self.plan.atomic_custody.len();
        emission_push_v1(&mut self.plan.atomic_custody, fact, budget)?;
        Ok(index)
    }

    fn atomic_pointer_node_v41(
        &mut self,
        ty: SemanticTypeIdV1,
        fact: SourceAtomicPointerCustodyV41,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        source_reference_emission_prepay_v29::<(SourceAtomicPointerCustodyV41, usize)>(budget)?;
        let custody = self.retain_atomic_custody_v41(fact, budget)?;
        let node = self.plain(ty, budget)?;
        self.plan.nodes[node].atomic_custody = Some(custody);
        Ok(node)
    }

    fn check_atomic_custody_v41(
        &self,
        fact: SourceAtomicPointerCustodyV41,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        source_reference_emission_prepay_v29::<SourceAtomicPointerCustodyV41>(budget)?;
        budget.charge_work(12)?;
        let loan = self
            .plan
            .loans
            .get(fact.parent)
            .ok_or(ArgumentResourceV1::Accounting)?;
        let origin = self
            .plan
            .origins
            .get(loan.origin)
            .ok_or(ArgumentResourceV1::Accounting)?;
        if origin.instance != fact.instance
            || origin.local != fact.local
            || origin.generation != fact.generation
            || origin.anchor != Some(fact.anchor)
            || origin.ty != fact.anchor.ty
            || !origin.projections.is_empty()
            || self.local(origin.instance, origin.local)?.generation != fact.generation
        {
            return Err(source_atomic_view_error_v41());
        }
        // This is the existing external allocation binding, not an address of
        // local C2 storage. The original loan checks its lender's live holder
        // and generation; no local-storage receipt is invented for that allocation.
        self.check_loan_use(fact.parent, budget)?;
        let Some(Type::Pointer(pointer)) =
            source_reference_anchor_type_v29(&self.plan, fact.anchor, origin.ty, budget)?
        else {
            return Err(source_atomic_view_error_v41());
        };
        let Type::Scalar(element) = lower_scalar_type(
            self.plan.instances.owner().source_semantic().types(),
            fact.scalar,
        )?
        else {
            return Err(source_atomic_view_error_v41());
        };
        if pointer.pointee.as_ref() != &Type::Scalar(element)
            || pointer.address_space != AddressSpace::Global
            || pointer.access != AccessMode::ReadWrite
        {
            return Err(source_atomic_view_error_v41());
        }
        Ok(())
    }

    // Called only after the common original-place read and loan checks. The
    // selected field is the exact sole non-ZST raw-pointer field authenticated
    // by source_reference_anchor_type_v29; it is not an arbitrary raw argument.
    fn capture_atomic_root_pointer_v41(
        &mut self,
        site: SourceReferenceSiteV29,
        source: &SemanticPlaceV1,
        resolved: &mut SourceReferencePlaceV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.plan.instances.owner().source_semantic().wire_version()
            != fe2o3_mir_model::semantic_mir_v1::SemanticMirWireVersionV1::V41
            || self.plan.nodes[resolved.node].atomic_custody.is_some()
        {
            return Ok(());
        }
        budget.charge_work(6)?;
        let Some(parent) = resolved.loan else {
            return Ok(());
        };
        let loan = self
            .plan
            .loans
            .get(parent)
            .ok_or(ArgumentResourceV1::Accounting)?;
        let origin = self
            .plan
            .origins
            .get(loan.origin)
            .ok_or(ArgumentResourceV1::Accounting)?;
        let Some(anchor) = origin.anchor else {
            return Ok(());
        };
        if origin.ty != anchor.ty
            || !origin.projections.is_empty()
            || resolved.instance != origin.instance
            || resolved.local != origin.local
            || resolved.generation != origin.generation
        {
            return Ok(());
        }
        let [projection] = resolved.projections.as_slice() else {
            return Ok(());
        };
        let SemanticProjectionKindV1::Field(field) = projection.kind() else {
            return Ok(());
        };
        let types = self.plan.instances.owner().source_semantic().types();
        let ty = self.plan.nodes[resolved.node].ty;
        let Some(pointer) = source_atomic_view_pointer_v41(types, ty) else {
            return Ok(());
        };
        if pointer.kind() != SemanticPointerKindV1::Raw
            || pointer.mutability() != SemanticMutabilityV1::Mutable
        {
            return Ok(());
        }
        let Some(SemanticTypeShapeV1::Aggregate(fields)) = types
            .get(anchor.ty.index() as usize)
            .map(SemanticTypeDeclV1::shape)
        else {
            return Ok(());
        };
        if fields.fields().get(field as usize).copied() != Some(ty)
            || projection.result_type() != ty
        {
            return Err(source_atomic_view_error_v41());
        }
        if !matches!(
            types
                .get(pointer.pointee().index() as usize)
                .map(SemanticTypeDeclV1::shape),
            Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                bits: 32,
                ..
            }))
        ) {
            return Ok(());
        }
        source_reference_emission_prepay_v29::<SourceAtomicPointerCustodyV41>(budget)?;
        let fact = SourceAtomicPointerCustodyV41 {
            parent,
            instance: origin.instance,
            local: origin.local,
            generation: origin.generation,
            anchor,
            field,
            scalar: pointer.pointee(),
            view: None,
        };
        // Only the original direct ExclusiveOwner ABI admits this capture.
        if source_reference_anchor_type_v29(&self.plan, anchor, origin.ty, budget)?.is_none() {
            return Ok(());
        }
        self.check_atomic_custody_v41(fact, budget)?;
        resolved.node = self.atomic_pointer_node_v41(ty, fact, budget)?;
        self.retain_atomic_capture_v41(site, source, resolved.node, budget)
    }

    fn retain_atomic_formation_v41(
        &mut self,
        row: SourceAtomicViewFormationV41,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        source_reference_emission_prepay_v29::<SourceAtomicViewFormationV41>(budget)?;
        for prior in &self.plan.atomic_formations {
            budget.charge_work(1)?;
            if prior.site == row.site {
                return if *prior == row {
                    Ok(())
                } else {
                    Err(source_atomic_view_error_v41())
                };
            }
        }
        emission_push_v1(&mut self.plan.atomic_formations, row, budget)
    }

    // The caller has already checked the exact original Cast statement and
    // consumed its genuine operand through the common read/move machinery.
    fn cast_atomic_view_v41(
        &mut self,
        site: SourceReferenceSiteV29,
        kind: SemanticCastKindV1,
        operand: &SemanticOperandV1,
        node: usize,
        output: SemanticTypeIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<usize>, ProductionSemanticKirErrorV1> {
        source_reference_emission_prepay_v29::<(
            SourceAtomicPointerCustodyV41,
            SourceAtomicViewFormationV41,
        )>(budget)?;
        let Some(mut fact) = self.plan.atomic_custody_v41(node, budget)? else {
            return Ok(None);
        };
        self.check_atomic_custody_v41(fact, budget)?;
        budget.charge_work(12)?;
        let types = self.plan.instances.owner().source_semantic().types();
        let input = source_atomic_view_pointer_v41(types, operand.ty())
            .ok_or_else(source_atomic_view_error_v41)?;
        let result = source_atomic_view_pointer_v41(types, output)
            .ok_or_else(source_atomic_view_error_v41)?;
        if kind != SemanticCastKindV1::Pointer
            || result.kind() != SemanticPointerKindV1::Raw
            || input.pointee()
                != fact
                    .view
                    .map_or(fact.scalar, SourceAtomicStoragePathV41::current_type)
        {
            return Err(source_atomic_view_error_v41());
        }
        source_reference_emission_prepay_v29::<Option<SourceAtomicStoragePathV41>>(budget)?;
        if let Some(mut path) = fact.view {
            if result.pointee() != path.current_type() {
                let advanced =
                    source_atomic_cast_descendant_v41(types, path, result.pointee(), budget)?;
                if advanced.is_none() {
                    return Err(source_atomic_view_error_v41());
                }
                path = advanced.ok_or(ArgumentResourceV1::Accounting)?;
            }
            fact.view = Some(path);
        } else {
            budget.charge_work(40)?;
            let children = fe2o3_mir_model::semantic_mir_v1::semantic_atomic_storage_chain_v41(
                types,
                result.pointee(),
            )
            .ok_or_else(source_atomic_view_error_v41)?;
            if children[2] != fact.scalar || input.kind() != SemanticPointerKindV1::Raw {
                return Err(source_atomic_view_error_v41());
            }
            fact.view = Some(SourceAtomicStoragePathV41 {
                wrapper: result.pointee(),
                children,
                depth: 0,
            });
        }
        let output_node = self.atomic_pointer_node_v41(output, fact, budget)?;
        self.retain_atomic_formation_v41(
            SourceAtomicViewFormationV41 {
                site,
                source: operand as *const SemanticOperandV1 as usize,
                operation: SourceAtomicViewOperationV41::Cast,
                input_type: operand.ty(),
                output_type: output,
                input: self.plan.nodes[node]
                    .atomic_custody
                    .ok_or(ArgumentResourceV1::Accounting)?,
                output: self.plan.nodes[output_node]
                    .atomic_custody
                    .ok_or(ArgumentResourceV1::Accounting)?,
            },
            budget,
        )?;
        Ok(Some(output_node))
    }

    // Called only after address_value authenticates the exact original AddressOf.
    // This is an atomic-only representation formation, not an ordinary raw
    // origin, mutable loan, address exposure, or pointee read/write permission.
    fn address_atomic_view_v41(
        &mut self,
        site: SourceReferenceSiteV29,
        source: &SemanticPlaceV1,
        output: SemanticTypeIdV1,
        mutability: SemanticMutabilityV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<usize>, ProductionSemanticKirErrorV1> {
        source_reference_emission_prepay_v29::<(
            SourceAtomicPointerCustodyV41,
            SourceAtomicViewFormationV41,
        )>(budget)?;
        let Some(node) = self.local(site.instance, source.local())?.node else {
            return Ok(None);
        };
        let Some(mut fact) = self.plan.atomic_custody_v41(node, budget)? else {
            return Ok(None);
        };
        self.check_atomic_custody_v41(fact, budget)?;
        let Some(mut path) = fact.view else {
            return Err(source_atomic_view_error_v41());
        };
        budget.charge_work(12)?;
        let statement = site.statement.ok_or(ArgumentResourceV1::Accounting)?;
        let function = self
            .plan
            .instances
            .instance(site.instance)
            .ok_or(ArgumentResourceV1::Accounting)?
            .declaration();
        let Some(SemanticStatementKindV1::Assign(assignment)) = function
            .blocks()
            .get(site.block.index() as usize)
            .and_then(|block| block.statements().get(statement))
            .map(|statement| statement.kind())
        else {
            return Err(source_atomic_view_error_v41());
        };
        let SemanticRvalueKindV1::AddressOf {
            place,
            mutability: original,
        } = assignment.value().kind()
        else {
            return Err(source_atomic_view_error_v41());
        };
        if !std::ptr::eq(place, source)
            || *original != mutability
            || assignment.value().result_type() != output
        {
            return Err(source_atomic_view_error_v41());
        }
        let input_type = self.plan.nodes[node].ty;
        let types = self.plan.instances.owner().source_semantic().types();
        let input = source_atomic_view_pointer_v41(types, input_type)
            .ok_or_else(source_atomic_view_error_v41)?;
        let result = source_atomic_view_pointer_v41(types, output)
            .ok_or_else(source_atomic_view_error_v41)?;
        if result.kind() != SemanticPointerKindV1::Raw
            || result.mutability() != mutability
            || input.pointee() != path.current_type()
            || source.projections().first().is_none_or(|projection| {
                projection.kind() != SemanticProjectionKindV1::Dereference
                    || projection.result_type() != path.current_type()
            })
        {
            return Err(source_atomic_view_error_v41());
        }
        for projection in &source.projections()[1..] {
            budget.charge_work(4)?;
            if path.next_field(projection.result_type()) != Some(*projection) {
                return Err(source_atomic_view_error_v41());
            }
            path.depth += 1;
        }
        if source.ty() != path.current_type() || result.pointee() != path.current_type() {
            return Err(source_atomic_view_error_v41());
        }
        // Resolve and record only the actual pointer holder. The original
        // RvaluePlace is consumed again by emission and the final source index.
        budget.charge_work(6)?;
        budget.reserve_storage(argument_sum_v1(&[
            std::mem::size_of::<SemanticPlaceV1>(),
            argument_product_v1(
                2,
                std::mem::size_of::<
                    Result<SemanticPlaceV1, fe2o3_mir_model::semantic_mir_v1::SemanticMirErrorV1>,
                >(),
            )?,
            argument_product_v1(2, std::mem::size_of::<Vec<SemanticProjectionV1>>())?,
            std::mem::size_of::<Box<[SemanticProjectionV1]>>(),
        ])?)?;
        let holder = SemanticPlaceV1::new(source.local(), Vec::new(), input_type)
            .map_err(|_| source_atomic_view_error_v41())?;
        let resolved = self.resolve_reference_place_recorded(
            site,
            &holder,
            SourceReferenceAccessV29::Read,
            false,
            budget,
        )?;
        if resolved.node != node {
            return Err(source_atomic_view_error_v41());
        }
        self.check_atomic_custody_v41(fact, budget)?;
        fact.view = Some(path);
        let output_node = self.atomic_pointer_node_v41(output, fact, budget)?;
        self.retain_atomic_formation_v41(
            SourceAtomicViewFormationV41 {
                site,
                source: source as *const SemanticPlaceV1 as usize,
                operation: SourceAtomicViewOperationV41::AddressOf,
                input_type,
                output_type: output,
                input: self.plan.nodes[node]
                    .atomic_custody
                    .ok_or(ArgumentResourceV1::Accounting)?,
                output: self.plan.nodes[output_node]
                    .atomic_custody
                    .ok_or(ArgumentResourceV1::Accounting)?,
            },
            budget,
        )?;
        Ok(Some(output_node))
    }

    fn borrow_atomic_view_v41(
        &mut self,
        site: SourceReferenceSiteV29,
        kind: SemanticBorrowKindV1,
        source: &SemanticPlaceV1,
        output: SemanticTypeIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<usize>, ProductionSemanticKirErrorV1> {
        source_reference_emission_prepay_v29::<(
            SourceAtomicPointerCustodyV41,
            SourceAtomicViewFormationV41,
        )>(budget)?;
        let Some(node) = self.local(site.instance, source.local())?.node else {
            return Ok(None);
        };
        let Some(mut fact) = self.plan.atomic_custody_v41(node, budget)? else {
            return Ok(None);
        };
        let Some(mut path) = fact.view else {
            return Err(source_atomic_view_error_v41());
        };
        source_reference_emission_prepay_v29::<SourceAtomicPointerCustodyV41>(budget)?;
        budget.charge_work(12)?;
        let input_type = self.plan.nodes[node].ty;
        let types = self.plan.instances.owner().source_semantic().types();
        let input = source_atomic_view_pointer_v41(types, input_type)
            .ok_or_else(source_atomic_view_error_v41)?;
        let result = source_atomic_view_pointer_v41(types, output)
            .ok_or_else(source_atomic_view_error_v41)?;
        if kind != SemanticBorrowKindV1::Shared
            || result.kind() != SemanticPointerKindV1::Reference
            || result.mutability() != SemanticMutabilityV1::Immutable
            || input.pointee() != path.current_type()
            || source.projections().first().is_none_or(|projection| {
                projection.kind() != SemanticProjectionKindV1::Dereference
                    || projection.result_type() != path.current_type()
            })
        {
            return Err(source_atomic_view_error_v41());
        }
        for projection in &source.projections()[1..] {
            budget.charge_work(4)?;
            if path.next_field(projection.result_type()) != Some(*projection) {
                return Err(source_atomic_view_error_v41());
            }
            path.depth += 1;
        }
        if source.ty() != path.current_type() || result.pointee() != path.current_type() {
            return Err(source_atomic_view_error_v41());
        }
        // This is a holder-only read, not a pointee read and not a mutable loan.
        // The original Borrow itself is bound above by instances.borrow_at.
        budget.charge_work(6)?;
        budget.reserve_storage(argument_sum_v1(&[
            std::mem::size_of::<SemanticPlaceV1>(),
            argument_product_v1(
                2,
                std::mem::size_of::<
                    Result<SemanticPlaceV1, fe2o3_mir_model::semantic_mir_v1::SemanticMirErrorV1>,
                >(),
            )?,
            argument_product_v1(2, std::mem::size_of::<Vec<SemanticProjectionV1>>())?,
            std::mem::size_of::<Box<[SemanticProjectionV1]>>(),
        ])?)?;
        let holder = SemanticPlaceV1::new(source.local(), Vec::new(), input_type)
            .map_err(|_| source_atomic_view_error_v41())?;
        let resolved = self.resolve_reference_place_recorded(
            site,
            &holder,
            SourceReferenceAccessV29::Read,
            false,
            budget,
        )?;
        if resolved.node != node {
            return Err(source_atomic_view_error_v41());
        }
        self.check_atomic_custody_v41(fact, budget)?;
        fact.view = Some(path);
        let output_node = self.atomic_pointer_node_v41(output, fact, budget)?;
        self.retain_atomic_formation_v41(
            SourceAtomicViewFormationV41 {
                site,
                source: source as *const SemanticPlaceV1 as usize,
                operation: SourceAtomicViewOperationV41::SharedBorrow,
                input_type,
                output_type: output,
                input: self.plan.nodes[node]
                    .atomic_custody
                    .ok_or(ArgumentResourceV1::Accounting)?,
                output: self.plan.nodes[output_node]
                    .atomic_custody
                    .ok_or(ArgumentResourceV1::Accounting)?,
            },
            budget,
        )?;
        Ok(Some(output_node))
    }
}

// Distinct original atomic effects. These do not increment ordinary referent
// read/write permissions and are never accepted for a Load or Store.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceAtomicUseV41 {
    site: SourceReferenceSiteV29,
    source: usize,
    address: usize,
    value: usize,
    destination: usize,
    node: usize,
    custody: usize,
    operation: SemanticAtomicRmwOpV1,
    access: fe2o3_mir_model::semantic_mir_v1::SemanticAtomicAccessV1,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceAtomicCaptureV41 {
    site: SourceReferenceSiteV29,
    source: usize,
    node: usize,
    custody: usize,
}

fn source_atomic_node_type_v41(
    plan: &SourceReferencePlanV29<'_, '_>,
    node: usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Option<Type>, ProductionSemanticKirErrorV1> {
    budget.source_reference_owner_v29(plan)?;
    budget.source_reference_charge_v29(plan, 12)?;
    let row = plan.nodes.get(node).ok_or(ArgumentResourceV1::Accounting)?;
    let Some(index) = row.atomic_custody else {
        return Ok(None);
    };
    source_reference_owned_prepay_v29::<Option<Type>>(plan, budget)?;
    source_reference_owned_prepay_v29::<SourceAtomicPointerCustodyV41>(plan, budget)?;
    let fact = *plan
        .atomic_custody
        .get(index)
        .ok_or(ArgumentResourceV1::Accounting)?;
    if plan.instances.owner().source_semantic().wire_version()
        != fe2o3_mir_model::semantic_mir_v1::SemanticMirWireVersionV1::V41
        || !matches!(row.kind, SourceReferenceNodeKindV29::Plain(None))
        || row.descriptor.is_some()
    {
        return Err(source_atomic_view_error_v41());
    }
    let types = plan.instances.owner().source_semantic().types();
    let pointer =
        source_atomic_view_pointer_v41(types, row.ty).ok_or_else(source_atomic_view_error_v41)?;
    if pointer.pointee()
        != fact
            .view
            .map_or(fact.scalar, SourceAtomicStoragePathV41::current_type)
        || (pointer.kind() == SemanticPointerKindV1::Reference
            && (fact.view.is_none() || pointer.mutability() != SemanticMutabilityV1::Immutable))
    {
        return Err(source_atomic_view_error_v41());
    }
    let parent = plan
        .loans
        .get(fact.parent)
        .ok_or(ArgumentResourceV1::Accounting)?;
    let origin = plan
        .origins
        .get(parent.origin)
        .ok_or(ArgumentResourceV1::Accounting)?;
    if origin.instance != fact.instance
        || origin.local != fact.local
        || origin.generation != fact.generation
        || origin.anchor != Some(fact.anchor)
        || !origin.projections.is_empty()
        || origin.ty != fact.anchor.ty
    {
        return Err(source_atomic_view_error_v41());
    }
    let expected = source_reference_anchor_type_v29(plan, fact.anchor, fact.anchor.ty, budget)?
        .ok_or_else(source_atomic_view_error_v41)?;
    let Type::Pointer(expected) = expected else {
        return Err(source_atomic_view_error_v41());
    };
    let scalar = lower_scalar_type(types, fact.scalar)?;
    if expected.address_space != AddressSpace::Global
        || expected.access != AccessMode::ReadWrite
        || expected.pointee.as_ref() != &scalar
    {
        return Err(source_atomic_view_error_v41());
    }
    // The original source pointer AS0 is Generic. The root producer emits the
    // actual Global->Generic cast before this owned transport can be bound.
    budget.source_reference_reserve_v29(plan, std::mem::size_of::<Type>())?;
    Ok(Some(Type::pointer(
        scalar,
        AddressSpace::Generic,
        AccessMode::ReadWrite,
    )))
}

fn source_atomic_root_type_v41(
    plan: &SourceReferencePlanV29<'_, '_>,
    loan: usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Option<Type>, ProductionSemanticKirErrorV1> {
    budget.source_reference_owner_v29(plan)?;
    if plan.atomic_captures.is_empty() {
        return Ok(None);
    }
    source_reference_owned_prepay_v29::<Option<Type>>(plan, budget)?;
    for capture in &plan.atomic_captures {
        budget.source_reference_charge_v29(plan, 4)?;
        let fact = plan
            .atomic_custody
            .get(capture.custody)
            .ok_or(ArgumentResourceV1::Accounting)?;
        if fact.parent != loan {
            continue;
        }
        source_atomic_node_type_v41(plan, capture.node, budget)?
            .ok_or_else(source_atomic_view_error_v41)?;
        return source_reference_anchor_type_v29(plan, fact.anchor, fact.anchor.ty, budget);
    }
    Ok(None)
}

fn source_atomic_abi_scalar_v41(
    plan: &SourceReferencePlanV29<'_, '_>,
    node: usize,
    ty: SemanticTypeIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<SemanticBackendScalarV1, ProductionSemanticKirErrorV1> {
    source_reference_owned_prepay_v29::<SemanticBackendScalarV1>(plan, budget)?;
    source_atomic_node_type_v41(plan, node, budget)?.ok_or_else(source_atomic_view_error_v41)?;
    budget.charge_work(8)?;
    let row = plan.nodes.get(node).ok_or(ArgumentResourceV1::Accounting)?;
    let declaration = &plan.instances.owner().source_semantic().types()[ty.index() as usize];
    let SemanticBackendReprV1::Scalar(scalar) = declaration.layout().backend_repr() else {
        return Err(source_atomic_view_error_v41());
    };
    if row.ty != ty
        || declaration.layout().is_uninhabited()
        || scalar.primitive() != SemanticBackendPrimitiveV1::pointer(0, 8, 8)
    {
        return Err(source_atomic_view_error_v41());
    }
    Ok(*scalar)
}

impl SourceReferenceBuilderV29<'_, '_, '_> {
    fn retain_atomic_capture_v41(
        &mut self,
        site: SourceReferenceSiteV29,
        source: &SemanticPlaceV1,
        node: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        source_reference_emission_prepay_v29::<SourceAtomicCaptureV41>(budget)?;
        let custody = self.plan.nodes[node]
            .atomic_custody
            .ok_or(ArgumentResourceV1::Accounting)?;
        let row = SourceAtomicCaptureV41 {
            site,
            source: source as *const SemanticPlaceV1 as usize,
            node,
            custody,
        };
        for prior in &self.plan.atomic_captures {
            budget.charge_work(2)?;
            if prior.site == site && prior.source == row.source {
                return if prior.custody == custody {
                    Ok(())
                } else {
                    Err(source_atomic_view_error_v41())
                };
            }
        }
        emission_push_v1(&mut self.plan.atomic_captures, row, budget)
    }

    fn atomic_rmw_v41(
        &mut self,
        site: SourceReferenceSiteV29,
        atomic: &SemanticAtomicRmwV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        source_reference_emission_prepay_v29::<(SourceAtomicUseV41, SourceAtomicPointerCustodyV41)>(
            budget,
        )?;
        budget.charge_work(14)?;
        let declaration = self
            .plan
            .instances
            .instance(site.instance)
            .ok_or(ArgumentResourceV1::Accounting)?
            .declaration();
        let original = site
            .statement
            .and_then(|s| {
                declaration
                    .blocks()
                    .get(site.block.index() as usize)
                    .and_then(|b| b.statements().get(s))
            })
            .ok_or_else(source_atomic_view_error_v41)?;
        if !matches!(original.kind(), SemanticStatementKindV1::AtomicRmw(value) if std::ptr::eq(value, atomic))
            || !atomic.destination().projections().is_empty()
        {
            return Err(source_atomic_view_error_v41());
        }
        let node = self
            .local(site.instance, atomic.address().local())?
            .node
            .ok_or_else(source_atomic_view_error_v41)?;
        let fact = self
            .plan
            .atomic_custody_v41(node, budget)?
            .ok_or_else(source_atomic_view_error_v41)?;
        self.check_atomic_custody_v41(fact, budget)?;
        let path = fact
            .view
            .filter(|path| path.is_scalar_leaf())
            .ok_or_else(source_atomic_view_error_v41)?;
        let pointer = source_atomic_view_pointer_v41(
            self.plan.instances.owner().source_semantic().types(),
            self.plan.nodes[node].ty,
        )
        .ok_or_else(source_atomic_view_error_v41)?;
        let [projection] = atomic.address().projections() else {
            return Err(source_atomic_view_error_v41());
        };
        if self.plan.instances.owner().source_semantic().wire_version()
            != fe2o3_mir_model::semantic_mir_v1::SemanticMirWireVersionV1::V41
            || pointer.pointee() != path.current_type()
            || fact.scalar != path.current_type()
            || projection.kind() != SemanticProjectionKindV1::Dereference
            || projection.result_type() != fact.scalar
            || atomic.address().ty() != fact.scalar
            || atomic.value().ty() != fact.scalar
            || atomic.destination().ty() != fact.scalar
            || atomic.access().scope() != SemanticAtomicScopeV1::System
        {
            return Err(source_atomic_view_error_v41());
        }
        // Read the original RHS through ordinary source use; the pointer's
        // memory effect remains the distinct original AtomicRmw record.
        self.operand(site, atomic.value(), budget)?;
        let row = SourceAtomicUseV41 {
            site,
            source: atomic as *const SemanticAtomicRmwV1 as usize,
            address: atomic.address() as *const SemanticPlaceV1 as usize,
            value: atomic.value() as *const SemanticOperandV1 as usize,
            destination: atomic.destination() as *const SemanticPlaceV1 as usize,
            node,
            custody: self.plan.nodes[node]
                .atomic_custody
                .ok_or(ArgumentResourceV1::Accounting)?,
            operation: atomic.operation(),
            access: atomic.access(),
        };
        let mut prior = false;
        for old in &self.plan.atomic_uses {
            budget.charge_work(4)?;
            if old.site == site {
                if *old != row {
                    return Err(source_atomic_view_error_v41());
                }
                prior = true;
            }
        }
        if !prior {
            emission_push_v1(&mut self.plan.atomic_uses, row, budget)?;
        }
        let result = self.plain(fact.scalar, budget)?;
        self.assign(site, atomic.destination(), result, budget)
    }
}
