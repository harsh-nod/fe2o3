use super::*;
use fe2o3_lower_mir_kernel::{
    ProductionProjectionArgumentCandidateV1, ProductionProjectionArgumentComponentV1,
    ProductionProjectionControlBlockV1, ProductionProjectionControlCandidateV1,
};

/// Inert annotations from the existing emitter, never a second executable CFG.
/// The enclosing canonical scope must keep this payload reserved through the
/// completing callback and drop it before restoring that scope's live floor.
pub(super) struct CanonicalMemoryControlRecorderV1 {
    candidate: ProductionProjectionControlCandidateV1,
}

/// Inert description for the recorder-only CFG path. The lowerer independently
/// authenticates every source, SSA, N and O occurrence before D can complete.
#[derive(Clone, Copy)]
pub(super) struct CanonicalIdentitySliceV1 {
    pub(super) producer: usize,
    pub(super) getter: usize,
    pub(super) switch: usize,
    pub(super) some: usize,
    pub(super) none: usize,
    pub(super) fallback: Option<usize>,
    pub(super) index: ProductionRankedValueV1,
    pub(super) extent: ProductionRankedValueV1,
    pub(super) view: ProductionRankedValueIdV1,
}

fn resource(
    error: fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1,
) -> ProductionRankedProjectionErrorV1 {
    ProductionRankedProjectionErrorV1::CanonicalAssertions(
        canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(error),
    )
}

fn push<T>(
    values: &mut Vec<T>,
    value: T,
    facts: &mut impl ProjectedAssertionFactsV1,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
    facts.charge_private_array_work(6)?;
    if values.len() == values.capacity() {
        let old = values.capacity();
        facts.charge_private_array_work(old)?;
        let requested = old.max(1);
        let bytes = requested
            .checked_mul(std::mem::size_of::<T>())
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        facts.reserve_checked_control_storage_v1(bytes)?;
        values
            .try_reserve_exact(requested)
            .map_err(|_| resource(Resource::Allocation))?;
        // Capacity reconciliation precedes every later fallible action, even
        // when allocation rounded upward. The enclosing scope owns rollback.
        let actual = values
            .capacity()
            .checked_sub(old)
            .ok_or_else(|| resource(Resource::Accounting))?;
        let extra = actual
            .checked_sub(requested)
            .ok_or_else(|| resource(Resource::Accounting))?;
        if extra != 0 {
            let bytes = extra
                .checked_mul(std::mem::size_of::<T>())
                .ok_or_else(|| resource(Resource::Arithmetic))?;
            facts.reserve_checked_control_storage_v1(bytes)?;
        }
    }
    values.push(value);
    Ok(())
}

