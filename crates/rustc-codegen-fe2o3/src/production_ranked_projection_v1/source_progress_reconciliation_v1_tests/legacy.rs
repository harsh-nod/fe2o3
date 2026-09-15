// Frozen pre-conversion functions for differential tests only.
fn legacy_source_induction_update_v1<'a>(
    function: &'a SemanticFunctionDeclV1,
    graph: &ProjectedLoopCfgV1,
    assertion_proofs: &mut SemanticAssertProofsV1<'_>,
    topology: &ProjectedNaturalLoopTopologyV1,
    induction: SemanticLocalIdV1,
    latch_statement: usize,
    local_definitions: &[u8],
    assignment_sites: &[Option<ScalarAssignmentSiteV1>],
    alias_work: &mut usize,
) -> Result<
    Option<(ProjectedSourceInductionUpdateV1, &'a SemanticOperandV1)>,
    ProductionRankedProjectionErrorV1,
> {
    let Some(latch) = function
        .blocks()
        .get(topology.latch)
        .and_then(|block| block.statements().get(latch_statement))
    else {
        return Ok(None);
    };
    let SemanticStatementKindV1::Assign(latch) = latch.kind() else {
        return Ok(None);
    };
    if !latch.destination().projections().is_empty()
        || latch.destination().local() != induction
        || latch.value().result_type() != latch.destination().ty()
    {
        return Ok(None);
    }

    let (update, producer_block, producer_statement, left, right) = match latch.value().kind() {
        SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::Add,
            left,
            right,
        } => (
            ProjectedSourceInductionUpdateV1::Ordinary,
            topology.latch,
            latch_statement,
            left,
            right,
        ),
        SemanticRvalueKindV1::UncheckedBinary(unchecked)
            if unchecked.operation() == SemanticUncheckedBinaryOpV1::Add =>
        {
            (
                ProjectedSourceInductionUpdateV1::Unchecked,
                topology.latch,
                latch_statement,
                unchecked.left(),
                unchecked.right(),
            )
        }
        SemanticRvalueKindV1::Use(result) => {
            let result_local = tuple_field_operand_local_v1(result, 0).ok_or(
                ProductionRankedProjectionErrorV1::Incomplete(
                    "a checked induction latch does not copy field zero of one checked result",
                ),
            )?;
            if result.ty() != latch.destination().ty()
                || local_definitions
                    .get(result_local.index() as usize)
                    .copied()
                    != Some(1)
            {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "a checked induction result has the wrong value type or definition count",
                ));
            }
            let Some(authenticated) = assertion_proofs.authenticated_checked_binary_value_v1(
                result,
                topology.latch,
                latch_statement,
            )?
            else {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "a checked induction overflow assertion does not authenticate its exact Add result and success edge",
                ));
            };
            let producer_block = authenticated.definition.block;
            let producer_statement = authenticated.definition.statement;
            let SemanticStatementKindV1::Assign(definition) =
                function.blocks()[producer_block].statements()[producer_statement].kind()
            else {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "a checked induction result has no assignment definition",
                ));
            };
            let SemanticRvalueKindV1::CheckedBinary(checked) = definition.value().kind() else {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "a checked induction result is not defined by checked arithmetic",
                ));
            };
            if checked.operation() != SemanticCheckedBinaryOpV1::Add {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "a checked induction result is not defined by checked Add",
                ));
            }
            if producer_statement + 1 != function.blocks()[producer_block].statements().len()
                || topology.loop_blocks.binary_search(&producer_block).is_err()
            {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "a checked induction producer is not the final statement inside its loop",
                ));
            }
            if graph.predecessors.get(topology.latch).map(Vec::as_slice) != Some(&[producer_block])
            {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "a checked induction latch does not have one exact producer predecessor",
                ));
            }
            let SemanticTerminatorKindV1::Assert { target, .. } =
                function.blocks()[producer_block].terminator().kind()
            else {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "a checked induction producer does not terminate with an assertion",
                ));
            };
            if authenticated.assertion_block != producer_block
                || target.target().index() as usize != topology.latch
            {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "a checked induction overflow assertion does not authenticate its exact Add result and success edge",
                ));
            }
            (
                ProjectedSourceInductionUpdateV1::Checked {
                    producer_block,
                    producer_statement,
                    result_local,
                },
                producer_block,
                producer_statement,
                checked.left(),
                checked.right(),
            )
        }
        _ => return Ok(None),
    };

    if left.ty() != latch.destination().ty() || right.ty() != latch.destination().ty() {
        return Ok(None);
    }
    let use_site = ScalarAssignmentSiteV1 {
        block: producer_block,
        statement: producer_statement,
    };
    let left_origin = resolve_block_copy_alias_before_v1(
        function,
        use_site,
        left,
        local_definitions,
        assignment_sites,
        &assertion_proofs.address_escaped,
        alias_work,
    )?;
    let right_origin = resolve_block_copy_alias_before_v1(
        function,
        use_site,
        right,
        local_definitions,
        assignment_sites,
        &assertion_proofs.address_escaped,
        alias_work,
    )?;
    let step = if left_origin == Some(induction) {
        right
    } else if right_origin == Some(induction) {
        left
    } else {
        return Ok(None);
    };
    Ok(Some((update, step)))
}

