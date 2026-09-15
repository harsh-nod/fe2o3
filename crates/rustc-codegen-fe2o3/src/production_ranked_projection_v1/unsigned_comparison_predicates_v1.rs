use super::*;

#[cfg(test)]
mod tests;

struct SwitchUsesV1 {
    heads: Vec<usize>,
    entries: Vec<(usize, usize)>,
}

impl SwitchUsesV1 {
    fn new(locals: usize, blocks: usize) -> Result<Self, ProductionRankedProjectionErrorV1> {
        let within_limit = |heads: usize, entries: usize| {
            entries
                .checked_mul(2)
                .and_then(|entries| heads.checked_add(entries))
                .is_some_and(|cells| cells <= MAX_PROJECTED_CAPABILITY_STATE_ENTRIES_V1)
        };
        if !within_limit(locals, blocks) {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "unsigned comparison switch-use storage exceeds the state limit",
            ));
        }
        let mut heads = Vec::new();
        let mut entries = Vec::new();
        heads
            .try_reserve_exact(locals)
            .and_then(|_| entries.try_reserve_exact(blocks))
            .map_err(|_| {
                ProductionRankedProjectionErrorV1::Unsupported(
                    "unsigned comparison switch-use storage cannot be reserved",
                )
            })?;
        if !within_limit(heads.capacity(), entries.capacity()) {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "unsigned comparison switch-use storage exceeds the state limit",
            ));
        }
        heads.resize(locals, usize::MAX);
        Ok(Self { heads, entries })
    }
}

