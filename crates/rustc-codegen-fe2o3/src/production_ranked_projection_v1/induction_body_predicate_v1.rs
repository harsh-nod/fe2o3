#[derive(Clone, Debug, Eq, PartialEq)]
enum ProjectedInductionPredicateOperandV1 {
    Induction {
        ordinal: usize,
        source_local: SemanticLocalIdV1,
        source_type: SemanticTypeIdV1,
    },
    Uniform(ProductionRankedValueV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProjectedInductionBodyPredicateV1 {
    block: usize,
    source_statement: usize,
    source_assignment: SemanticStatementKindV1,
    source_terminator: SemanticTerminatorKindV1,
    lhs: ProjectedInductionPredicateOperandV1,
    rhs: ProjectedInductionPredicateOperandV1,
    true_block: usize,
    false_block: usize,
}

enum InductionPredicateSourceOperandV1 {
    Induction(usize),
    Uniform(SemanticOperandV1),
}

enum InductionPredicateProofsV18<'p, 'a> {
    Legacy(&'p mut SemanticAssertProofsV1<'a>),
    Source(&'p mut SemanticAssertProofsV1<'a>),
}

impl InductionPredicateProofsV18<'_, '_> {
    fn original(&self) -> &SemanticAssertProofsV1<'_> {
        match self { Self::Legacy(proof) | Self::Source(proof) => proof }
    }
    fn assignment_dominates_use(
        &mut self, definition: ScalarAssignmentSiteV1, block: usize, statement: usize,
        allocation: &mut source_ranked_consumer_resources_v18::ProjectionAllocationV18<'_>,
    ) -> Result<bool, ProductionRankedProjectionErrorV1> {
        match (self, allocation) {
            (Self::Legacy(proof), source_ranked_consumer_resources_v18::ProjectionAllocationV18::Legacy) =>
                proof.assignment_dominates_use(definition, block, statement),
            (Self::Source(proof), allocation @ source_ranked_consumer_resources_v18::ProjectionAllocationV18::Source(_)) =>
                proof.assignment_dominates_use_live_v18(definition, block, statement, allocation),
            _ => Err(ProductionRankedProjectionErrorV1::Incomplete("induction predicate changed its proof allocation mode")),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn induction_predicate_source_operand_v1(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    operand: &SemanticOperandV1,
    use_site: ScalarAssignmentSiteV1,
    constants: &[Option<u64>],
    local_definitions: &[u8],
    proofs: &mut SemanticAssertProofsV1<'_>,
    inductions: &[ProjectedUniformInductionV1],
    work: &mut usize,
) -> Result<Option<InductionPredicateSourceOperandV1>, ProductionRankedProjectionErrorV1> {
    induction_predicate_source_operand_core_v18(types, function, operand, use_site, constants,
        local_definitions, &mut InductionPredicateProofsV18::Legacy(proofs), inductions, work,
        &mut source_ranked_consumer_resources_v18::ProjectionAllocationV18::Legacy)
}

fn induction_predicate_source_operand_core_v18(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    operand: &SemanticOperandV1,
    use_site: ScalarAssignmentSiteV1,
    constants: &[Option<u64>],
    local_definitions: &[u8],
    proofs: &mut InductionPredicateProofsV18<'_, '_>,
    inductions: &[ProjectedUniformInductionV1],
    work: &mut usize,
    allocation: &mut source_ranked_consumer_resources_v18::ProjectionAllocationV18<'_>,
) -> Result<Option<InductionPredicateSourceOperandV1>, ProductionRankedProjectionErrorV1> {
    allocation.charge(1)?;
    let Some(bits) = unsigned_index_bits_v1(types, operand.ty()) else {
        return Ok(None);
    };
    if let SemanticOperandV1::Constant(_) = operand {
        return match constant_operand_value(operand, constants)
            .filter(|value| bits == 64 || *value < (1_u64 << bits)) {
            Some(_) => Ok(Some(InductionPredicateSourceOperandV1::Uniform(
                copy_induction_body_operand_v18(operand, allocation)?))),
            None => Ok(None),
        };
    }
    allocation.header::<Result<Option<SemanticLocalIdV1>, AliasResolutionErrorV18>>()?;
    let local = match resolve_block_copy_alias_before_core_v18(
        function,
        use_site,
        operand,
        local_definitions,
        proofs.original().assignments(),
        proofs.original().address_escaped(),
        work,
        MAX_PROJECTED_CAPABILITY_DATAFLOW_WORK_V1,
        allocation,
    ) {
        Ok(local) => local,
        // Precision is optional; unresolved aliases retain their original split.
        Err(AliasResolutionErrorV18::Logical(ProductionRankedProjectionErrorV1::Incomplete(_))) => return Ok(None),
        Err(error) => return Err(error.original()),
    };
    let Some(local) = local else {
        return Ok(None);
    };
    let index = local.index() as usize;
    let Some(declaration) = function.locals().get(index) else {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "an induction body operand is outside the semantic local table",
        ));
    };
    if declaration.ty() != operand.ty() {
        return Ok(None);
    }
    for (ordinal, induction) in inductions.iter().enumerate() {
        allocation.charge(1)?;
        project_loop_graph_charge_v1(work, 1)?;
        if local == induction.source_progress.induction {
            allocation.charge(1 + (usize::BITS - induction.loop_blocks.len().leading_zeros()) as usize)?;
            return Ok((induction.source_progress.induction_type == operand.ty()
                && induction.contains_block(use_site.block)
                && use_site.block != induction.header
                && use_site.block != induction.initializer_block
                && use_site.block != induction.latch
                && local_definitions.get(index).copied() == Some(2))
            .then_some(InductionPredicateSourceOperandV1::Induction(ordinal)));
        }
    }
    let admitted = match local_definitions.get(index).copied() {
        Some(0) => matches!(declaration.role(), SemanticLocalRoleV1::Argument(_)),
        Some(1) if constants.get(index).copied().flatten().is_some() => {
            let Some(definition) = proofs.original().assignments().get(index).copied().flatten() else {
                return Ok(None);
            };
            proofs.assignment_dominates_use(definition, use_site.block, use_site.statement, allocation)?
        }
        _ => false,
    };
    if !admitted {
        return Ok(None);
    }
    let operand = SemanticOperandV1::Copy(
        SemanticPlaceV1::new(local, allocation.empty()?, operand.ty()).map_err(|_| {
            ProductionRankedProjectionErrorV1::Unsupported(
                "an induction body operand has an invalid exact scalar place",
            )
        })?,
    );
    Ok(Some(InductionPredicateSourceOperandV1::Uniform(operand)))
}

#[allow(clippy::too_many_arguments)]
fn project_induction_body_predicates_v1(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    constants: &[Option<u64>],
    stable_argument_origins: &[Option<u32>],
    local_definitions: &[u8],
    inductions: &mut [ProjectedUniformInductionV1],
    arguments: &mut [Option<u32>],
    next_argument: &mut usize,
    operations: &mut Vec<ProductionRankedOperationV1>,
    next_value: &mut u32,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    project_induction_body_predicates_core_v18(types, function, constants, stable_argument_origins,
        local_definitions, inductions, arguments, next_argument, operations, next_value,
        &mut source_ranked_consumer_resources_v18::ProjectionAllocationV18::Legacy)
}

fn project_induction_body_predicates_core_v18(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    constants: &[Option<u64>],
    stable_argument_origins: &[Option<u32>],
    local_definitions: &[u8],
    inductions: &mut [ProjectedUniformInductionV1],
    arguments: &mut [Option<u32>],
    next_argument: &mut usize,
    operations: &mut Vec<ProductionRankedOperationV1>,
    next_value: &mut u32,
    allocation: &mut source_ranked_consumer_resources_v18::ProjectionAllocationV18<'_>,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    if inductions.is_empty() {
        return Ok(());
    }
    allocation.charge(inductions.len())?;
    if function.blocks().len() > MAX_RANKED_BOUNDS_BLOCKS
        || inductions
            .iter()
            .any(|induction| !induction.body_predicates.is_empty())
    {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "induction body predicate preparation has excessive blocks or stale predicates",
        ));
    }
    let mut proof = if matches!(allocation, source_ranked_consumer_resources_v18::ProjectionAllocationV18::Legacy) {
        SemanticAssertProofsV1::new(types, function)?
    } else { SemanticAssertProofsV1::new_live_v18(types, function, allocation)? };
    allocation.header::<InductionPredicateProofsV18<'_, '_>>()?;
    let mut proofs = if matches!(allocation, source_ranked_consumer_resources_v18::ProjectionAllocationV18::Legacy) {
        InductionPredicateProofsV18::Legacy(&mut proof)
    } else { InductionPredicateProofsV18::Source(&mut proof) };
    let mut work = 0;
    for (block_index, block) in function.blocks().iter().enumerate() {
        allocation.charge(1)?;
        project_loop_graph_charge_v1(&mut work, 1)?;
        let mut in_body = false;
        for induction in inductions.iter() {
            allocation.charge(1)?;
            project_loop_graph_charge_v1(&mut work, 1)?;
            allocation.charge(1 + (usize::BITS - induction.loop_blocks.len().leading_zeros()) as usize)?;
            in_body |= induction.contains_block(block_index)
                && ![
                    induction.initializer_block,
                    induction.header,
                    induction.latch,
                ]
                .contains(&block_index);
        }
        if !in_body {
            continue;
        }
        let SemanticTerminatorKindV1::SwitchInt {
            discriminant,
            targets,
        } = block.terminator().kind()
        else {
            continue;
        };
        let Some(condition) = simple_operand_local(discriminant) else {
            continue;
        };
        let condition_index = condition.index() as usize;
        let [explicit] = targets.values() else {
            continue;
        };
        if explicit.value() > 1
            || explicit.edge().role() != SemanticEdgeRoleV1::SwitchValue
            || targets.otherwise().role() != SemanticEdgeRoleV1::SwitchOtherwise
            || local_definitions.get(condition_index).copied() != Some(1)
            || proofs.original().address_escaped().get(condition_index).copied() != Some(false)
            || !matches!(
                types
                    .get(discriminant.ty().index() as usize)
                    .map(|ty| ty.shape()),
                Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool))
            )
        {
            continue;
        }
        let Some(definition) = proofs.original().assignments().get(condition_index).copied().flatten() else {
            continue;
        };
        // A body predicate is tied to this exact use, never cached as an entry value.
        if definition.block != block_index || definition.statement >= block.statements().len() {
            continue;
        }
        let SemanticStatementKindV1::Assign(assignment) =
            block.statements()[definition.statement].kind()
        else {
            continue;
        };
        let SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::LessThan,
            left,
            right,
        } = assignment.value().kind()
        else {
            continue;
        };
        if !assignment.destination().projections().is_empty()
            || assignment.destination().local() != condition
            || assignment.destination().ty() != discriminant.ty()
            || assignment.value().result_type() != discriminant.ty()
            || left.ty() != right.ty()
        {
            continue;
        }
        let Some(lhs) = induction_predicate_source_operand_core_v18(
            types,
            function,
            left,
            definition,
            constants,
            local_definitions,
            &mut proofs,
            inductions,
            &mut work,
            allocation,
        )?
        else {
            continue;
        };
        let Some(rhs) = induction_predicate_source_operand_core_v18(
            types,
            function,
            right,
            definition,
            constants,
            local_definitions,
            &mut proofs,
            inductions,
            &mut work,
            allocation,
        )?
        else {
            continue;
        };
        let owner = match (&lhs, &rhs) {
            (InductionPredicateSourceOperandV1::Induction(index), _)
            | (_, InductionPredicateSourceOperandV1::Induction(index)) => *index,
            _ => continue,
        };
        let mut materialize = |operand| match operand {
            InductionPredicateSourceOperandV1::Induction(ordinal) => {
                let progress = &inductions[ordinal].source_progress;
                Ok(Some(ProjectedInductionPredicateOperandV1::Induction {
                    ordinal,
                    source_local: progress.induction,
                    source_type: progress.induction_type,
                }))
            }
            InductionPredicateSourceOperandV1::Uniform(operand) => {
                if constant_operand_value(&operand, constants).is_none()
                    && let Some(origin) = simple_operand_local(&operand)
                        .and_then(|local| stable_argument_origins.get(local.index() as usize))
                        .copied()
                        .flatten()
                    && arguments.get(origin as usize).copied().flatten().is_none()
                    && *next_argument >= HARD_MAX_PRODUCTION_RANKED_ARGUMENTS
                {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "an induction body predicate exceeds the ranked argument limit",
                    ));
                }
                project_uniform_switch_operand_core_v18(
                    &operand,
                    constants,
                    stable_argument_origins,
                    arguments,
                    next_argument,
                    operations,
                    next_value,
                    allocation,
                )
                .map(|value| value.map(ProjectedInductionPredicateOperandV1::Uniform))
            }
        };
        let (Some(lhs), Some(rhs)) = (materialize(lhs)?, materialize(rhs)?) else {
            continue;
        };
        let explicit_block = explicit.edge().target().index() as usize;
        let otherwise = targets.otherwise().target().index() as usize;
        let (true_block, false_block) = if explicit.value() == 0 {
            (otherwise, explicit_block)
        } else {
            (explicit_block, otherwise)
        };
        if true_block == false_block {
            continue;
        }
        let predicates = &mut inductions[owner].body_predicates;
        allocation.reserve(predicates, 1, false, "induction body predicate storage cannot be reserved")?;
        let (source_assignment, source_terminator) = copy_induction_body_source_v18(
            block.statements()[definition.statement].kind(), block.terminator().kind(), allocation)?;
        allocation.push(predicates, ProjectedInductionBodyPredicateV1 {
            block: block_index,
            source_statement: definition.statement,
            source_assignment,
            source_terminator,
            lhs,
            rhs,
            true_block,
            false_block,
        })?;
    }
    Ok(())
}

