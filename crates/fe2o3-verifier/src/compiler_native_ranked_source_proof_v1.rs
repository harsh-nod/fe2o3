//! Typed native-source replay. Diagnostic ranked text is never parsed as proof.

use super::*;
use crate::InertFunctionalRefinementReceiptSignatureV2;
use fe2o3_functional_proof::{
    FunctionalRefinementBoundaryV2, FunctionalRefinementImportExpectationV2,
    FunctionalRefinementImportPolicyV2, FunctionalRefinementReceiptImporterV2,
    ImportedFunctionalRefinementProofV2, VerusToolchainIdentityV2,
};
use fe2o3_lower_mir_kernel::{
    NativeRankedSourceCandidateV1, ReplayedRankedNativeSourceV1,
    attach_replayed_native_source_ranked_v1,
};
use fe2o3_pliron::{
    ProductionConstructionV1, ProductionMiddleEndEvidenceV5, ProductionRankedKernelLoweringInputV1,
    ProductionRankedOperationV1, ProductionRefinementStagingPolicyV2, ProductionSessionLimitsV1,
    compile_ranked_kernel_with_policy_checked_refinement_staging_v2,
};

/// An inert typed recipe and all signed effect receipts in operation order.
/// Neither the embedded keys nor candidate construction authenticate origin.
#[derive(Clone, Copy)]
pub struct NativeCompilerRankedRootV1<'a> {
    pub candidate: NativeRankedSourceCandidateV1<'a>,
    pub effect_receipts: &'a [InertFunctionalRefinementReceiptSignatureV2],
}

/// The complete source packet plus a complete semantic-root-ordered typed roster.
/// This in-process representation is not a native artifact wire-format codec.
#[derive(Clone, Copy)]
pub struct NativeCompilerRankedSourceProofInputsV1<'a> {
    pub source: NativeCompilerSourceProofInputsV1<'a>,
    pub ranked_roots: &'a [NativeCompilerRankedRootV1<'a>],
}

/// Independent source/N owner with freshly imported, recompiled ranked checks.
/// Retaining the attached owner prevents a detached success flag from replacing
/// the actual source/graph relation. Compiler and launch origin remain unproved.
pub struct ValidatedNativeCompilerRankedSourceProofV1 {
    source: ReplayedRankedNativeSourceV1,
    middle: Roster,
    correspondence: Roster,
    verus: Roster,
    roots: Vec<CheckedRoot>,
}

