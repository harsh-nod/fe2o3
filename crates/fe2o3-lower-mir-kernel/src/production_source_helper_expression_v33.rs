// Source-only, total acyclic helper expressions. The actual call/return transport
// remains separately checked; no physical definition chooses a source result.
#[derive(Clone, Copy, Debug)]
struct OriginalHelperCallV33 {
    key: [usize; 3],
    child: usize,
    function: SemanticFunctionIdV1,
}

fn source_helper_expression_headers_v33() -> Result<usize, ArgumentResourceV1> {
    type Frame<'a> = (
        [&'a (); 28],
        [usize; 24],
        [u32; 6],
        [bool; 4],
        OriginalHelperCallV33,
        OriginalEntryDefinitionRowV20,
        [ProductionSemanticExpressionV2; 3],
        Option<ProductionSemanticExpressionV2>,
        SourceOwnedResultV18<ProductionSemanticExpressionV2>,
        SourceOwnedResultV18<OriginalHelperCallV33>,
        SourceOwnedResultV18<()>,
        Vec<usize>,
        SemanticPlaceV1,
        SemanticOperandV1,
        EntrySiteV20,
        EntryValueV20,
        SemanticTypeIdV1,
        ProductionSemanticScalarTypeV2,
    );
    argument_product_v1(
        MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 + 1,
        argument_sum_v1(&[size_of::<Frame<'_>>(), std::mem::align_of::<Frame<'_>>()])?,
    )
}

fn source_helper_calls_v33(
    relation: &ProductionSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Vec<OriginalHelperCallV33>> {
    let source = relation.source;
    // Constructor temporaries coexist with the retained index. This fixed credit
    // stays with that index's existing outer source-owned cleanup scope.
    budget.reserve_storage(argument_sum_v1(&[
        size_of::<Vec<OriginalHelperCallV33>>(),
        size_of::<OriginalHelperCallV33>(),
        16 * size_of::<usize>(),
        12 * size_of::<&()>(),
        size_of::<SourceOwnedResultV18<Vec<OriginalHelperCallV33>>>(),
    ])?)?;
    let roots = source.root_count(budget)?;
    let semantic = source.source_semantic(budget)?;
    let mut capacity = 0usize;
    for root in 0..roots {
        budget.charge_work(1)?;
        capacity = argument_sum_v1(&[capacity, source.instance_count(root, budget)?])?;
    }
    let mut calls = emission_vec_v1(capacity, budget).map_err(source_emission_error_v18)?;
    for root in 0..roots {
        let count = source.instance_count(root, budget)?;
        for child in 0..count {
            budget.charge_work(7)?;
            let (function, incoming) = source.instance(root, child, budget)?;
            let Some((parent, block)) = incoming else {
                if child != 0 || function != source.root(root, budget)?.0 {
                    return source.missing("source helper incoming root differs");
                }
                continue;
            };
            if parent >= child {
                return source.missing("source helper invocation order differs");
            }
            let (parent_function, _) = source.instance(root, parent, budget)?;
            let callee = match semantic
                .functions()
                .get(parent_function.index() as usize)
                .and_then(|function| function.blocks().get(block.index() as usize))
                .map(|block| block.terminator().kind())
            {
                Some(SemanticTerminatorKindV1::Call(call)) => call.callee(),
                Some(SemanticTerminatorKindV1::TailCall(call)) => call.callee(),
                _ => return source.missing("source helper original call absent"),
            };
            if !matches!(semantic.callables().get(callee.index() as usize),
                Some(SemanticCallableDeclV1::Defined { function: original }) if *original == function)
            {
                return source.missing("source helper original callee differs");
            }
            calls.push(OriginalHelperCallV33 {
                key: [root, parent, block.index() as usize],
                child,
                function,
            });
        }
    }
    private_array_heapsort_v1(
        &mut calls,
        |row| row.key,
        &mut SourceCorrespondenceWorkV18(budget),
        || ArgumentResourceV1::Arithmetic.into(),
    )?;
    for pair in calls.windows(2) {
        budget.charge_work(1)?;
        if pair[0].key == pair[1].key {
            return source.missing("source helper invocation is ambiguous");
        }
    }
    Ok(calls)
}

fn source_helper_scalar_v33(
    source: &ProductionSourceOwnedViewV18<'_>,
    ty: SemanticTypeIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<ProductionSemanticScalarTypeV2> {
    budget.charge_work(2)?;
    kir_semantic_scalar_v1(
        &lower_scalar_type(source.source_semantic(budget)?.types(), ty)
            .map_err(source_emission_error_v18)?,
    )
    .filter(|scalar| {
        matches!(
            scalar,
            ProductionSemanticScalarTypeV2::Bool
                | ProductionSemanticScalarTypeV2::Integer {
                    bits: 8 | 16 | 32 | 64,
                    ..
                }
        )
    })
    .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
        "source helper scalar type is not modeled",
    ))
}