fn indexed_induction_body_predicates_v1<'a>(
    function: &SemanticFunctionDeclV1,
    inductions: &'a [ProjectedUniformInductionV1],
) -> Result<Vec<Option<&'a ProjectedInductionBodyPredicateV1>>, ProductionRankedProjectionErrorV1> {
    indexed_induction_body_predicates_core_v18(function, inductions,
        &mut source_ranked_consumer_resources_v18::ProjectionAllocationV18::Legacy)
}

fn indexed_induction_body_predicates_core_v18<'a>(
    function: &SemanticFunctionDeclV1,
    inductions: &'a [ProjectedUniformInductionV1],
    allocation: &mut source_ranked_consumer_resources_v18::ProjectionAllocationV18<'_>,
) -> Result<Vec<Option<&'a ProjectedInductionBodyPredicateV1>>, ProductionRankedProjectionErrorV1> {
    if function.blocks().len() > MAX_RANKED_BOUNDS_BLOCKS {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "induction body predicate index exceeds the ranked block limit",
        ));
    }
    let mut indexed = allocation.optional(function.blocks().len())?;
    let mut work = 0;
    for (owner, induction) in inductions.iter().enumerate() {
        allocation.charge(1)?;
        project_loop_graph_charge_v1(&mut work, 1)?;
        for predicate in &induction.body_predicates {
            allocation.charge(1)?;
            project_loop_graph_charge_v1(&mut work, 1)?;
            allocation.charge(1 + (usize::BITS - induction.loop_blocks.len().leading_zeros()) as usize)?;
            if !induction.contains_block(predicate.block)
                || [induction.initializer_block, induction.header, induction.latch].contains(&predicate.block)
                || ![&predicate.lhs, &predicate.rhs].iter().any(|operand| {
                    matches!(operand, ProjectedInductionPredicateOperandV1::Induction { ordinal, .. } if *ordinal == owner)
                })
            {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "an induction body predicate has a stale live owner",
                ));
            }
            for operand in [&predicate.lhs, &predicate.rhs] {
                allocation.charge(1)?;
                let ProjectedInductionPredicateOperandV1::Induction {
                    ordinal,
                    source_local,
                    source_type,
                } = operand
                else {
                    continue;
                };
                let Some(source) = inductions.get(*ordinal) else {
                    return Err(ProductionRankedProjectionErrorV1::Incomplete(
                        "an induction body predicate has a stale operand owner",
                    ));
                };
                allocation.charge(1 + (usize::BITS - source.loop_blocks.len().leading_zeros()) as usize)?;
                if source.source_progress.induction != *source_local
                    || source.source_progress.induction_type != *source_type
                    || !source.contains_block(predicate.block)
                    || [source.initializer_block, source.header, source.latch]
                        .contains(&predicate.block)
                {
                    return Err(ProductionRankedProjectionErrorV1::Incomplete(
                        "an induction body predicate changed its exact induction operand",
                    ));
                }
            }
            let slot = indexed.get_mut(predicate.block).ok_or(
                ProductionRankedProjectionErrorV1::Unsupported(
                    "an induction body predicate is outside the semantic CFG",
                ),
            )?;
            if slot.replace(predicate).is_some() {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "multiple induction owners claim one body predicate",
                ));
            }
        }
    }
    Ok(indexed)
}