impl ValidatedNativeCompilerRankedSourceProofV1 {
    pub fn source(&self) -> &ReplayedRankedNativeSourceV1 {
        &self.source
    }
    pub fn root_count(&self) -> usize {
        self.roots.len()
    }
    pub fn middle_end_roster(&self) -> &Roster {
        &self.middle
    }
    pub fn correspondence_roster(&self) -> &Roster {
        &self.correspondence
    }
    pub fn verus_roster(&self) -> &Roster {
        &self.verus
    }
    pub fn signed_ranked_proof(&self, root: usize) -> Option<&Signed> {
        self.roots.get(root).map(|root| &root.signed)
    }
    pub const fn replays_ranked_source_relation(&self) -> bool {
        true
    }
    /// Existing translation checks exclude whole operational equivalence and
    /// complete indexed-address equivalence; typed reconstruction adds neither.
    pub const fn proves_whole_operational_or_indexed_address_equivalence(&self) -> bool {
        false
    }
    pub const fn authenticates_compiler_or_launch_origin(&self) -> bool {
        false
    }
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

/// Payload transferred to the caller. New vectors and wire visits use the
/// canonical ledger; inherited PLIRON construction/analysis and semantic codecs
/// retain their existing bounded domains, not a claimed heap/RSS census.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeCompilerRankedSourceProofStorageV1(usize);
impl NativeCompilerRankedSourceProofStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

fn reserve_vec<T>(count: usize, budget: &mut Budget<'_>) -> Result<(Vec<T>, usize), E> {
    budget.charge_work(3)?;
    let requested = count
        .checked_mul(std::mem::size_of::<T>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(requested)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Resource::Allocation)?;
    let actual = values
        .capacity()
        .checked_mul(std::mem::size_of::<T>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(actual.checked_sub(requested).ok_or(Resource::Accounting)?)?;
    Ok((values, actual))
}

fn commitment(proof: &ImportedFunctionalRefinementProofV2) -> NativeCompilerStagingCommitmentV1 {
    let toolchain = proof.toolchain();
    NativeCompilerStagingCommitmentV1 {
        receipt: *proof.receipt_identity().digest().as_bytes(),
        effect: *proof
            .binding()
            .normalized_obligation_effect_ir_hash()
            .as_bytes(),
        signer: *proof.signer_identity().as_bytes(),
        execution: *proof.execution_identity().as_bytes(),
        toolchain: [
            *toolchain.verus_executable().as_bytes(),
            *toolchain.verus_configuration().as_bytes(),
            *toolchain.solver_executable().as_bytes(),
            *toolchain.solver_configuration().as_bytes(),
            *toolchain.runtime_closure().as_bytes(),
        ],
    }
}

fn recompile_root(
    root: NativeCompilerRankedRootV1<'_>,
    expected: &[NativeCompilerStagingCommitmentV1],
    toolchain: VerusToolchainIdentityV2,
    budget: &mut Budget<'_>,
) -> Result<ProductionRankedKernelLoweringInputV1, E> {
    budget.charge_work(4)?;
    if root.effect_receipts.len() != expected.len() || expected.is_empty() {
        return Err(E::Mismatch("complete nonempty signed effect roster"));
    }
    let (mut imported, imported_storage) = reserve_vec(expected.len(), budget)?;
    let (mut signers, signer_storage) = reserve_vec(expected.len(), budget)?;
    let mut ordinal = 0usize;
    for block in root.candidate.kernel().blocks() {
        budget.charge_work(1)?;
        for operation in block.operations() {
            budget.charge_work(1)?;
            let request = match operation {
                ProductionRankedOperationV1::RequireAuthenticatedReferenceEquivalent {
                    proof,
                    ..
                }
                | ProductionRankedOperationV1::RequireEffectRefinement { proof, .. }
                | ProductionRankedOperationV1::RequireNumericalRefinement { proof, .. }
                | ProductionRankedOperationV1::RequireTensorRefinement { proof, .. } => proof,
                ProductionRankedOperationV1::RequestAuthenticatedReferenceEquivalent { .. }
                | ProductionRankedOperationV1::RequestEffectRefinement { .. }
                | ProductionRankedOperationV1::RequestNumericalRefinement { .. }
                | ProductionRankedOperationV1::RequestTensorRefinement { .. } => {
                    return Err(E::Mismatch("unbound typed effect request"));
                }
                _ => continue,
            };
            let wire = root
                .effect_receipts
                .get(ordinal)
                .ok_or(E::Mismatch("missing ordered signed effect receipt"))?;
            let expected = expected
                .get(ordinal)
                .ok_or(E::Mismatch("missing ordered staging commitment"))?;
            budget.charge_work(
                wire.wire()
                    .len()
                    .checked_add(32 + 289)
                    .ok_or(Resource::Arithmetic)?,
            )?;
            let policy = FunctionalRefinementImportPolicyV2::new(
                *wire.verifying_key(),
                toolchain,
                FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir,
            )
            .map_err(E::EffectReceipt)?;
            let mut importer =
                FunctionalRefinementReceiptImporterV2::new(policy, 1).map_err(E::EffectReceipt)?;
            let proof = importer
                .import(
                    FunctionalRefinementImportExpectationV2::new(request.binding()),
                    wire.wire(),
                )
                .map_err(E::EffectReceipt)?;
            if proof.receipt_identity() != request.receipt_identity()
                || commitment(&proof) != *expected
            {
                return Err(E::Mismatch("exact signed effect identity and staging row"));
            }
            signers.push(proof.signer_identity());
            imported.push(proof);
            ordinal = ordinal.checked_add(1).ok_or(Resource::Arithmetic)?;
        }
    }
    budget.charge_work(1)?;
    if ordinal != root.effect_receipts.len() {
        return Err(E::Mismatch("unused signed effect receipt"));
    }
    let policy =
        ProductionRefinementStagingPolicyV2::new(signers, toolchain).map_err(E::RankedRecipe)?;
    let construction = ProductionConstructionV1::ranked_kernel(
        "native_source_replay",
        root.candidate.kernel().clone(),
    )
    .map_err(|_| E::Mismatch("typed ranked construction"))?;
    // The existing staged compiler independently checks normalized obligations,
    // duplicate receipt claims, unused receipts, and its complete verifier pipeline.
    let lowering = compile_ranked_kernel_with_policy_checked_refinement_staging_v2(
        construction,
        ProductionSessionLimitsV1::default(),
        imported,
        policy,
    )
    .map_err(|error| E::RankedCompile(Box::new(error)))?;
    budget.release_storage(
        imported_storage
            .checked_add(signer_storage)
            .ok_or(Resource::Arithmetic)?,
    )?;
    Ok(lowering)
}

fn check_rederived_aggregate(
    ranked: &ProductionRankedKernelLoweringInputV1,
    evidence: &ProductionMiddleEndEvidenceV5,
    signed: &Signed,
) -> Result<(), E> {
    use crate::mir_pliron_per_compilation_verus_v1::rederive_mir_pliron_aggregate_preparation_v1;
    let rebuilt = rederive_mir_pliron_aggregate_preparation_v1(ranked, evidence)
        .map_err(|error| E::Aggregate(Box::new(error)))?;
    let claims = signed.claims();
    if rebuilt.binding != signed.imported_proof().binding()
        || rebuilt.contract_identity != claims.contract_identity()
        || rebuilt.parallel_contract_identity != claims.parallel_contract_identity()
        || rebuilt.pliron_evidence_identity != claims.pliron_evidence_identity()
        || rebuilt.composition_template_identity != claims.composition_template_identity()
        || rebuilt.generated_source_identity != claims.generated_source_identity()
        || rebuilt.retained_count != claims.retained_policy_checked_staging()
    {
        return Err(E::Mismatch("fresh source/ranked aggregate subjects"));
    }
    Ok(())
}

pub(super) struct RecompiledNativeRankedRootsV1<'a> {
    pub(super) candidates: Vec<NativeRankedSourceCandidateV1<'a>>,
    pub(super) lowerings: Vec<ProductionRankedKernelLoweringInputV1>,
    pub(super) candidate_storage: usize,
    pub(super) lowering_vector_storage: usize,
    pub(super) lowering_storage: usize,
}

pub(super) fn recompile_native_ranked_roots_v1<'a>(
    source: &fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1,
    middle: &Roster,
    roots: &[CheckedRoot],
    inputs: &[NativeCompilerRankedRootV1<'a>],
    budget: &mut Budget<'_>,
) -> Result<RecompiledNativeRankedRootsV1<'a>, E> {
    let (mut candidates, candidate_storage) = reserve_vec(inputs.len(), budget)?;
    let (mut lowerings, lowering_vector_storage) = reserve_vec(inputs.len(), budget)?;
    let mut lowering_storage = 0usize;
    for (ordinal, input) in inputs.iter().enumerate() {
        let row = middle
            .root(ordinal)
            .ok_or(E::Mismatch("typed ranked root row"))?;
        let root = &roots[ordinal];
        budget.charge_work(3)?;
        if input.candidate.semantic_root() != row.semantic_root()
            || input.candidate.launch_rank() != row.source_rank()
        {
            return Err(E::Mismatch("ordered typed ranked root/rank"));
        }
        // Exact transport equality bounds the subsequent V5 allocation;
        // diagnostic text is never parsed or accepted as a semantic proof.
        budget.charge_work(1)?;
        let diagnostic = input.candidate.ranked_ir();
        if diagnostic.len() != root.middle.ranked_ir().len() {
            return Err(E::Mismatch("exact typed ranked diagnostic text"));
        }
        budget.charge_work(diagnostic.len())?;
        if diagnostic != root.middle.ranked_ir() {
            return Err(E::Mismatch("exact typed ranked diagnostic text"));
        }
        let lowering = recompile_root(
            *input,
            &root.staging,
            root.signed.imported_proof().toolchain(),
            budget,
        )?;
        let retained = lowering.production_analysis_retained_storage_upper_bound_v1();
        budget.reserve_storage(retained)?;
        lowering_storage = lowering_storage
            .checked_add(retained)
            .ok_or(Resource::Arithmetic)?;
        let evidence_storage = row
            .payload()
            .len()
            .checked_mul(2)
            .and_then(|n| n.checked_add(std::mem::size_of::<ProductionMiddleEndEvidenceV5>()))
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(evidence_storage)?;
        budget.charge_work(
            input
                .candidate
                .ranked_ir()
                .len()
                .checked_add(row.payload().len())
                .ok_or(Resource::Arithmetic)?,
        )?;
        let evidence = ProductionMiddleEndEvidenceV5::try_new(
            source.semantic_ssa().source_owner(),
            &lowering,
            input.candidate.ranked_ir(),
        )
        .map_err(E::Middle)?;
        budget.charge_work(evidence.as_inert().canonical_bytes().len())?;
        if evidence.as_inert().canonical_bytes() != row.payload() {
            return Err(E::Mismatch("fresh typed ranked V5 evidence"));
        }
        check_rederived_aggregate(&lowering, &evidence, &root.signed)?;
        drop(evidence);
        budget.release_storage(evidence_storage)?;
        candidates.push(input.candidate);
        lowerings.push(lowering);
    }
    Ok(RecompiledNativeRankedRootsV1 {
        candidates,
        lowerings,
        candidate_storage,
        lowering_vector_storage,
        lowering_storage,
    })
}