fn source_helper_equal_work_v33(
    expression: &ProductionSemanticExpressionV2,
    depth: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<usize> {
    budget.charge_work(1)?;
    if depth > MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 {
        return Err(ProductionSourceOwnedViewErrorV18::Binding(
            "source helper equality exceeds expression depth",
        ));
    }
    let mut work = 16usize;
    match expression {
        ProductionSemanticExpressionV2::Symbol { .. }
        | ProductionSemanticExpressionV2::Constant { .. } => {}
        ProductionSemanticExpressionV2::Load(load) => {
            work = argument_sum_v1(&[work, argument_product_v1(load.indices.len(), 8)?])?;
        }
        ProductionSemanticExpressionV2::Unary { operand, .. }
        | ProductionSemanticExpressionV2::Cast { operand, .. } => {
            work = argument_sum_v1(&[
                work,
                source_helper_equal_work_v33(operand, depth + 1, budget)?,
            ])?;
        }
        ProductionSemanticExpressionV2::Binary { lhs, rhs, .. }
        | ProductionSemanticExpressionV2::Compare { lhs, rhs, .. } => {
            work = argument_sum_v1(&[
                work,
                source_helper_equal_work_v33(lhs, depth + 1, budget)?,
                source_helper_equal_work_v33(rhs, depth + 1, budget)?,
            ])?;
        }
        ProductionSemanticExpressionV2::Select {
            condition,
            when_true,
            when_false,
            ..
        } => {
            work = argument_sum_v1(&[
                work,
                source_helper_equal_work_v33(condition, depth + 1, budget)?,
                source_helper_equal_work_v33(when_true, depth + 1, budget)?,
                source_helper_equal_work_v33(when_false, depth + 1, budget)?,
            ])?;
        }
    }
    Ok(work)
}

impl OriginalEntryIndexV20<'_, '_> {
    fn helper_child_v33(
        &self,
        root: usize,
        parent: usize,
        block: u32,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<OriginalHelperCallV33> {
        self.check(budget)?;
        let key = [root, parent, block as usize];
        let (mut lo, mut hi) = (0, self.helper_calls.len());
        while lo < hi {
            budget.charge_work(1)?;
            let middle = lo + (hi - lo) / 2;
            if self.helper_calls[middle].key < key {
                lo = middle + 1;
            } else {
                hi = middle;
            }
        }
        budget.charge_work(2)?;
        let row = self
            .helper_calls
            .get(lo)
            .filter(|row| row.key == key)
            .copied()
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "source helper call has no defined invocation",
            ))?;
        if !self
            .source
            .source
            .instance_active(root, row.child, budget)?
        {
            return self
                .source
                .source
                .missing("source helper invocation is inactive");
        }
        Ok(row)
    }

    fn private_call_expression_v33(
        &self,
        leaves: &ProductionSourceScalarLeavesV18<'_>,
        instance: usize,
        function: SemanticFunctionIdV1,
        definition: OriginalEntryDefinitionRowV20,
        value: EntryValueV20,
        block: u32,
        edge: usize,
        ty: SemanticTypeIdV1,
        scalar: ProductionSemanticScalarTypeV2,
        depth: usize,
        remaining: &mut usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionSemanticExpressionV2> {
        let EntryValueV20::Definition(value) = value else {
            return self
                .source
                .source
                .missing("source helper result is not an exact definition");
        };
        let call = source_entry_call_return_v32(
            self.source,
            function,
            edge,
            definition.local,
            value.get(),
            budget,
        )?;
        budget.charge_work(6)?;
        if definition.key != [function.index(), value.get()]
            || !matches!(self.source.source.source_ssa(budget)?.occurrences_v1()
                .and_then(|rows| rows.function(function))
                .and_then(|rows| rows.edge_definitions().get(edge).map(|row| row.edge().source().get())), Some(actual) if actual == block)
            || call
                .destination()
                .map(|destination| destination.place().ty())
                != Some(ty)
            || call.unwind() != SemanticUnwindActionV1::Unreachable
        {
            return self
                .source
                .source
                .missing("source helper call-return context differs");
        }
        let child = self.helper_child_v33(leaves.leaves.root, instance, block, budget)?;
        self.private_helper_arguments_v33(
            leaves, instance, function, block, call, child, depth, remaining, budget,
        )?;
        self.private_helper_body_v33(
            leaves,
            child.child,
            child.function,
            ty,
            scalar,
            depth,
            remaining,
            budget,
        )
    }

    fn private_helper_arguments_v33(
        &self,
        leaves: &ProductionSourceScalarLeavesV18<'_>,
        instance: usize,
        function: SemanticFunctionIdV1,
        block: u32,
        call: &SemanticDirectCallV1,
        child: OriginalHelperCallV33,
        depth: usize,
        remaining: &mut usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        let source = self.source.source.source_semantic(budget)?;
        let parent = leaves.original_function(instance, budget)?;
        let callee = leaves.original_function(child.child, budget)?;
        budget.charge_work(6)?;
        if !matches!(source.callables().get(call.callee().index() as usize),
            Some(SemanticCallableDeclV1::Defined { function }) if *function == child.function)
            || child.key != [leaves.leaves.root, instance, block as usize]
            || call.arguments().len() != callee.abi().source_input_types().len()
        {
            return self
                .source
                .source
                .missing("source helper argument roster differs");
        }
        let site = EntrySiteV20::Terminator {
            block: fe2o3_mir_model::SsaBlockIdV1::new(block),
        };
        for (ordinal, operand) in call.arguments().iter().enumerate() {
            budget.charge_work(5)?;
            let ty = semantic_operand_type(operand);
            if callee.abi().source_input_types().get(ordinal) != Some(&ty) {
                return self
                    .source
                    .source
                    .missing("source helper argument type differs");
            }
            let role = EntryOperandV20::CallArgument(
                u32::try_from(ordinal).map_err(|_| ArgumentResourceV1::Arithmetic)?,
            );
            match source
                .types()
                .get(ty.index() as usize)
                .map(SemanticTypeDeclV1::shape)
            {
                Some(SemanticTypeShapeV1::Pointer(_)) => {
                    // A whole SSA pointer read is not a pointee read. A later
                    // dereference still needs the existing source capture/loan.
                    let place = match operand {
                        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)
                            if place.projections().is_empty() =>
                        {
                            place
                        }
                        _ => {
                            return self.source.source.missing(
                                "source helper pointer argument evaluation is not modeled",
                            );
                        }
                    };
                    if parent
                        .locals()
                        .get(place.local().index() as usize)
                        .map(|local| local.ty())
                        != Some(ty)
                    {
                        return self
                            .source
                            .source
                            .missing("source helper pointer argument local differs");
                    }
                    self.promoted_use(function, site, role, place.local().index(), budget)?;
                }
                Some(SemanticTypeShapeV1::Unit) if matches!(operand, SemanticOperandV1::Constant(value) if value.value() == &SemanticConstantValueV1::ZeroSized) =>
                    {}
                _ => {
                    let scalar = source_helper_scalar_v33(self.source.source, ty, budget)?;
                    self.private_helper_discard_v33(
                        leaves,
                        instance,
                        ty,
                        scalar,
                        OriginalPrivateInputV22::Operand {
                            site,
                            role,
                            operand,
                        },
                        depth,
                        remaining,
                        budget,
                    )?;
                }
            }
        }
        Ok(())
    }

    fn private_helper_discard_v33(
        &self,
        leaves: &ProductionSourceScalarLeavesV18<'_>,
        instance: usize,
        ty: SemanticTypeIdV1,
        scalar: ProductionSemanticScalarTypeV2,
        input: OriginalPrivateInputV22<'_>,
        depth: usize,
        remaining: &mut usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        let floor = budget.storage();
        // Every read must still bind to its exact captured occurrence/loan in
        // the existing source-owned resolver, even if its value is unused.
        // Dropping this temporary quote does not remove that occurrence from
        // the mandatory memory census or discharge its read-validity obligation.
        let expression = self.private_expression_v22(
            leaves, instance, ty, scalar, input, depth, remaining, budget,
        )?;
        if expression.scalar() != scalar {
            return self
                .source
                .source
                .missing("source helper statement scalar differs");
        }
        drop(expression);
        budget.release_storage(
            budget
                .storage()
                .checked_sub(floor)
                .ok_or(ArgumentResourceV1::Accounting)?,
        )?;
        Ok(())
    }

    fn private_helper_body_v33(
        &self,
        leaves: &ProductionSourceScalarLeavesV18<'_>,
        instance: usize,
        function_id: SemanticFunctionIdV1,
        ty: SemanticTypeIdV1,
        scalar: ProductionSemanticScalarTypeV2,
        depth: usize,
        remaining: &mut usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionSemanticExpressionV2> {
        self.check(budget)?;
        budget.charge_work(8)?;
        if depth > MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 || *remaining == 0 {
            return self
                .source
                .source
                .missing("source helper exceeds existing expression bounds");
        }
        *remaining -= 1;
        let next = depth + 1;
        let function = leaves.original_function(instance, budget)?;
        if !std::ptr::eq(leaves.leaves.relation, self.source)
            || self
                .source
                .source
                .instance(leaves.leaves.root, instance, budget)?
                .0
                != function_id
            || function.abi().return_type() != ty
            || source_helper_scalar_v33(self.source.source, ty, budget)? != scalar
        {
            return self
                .source
                .source
                .missing("source helper declaring function or return type differs");
        }
        let owner = self.source.source.source_ssa(budget)?;
        let plan = owner
            .plan_for_function(function_id)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "source helper SSA plan absent",
            ))?
            .plan();
        let rows = owner
            .occurrences_v1()
            .and_then(|rows| rows.function(function_id))
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "source helper SSA occurrences absent",
            ))?;
        // A callee-local phi may not become a fresh opaque result symbol. The
        // accepted subset has no such use; caller phi cuts remain source-owned.
        for event in rows.events() {
            budget.charge_work(2)?;
            if event.is_reachable()
                && matches!(
                    event.resolved(),
                    Some(EntryEventV20::Use {
                        value: EntryValueV20::BlockArgument { .. },
                        ..
                    })
                )
            {
                return self
                    .source
                    .source
                    .missing("source helper SSA join computation is not modeled");
            }
        }
        let mut returned_local = None;
        for (ordinal, local) in function.locals().iter().enumerate() {
            budget.charge_work(2)?;
            if local.role() == SemanticLocalRoleV1::Return {
                if local.ty() != ty || returned_local.replace(ordinal as u32).is_some() {
                    return self
                        .source
                        .source
                        .missing("source helper return local differs");
                }
            }
        }
        let returned_local = returned_local.ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "source helper return local absent",
        ))?;
        let mut positions =
            emission_vec_v1(function.blocks().len(), budget).map_err(source_emission_error_v18)?;
        budget.charge_work(function.blocks().len())?;
        positions.resize(function.blocks().len(), usize::MAX);
        for (position, block) in plan.reverse_postorder().iter().enumerate() {
            budget.charge_work(3)?;
            let at = positions.get_mut(block.get() as usize).ok_or(
                ProductionSourceOwnedViewErrorV18::Binding("source helper control block absent"),
            )?;
            if *at != usize::MAX || !plan.is_reachable(*block) {
                return self
                    .source
                    .source
                    .missing("source helper control order differs");
            }
            *at = position;
        }
        let mut returned = None;
        for (block, declaration) in function.blocks().iter().enumerate() {
            budget.charge_work(3)?;
            let reachable = plan.is_reachable(fe2o3_mir_model::SsaBlockIdV1::new(block as u32));
            if reachable != (positions[block] != usize::MAX) {
                return self
                    .source
                    .source
                    .missing("source helper reachable control census differs");
            }
            if !reachable {
                continue;
            }
            declaration.terminator().kind().try_for_each_edge(|edge| {
                budget.charge_work(3)?;
                let target = *positions.get(edge.target().index() as usize).ok_or(
                    ProductionSourceOwnedViewErrorV18::Binding("source helper successor absent"),
                )?;
                if target == usize::MAX || target <= positions[block] {
                    return self
                        .source
                        .source
                        .missing("source helper cyclic control is not modeled");
                }
                Ok(())
            })?;
            for (statement, declaration) in declaration.statements().iter().enumerate() {
                budget.charge_work(4)?;
                match declaration.kind() {
                    SemanticStatementKindV1::Assign(assignment) => {
                        if !assignment.destination().projections().is_empty()
                            || function
                                .locals()
                                .get(assignment.destination().local().index() as usize)
                                .map(|local| local.ty())
                                != Some(assignment.destination().ty())
                            || matches!(assignment.value().kind(), SemanticRvalueKindV1::Load(load) if load.atomic().is_some() || load.volatility() != SemanticVolatilityV1::NonVolatile)
                        {
                            return self
                                .source
                                .source
                                .missing("source helper assignment effect is not modeled");
                        }
                        let ty = assignment.destination().ty();
                        let scalar = source_helper_scalar_v33(self.source.source, ty, budget)?;
                        self.private_helper_discard_v33(
                            leaves,
                            instance,
                            ty,
                            scalar,
                            OriginalPrivateInputV22::Rvalue {
                                block: block as u32,
                                statement: statement as u32,
                                value: assignment.value(),
                            },
                            next,
                            remaining,
                            budget,
                        )?;
                    }
                    SemanticStatementKindV1::StorageLive(_)
                    | SemanticStatementKindV1::StorageDead(_)
                    | SemanticStatementKindV1::Nop => {}
                    SemanticStatementKindV1::Deinitialize(place)
                        if place.projections().is_empty() => {}
                    _ => {
                        return self
                            .source
                            .source
                            .missing("source helper statement effect is not modeled");
                    }
                }
            }
            let site = EntrySiteV20::Terminator {
                block: fe2o3_mir_model::SsaBlockIdV1::new(block as u32),
            };
            match declaration.terminator().kind() {
                SemanticTerminatorKindV1::Goto(_) => {}
                SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => {
                    let ty = semantic_operand_type(discriminant);
                    let scalar = source_helper_scalar_v33(self.source.source, ty, budget)?;
                    self.private_helper_discard_v33(
                        leaves,
                        instance,
                        ty,
                        scalar,
                        OriginalPrivateInputV22::Operand {
                            site,
                            role: EntryOperandV20::SwitchDiscriminant,
                            operand: discriminant,
                        },
                        next,
                        remaining,
                        budget,
                    )?;
                }
                SemanticTerminatorKindV1::Call(call) => {
                    let destination =
                        call.destination()
                            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                                "source helper nested call diverges",
                            ))?;
                    if call.unwind() != SemanticUnwindActionV1::Unreachable
                        || destination.edge().role() != SemanticEdgeRoleV1::CallReturn
                        || !destination.place().projections().is_empty()
                    {
                        return self
                            .source
                            .source
                            .missing("source helper nested call effect is not modeled");
                    }
                    let child =
                        self.helper_child_v33(leaves.leaves.root, instance, block as u32, budget)?;
                    self.private_helper_arguments_v33(
                        leaves,
                        instance,
                        function_id,
                        block as u32,
                        call,
                        child,
                        next,
                        remaining,
                        budget,
                    )?;
                    let ty = destination.place().ty();
                    let scalar = source_helper_scalar_v33(self.source.source, ty, budget)?;
                    let floor = budget.storage();
                    let expression = self.private_helper_body_v33(
                        leaves,
                        child.child,
                        child.function,
                        ty,
                        scalar,
                        next,
                        remaining,
                        budget,
                    )?;
                    drop(expression);
                    budget.release_storage(
                        budget
                            .storage()
                            .checked_sub(floor)
                            .ok_or(ArgumentResourceV1::Accounting)?,
                    )?;
                }
                SemanticTerminatorKindV1::Return => {
                    let operand = SemanticOperandV1::Copy(
                        SemanticPlaceV1::new(
                            SemanticLocalIdV1::from_index(returned_local),
                            vec![],
                            ty,
                        )
                        .map_err(|_| {
                            ProductionSourceOwnedViewErrorV18::Binding(
                                "source helper return place differs",
                            )
                        })?,
                    );
                    let floor = budget.storage();
                    let expression = self.private_expression_v22(
                        leaves,
                        instance,
                        ty,
                        scalar,
                        OriginalPrivateInputV22::Operand {
                            site,
                            role: EntryOperandV20::ReturnValue,
                            operand: &operand,
                        },
                        next,
                        remaining,
                        budget,
                    )?;
                    if let Some(expected) = &returned {
                        let work = argument_sum_v1(&[
                            source_helper_equal_work_v33(expected, 0, budget)?,
                            source_helper_equal_work_v33(&expression, 0, budget)?,
                        ])?;
                        budget.charge_work(work)?;
                        if expected != &expression {
                            return self
                                .source
                                .source
                                .missing("source helper reachable return expressions differ");
                        }
                        drop(expression);
                        budget.release_storage(
                            budget
                                .storage()
                                .checked_sub(floor)
                                .ok_or(ArgumentResourceV1::Accounting)?,
                        )?;
                    } else {
                        returned = Some(expression);
                    }
                }
                _ => {
                    return self
                        .source
                        .source
                        .missing("source helper control effect is not modeled");
                }
            }
        }
        let storage = argument_product_v1(positions.capacity(), size_of::<usize>())?;
        drop(positions);
        budget.release_storage(storage)?;
        returned.ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "source helper has no reachable return",
        ))
    }
}