impl CanonicalMemoryControlRecorderV1 {
    /// Record only this full projection's own leaves. Unsupported source shapes
    /// add no identity claims; the lowerer still authenticates exact own uses.
    pub(super) fn identity_address_arguments_v1(
        &mut self,
        function: &SemanticFunctionDeclV1,
        callables: &[SemanticCallableDeclV1],
        intrinsic: &IntrinsicProjectionV1,
        checks: &[ProjectedBoundsCheckV1],
        facts: &mut impl ProjectedAssertionFactsV1,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
        facts.charge_private_array_work(8)?;
        let [access] = intrinsic.guarded_accesses.as_slice() else {
            return Ok(());
        };
        let [(index, extent)] = access.comparisons.as_slice() else {
            return Ok(());
        };
        if !checks.is_empty()
            || access.indices.as_slice() != [*index]
            || access.checked_success.is_some()
            || access.memory_space != MemorySpaceAttr::Global
            || access.access != AccessKindAttr::Write
        {
            return Ok(());
        }
        let mut getter = None;
        for source in function.blocks() {
            facts.charge_private_array_work(4)?;
            let SemanticTerminatorKindV1::Call(call) = source.terminator().kind() else {
                continue;
            };
            if matches!(
                callables.get(call.callee().index() as usize),
                Some(SemanticCallableDeclV1::CompilerIntrinsic {
                    operation: SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut { .. },
                    ..
                })
            ) && getter.replace(call).is_some()
            {
                return Ok(());
            }
        }
        let Some(call) = getter else {
            return Ok(());
        };
        facts.charge_private_array_work(
            call.arguments()
                .len()
                .checked_add(8)
                .ok_or_else(|| resource(Resource::Arithmetic))?,
        )?;
        let [receiver, witness] = call.arguments() else {
            return Ok(());
        };
        let Some(receiver) =
            raw_operand_place(receiver).filter(|place| place.projections().is_empty())
        else {
            return Ok(());
        };
        let Some(witness) = raw_operand_place(witness).filter(|place| place.projections().is_empty())
        else {
            return Ok(());
        };
        let Some(projected) = intrinsic
            .index_values
            .get(witness.local().index() as usize)
            .copied()
            .flatten()
        else {
            return Ok(());
        };
        if projected.mapping != SemanticDisjointIndexSpaceV1::Index1d
            || projected.precondition.is_some()
            || projected.availability.is_some()
            || projected.value != *index
            || !matches!(call.unwind(), SemanticUnwindActionV1::Unreachable)
        {
            return Ok(());
        }
        let mut slice = None;
        for source in function.blocks() {
            facts.charge_private_array_work(5)?;
            for statement in source.statements() {
                facts.charge_private_array_work(14)?;
                let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                    continue;
                };
                if assignment.destination().local() != receiver.local() {
                    continue;
                }
                let SemanticRvalueKindV1::Borrow {
                    kind: fe2o3_mir_model::semantic_mir_v1::SemanticBorrowKindV1::Mutable,
                    place,
                } = assignment.value().kind()
                else {
                    return Ok(());
                };
                if !assignment.destination().projections().is_empty()
                    || !place.projections().is_empty()
                    || !matches!(
                        function
                            .locals()
                            .get(place.local().index() as usize)
                            .map(|local| local.role()),
                        Some(SemanticLocalRoleV1::Argument(_))
                    )
                    || slice.replace(place.local()).is_some()
                {
                    return Ok(());
                }
            }
        }
        let Some(slice) = slice else {
            return Ok(());
        };
        for (ranked_value, source_local, component) in [
            (
                *index,
                witness.local(),
                ProductionProjectionArgumentComponentV1::Scalar,
            ),
            (
                *extent,
                slice,
                ProductionProjectionArgumentComponentV1::SliceLength,
            ),
        ] {
            self.identity_argument_v1(ranked_value, source_local, component, facts)?;
        }
        Ok(())
    }

    fn identity_argument_v1(
        &mut self,
        ranked_value: ProductionRankedValueV1,
        source_local: SemanticLocalIdV1,
        component: ProductionProjectionArgumentComponentV1,
        facts: &mut impl ProjectedAssertionFactsV1,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        facts.charge_private_array_work(self.candidate.arguments.len().checked_add(3).ok_or_else(|| {
            resource(fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)
        })?)?;
        if let Some(row) = self
            .candidate
            .arguments
            .iter()
            .find(|row| row.ranked_value == ranked_value)
        {
            if row.source_local != source_local || row.component != component {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "canonical identity anchor conflicts with another claim",
                ));
            }
        } else {
            push(
                &mut self.candidate.arguments,
                ProductionProjectionArgumentCandidateV1 {
                    ranked_value,
                    source_local,
                    component,
                },
                facts,
            )?;
        }
        Ok(())
    }

    pub(super) fn identity_slice(
        &mut self,
        function: &SemanticFunctionDeclV1,
        callables: &[SemanticCallableDeclV1],
        intrinsic: &IntrinsicProjectionV1,
        checks: &[ProjectedBoundsCheckV1],
        facts: &mut impl ProjectedAssertionFactsV1,
    ) -> Result<CanonicalIdentitySliceV1, ProductionRankedProjectionErrorV1> {
        let invalid = ProductionRankedProjectionErrorV1::Incomplete;
        facts.charge_private_array_work(8)?;
        if !checks.is_empty() || intrinsic.guarded_accesses.len() != 1 {
            return Err(invalid(
                "canonical identity getter cannot mix guarded origins",
            ));
        }
        facts.reserve_checked_control_storage_v1(std::mem::size_of::<
            Option<CanonicalIdentitySliceV1>,
        >())?;
        let access = &intrinsic.guarded_accesses[0];
        let [(index, extent)] = access.comparisons.as_slice() else {
            return Err(invalid("canonical identity getter has extra comparisons"));
        };
        if access.indices.as_slice() != [*index]
            || access.checked_success.is_some()
            || access.memory_space != MemorySpaceAttr::Global
        {
            return Err(invalid("canonical identity getter access differs"));
        }
        let mut getter = None;
        for (block, source) in function.blocks().iter().enumerate() {
            facts.charge_private_array_work(4)?;
            let SemanticTerminatorKindV1::Call(call) = source.terminator().kind() else {
                continue;
            };
            if matches!(
                callables.get(call.callee().index() as usize),
                Some(SemanticCallableDeclV1::CompilerIntrinsic {
                    operation: SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut { .. },
                    ..
                })
            ) && getter.replace((block, call)).is_some()
            {
                return Err(invalid("canonical identity getter is not unique"));
            }
        }
        let (getter, call) = getter.ok_or(invalid("canonical identity getter is absent"))?;
        facts.charge_private_array_work(call.arguments().len().checked_add(8).ok_or_else(
            || resource(fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Arithmetic),
        )?)?;
        let [receiver, witness] = call.arguments() else {
            return Err(invalid("canonical identity getter argument count differs"));
        };
        let receiver = raw_operand_place(receiver)
            .filter(|place| place.projections().is_empty())
            .ok_or(invalid("canonical identity getter receiver differs"))?;
        let witness = raw_operand_place(witness)
            .filter(|place| place.projections().is_empty())
            .ok_or(invalid("canonical identity getter witness differs"))?;
        let projected = intrinsic
            .index_values
            .get(witness.local().index() as usize)
            .copied()
            .flatten()
            .ok_or(invalid("canonical identity witness projection is absent"))?;
        if projected.mapping != SemanticDisjointIndexSpaceV1::Index1d
            || projected.precondition.is_some()
            || projected.availability.is_some()
            || projected.value != *index
            || !matches!(call.unwind(), SemanticUnwindActionV1::Unreachable)
        {
            return Err(invalid(
                "canonical identity getter witness is not direct identity",
            ));
        }
        let option = simple_call_destination(call)?;
        let mut producer = None;
        let mut slice = None;
        let mut discriminator = None;
        for (block, source) in function.blocks().iter().enumerate() {
            facts.charge_private_array_work(5)?;
            if let SemanticTerminatorKindV1::Call(call) = source.terminator().kind()
                && call
                    .destination()
                    .is_some_and(|to| to.place().local() == witness.local())
            {
                if !matches!(
                    callables.get(call.callee().index() as usize),
                    Some(SemanticCallableDeclV1::CompilerIntrinsic {
                        operation: SemanticCompilerIntrinsicOperationV1::ThreadIndex1d { .. },
                        ..
                    })
                ) || !call.arguments().is_empty()
                    || !matches!(call.unwind(), SemanticUnwindActionV1::Unreachable)
                    || producer.replace(block).is_some()
                {
                    return Err(invalid("canonical identity witness producer differs"));
                }
            }
            for statement in source.statements() {
                facts.charge_private_array_work(5)?;
                let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                    continue;
                };
                if assignment.destination().local() == receiver.local() {
                    let SemanticRvalueKindV1::Borrow {
                        kind: fe2o3_mir_model::semantic_mir_v1::SemanticBorrowKindV1::Mutable,
                        place,
                    } = assignment.value().kind()
                    else {
                        return Err(invalid(
                            "canonical identity receiver is not a mutable borrow",
                        ));
                    };
                    if !assignment.destination().projections().is_empty()
                        || !place.projections().is_empty()
                        || !matches!(
                            function
                                .locals()
                                .get(place.local().index() as usize)
                                .map(|local| local.role()),
                            Some(SemanticLocalRoleV1::Argument(_))
                        )
                        || slice.replace(place.local()).is_some()
                    {
                        return Err(invalid("canonical identity receiver formal differs"));
                    }
                }
                if let SemanticRvalueKindV1::Discriminant(place) = assignment.value().kind()
                    && place.local() == option
                {
                    if !place.projections().is_empty()
                        || !assignment.destination().projections().is_empty()
                        || discriminator
                            .replace(assignment.destination().local())
                            .is_some()
                    {
                        return Err(invalid("canonical identity discriminator differs"));
                    }
                }
            }
        }
        let producer = producer.ok_or(invalid("canonical identity producer is absent"))?;
        let slice = slice.ok_or(invalid("canonical identity receiver borrow is absent"))?;
        let discriminator =
            discriminator.ok_or(invalid("canonical identity discriminator absent"))?;
        let mut selection = None;
        for (block, source) in function.blocks().iter().enumerate() {
            facts.charge_private_array_work(4)?;
            let SemanticTerminatorKindV1::SwitchInt {
                discriminant,
                targets,
            } = source.terminator().kind()
            else {
                continue;
            };
            if simple_operand_local(discriminant) != Some(discriminator) {
                continue;
            }
            facts.charge_private_array_work(targets.values().len().checked_add(4).ok_or_else(
                || {
                    resource(
                        fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Arithmetic,
                    )
                },
            )?)?;
            let (none, some, fallback) = match targets.values() {
                [value] if value.value() == 0 => {
                    (value.edge().target(), targets.otherwise().target(), None)
                }
                [value] if value.value() == 1 => {
                    (targets.otherwise().target(), value.edge().target(), None)
                }
                [zero, one] if zero.value() == 0 && one.value() == 1 => {
                    let fallback = targets.otherwise().target().index() as usize;
                    if !switch_fallback_is_empty_unreachable_v1(function, fallback) {
                        return Err(invalid(
                            "canonical identity default is not empty unreachable",
                        ));
                    }
                    (zero.edge().target(), one.edge().target(), Some(fallback))
                }
                _ => return Err(invalid("canonical identity switch is not exact None/Some")),
            };
            if none == some
                || selection
                    .replace((
                        block,
                        none.index() as usize,
                        some.index() as usize,
                        fallback,
                    ))
                    .is_some()
            {
                return Err(invalid(
                    "canonical identity switch occurrence is not unique",
                ));
            }
        }
        let (switch, none, some, fallback) =
            selection.ok_or(invalid("canonical identity switch absent"))?;
        for (ranked_value, source_local, component) in [
            (
                *index,
                witness.local(),
                ProductionProjectionArgumentComponentV1::Scalar,
            ),
            (
                *extent,
                slice,
                ProductionProjectionArgumentComponentV1::SliceLength,
            ),
        ] {
            self.identity_argument_v1(ranked_value, source_local, component, facts)?;
        }
        Ok(CanonicalIdentitySliceV1 {
            producer,
            getter,
            switch,
            none,
            some,
            fallback,
            index: *index,
            extent: *extent,
            view: access.view,
        })
    }

    pub(super) fn new(
        facts: &mut impl ProjectedAssertionFactsV1,
    ) -> Result<Self, ProductionRankedProjectionErrorV1> {
        facts.charge_private_array_work(1)?;
        facts.reserve_checked_control_storage_v1(std::mem::size_of::<Self>())?;
        Ok(Self {
            candidate: ProductionProjectionControlCandidateV1::default(),
        })
    }

    /// Call while the existing argument-allocation maps still live. A local is
    /// only a claim here; the lowerer independently rejects opaque nonformals.
    pub(super) fn arguments(
        &mut self,
        scalars: &[Option<u32>],
        slice_lengths: &[Option<u32>],
        facts: &mut impl ProjectedAssertionFactsV1,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
        for (map, component) in [
            (scalars, ProductionProjectionArgumentComponentV1::Scalar),
            (
                slice_lengths,
                ProductionProjectionArgumentComponentV1::SliceLength,
            ),
        ] {
            for (local, argument) in map.iter().enumerate() {
                facts.charge_private_array_work(2)?;
                if let Some(argument) = argument {
                    let local = u32::try_from(local).map_err(|_| resource(Resource::Arithmetic))?;
                    push(
                        &mut self.candidate.arguments,
                        ProductionProjectionArgumentCandidateV1 {
                            ranked_value: ProductionRankedValueV1::Argument(*argument),
                            source_local: SemanticLocalIdV1::from_index(local),
                            component,
                        },
                        facts,
                    )?;
                }
            }
        }
        Ok(())
    }

    /// Bounds projection emits unknown index leaves rather than arguments.
    /// Their numerical meaning comes only from the independently checked
    /// source binding and exact O guard use, never from IndexUnknown itself.
    pub(super) fn bounds(
        &mut self,
        checks: &[ProjectedBoundsCheckV1],
        facts: &mut impl ProjectedAssertionFactsV1,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        for check in checks {
            facts.charge_private_array_work(4)?;
            for (ranked_value, source_local, component) in [
                (
                    check.index,
                    check.index_local,
                    ProductionProjectionArgumentComponentV1::Scalar,
                ),
                (
                    check.extent,
                    check.slice_local,
                    ProductionProjectionArgumentComponentV1::SliceLength,
                ),
            ] {
                facts.charge_private_array_work(self.candidate.arguments.len())?;
                if let Some(existing) = self
                    .candidate
                    .arguments
                    .iter()
                    .find(|row| row.ranked_value == ranked_value)
                {
                    if existing.source_local != source_local || existing.component != component {
                        return Err(ProductionRankedProjectionErrorV1::Incomplete(
                            "one projected bounds leaf has differing source anchors",
                        ));
                    }
                    continue;
                }
                push(
                    &mut self.candidate.arguments,
                    ProductionProjectionArgumentCandidateV1 {
                        ranked_value,
                        source_local,
                        component,
                    },
                    facts,
                )?;
            }
        }
        Ok(())
    }

    /// Record after item emission has fixed `current`, with the precomputed
    /// next base (or total block_count) as `end`. No block/operation is cloned.
    pub(super) fn block(
        &mut self,
        source: usize,
        first: usize,
        tail: usize,
        end: usize,
        facts: &mut impl ProjectedAssertionFactsV1,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
        facts.charge_private_array_work(4)?;
        let narrow = |value| u32::try_from(value).map_err(|_| resource(Resource::Arithmetic));
        push(
            &mut self.candidate.blocks,
            ProductionProjectionControlBlockV1 {
                source_block: SemanticBlockIdV1::from_index(narrow(source)?),
                first: narrow(first)?,
                tail: narrow(tail)?,
                end: narrow(end)?,
            },
            facts,
        )
    }

    pub(super) const fn candidate(&self) -> &ProductionProjectionControlCandidateV1 {
        &self.candidate
    }

    #[cfg(test)]
    pub(super) fn candidate_mut(&mut self) -> &mut ProductionProjectionControlCandidateV1 {
        &mut self.candidate
    }
}
