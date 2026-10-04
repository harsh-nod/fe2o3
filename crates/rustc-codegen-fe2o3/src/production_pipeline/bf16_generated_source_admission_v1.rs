//! Fixed primitive source-admission facts. This is not a normal compilation,
//! simulator, artifact, or transferable compiler owner.
use super::*;

const SOURCE_CAP: usize = 64 * 1024;
const NOMINAL_RANKED_REFUSAL: &str = "BF16 nominal source-ranked projection";

#[derive(Clone, Copy)]
pub(crate) struct Bf16GeneratedSourceAdmissionV1 {
    pub(crate) source_sha256: [u8; 32],
    pub(crate) semantic_sha256: [u8; 32],
    pub(crate) root_mir_sha256: [u8; 32],
    pub(crate) helper_mir_sha256: [u8; 32],
    pub(crate) helper_source_signature_sha256: [u8; 32],
    pub(crate) helper_fn_abi_sha256: [u8; 32],
    // Filled only from the actual materialized nominal owner's identity.
    pub(crate) canonical_identity: [u8; 32],
    pub(crate) source_bytes: usize,
    pub(crate) root: u32,
    pub(crate) helper: u32,
    pub(crate) call_block: u32,
    pub(crate) return_permutation: [u8; 4],
    pub(crate) normal_refusal: &'static str,
}

fn supported_permutation(order: [u8; 4]) -> bool {
    matches!(order, [0, 1, 2, 3] | [1, 0, 2, 3])
}

fn require_nominal_ranked_refusal(
    error: ProductionPipelineError,
) -> Result<(), ProductionPipelineError> {
    match error {
        ProductionPipelineError::RankedProjection(
            crate::production_ranked_projection_v1::ProductionRankedProjectionErrorV1::StructuralValidation(
                fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1::LocalHelperSourceConsumerUnavailable {
                    consumer: NOMINAL_RANKED_REFUSAL,
                },
            ),
        ) => Ok(()),
        other => Err(other),
    }
}

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    /// Fixed read-only command over the authenticated source seed and existing
    /// nominal constructor. It then consumes the owner in the unchanged ranked
    /// gate and requires its exact typed refusal, never normal success.
    pub(crate) fn inspect_bf16_generated_source_admission_v1(
        self,
    ) -> Result<Bf16GeneratedSourceAdmissionV1, Box<ProductionPipelineError>> {
        let (ordinary, mut facts) = self.materialize_bf16_tile_values_with_mode_v1(
            MaterializationModeV1::NominalInspection,
            |source, budget| {
                if source.source().bytes().len() > SOURCE_CAP {
                    return Err(Error::Unavailable("BF16 candidate source exceeds 64 KiB"));
                }
                let relation = source.relation();
                let order = relation.return_permutation();
                if !supported_permutation(order)
                    || relation.root() == relation.helper()
                    || source.grants_artifact_or_launch_authority()
                {
                    return Err(Error::Unavailable("BF16 candidate source profile"));
                }
                // Fixed copied outcome and conservative coexistence/copy debit.
                // Existing source/constructor costs keep their original scopes;
                // this is not a whole-action or allocator-memory claim.
                let fixed = std::mem::size_of::<Bf16GeneratedSourceAdmissionV1>()
                    .checked_mul(3)
                    .ok_or(Resource::Arithmetic)?;
                budget.reserve_storage(fixed)?;
                budget.charge_work(fixed)?;
                Ok(Bf16GeneratedSourceAdmissionV1 {
                    source_sha256: *source.source().sha256(),
                    semantic_sha256: *relation
                        .owner()
                        .source_semantic()
                        .semantic_sha256()
                        .as_bytes(),
                    root_mir_sha256: *source.root_mir_sha256(),
                    helper_mir_sha256: *source.helper_mir_sha256(),
                    helper_source_signature_sha256: *source.helper_source_signature_sha256(),
                    helper_fn_abi_sha256: *source.helper_fn_abi_sha256(),
                    canonical_identity: [0; 32],
                    source_bytes: source.source().bytes().len(),
                    root: relation.root().index(),
                    helper: relation.helper().index(),
                    call_block: relation.call_block().index(),
                    return_permutation: order,
                    normal_refusal: NOMINAL_RANKED_REFUSAL,
                })
            },
        )?;
        // The shared core completed every source and ledger postflight. Read
        // only the resulting same-source owner; never reconstruct from facts.
        let owner = &ordinary.materialized;
        let emission = owner
            .bf16_call_instance_emission_v1()
            .ok_or_else(|| Box::new(unavailable("BF16 actual nominal emission absent")))?;
        if owner.helper_source_policy_v1()
            != fe2o3_lower_mir_kernel::ProductionHelperSourcePolicyV1::Bf16Nominal
            || owner
                .semantic_ssa()
                .source_semantic()
                .semantic_sha256()
                .as_bytes()
                != &facts.semantic_sha256
            || !std::ptr::eq(emission.owner(), owner)
            || emission.root().index() != facts.root
            || emission.helper().index() != facts.helper
            || emission.source_call_block().index() != facts.call_block
            || emission.return_permutation() != facts.return_permutation
        {
            return Err(Box::new(unavailable(
                "BF16 admitted source/emission join differs",
            )));
        }
        facts.canonical_identity = *owner.executable().canonical().identity().digest();
        match ordinary.verify_general_kernel_checks() {
            Err(error) => require_nominal_ranked_refusal(error).map_err(Box::new)?,
            Ok(unexpected) => {
                drop(unexpected);
                return Err(Box::new(unavailable(
                    "BF16 normal ranked admission changed; separate qualification required",
                )));
            }
        }
        Ok(facts)
    }
}

#[cfg(test)]
#[path = "bf16_generated_source_admission_v1_tests.rs"]
mod tests;
