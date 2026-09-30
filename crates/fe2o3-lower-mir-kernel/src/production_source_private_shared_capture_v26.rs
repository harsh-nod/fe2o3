// A promoted shared reference carries the scalar captured at its original
// borrow. Only original SSA transports can reconnect a later dereference to
// that actual read; an equal optimized value is never an origin witness.
fn original_shared_capture_headers_v26() -> Result<usize, ArgumentResourceV1> {
    type Frame<'a> = (
        &'a OriginalEntryIndexV20<'a, 'a>,
        &'a ProductionSourceScalarLeavesV18<'a>,
        &'a SemanticFunctionDeclV1,
        &'a SemanticFunctionDeclV1,
        &'a SemanticPlaceV1,
        &'a SemanticPlaceV1,
        Option<&'a SemanticPlaceV1>,
        Option<&'a SemanticOperandV1>,
        Option<&'a SemanticTypeDeclV1>,
        Option<&'a SemanticTypeShapeV1>,
        Option<&'a fe2o3_mir_model::semantic_mir_v1::SemanticLocalDeclV1>,
        OriginalEntryDefinitionRowV20,
        EntryValueV20,
        EntrySiteV20,
        EntryOperandV20,
        Option<(usize, SemanticBlockIdV1)>,
        SourceOwnedResultV18<ProductionSemanticExpressionV2>,
        Option<ProductionSemanticExpressionV2>,
        [usize; 24],
        [&'a (); 20],
    );
    argument_sum_v1(&[
        size_of::<Frame<'_>>(),
        size_of::<SourceOwnedResultV18<Frame<'_>>>(),
    ])
}

