//! Opt-in compiler-private source-to-handoff connection.
//! No Worker execution, normal admission, publication, or launch capability.
//! The live source seed and actual SSA move through the existing constructors;
//! neither copied evidence nor a test-only materializer can supply this owner.
use super::super::retained_materialization_phase_v1::RetainedMaterializationPhaseV1;
use super::*;

const SOURCE_CAP: usize = 64 * 1024;
const PROFILE_REFUSAL: &str = "BF16 same-owner source profile";

fn source_profile(
    bytes: usize,
    root: u32,
    helper: u32,
    source_order: [u8; 4],
    requested_order: [u8; 4],
    grants_authority: bool,
) -> Result<(), Error> {
    if bytes == 0
        || bytes > SOURCE_CAP
        || root == helper
        || !matches!(source_order, [0, 1, 2, 3] | [1, 0, 2, 3])
        || source_order != requested_order
        || grants_authority
    {
        return Err(Error::Unavailable(PROFILE_REFUSAL));
    }
    Ok(())
}

/// Opaque, non-Clone custody. It has no owner/bytes extraction method, Worker
/// method, serializer, or publication conversion. Dropping this value drops
/// the actual handoff before the retained original materialization account.
#[allow(dead_code)]
pub(crate) struct Bf16SameOwnerHandoffV1 {
    phase: RetainedMaterializationPhaseV1<PrivateBf16WorkerHandoffCompilationV1>,
}

impl Bf16SameOwnerHandoffV1 {
    #[allow(dead_code)]
    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    /// Closed, opt-in, extraction-only continuation. The trusted compiler-local
    /// inspector borrows the real source and must pay its own extra work/storage;
    /// it supplies no owned compiler payload and grants no authority. The same
    /// production materializer body serves the unchanged public refusal entry.
    ///
    /// This retains the original materialization account and the separate
    /// projection account. It is not an aggregate allocator/RSS/whole-action
    /// bound, and it does not turn raw Incomplete memory analysis into ordinary
    /// formal admission. Existing guarded bounds/alias duties are preserved.
    #[allow(dead_code)]
    pub(crate) fn prepare_bf16_same_owner_handoff_v1(
        self,
        requested_return: [u8; 4],
        inspect: impl for<'a, 'work> FnOnce(
            &SourceOwnedBf16TileValuesRegionV1<'a, 'tcx>,
            &mut Budget<'work>,
        ) -> Result<(), Error>,
    ) -> Result<Bf16SameOwnerHandoffV1, Box<ProductionPipelineError>> {
        let (ssa, source_seed) = self.prepare_bf16_tile_values_source_v1()?;
        let materialized = ssa.with_retained_materialization_budget_v1(|prepared, budget| {
            materialize_prepared_bf16_source_v1(
                prepared,
                source_seed,
                MaterializationModeV1::NominalInspection,
                |source, budget| {
                    let relation = source.relation();
                    source_profile(
                        source.source().bytes().len(),
                        relation.root().index(),
                        relation.helper().index(),
                        relation.return_permutation(),
                        requested_return,
                        source.grants_artifact_or_launch_authority(),
                    )?;
                    inspect(source, budget)
                },
                budget,
            )
        })?;
        let ranked = materialized.try_map(|prepared, budget| {
            budget
                .check_prior_denials_v1()
                .map_err(materialization_resource_error_v29)?;
            let PreparedMaterializationV29 {
                materialized: (materialized, ()),
                ranked_roots,
                bindings,
            } = prepared;
            // The original constructor's retained receipt is already paid once
            // by the shared core. The replay reserves its own temporary envelope
            // while that first receipt remains live on this same original meter.
            materialized
                .verify_bf16_nominal_equivalence_with_budget_v1(budget)
                .map_err(ProductionPipelineError::PreRankedMaterialization)?;
            let emission = materialized
                .bf16_call_instance_emission_v1()
                .ok_or_else(|| unavailable("BF16 same-owner emission absent"))?;
            if materialized.helper_source_policy_v1()
                != fe2o3_lower_mir_kernel::ProductionHelperSourcePolicyV1::Bf16Nominal
                || !std::ptr::eq(emission.owner(), &materialized)
                || emission.return_permutation() != requested_return
            {
                return Err(Box::new(unavailable("BF16 same-owner emission join")));
            }
            MaterializedNeutralProductionCompilation {
                materialized,
                ranked_roots,
                bindings,
            }
            .verify_private_nominal_kernel_checks_v1()
            .map_err(Box::new)
        })?;
        let handoff = ranked
            .admit_private_bf16_formal_retained_v1(requested_return)?
            .bind_private_bf16_target_retained_v1(requested_return)?
            .optimize_private_bf16_target_retained_v1(requested_return)?
            .verify_private_bf16_output_guarded_safety_v1(requested_return)?
            .lower_private_bf16_llvm_retained_v1(requested_return)?
            .prepare_private_bf16_descriptor_retained_v1(requested_return)?
            .prepare_private_bf16_worker_handoff_retained_v1(requested_return)?
            .revalidate_private_bf16_worker_handoff_retained_v1(requested_return)?;
        Ok(Bf16SameOwnerHandoffV1 { phase: handoff })
    }
}

#[cfg(test)]
#[path = "bf16_same_owner_handoff_v1_tests.rs"]
mod tests;
