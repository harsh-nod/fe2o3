//! Exact source comparisons, before dependency-only switch projection.
use super::*;

#[path = "source_unsigned_switch_v1/optional_precision_v1.rs"]
mod optional_precision_v1;
pub(super) use optional_precision_v1::OptionalSourceComparisonV1;

#[cfg(test)]
#[path = "source_unsigned_switch_v1/replay_tests.rs"]
mod replay_tests;

#[derive(Clone, Copy)]
pub(super) struct SourceComparisonV1 {
    pub(super) assignment: *const fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1,
    pub(super) left: Option<AllocationContractV1>,
    pub(super) right: Option<AllocationContractV1>,
}

impl ProjectedGlobalSemanticUsesV1 {
    pub(super) fn record_source_unsigned_comparison_v1(
        &mut self,
        assignment: &fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1,
        state: &ProjectedCapabilityStateV1,
        site: (usize, usize),
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        let SemanticRvalueKindV1::Binary {
            operation,
            left,
            right,
        } = assignment.value().kind()
        else {
            return Ok(());
        };
        let equality = matches!(
            operation,
            SemanticBinaryOpV1::Equal | SemanticBinaryOpV1::NotEqual
        );
        let checked_ordering = matches!(
            operation,
            SemanticBinaryOpV1::LessThan
                | SemanticBinaryOpV1::GreaterThan
                | SemanticBinaryOpV1::LessOrEqual
                | SemanticBinaryOpV1::GreaterOrEqual
        ) && (tuple_field_operand_local_v1(left, 0).is_some()
            || tuple_field_operand_local_v1(right, 0).is_some());
        if !equality && !checked_ordering {
            return Ok(());
        }
        charge_capability_dataflow_work_v1(&mut self.work, 2)?;
        let extent = |operand| match capability_known_origin_v1(state, operand) {
            Some(ProjectedCapabilityOriginV1::GlobalExtent(allocation)) => Some(allocation),
            _ => None,
        };
        let (left, right) = (extent(left), extent(right));
        if equality && left.is_none() && right.is_none() {
            return Ok(());
        }
        if self.source_unsigned_comparisons.len() >= MAX_PROJECTED_CAPABILITY_STATE_ENTRIES_V1 {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "source comparison extent inventory exceeds the state limit",
            ));
        }
        self.source_unsigned_comparisons.insert(
            site,
            SourceComparisonV1 {
                assignment,
                left,
                right,
            },
        );
        Ok(())
    }
}

impl ProjectedGlobalSemanticUsesV1 {
    pub(super) fn ensure_source_comparison_proofs_v1<'model>(
        &self,
        types: &'model [SemanticTypeDeclV1],
        function: &'model SemanticFunctionDeclV1,
        launch_upper_bound: Option<u64>,
        proofs: &mut Option<SemanticAssertProofsV1<'model>>,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        if launch_upper_bound.is_none() || self.source_unsigned_comparisons.is_empty() {
            return Ok(());
        }
        if let Some(proof) = proofs.as_ref() {
            if !std::ptr::eq(types, proof.types) || !std::ptr::eq(function, proof.function) {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "source comparison proof owner differs from its exact execution function",
                ));
            }
        } else {
            // Comparisons also occur with disjoint stores or read-only roots.
            // Initialize once; preserve all earlier address-proof work and facts.
            *proofs = Some(SemanticAssertProofsV1::new(types, function)?);
        }
        Ok(())
    }
}