fn materialize_induction_body_predicate_v1(
    function: &SemanticFunctionDeclV1,
    predicate: &ProjectedInductionBodyPredicateV1,
    terminator: &ProjectedCfgTerminatorV1,
    block: u32,
    live: &[usize],
    base_blocks: &[Option<usize>],
    live_inductions: &[Vec<usize>],
) -> Result<ProductionRankedTerminatorV1, ProductionRankedProjectionErrorV1> {
    materialize_induction_body_predicate_core_v18(function, predicate, terminator, block,
        live, base_blocks, live_inductions,
        &mut source_ranked_consumer_resources_v18::ProjectionAllocationV18::Legacy)
}

fn materialize_induction_body_predicate_core_v18(
    function: &SemanticFunctionDeclV1,
    predicate: &ProjectedInductionBodyPredicateV1,
    terminator: &ProjectedCfgTerminatorV1,
    block: u32,
    live: &[usize],
    base_blocks: &[Option<usize>],
    live_inductions: &[Vec<usize>],
    allocation: &mut source_ranked_consumer_resources_v18::ProjectionAllocationV18<'_>,
) -> Result<ProductionRankedTerminatorV1, ProductionRankedProjectionErrorV1> {
    allocation.charge(1)?;
    let semantic_block = function.blocks().get(predicate.block).ok_or(
        ProductionRankedProjectionErrorV1::Unsupported(
            "an induction body predicate source is outside the semantic CFG",
        ),
    )?;
    if !matches!(allocation, source_ranked_consumer_resources_v18::ProjectionAllocationV18::Legacy) {
        let actual = semantic_block.statements().get(predicate.source_statement).ok_or(
            ProductionRankedProjectionErrorV1::Incomplete("an induction body predicate changed its exact semantic source"))?;
        check_induction_body_source_shape_v18(&predicate.source_assignment, &predicate.source_terminator, allocation)?;
        check_induction_body_source_shape_v18(actual.kind(), semantic_block.terminator().kind(), allocation)?;
        // Both compared ASTs now have fixed scalar operands and one target;
        // this is a bounded equality, not an unmetered arbitrary AST walk.
        allocation.charge(64)?;
    }
    if semantic_block.terminator().kind() != &predicate.source_terminator
        || semantic_block
            .statements()
            .get(predicate.source_statement)
            .map(|statement| statement.kind())
            != Some(&predicate.source_assignment)
    {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "an induction body predicate changed its exact semantic source",
        ));
    }
    let SemanticTerminatorKindV1::SwitchInt { targets, .. } = &predicate.source_terminator else {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "an induction body predicate lost its semantic switch",
        ));
    };
    let [explicit] = targets.values() else {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "an induction body predicate changed its switch cardinality",
        ));
    };
    let expected = if explicit.value() == 0 {
        (targets.otherwise().target(), explicit.edge().target())
    } else {
        (explicit.edge().target(), targets.otherwise().target())
    };
    if explicit.value() > 1
        || explicit.edge().role() != SemanticEdgeRoleV1::SwitchValue
        || targets.otherwise().role() != SemanticEdgeRoleV1::SwitchOtherwise
        || expected.0 == expected.1
        || (expected.0.index() as usize, expected.1.index() as usize)
            != (predicate.true_block, predicate.false_block)
    {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "an induction body predicate changed its exact edge roles or polarity",
        ));
    }
    let ProjectedCfgTerminatorV1::AnalysisSplit {
        first_block,
        second_block,
    } = terminator
    else {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "an induction body predicate changed its unresolved source split",
        ));
    };
    if !((*first_block == predicate.true_block && *second_block == predicate.false_block)
        || (*first_block == predicate.false_block && *second_block == predicate.true_block))
    {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "an induction body predicate changed its exact source successors",
        ));
    }
    let operand = |operand: &ProjectedInductionPredicateOperandV1,
        allocation: &mut source_ranked_consumer_resources_v18::ProjectionAllocationV18<'_>|
        -> Result<ProductionRankedValueV1, ProductionRankedProjectionErrorV1> { match operand {
        ProjectedInductionPredicateOperandV1::Uniform(value) => Ok(*value),
        ProjectedInductionPredicateOperandV1::Induction { ordinal, .. } => {
            allocation.charge(live.len())?;
            let argument = live
                .iter()
                .position(|candidate| candidate == ordinal)
                .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                    "an induction body predicate uses an induction outside its live body",
                ))?;
            Ok(ProductionRankedValueV1::BlockArgument {
                block,
                argument: u32::try_from(argument).map_err(|_| {
                    ProductionRankedProjectionErrorV1::Unsupported(
                        "live induction argument count does not fit u32",
                    )
                })?,
            })
        }
    }
    };
    let arguments_for = |target: usize, allocation: &mut source_ranked_consumer_resources_v18::ProjectionAllocationV18<'_>| {
        let target_live =
            live_inductions
                .get(target)
                .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                    "an induction body predicate target is outside the semantic CFG",
                ))?;
        forward_live_inductions_with_allocation_v18(block, live, target_live, allocation)
    };
    Ok(ProductionRankedTerminatorV1::IndexLessThanArgs {
        lhs: operand(&predicate.lhs, allocation)?,
        rhs: operand(&predicate.rhs, allocation)?,
        true_arguments: arguments_for(predicate.true_block, allocation)?,
        false_arguments: arguments_for(predicate.false_block, allocation)?,
        true_block: ranked_block_id(projected_target(base_blocks, predicate.true_block)?)?,
        false_block: ranked_block_id(projected_target(base_blocks, predicate.false_block)?)?,
    })
}