/// Reconstructs source/N, imports every effect signature against its exact typed
/// request, invokes the real staged ranked compiler, rebuilds exact V5 evidence
/// and aggregate proof inputs, then consumes the fresh lowerings into the source
/// owner and replays correspondence. Missing typed roots or receipt bytes fail.
/// There is no unsigned or commitment-only fallback, and no origin authority.
///
/// Entry8; new vectors reserve their actual capacities and receipt bytes are
/// paid before import. Existing signature, ranked construction, contract/source
/// generation and semantic engines retain their own bounded resource domains.
/// Both success and failure restore the incoming canonical storage floor. The
/// result receipt transfers all retained logical payload to its caller.
pub fn validate_native_compiler_ranked_source_proof_v1(
    inputs: NativeCompilerRankedSourceProofInputsV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<
    (
        ValidatedNativeCompilerRankedSourceProofV1,
        NativeCompilerRankedSourceProofStorageV1,
    ),
    E,
> {
    budget.charge_work(8)?;
    let floor = budget.storage();
    let token = budget.work_ledger_identity_v1();
    let slot = budget as *const Budget<'_> as usize;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let (checked, old_storage) =
            validate_native_compiler_source_proof_v1(inputs.source, budget)?;
        budget.reserve_storage(old_storage.retained_storage())?;
        budget.charge_work(1)?;
        if inputs.ranked_roots.len() != checked.roots.len() {
            return Err(E::Mismatch("complete typed ranked root roster"));
        }
        let wrapper = std::mem::size_of::<ValidatedNativeCompilerRankedSourceProofV1>();
        budget.reserve_storage(wrapper)?;
        let RecompiledNativeRankedRootsV1 {
            candidates,
            lowerings,
            candidate_storage,
            lowering_vector_storage,
            lowering_storage,
        } = recompile_native_ranked_roots_v1(
            checked.source.source(),
            &checked.middle,
            &checked.roots,
            inputs.ranked_roots,
            budget,
        )?;
        let ValidatedNativeCompilerSourceProofV1 {
            source,
            middle,
            correspondence,
            verus,
            roots,
        } = checked;
        let (source, attachment_storage) =
            attach_replayed_native_source_ranked_v1(source, &candidates, lowerings, budget)
                .map_err(E::Source)?;
        drop(candidates);
        budget.release_storage(
            candidate_storage
                .checked_add(lowering_vector_storage)
                .ok_or(Resource::Arithmetic)?,
        )?;
        let retained = old_storage
            .retained_storage()
            .checked_add(wrapper)
            .and_then(|n| n.checked_add(lowering_storage))
            .and_then(|n| n.checked_add(attachment_storage.retained_storage()))
            .ok_or(Resource::Arithmetic)?;
        Ok((
            ValidatedNativeCompilerRankedSourceProofV1 {
                source,
                middle,
                correspondence,
                verus,
                roots,
            },
            NativeCompilerRankedSourceProofStorageV1(retained),
        ))
    }));
    if token != budget.work_ledger_identity_v1() || slot != budget as *const Budget<'_> as usize {
        drop(result);
        return Err(Resource::Accounting.into());
    }
    let Some(release) = budget.storage().checked_sub(floor) else {
        drop(result);
        return Err(Resource::Accounting.into());
    };
    if let Err(error) = budget.release_storage(release) {
        drop(result);
        return Err(error.into());
    }
    match result {
        Ok(result) => result,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

#[cfg(test)]
#[path = "compiler_native_ranked_source_proof_v1_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "compiler_native_ranked_source_fixture_v1_tests.rs"]
mod full_fixture_tests;