impl TotalUnsignedIndexProjectorV1<'_, '_, '_> {
    pub(super) fn source_unsigned_switches_v1(
        &mut self,
        uses: &ProjectedGlobalSemanticUsesV1,
        allocations: &[Option<AllocationContractV1>],
        extent_arguments: &mut [Option<u32>],
    ) -> Result<Vec<Option<ProjectedDeterministicSwitchV1>>, ProductionRankedProjectionErrorV1>
    {
        if !matches!(self.roots, TotalUnsignedIndexRootsV1::Invocation { .. })
            || allocations.len() != self.function.locals().len()
            || extent_arguments.len() != self.function.locals().len()
        {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "source comparison tables lack their exact invocation owner",
            ));
        }
        if uses.source_unsigned_comparisons.is_empty() {
            return Ok(Vec::new());
        }
        self.assertion_proofs.charge(self.function.blocks().len())?;
        let mut result = vec![None; self.function.blocks().len()];
        // Results are block-indexed, never a callee/global Boolean summary.
        // Every individual switch use authenticates definition and operand custody.
        for (block_index, block) in self.function.blocks().iter().enumerate() {
            if self.optional_source_exhausted_v1() {
                break;
            }
            self.assertion_proofs.charge(1)?;
            let SemanticTerminatorKindV1::SwitchInt {
                discriminant,
                targets,
            } = block.terminator().kind()
            else {
                continue;
            };
            let Some(local) = simple_operand_local(discriminant).map(|l| l.index() as usize) else {
                continue;
            };
            let Some(site) = self.definitions().get(local).copied().flatten() else {
                continue;
            };
            let Some(extents) = uses
                .source_unsigned_comparisons
                .get(&(site.block, site.statement))
            else {
                continue;
            };
            let SemanticStatementKindV1::Assign(assignment) =
                self.function.blocks()[site.block].statements()[site.statement].kind()
            else {
                continue;
            };
            if !std::ptr::eq(extents.assignment, assignment) {
                continue;
            }
            if self.local_definitions.get(local).copied() != Some(1)
                || self.address_escaped().get(local).copied() != Some(false)
                || self.function.locals()[local].ty() != discriminant.ty()
                || !matches!(
                    self.types
                        .get(discriminant.ty().index() as usize)
                        .map(SemanticTypeDeclV1::shape),
                    Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool))
                )
                || !self.assertion_proofs.assignment_dominates_use(
                    site,
                    block_index,
                    block.statements().len(),
                )?
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
            if assignment.destination().local().index() as usize != local
                || !assignment.destination().projections().is_empty()
                || assignment.destination().ty() != discriminant.ty()
                || assignment.value().result_type() != discriminant.ty()
                || left.ty() != right.ty()
                || !self.source_comparison_operand_type_v1(left)
                || !self.source_comparison_operand_type_v1(right)
            {
                continue;
            }
            let Some(maximum) = self.unsigned_maximum(left.ty()) else {
                continue;
            };
            let equality = matches!(
                operation,
                SemanticBinaryOpV1::Equal | SemanticBinaryOpV1::NotEqual
            );
            let checked_pair = tuple_field_operand_local_v1(left, 0).is_some()
                || tuple_field_operand_local_v1(right, 0).is_some();
            if equality {
                if extents.left.is_some() == extents.right.is_some() {
                    continue;
                }
            } else if !checked_pair
                || !matches!(
                    operation,
                    SemanticBinaryOpV1::LessThan
                        | SemanticBinaryOpV1::GreaterThan
                        | SemanticBinaryOpV1::LessOrEqual
                        | SemanticBinaryOpV1::GreaterOrEqual
                )
            {
                continue;
            }
            // The closed Boolean profile never scans an arbitrary switch table.
            if !(1..=2).contains(&targets.values().len()) {
                continue;
            }
            self.assertion_proofs.charge(targets.values().len() + 1)?;
            let Some((yes, no)) = boolean_targets(self.function, targets) else {
                continue;
            };
            let operation = *operation;
            let left_operand = left.clone();
            let right_operand = right.clone();
            let extents = *extents;
            self.optional_source_site_v1(site, block_index, optional_precision_v1::OperandSideV1::Left);
            let Some(mut left) = self.source_comparison_value_v1(
                &left_operand,
                site,
                extents.left,
                allocations,
                extent_arguments,
            )?
            else {
                continue;
            };
            self.optional_source_site_v1(site, block_index, optional_precision_v1::OperandSideV1::Right);
            let Some(mut right) = self.source_comparison_value_v1(
                &right_operand,
                site,
                extents.right,
                allocations,
                extent_arguments,
            )?
            else {
                continue;
            };
            if equality {
                // Exactly one source-owned length and one total exact constant.
                // Do not equate arbitrary metadata roots or symbolic expressions.
                match (extents.left.is_some(), extents.right.is_some()) {
                    (true, false) if right.exact.is_some() => {}
                    (false, true) if left.exact.is_some() => std::mem::swap(&mut left, &mut right),
                    _ => continue,
                }
            } else if !left.invocation_dependent && !right.invocation_dependent {
                continue;
            }
            if let Some(value) = left.exact {
                left = self.constant(value, maximum)?;
            }
            if let Some(value) = right.exact {
                right = self.constant(value, maximum)?;
            }
            let (kind, lhs, rhs, yes, no, source_value) = match operation {
                SemanticBinaryOpV1::Equal => (
                    SemanticBinaryOpV1::Equal,
                    left.ranked,
                    right.ranked,
                    yes,
                    no,
                    1,
                ),
                SemanticBinaryOpV1::NotEqual => (
                    SemanticBinaryOpV1::Equal,
                    left.ranked,
                    right.ranked,
                    no,
                    yes,
                    0,
                ),
                SemanticBinaryOpV1::LessThan => (
                    SemanticBinaryOpV1::LessThan,
                    left.ranked,
                    right.ranked,
                    yes,
                    no,
                    1,
                ),
                SemanticBinaryOpV1::GreaterOrEqual => (
                    SemanticBinaryOpV1::LessThan,
                    left.ranked,
                    right.ranked,
                    no,
                    yes,
                    0,
                ),
                SemanticBinaryOpV1::GreaterThan => (
                    SemanticBinaryOpV1::LessThan,
                    right.ranked,
                    left.ranked,
                    yes,
                    no,
                    1,
                ),
                SemanticBinaryOpV1::LessOrEqual => (
                    SemanticBinaryOpV1::LessThan,
                    right.ranked,
                    left.ranked,
                    no,
                    yes,
                    0,
                ),
                _ => unreachable!(),
            };
            result[block_index] = Some(ProjectedDeterministicSwitchV1 {
                source_discriminant: discriminant.clone(),
                discriminant: lhs,
                targets: vec![(source_value, rhs, yes)],
                otherwise: no,
                lane_uniform: false,
                normalized_comparison: Some(kind),
            });
        }
        Ok(result)
    }

    fn source_comparison_operand_type_v1(&self, operand: &SemanticOperandV1) -> bool {
        if let Some(local) = tuple_field_operand_local_v1(operand, 0) {
            let Some(declaration) = self.function.locals().get(local.index() as usize) else {
                return false;
            };
            let Some(SemanticTypeShapeV1::Tuple(fields)) = self
                .types
                .get(declaration.ty().index() as usize)
                .map(SemanticTypeDeclV1::shape)
            else {
                return false;
            };
            let [value, overflow] = fields.fields() else {
                return false;
            };
            return *value == operand.ty()
                && self.unsigned_maximum(*value).is_some()
                && matches!(
                    self.types
                        .get(overflow.index() as usize)
                        .map(SemanticTypeDeclV1::shape),
                    Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool))
                );
        }
        self.comparison_operand_has_exact_type_v1(operand)
    }

    fn source_comparison_value_v1(
        &mut self,
        operand: &SemanticOperandV1,
        site: ScalarAssignmentSiteV1,
        extent: Option<AllocationContractV1>,
        allocations: &[Option<AllocationContractV1>],
        extent_arguments: &mut [Option<u32>],
    ) -> Result<Option<TotalUnsignedIndexValueV1>, ProductionRankedProjectionErrorV1> {
        let Some(extent) = extent else {
            return self.resolve_operand(operand, site.block, site.statement);
        };
        if self.unsigned_bits(operand.ty()) != Some(64) || extent.singleton_object {
            return Ok(None);
        }
        let Some(origin) = usize::try_from(extent.allocation_origin)
            .ok()
            .and_then(|n| n.checked_sub(1))
        else {
            return Ok(None);
        };
        let Ok(argument) = u32::try_from(origin) else {
            return Ok(None);
        };
        self.assertion_proofs.charge(self.function.locals().len())?;
        let exact_root = self
            .function
            .locals()
            .iter()
            .enumerate()
            .any(|(local, declaration)| {
                declaration.role() == SemanticLocalRoleV1::Argument(argument)
                    && allocations.get(local).copied().flatten() == Some(extent)
            });
        if !exact_root {
            return Ok(None);
        }
        self.journal_optional_extent_v1(origin, extent_arguments)?;
        let ranked =
            project_allocation_extent_argument_v1(origin, extent_arguments, self.next_argument)?;
        Ok(Some(TotalUnsignedIndexValueV1 {
            ranked,
            maximum: u64::MAX,
            exact: None,
            invocation_dependent: false,
        }))
    }
}