fn check_induction_body_operand_shape_v18(
    operand: &SemanticOperandV1,
    allocation: &mut source_ranked_consumer_resources_v18::ProjectionAllocationV18<'_>,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    allocation.charge(1)?;
    match operand {
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)
            if place.projections().is_empty() => Ok(()),
        SemanticOperandV1::Constant(value)
            if matches!(value.value(), SemanticConstantValueV1::Scalar(_)) => Ok(()),
        _ => Err(ProductionRankedProjectionErrorV1::Incomplete("an induction body predicate left its exact scalar operand profile")),
    }
}

fn copy_induction_body_operand_v18(
    operand: &SemanticOperandV1,
    allocation: &mut source_ranked_consumer_resources_v18::ProjectionAllocationV18<'_>,
) -> Result<SemanticOperandV1, ProductionRankedProjectionErrorV1> {
    if matches!(allocation, source_ranked_consumer_resources_v18::ProjectionAllocationV18::Legacy) {
        return Ok(operand.clone());
    }
    check_induction_body_operand_shape_v18(operand, allocation)?;
    match operand {
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
            let copied = SemanticPlaceV1::new(place.local(), allocation.empty()?, place.ty())
                .map_err(|_| ProductionRankedProjectionErrorV1::Incomplete("an induction predicate scalar place changed shape"))?;
            Ok(if matches!(operand, SemanticOperandV1::Copy(_)) { SemanticOperandV1::Copy(copied) }
                else { SemanticOperandV1::Move(copied) })
        }
        SemanticOperandV1::Constant(constant) => {
            let SemanticConstantValueV1::Scalar(value) = constant.value() else {
                return Err(ProductionRankedProjectionErrorV1::Incomplete("an induction predicate constant is not the exact scalar"));
            };
            Ok(SemanticOperandV1::Constant(fe2o3_mir_model::semantic_mir_v1::SemanticConstantV1::new(
                constant.ty(), SemanticConstantValueV1::Scalar(*value))))
        }
    }
}

