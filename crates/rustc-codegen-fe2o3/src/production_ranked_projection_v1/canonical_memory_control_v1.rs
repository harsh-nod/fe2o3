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