pub(super) fn boolean_targets(
    function: &SemanticFunctionDeclV1,
    targets: &SemanticSwitchTargetsV1,
) -> Option<(usize, usize)> {
    if !(1..=2).contains(&targets.values().len()) {
        return None;
    }
    let otherwise = targets.otherwise().target().index() as usize;
    if otherwise >= function.blocks().len()
        || targets.otherwise().role() != SemanticEdgeRoleV1::SwitchOtherwise
    {
        return None;
    }
    if targets.values().iter().any(|v| {
        v.edge().role() != SemanticEdgeRoleV1::SwitchValue
            || v.edge().target().index() as usize >= function.blocks().len()
    }) {
        return None;
    }
    match targets.values() {
        [value] => match value.value() {
            0 => Some((otherwise, value.edge().target().index() as usize)),
            1 => Some((value.edge().target().index() as usize, otherwise)),
            _ => None,
        },
        [first, second]
            if first.value() != second.value()
                && switch_fallback_is_empty_unreachable_v1(function, otherwise) =>
        {
            let zero = [first, second].into_iter().find(|v| v.value() == 0)?;
            let one = [first, second].into_iter().find(|v| v.value() == 1)?;
            Some((
                one.edge().target().index() as usize,
                zero.edge().target().index() as usize,
            ))
        }
        _ => None,
    }
}