fn copy_induction_body_source_v18(
    statement: &SemanticStatementKindV1,
    terminator: &SemanticTerminatorKindV1,
    allocation: &mut source_ranked_consumer_resources_v18::ProjectionAllocationV18<'_>,
) -> Result<(SemanticStatementKindV1, SemanticTerminatorKindV1), ProductionRankedProjectionErrorV1> {
    if matches!(allocation, source_ranked_consumer_resources_v18::ProjectionAllocationV18::Legacy) {
        return Ok((statement.clone(), terminator.clone()));
    }
    allocation.header::<Result<(SemanticStatementKindV1, SemanticTerminatorKindV1), ProductionRankedProjectionErrorV1>>()?;
    check_induction_body_source_shape_v18(statement, terminator, allocation)?;
    let SemanticStatementKindV1::Assign(assignment) = statement else {
        return Err(ProductionRankedProjectionErrorV1::Incomplete("an induction predicate source assignment changed kind"));
    };
    let SemanticRvalueKindV1::Binary { operation: SemanticBinaryOpV1::LessThan, left, right } = assignment.value().kind() else {
        return Err(ProductionRankedProjectionErrorV1::Incomplete("an induction predicate source arithmetic changed kind"));
    };
    let destination = assignment.destination();
    let destination = SemanticPlaceV1::new(destination.local(), allocation.empty()?, destination.ty())
        .map_err(|_| ProductionRankedProjectionErrorV1::Incomplete("an induction predicate destination changed shape"))?;
    let copied_assignment = fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1::new(destination,
        SemanticRvalueV1::new(assignment.value().result_type(), SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::LessThan,
            left: copy_induction_body_operand_v18(left, allocation)?,
            right: copy_induction_body_operand_v18(right, allocation)?,
        }));
    let SemanticTerminatorKindV1::SwitchInt { discriminant, targets } = terminator else {
        return Err(ProductionRankedProjectionErrorV1::Incomplete("an induction predicate source switch changed kind"));
    };
    let discriminant = copy_induction_body_operand_v18(discriminant, allocation)?;
    let values = allocation.copy_slice(targets.values())?;
    if values.capacity() != values.len() || values.len() != 1 {
        return Err(source_ranked_consumer_resources_v18::resource(
            fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Accounting));
    }
    let targets = SemanticSwitchTargetsV1::new(values, targets.otherwise())
        .map_err(|_| ProductionRankedProjectionErrorV1::Incomplete("an induction predicate exact switch targets changed shape"))?;
    Ok((SemanticStatementKindV1::Assign(copied_assignment), SemanticTerminatorKindV1::SwitchInt { discriminant, targets }))
}

