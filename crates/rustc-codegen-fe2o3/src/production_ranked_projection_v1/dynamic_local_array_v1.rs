fn reconcile_bounds_ordinary_index_v1(
    block_index: usize,
    index_local: SemanticLocalIdV1,
    ordinary_indices: &[Option<ProjectedOrdinaryIndexV1>],
    enum_payload_dominance: Option<&SemanticEnumPayloadDominanceV1>,
    local_values: &mut [Option<ProductionRankedValueV1>],
) -> Result<(), ProductionRankedProjectionErrorV1> {
    if let Some(ordinary) = ordinary_indices
        .get(index_local.index() as usize)
        .copied()
        .flatten()
    {
        let Some(enum_payload_dominance) = enum_payload_dominance else {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "ordinary enum index facts lack enum-dominance evidence",
            ));
        };
        if !enum_payload_dominance.allows(
            ordinary.availability,
            SemanticBlockIdV1::from_index(block_index as u32),
        ) {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "an ordinary enum index payload is used outside its authenticated variant edge",
            ));
        }
        let slot = local_values.get_mut(index_local.index() as usize).ok_or(
            ProductionRankedProjectionErrorV1::Unsupported(
                "an ordinary enum index payload is outside the semantic local table",
            ),
        )?;
        match *slot {
            None => *slot = Some(ordinary.value),
            Some(existing) if existing == ordinary.value => {}
            Some(_) => {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "an ordinary enum index payload conflicts with index capability authority",
                ));
            }
        }
    }
    Ok(())
}

fn immutable_local_array_candidates_v1(
    function: &SemanticFunctionDeclV1,
    inventory: &AssertionDefinitionInventoryV1,
) -> Vec<bool> {
    function
        .locals()
        .iter()
        .enumerate()
        .map(|(local, declaration)| {
            !declaration.role().is_entry_argument()
                && inventory.counts[local] == 1
                && inventory.assignments[local].is_some()
                && !inventory.address_escaped[local]
        })
        .collect()
}

fn authenticate_fixed_array_guard_v1(
    proof: &mut SemanticAssertProofsV1<'_>,
    guard: usize,
    success: usize,
    condition: &SemanticOperandV1,
    index: &SemanticOperandV1,
    bound: &SemanticOperandV1,
) -> Result<(SemanticLocalIdV1, u64), ProductionRankedProjectionErrorV1> {
    authenticate_fixed_array_guard_core_v18(proof, guard, success, condition, index, bound,
        &mut source_ranked_consumer_resources_v18::ProjectionAllocationV18::Legacy)
}

#[allow(clippy::too_many_arguments)]
fn authenticate_fixed_array_guard_core_v18(
    proof: &mut SemanticAssertProofsV1<'_>,
    guard: usize,
    success: usize,
    condition: &SemanticOperandV1,
    index: &SemanticOperandV1,
    bound: &SemanticOperandV1,
    allocation: &mut source_ranked_consumer_resources_v18::ProjectionAllocationV18<'_>,
) -> Result<(SemanticLocalIdV1, u64), ProductionRankedProjectionErrorV1> {
    let refuse = || {
        ProductionRankedProjectionErrorV1::Incomplete(
            "a fixed-array bounds check lacks stable exact unsigned index < literal extent evidence",
        )
    };
    allocation.charge(1)?;
    proof.charge(1)?;
    let index_local = simple_operand_local(index).ok_or_else(refuse)?;
    let index_slot = index_local.index() as usize;
    let condition_local = simple_operand_local(condition).ok_or_else(refuse)?;
    let condition_slot = condition_local.index() as usize;
    let bits = unsigned_index_bits_v1(proof.types, index.ty()).ok_or_else(refuse)?;
    let declaration = proof.function.locals().get(index_slot).ok_or_else(refuse)?;
    if declaration.ty() != index.ty()
        || bound.ty() != index.ty()
        || proof.address_escaped().get(index_slot) != Some(&false)
        || proof.address_escaped().get(condition_slot) != Some(&false)
        || proof.definition_counts().get(condition_slot) != Some(&1)
        || !matches!(
            proof
                .types
                .get(condition.ty().index() as usize)
                .map(SemanticTypeDeclV1::shape),
            Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool))
        )
    {
        return Err(refuse());
    }
    let SemanticOperandV1::Constant(constant) = bound else {
        return Err(refuse());
    };
    let SemanticConstantValueV1::Scalar(value) = constant.value() else {
        return Err(refuse());
    };
    if u16::from(value.size_bytes()) * 8 != bits
        || value.bits() == 0
        || (bits < 64 && value.bits() >= (1_u128 << bits))
    {
        return Err(refuse());
    }
    let extent = u64::try_from(value.bits()).map_err(|_| refuse())?;
    let comparison = proof
        .assignments()
        .get(condition_slot)
        .copied()
        .flatten()
        .ok_or_else(refuse)?;
    if comparison.block != guard || !match allocation {
        source_ranked_consumer_resources_v18::ProjectionAllocationV18::Legacy => proof.block_dominates(guard, success)?,
        source_ranked_consumer_resources_v18::ProjectionAllocationV18::Source(_) => proof.block_dominates_live_v18(guard, success, allocation)?,
    } {
        return Err(refuse());
    }
    let SemanticStatementKindV1::Assign(assignment) =
        proof.function.blocks()[guard].statements()[comparison.statement].kind()
    else {
        return Err(refuse());
    };
    if matches!(allocation, source_ranked_consumer_resources_v18::ProjectionAllocationV18::Source(_)) {
        allocation.charge(8)?;
        let SemanticRvalueKindV1::Binary { left, right, .. } = assignment.value().kind() else { return Err(refuse()); };
        if simple_operand_local(left) != Some(index_local)
            || !matches!(right, SemanticOperandV1::Constant(value)
                if matches!(value.value(), SemanticConstantValueV1::Scalar(_)))
        { return Err(refuse()); }
    }
    if !matches!(assignment.value().kind(), SemanticRvalueKindV1::Binary {
        operation: SemanticBinaryOpV1::LessThan, left, right,
    } if left == index && right == bound)
    {
        return Err(refuse());
    }
    if proof.definition_counts().get(index_slot) == Some(&0)
        && declaration.role().is_entry_argument()
    {
        return Ok((index_local, extent));
    }
    if proof.definition_counts().get(index_slot) != Some(&1) {
        return Err(refuse());
    }
    let definition = proof
        .assignments()
        .get(index_slot)
        .copied()
        .flatten()
        .ok_or_else(refuse)?;
    let mut dominates = |block, statement| match allocation {
        source_ranked_consumer_resources_v18::ProjectionAllocationV18::Legacy =>
            proof.assignment_dominates_use(definition, block, statement),
        source_ranked_consumer_resources_v18::ProjectionAllocationV18::Source(_) =>
            proof.assignment_dominates_use_live_v18(definition, block, statement, allocation),
    };
    if !dominates(guard, comparison.statement)? || !dominates(success, 0)?
    {
        return Err(refuse());
    }
    Ok((index_local, extent))
}