impl OriginalEntryIndexV20<'_, '_> {
    #[allow(clippy::too_many_arguments)]
    fn captured_shared_reference_expression_v26(
        &self,
        leaves: &ProductionSourceScalarLeavesV18<'_>,
        mut instance: usize,
        ty: SemanticTypeIdV1,
        scalar: ProductionSemanticScalarTypeV2,
        mut site: EntrySiteV20,
        mut role: EntryOperandV20,
        place: &SemanticPlaceV1,
        mut depth: usize,
        remaining: &mut usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionSemanticExpressionV2> {
        self.check(budget)?;
        let mut place = place;
        let mut dereference = true;
        loop {
            budget.charge_work(96)?;
            if depth > MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 || *remaining == 0 {
                return self
                    .source
                    .source
                    .missing("private shared capture exceeds expression bounds");
            }
            depth += 1;
            *remaining -= 1;
            let function = leaves.original_function(instance, budget)?;
            let (function_id, incoming) =
                self.source
                    .source
                    .instance(leaves.leaves.root, instance, budget)?;
            let semantic = self.source.source.source_semantic(budget)?;
            if !std::ptr::eq(leaves.leaves.relation, self.source)
                || !scoped_object_original_place_v29(function, site, role)
                    .is_some_and(|original| std::ptr::eq(original, place))
            {
                return self
                    .source
                    .source
                    .missing("private shared capture original place differs");
            }
            let local = function
                .locals()
                .get(place.local().index() as usize)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "private shared capture holder absent",
                ))?;
            let reference_ty = local.ty();
            if !matches!(semantic.types().get(reference_ty.index() as usize).map(SemanticTypeDeclV1::shape),
                Some(SemanticTypeShapeV1::Pointer(pointer))
                    if pointer.kind() == SemanticPointerKindV1::Reference
                        && pointer.mutability() == SemanticMutabilityV1::Immutable
                        && pointer.metadata() == SemanticPointerMetadataV1::None
                        && pointer.pointee() == ty)
                || if dereference {
                    place.ty() != ty
                        || !matches!(place.projections(), [projection]
                        if projection.kind() == SemanticProjectionKindV1::Dereference && projection.result_type() == ty)
                } else {
                    place.ty() != reference_ty || !place.projections().is_empty()
                }
            {
                return self.source.source.missing(
                    "private shared capture requires an exact immutable scalar reference",
                );
            }
            let value =
                self.promoted_use(function_id, site, role, place.local().index(), budget)?;
            let definition = self.definition(function_id, value, budget)?;
            if definition.local != place.local().index() {
                return self
                    .source
                    .source
                    .missing("private shared capture SSA holder differs");
            }
            match definition.origin {
                OriginalEntryDefinitionV20::CallReturn { .. } => {
                    return self
                        .source
                        .source
                        .missing("private shared call-return referent is not interpreted");
                }
                OriginalEntryDefinitionV20::Argument(argument) => {
                    let Some((caller, block)) = incoming else {
                        return self
                            .source
                            .source
                            .missing("private shared capture root reference remains external");
                    };
                    if caller >= instance || local.role() != SemanticLocalRoleV1::Argument(argument)
                    {
                        return self
                            .source
                            .source
                            .missing("private shared capture caller identity differs");
                    }
                    let caller_function = leaves.original_function(caller, budget)?;
                    let Some(SemanticTerminatorKindV1::Call(call)) = caller_function
                        .blocks()
                        .get(block.index() as usize)
                        .map(|block| block.terminator().kind())
                    else {
                        return self
                            .source
                            .source
                            .missing("private shared capture original call absent");
                    };
                    if !matches!(semantic.callables().get(call.callee().index() as usize),
                        Some(SemanticCallableDeclV1::Defined { function }) if *function == function_id)
                    {
                        return self
                            .source
                            .source
                            .missing("private shared capture original callee differs");
                    }
                    let Some(
                        SemanticOperandV1::Copy(argument_place)
                        | SemanticOperandV1::Move(argument_place),
                    ) = call.arguments().get(argument as usize)
                    else {
                        return self
                            .source
                            .source
                            .missing("private shared capture caller argument is not a holder");
                    };
                    if argument_place.ty() != reference_ty {
                        return self
                            .source
                            .source
                            .missing("private shared capture caller argument type differs");
                    }
                    instance = caller;
                    site = EntrySiteV20::Terminator {
                        block: fe2o3_mir_model::SsaBlockIdV1::new(block.index()),
                    };
                    role = EntryOperandV20::CallArgument(argument);
                    place = argument_place;
                    dereference = false;
                }
                OriginalEntryDefinitionV20::Assignment { block, statement } => {
                    let Some(SemanticStatementKindV1::Assign(assignment)) = function
                        .blocks()
                        .get(block as usize)
                        .and_then(|block| block.statements().get(statement as usize))
                        .map(|statement| statement.kind())
                    else {
                        return self
                            .source
                            .source
                            .missing("private shared capture original assignment absent");
                    };
                    if assignment.destination().local().index() != definition.local
                        || !assignment.destination().projections().is_empty()
                        || assignment.destination().ty() != reference_ty
                        || assignment.value().result_type() != reference_ty
                    {
                        return self
                            .source
                            .source
                            .missing("private shared capture original destination differs");
                    }
                    site = EntrySiteV20::Statement {
                        block: fe2o3_mir_model::SsaBlockIdV1::new(block),
                        statement,
                    };
                    match assignment.value().kind() {
                        SemanticRvalueKindV1::Use(
                            SemanticOperandV1::Copy(holder) | SemanticOperandV1::Move(holder),
                        ) => {
                            if holder.ty() != reference_ty {
                                return self
                                    .source
                                    .source
                                    .missing("private shared capture transport type differs");
                            }
                            place = holder;
                            role = EntryOperandV20::RvalueOperand(0);
                            dereference = false;
                        }
                        SemanticRvalueKindV1::Borrow {
                            kind: SemanticBorrowKindV1::Shared,
                            place: referent,
                        } => {
                            if referent.ty() != ty {
                                return self
                                    .source
                                    .source
                                    .missing("private shared capture borrow type differs");
                            }
                            if !referent.projections().is_empty() {
                                let base = function
                                    .locals()
                                    .get(referent.local().index() as usize)
                                    .and_then(|local| {
                                        semantic.types().get(local.ty().index() as usize)
                                    })
                                    .map(SemanticTypeDeclV1::shape);
                                if !matches!(referent.projections(), [projection]
                                    if projection.kind() == SemanticProjectionKindV1::Dereference && projection.result_type() == ty)
                                    || !matches!(base, Some(SemanticTypeShapeV1::Pointer(pointer))
                                        if pointer.kind() == SemanticPointerKindV1::Reference
                                            && pointer.mutability() == SemanticMutabilityV1::Immutable
                                            && pointer.metadata() == SemanticPointerMetadataV1::None
                                            && pointer.pointee() == ty)
                                {
                                    return self.source.source.missing("private shared capture reborrow changes its immutable whole-scalar origin");
                                }
                            }
                            if let Some(expression) =
                                leaves.original_place(instance, function, referent, budget)?
                            {
                                if expression.scalar() != scalar {
                                    return self
                                        .source
                                        .source
                                        .missing("private shared capture scalar differs");
                                }
                                return Ok(expression);
                            }
                            place = referent;
                            role = EntryOperandV20::RvaluePlace;
                            dereference = true;
                        }
                        _ => {
                            return self.source.source.missing(
                                "private shared capture unsupported original reference origin",
                            );
                        }
                    }
                }
            }
        }
    }
}