impl TotalUnsignedIndexProjectorV1<'_, '_, '_> {
    pub(super) fn retain_unsigned_comparison_predicates_v1(
        &mut self,
        predicates: &mut [Option<GuardPredicateV1>],
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        if predicates.len() != self.function.locals().len()
            || !matches!(self.roots, TotalUnsignedIndexRootsV1::Invocation { .. })
        {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "unsigned comparison predicates lack their exact invocation owner",
            ));
        }
        self.assertion_proofs.charge(predicates.len())?;
        let mut uses = SwitchUsesV1::new(predicates.len(), self.function.blocks().len())?;
        // Collect every switch use first. A predicate is local-indexed, so one
        // non-dominated or ill-typed use must prevent publication for all uses.
        for (block_index, block) in self.function.blocks().iter().enumerate() {
            self.assertion_proofs.charge(1)?;
            let SemanticTerminatorKindV1::SwitchInt { discriminant, .. } =
                block.terminator().kind()
            else {
                continue;
            };
            let Some(local) = simple_operand_local(discriminant) else {
                continue;
            };
            let local = local.index() as usize;
            let Some(slot) = uses.heads.get_mut(local) else {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "unsigned comparison switch local is outside its owner",
                ));
            };
            if predicates[local].is_none() {
                let index = uses.entries.len();
                uses.entries.push((block_index, *slot));
                *slot = index;
            }
        }
        for (local, &head) in uses.heads.iter().enumerate() {
            self.assertion_proofs.charge(1)?;
            if head == usize::MAX
                || self.local_definitions[local] != 1
                || self.address_escaped()[local]
            {
                continue;
            }
            let Some(site) = self.definitions()[local] else {
                continue;
            };
            let SemanticStatementKindV1::Assign(assignment) =
                self.function.blocks()[site.block].statements()[site.statement].kind()
            else {
                continue;
            };
            let ty = self.function.locals()[local].ty();
            if !matches!(
                self.types
                    .get(ty.index() as usize)
                    .map(SemanticTypeDeclV1::shape),
                Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool))
            ) || assignment.destination().local().index() as usize != local
                || !assignment.destination().projections().is_empty()
                || assignment.destination().ty() != ty
                || assignment.value().result_type() != ty
            {
                continue;
            }
            let SemanticRvalueKindV1::Binary {
                operation,
                left,
                right,
            } = assignment.value().kind()
            else {
                continue;
            };
            if !matches!(
                operation,
                SemanticBinaryOpV1::LessThan
                    | SemanticBinaryOpV1::LessOrEqual
                    | SemanticBinaryOpV1::GreaterThan
                    | SemanticBinaryOpV1::GreaterOrEqual
            ) || left.ty() != right.ty()
            {
                continue;
            }
            let Some(maximum) = self.unsigned_maximum(left.ty()) else {
                continue;
            };
            if !self.comparison_operand_has_exact_type_v1(left)
                || !self.comparison_operand_has_exact_type_v1(right)
            {
                continue;
            }
            let mut exact_uses = true;
            let mut next = head;
            while next != usize::MAX {
                self.assertion_proofs.charge(1)?;
                let (use_block, previous) = uses.entries[next];
                next = previous;
                let block = &self.function.blocks()[use_block];
                let SemanticTerminatorKindV1::SwitchInt { discriminant, .. } =
                    block.terminator().kind()
                else {
                    unreachable!("collected switch changed in immutable function")
                };
                if discriminant.ty() != ty
                    || !self.assertion_proofs.assignment_dominates_use(
                        site,
                        use_block,
                        block.statements().len(),
                    )?
                {
                    exact_uses = false;
                    break;
                }
            }
            if !exact_uses {
                continue;
            }
            let operation = *operation;
            let left = left.clone();
            let right = right.clone();
            self.assertion_proofs.charge(1)?;
            let Some(left) = self.resolve_operand(&left, site.block, site.statement)? else {
                continue;
            };
            let Some(right) = self.resolve_operand(&right, site.block, site.statement)? else {
                continue;
            };
            if !left.invocation_dependent && !right.invocation_dependent {
                continue;
            }
            if let Some((lhs, rhs)) =
                self.unsigned_comparison_pair_v1(operation, left, right, maximum)?
            {
                retain_identical_direct_switch_predicate_v1(
                    &mut predicates[local],
                    GuardPredicateV1 {
                        comparisons: vec![(lhs, rhs)],
                    },
                )?;
            }
        }
        Ok(())
    }

    pub(super) fn comparison_operand_has_exact_type_v1(&self, operand: &SemanticOperandV1) -> bool {
        match operand {
            SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
                place.projections().is_empty()
                    && self
                        .function
                        .locals()
                        .get(place.local().index() as usize)
                        .is_some_and(|local| local.ty() == place.ty())
            }
            SemanticOperandV1::Constant(constant) => {
                let SemanticConstantValueV1::Scalar(value) = constant.value() else {
                    return false;
                };
                self.unsigned_bits(constant.ty()).is_some_and(|bits| {
                    u16::from(value.size_bytes()) == bits.div_ceil(8)
                        && self
                            .unsigned_maximum(constant.ty())
                            .is_some_and(|maximum| value.bits() <= u128::from(maximum))
                })
            }
        }
    }

    fn unsigned_comparison_pair_v1(
        &mut self,
        operation: SemanticBinaryOpV1,
        left: TotalUnsignedIndexValueV1,
        right: TotalUnsignedIndexValueV1,
        maximum: u64,
    ) -> Result<
        Option<(ProductionRankedValueV1, ProductionRankedValueV1)>,
        ProductionRankedProjectionErrorV1,
    > {
        // Preserve Boolean polarity. Non-strict comparisons are normalized
        // only with an exact adjacent constant; no runtime arithmetic is added.
        let pair = match operation {
            SemanticBinaryOpV1::LessThan => (left.ranked, right.ranked),
            SemanticBinaryOpV1::GreaterThan => (right.ranked, left.ranked),
            SemanticBinaryOpV1::GreaterOrEqual => {
                if let Some(bound) = right.exact.and_then(|value| value.checked_sub(1)) {
                    (self.constant(bound, maximum)?.ranked, left.ranked)
                } else if let Some(bound) = left
                    .exact
                    .and_then(|value| value.checked_add(1))
                    .filter(|value| *value <= maximum)
                {
                    (right.ranked, self.constant(bound, maximum)?.ranked)
                } else {
                    return Ok(None);
                }
            }
            SemanticBinaryOpV1::LessOrEqual => {
                if let Some(bound) = left.exact.and_then(|value| value.checked_sub(1)) {
                    (self.constant(bound, maximum)?.ranked, right.ranked)
                } else if let Some(bound) = right
                    .exact
                    .and_then(|value| value.checked_add(1))
                    .filter(|value| *value <= maximum)
                {
                    (left.ranked, self.constant(bound, maximum)?.ranked)
                } else {
                    return Ok(None);
                }
            }
            _ => return Ok(None),
        };
        Ok(Some(pair))
    }
}