#[allow(clippy::too_many_arguments)]
fn legacy_reconcile_source_progress_and_emit_unsigned_casts_v1(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    constants: &[Option<u64>],
    stable_argument_origins: &[Option<u32>],
    local_definitions: &[u8],
    arguments: &[Option<u32>],
    inductions: &mut [ProjectedUniformInductionV1],
    operations: &mut Vec<ProductionRankedOperationV1>,
    next_value: &mut u32,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    let mut reconciled = BTreeMap::new();
    let graph = projected_loop_cfg_graph_v1(function)?;
    let mut semantic_ranges = SemanticAssertProofsV1::new(types, function)?;
    let assignment_sites = semantic_ranges.assignments.clone();
    let address_escaped = semantic_ranges.address_escaped.clone();
    let mut alias_work = 0_usize;
    for induction in inductions.iter() {
        let source = &induction.source_progress;
        let induction_type = function
            .locals()
            .get(source.induction.index() as usize)
            .map(|local| local.ty())
            .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                "a compiler-derived source induction has a stale local",
            ))?;
        let comparison = function
            .blocks()
            .get(induction.header)
            .and_then(|block| block.statements().get(source.header_statement))
            .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                "a compiler-derived source induction has a stale comparison site",
            ))?;
        let SemanticStatementKindV1::Assign(comparison) = comparison.kind() else {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a compiler-derived source induction comparison is no longer an assignment",
            ));
        };
        let SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::LessThan,
            left,
            right: bound_operand,
        } = comparison.value().kind()
        else {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a compiler-derived source induction comparison is no longer an exact less-than",
            ));
        };
        let compared_induction = resolve_block_copy_alias_before_v1(
            function,
            ScalarAssignmentSiteV1 {
                block: induction.header,
                statement: source.header_statement,
            },
            left,
            local_definitions,
            &assignment_sites,
            &address_escaped,
            &mut alias_work,
        )?;
        if induction_type != source.induction_type
            || left.ty() != source.induction_type
            || compared_induction != Some(source.induction)
            || bound_operand != &source.bound_operand
            || induction.bound != source.ranked_bound
            || induction.step != source.ranked_step
        {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a compiler-derived source induction comparison changed type or value",
            ));
        }
        let bound_range = semantic_ranges
            .range_at_operand(bound_operand, induction.header, source.header_statement)?
            .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                "a compiler-derived source induction has no finite unsigned bound",
            ))?;
        let latch = function
            .blocks()
            .get(induction.latch)
            .and_then(|block| block.statements().get(source.latch_statement))
            .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                "a compiler-derived source induction has a stale latch site",
            ))?;
        let SemanticStatementKindV1::Assign(latch) = latch.kind() else {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a compiler-derived source induction latch is no longer an assignment",
            ));
        };
        let topology = ProjectedNaturalLoopTopologyV1 {
            preheader: induction.preheader,
            preheader_control: induction.preheader_control.clone(),
            latch: induction.latch,
            loop_blocks: induction.loop_blocks.clone(),
        };
        let (replayed_update, replayed_step) = legacy_source_induction_update_v1(
            function,
            &graph,
            &mut semantic_ranges,
            &topology,
            source.induction,
            source.latch_statement,
            local_definitions,
            &assignment_sites,
            &mut alias_work,
        )?
        .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
            "a compiler-derived source induction latch is no longer an authenticated Add recurrence",
        ))?;
        if !latch.destination().projections().is_empty()
            || latch.destination().local() != source.induction
            || latch.destination().ty() != source.induction_type
            || latch.value().result_type() != source.induction_type
            || replayed_update != source.update
            || replayed_step != &source.step_operand
            || positive_unsigned_constant_operand_v1(replayed_step, constants, types)
                != Some(source.step_value)
        {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a compiler-derived source induction latch changed type, step, or overflow semantics",
            ));
        }
        let induction_maximum = semantic_ranges
            .scalar_unsigned_maximum(source.induction_type)
            .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                "a compiler-derived source induction does not use a supported unsigned scalar type",
            ))?;
        if bound_range.maximum != 0
            && (bound_range.maximum - 1)
                .checked_add(u128::from(source.step_value))
                .is_none_or(|updated| updated > induction_maximum)
        {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a compiler-derived source induction update may overflow its exact unsigned type",
            ));
        }
        let Some(range) = &induction.bound_cast else {
            continue;
        };
        if range.header != induction.header || range.comparison_statement != source.header_statement
        {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a compiler-derived unsigned cast has a stale semantic statement site",
            ));
        }
        let statement = function
            .blocks()
            .get(range.header)
            .and_then(|block| block.statements().get(range.comparison_statement))
            .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                "a compiler-derived unsigned cast has a stale semantic statement site",
            ))?;
        let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a compiler-derived unsigned cast no longer names an assignment",
            ));
        };
        let SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::LessThan,
            right: source_operand,
            ..
        } = assignment.value().kind()
        else {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a compiler-derived unsigned cast no longer names a loop comparison",
            ));
        };
        let reconciled_width = unsigned_bit_width_for_maximum_v1(bound_range.maximum);
        if source_operand != &range.source_operand
            || source_operand.ty() != range.source_type
            || reconciled_width != Some(range.bit_width)
            || range.ranked_value != induction.bound
        {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a compiler-derived unsigned cast does not match its exact semantic range and ranked source",
            ));
        }
        if constant_operand_value(source_operand, constants).is_some() {
            continue;
        }
        match range.ranked_value {
            ProductionRankedValueV1::Argument(ranked_argument) => {
                let local = simple_operand_local(source_operand).ok_or(
                    ProductionRankedProjectionErrorV1::Incomplete(
                        "a compiler-derived argument cast has no exact scalar local",
                    ),
                )?;
                let local_index = local.index() as usize;
                let origin = stable_argument_origins
                    .get(local_index)
                    .copied()
                    .flatten()
                    .or_else(|| {
                        (local_definitions.get(local_index).copied() == Some(0)
                            && function.locals().get(local_index).is_some_and(|local| {
                                matches!(local.role(), SemanticLocalRoleV1::Argument(_))
                            }))
                        .then_some(local.index())
                    })
                    .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                        "a compiler-derived unsigned cast is not backed by one stable semantic argument",
                    ))? as usize;
                let argument = arguments.get(origin).copied().flatten().ok_or(
                    ProductionRankedProjectionErrorV1::Incomplete(
                        "a compiler-derived unsigned cast has no final ranked argument mapping",
                    ),
                )?;
                if ranked_argument != argument {
                    return Err(ProductionRankedProjectionErrorV1::Incomplete(
                        "a compiler-derived unsigned cast was substituted onto a different ranked source",
                    ));
                }
            }
            // A local source was emitted by the authenticated pure-uniform
            // projector and is already tied above to both the exact semantic
            // comparison operand and the retained source-progress value.
            ProductionRankedValueV1::Local(_) => {}
            ProductionRankedValueV1::BlockArgument { .. } => {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "a compiler-derived unsigned cast cannot use a loop-carried ranked source",
                ));
            }
        }
        match reconciled.insert(range.ranked_value, range.bit_width) {
            Some(previous) if previous != range.bit_width => {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "one ranked source has incompatible compiler-derived unsigned cast widths",
                ));
            }
            _ => {}
        }
    }
    let mut converted = BTreeMap::new();
    for (source, bit_width) in reconciled {
        reserve_operation(operations)?;
        let result = next_value_id(next_value)?;
        let value = ProductionRankedValueV1::Local(result);
        operations.push(ProductionRankedOperationV1::IndexUnsignedCast {
            result,
            source,
            bit_width,
        });
        converted.insert(source, value);
    }
    for induction in inductions {
        let Some(range) = &induction.bound_cast else {
            continue;
        };
        if constant_operand_value(&range.source_operand, constants).is_some() {
            continue;
        }
        induction.bound = converted.get(&range.ranked_value).copied().ok_or(
            ProductionRankedProjectionErrorV1::Incomplete(
                "a reconciled unsigned cast result is missing from the ranked recipe",
            ),
        )?;
    }
    Ok(())
}