fn check_induction_body_source_shape_v18(
    statement: &SemanticStatementKindV1,
    terminator: &SemanticTerminatorKindV1,
    allocation: &mut source_ranked_consumer_resources_v18::ProjectionAllocationV18<'_>,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    allocation.charge(1)?;
    let SemanticStatementKindV1::Assign(assignment) = statement else {
        return Err(ProductionRankedProjectionErrorV1::Incomplete("an induction body predicate lost its source assignment"));
    };
    let SemanticRvalueKindV1::Binary { operation: SemanticBinaryOpV1::LessThan, left, right } = assignment.value().kind() else {
        return Err(ProductionRankedProjectionErrorV1::Incomplete("an induction body predicate lost its exact LessThan source"));
    };
    if !assignment.destination().projections().is_empty() {
        return Err(ProductionRankedProjectionErrorV1::Incomplete("an induction body predicate changed its scalar destination"));
    }
    check_induction_body_operand_shape_v18(left, allocation)?;
    check_induction_body_operand_shape_v18(right, allocation)?;
    let SemanticTerminatorKindV1::SwitchInt { discriminant, targets } = terminator else {
        return Err(ProductionRankedProjectionErrorV1::Incomplete("an induction body predicate lost its exact SwitchInt source"));
    };
    if targets.values().len() != 1 {
        return Err(ProductionRankedProjectionErrorV1::Incomplete("an induction body predicate changed its switch cardinality"));
    }
    check_induction_body_operand_shape_v18(discriminant, allocation)
}
