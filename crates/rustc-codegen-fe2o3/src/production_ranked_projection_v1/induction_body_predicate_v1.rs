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
    let Some(bits) = unsigned_index_bits_v1(types, operand.ty()) else {
        return Ok(None);
    };
    if let SemanticOperandV1::Constant(_) = operand {
        return Ok(constant_operand_value(operand, constants)
            .filter(|value| bits == 64 || *value < (1_u64 << bits))
            .map(|_| InductionPredicateSourceOperandV1::Uniform(operand.clone())));
    }
    let local = match resolve_block_copy_alias_before_v1(
        function,
        use_site,
        operand,
        local_definitions,
        &proofs.assignments,
        &proofs.address_escaped,
        work,
    ) {
        Ok(local) => local,
        // Precision is optional; unresolved aliases retain their original split.
        Err(ProductionRankedProjectionErrorV1::Incomplete(_)) => return Ok(None),
        Err(error) => return Err(error),
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
        project_loop_graph_charge_v1(work, 1)?;
        if local == induction.source_progress.induction {
            return Ok((induction.source_progress.induction_type == operand.ty()
                && induction.contains_block(use_site.block)
                && use_site.block != induction.header
                && use_site.block != induction.preheader
                && use_site.block != induction.latch
                && local_definitions.get(index).copied() == Some(2))
            .then_some(InductionPredicateSourceOperandV1::Induction(ordinal)));
        }
    }
    let admitted = match local_definitions.get(index).copied() {
        Some(0) => matches!(declaration.role(), SemanticLocalRoleV1::Argument(_)),
        Some(1) if constants.get(index).copied().flatten().is_some() => {
            let Some(definition) = proofs.assignments.get(index).copied().flatten() else {
                return Ok(None);
            };
            proofs.assignment_dominates_use(definition, use_site.block, use_site.statement)?
        }
        _ => false,
    };
    if !admitted {
        return Ok(None);
    }
    let operand = SemanticOperandV1::Copy(
        SemanticPlaceV1::new(local, vec![], operand.ty()).map_err(|_| {
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
    if inductions.is_empty() {
        return Ok(());
    }
    if function.blocks().len() > MAX_RANKED_BOUNDS_BLOCKS
        || inductions
            .iter()
            .any(|induction| !induction.body_predicates.is_empty())
    {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "induction body predicate preparation has excessive blocks or stale predicates",
        ));
    }
    let mut proofs = SemanticAssertProofsV1::new(types, function)?;
    let mut work = 0;
    for (block_index, block) in function.blocks().iter().enumerate() {
        project_loop_graph_charge_v1(&mut work, 1)?;
        let mut in_body = false;
        for induction in inductions.iter() {
            project_loop_graph_charge_v1(&mut work, 1)?;
            in_body |= induction.contains_block(block_index)
                && ![induction.preheader, induction.header, induction.latch].contains(&block_index);
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
            || proofs.address_escaped.get(condition_index).copied() != Some(false)
            || !matches!(
                types
                    .get(discriminant.ty().index() as usize)
                    .map(|ty| ty.shape()),
                Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool))
            )
        {
            continue;
        }
        let Some(definition) = proofs.assignments.get(condition_index).copied().flatten() else {
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
        let Some(lhs) = induction_predicate_source_operand_v1(
            types,
            function,
            left,
            definition,
            constants,
            local_definitions,
            &mut proofs,
            inductions,
            &mut work,
        )?
        else {
            continue;
        };
        let Some(rhs) = induction_predicate_source_operand_v1(
            types,
            function,
            right,
            definition,
            constants,
            local_definitions,
            &mut proofs,
            inductions,
            &mut work,
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
                project_uniform_switch_operand_v1(
                    &operand,
                    constants,
                    stable_argument_origins,
                    arguments,
                    next_argument,
                    operations,
                    next_value,
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
        predicates.try_reserve(1).map_err(|_| {
            ProductionRankedProjectionErrorV1::Unsupported(
                "induction body predicate storage cannot be reserved",
            )
        })?;
        predicates.push(ProjectedInductionBodyPredicateV1 {
            block: block_index,
            source_statement: definition.statement,
            source_assignment: block.statements()[definition.statement].kind().clone(),
            source_terminator: block.terminator().kind().clone(),
            lhs,
            rhs,
            true_block,
            false_block,
        });
    }
    Ok(())
}

fn indexed_induction_body_predicates_v1<'a>(
    function: &SemanticFunctionDeclV1,
    inductions: &'a [ProjectedUniformInductionV1],
) -> Result<Vec<Option<&'a ProjectedInductionBodyPredicateV1>>, ProductionRankedProjectionErrorV1> {
    if function.blocks().len() > MAX_RANKED_BOUNDS_BLOCKS {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "induction body predicate index exceeds the ranked block limit",
        ));
    }
    let mut indexed = vec![None; function.blocks().len()];
    let mut work = 0;
    for (owner, induction) in inductions.iter().enumerate() {
        project_loop_graph_charge_v1(&mut work, 1)?;
        for predicate in &induction.body_predicates {
            project_loop_graph_charge_v1(&mut work, 1)?;
            if !induction.contains_block(predicate.block)
                || [induction.preheader, induction.header, induction.latch].contains(&predicate.block)
                || ![&predicate.lhs, &predicate.rhs].iter().any(|operand| {
                    matches!(operand, ProjectedInductionPredicateOperandV1::Induction { ordinal, .. } if *ordinal == owner)
                })
            {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "an induction body predicate has a stale live owner",
                ));
            }
            for operand in [&predicate.lhs, &predicate.rhs] {
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
                if source.source_progress.induction != *source_local
                    || source.source_progress.induction_type != *source_type
                    || !source.contains_block(predicate.block)
                    || [source.preheader, source.header, source.latch].contains(&predicate.block)
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
    let semantic_block = function.blocks().get(predicate.block).ok_or(
        ProductionRankedProjectionErrorV1::Unsupported(
            "an induction body predicate source is outside the semantic CFG",
        ),
    )?;
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
    let operand = |operand: &ProjectedInductionPredicateOperandV1| match operand {
        ProjectedInductionPredicateOperandV1::Uniform(value) => Ok(*value),
        ProjectedInductionPredicateOperandV1::Induction { ordinal, .. } => {
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
    };
    let arguments_for = |target: usize| {
        let target_live =
            live_inductions
                .get(target)
                .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                    "an induction body predicate target is outside the semantic CFG",
                ))?;
        forward_live_inductions(block, live, target_live)
    };
    Ok(ProductionRankedTerminatorV1::IndexLessThanArgs {
        lhs: operand(&predicate.lhs)?,
        rhs: operand(&predicate.rhs)?,
        true_arguments: arguments_for(predicate.true_block)?,
        false_arguments: arguments_for(predicate.false_block)?,
        true_block: ranked_block_id(projected_target(base_blocks, predicate.true_block)?)?,
        false_block: ranked_block_id(projected_target(base_blocks, predicate.false_block)?)?,
    })
}
