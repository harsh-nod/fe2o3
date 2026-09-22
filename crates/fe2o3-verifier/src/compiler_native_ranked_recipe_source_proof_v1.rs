//! Durable recipe reconstruction feeding the same independent typed replay.

use super::*;
use crate::InertFunctionalRefinementReceiptSignatureV2;
use fe2o3_functional_proof::VerusToolchainIdentityV2;
use fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12;
use fe2o3_lower_mir_kernel::{
    NativeRankedSourceCandidateV1, ProductionRankedAccessSourceV1,
    ProductionRankedExecutableEffectSourceV1,
};
use fe2o3_pliron::{
    ProductionRankedKernelV1, ProductionRankedRecipeDecodeErrorV1,
    ProductionRankedRecipeProofClaimV1, ProductionRankedRecipeProofResolverV1,
    ProductionRankedRecipeResolverWorkV1, ProductionRankedRecipeStorageV1,
    ProductionReferenceProofV2, decode_production_ranked_recipe_v1,
};
use std::mem::size_of;

/// Inert recipe bytes with the complete ordered replay metadata. Auxiliary rows
/// are still borrowed typed inputs, not a complete native artifact wire format.
#[derive(Clone, Copy)]
pub struct NativeCompilerRankedRecipeRootV1<'a> {
    pub semantic_root: u32,
    pub launch_rank: u8,
    pub recipe_bytes: &'a [u8],
    pub access_sources: &'a [ProductionRankedAccessSourceV1],
    pub executable_effect_sources: &'a [ProductionRankedExecutableEffectSourceV1],
    pub ranked_ir: &'a str,
    pub effect_receipts: &'a [InertFunctionalRefinementReceiptSignatureV2],
}

#[derive(Clone, Copy)]
pub struct NativeCompilerRankedRecipeSourceProofInputsV1<'a> {
    pub source: NativeCompilerSourceProofInputsV1<'a>,
    pub ranked_roots: &'a [NativeCompilerRankedRecipeRootV1<'a>],
}

#[derive(Clone, Copy)]
pub struct NativeCompilerUnitLocalErasedRecipeSourceProofInputsV1<'a> {
    pub original: NativeCompilerSourceProofInputsV1<'a>,
    pub ranked_roots: &'a [NativeCompilerRankedRecipeRootV1<'a>],
    pub erased: &'a VerifiedCanonicalKernelIrModuleV12,
}

struct OrderedResolver<'a> {
    signatures: &'a [InertFunctionalRefinementReceiptSignatureV2],
    expected: &'a [NativeCompilerStagingCommitmentV1],
    toolchain: VerusToolchainIdentityV2,
    cursor: usize,
}
impl<'a> OrderedResolver<'a> {
    fn new(
        signatures: &'a [InertFunctionalRefinementReceiptSignatureV2],
        expected: &'a [NativeCompilerStagingCommitmentV1],
        toolchain: VerusToolchainIdentityV2,
    ) -> Result<Self, E> {
        if signatures.len() != expected.len() || expected.is_empty() {
            return Err(E::Mismatch("complete nonempty signed effect roster"));
        }
        Ok(Self {
            signatures,
            expected,
            toolchain,
            cursor: 0,
        })
    }
}
impl ProductionRankedRecipeProofResolverV1 for OrderedResolver<'_> {
    type Error = E;

    fn resolve(
        &mut self,
        claim: ProductionRankedRecipeProofClaimV1,
        work: &mut ProductionRankedRecipeResolverWorkV1<'_, '_>,
    ) -> Result<ProductionReferenceProofV2, E> {
        if usize::try_from(claim.ordinal).map_err(|_| Resource::Arithmetic)? != self.cursor {
            return Err(E::Mismatch("ordered recipe proof claim"));
        }
        let signature = self
            .signatures
            .get(self.cursor)
            .ok_or(E::Mismatch("missing ordered signed effect receipt"))?;
        let expected = self
            .expected
            .get(self.cursor)
            .ok_or(E::Mismatch("missing ordered staging commitment"))?;
        let proof = ranked_source::import_ordered_effect_v1(
            claim.receipt_digest,
            claim.binding,
            signature,
            expected,
            self.toolchain,
            |amount| work.charge_work(amount),
        )?;
        self.cursor = self.cursor.checked_add(1).ok_or(Resource::Arithmetic)?;
        Ok(ProductionReferenceProofV2::request_exact(
            proof.receipt_identity(),
            proof.binding(),
        ))
    }

    fn finish(
        &mut self,
        resolved: u32,
        work: &mut ProductionRankedRecipeResolverWorkV1<'_, '_>,
    ) -> Result<(), E> {
        work.charge_work(1)?;
        if usize::try_from(resolved).map_err(|_| Resource::Arithmetic)? != self.cursor
            || self.cursor != self.signatures.len()
            || self.cursor != self.expected.len()
        {
            return Err(E::Mismatch("unused signed effect receipt"));
        }
        Ok(())
    }
}

struct DecodedRecipes {
    kernels: Vec<ProductionRankedKernelV1>,
    storage: usize,
}

pub(super) fn decode_signed_recipe_v1(
    bytes: &[u8],
    signatures: &[InertFunctionalRefinementReceiptSignatureV2],
    expected: &[NativeCompilerStagingCommitmentV1],
    toolchain: VerusToolchainIdentityV2,
    budget: &mut Budget<'_>,
) -> Result<(ProductionRankedKernelV1, ProductionRankedRecipeStorageV1), E> {
    let mut resolver = OrderedResolver::new(signatures, expected, toolchain)?;
    decode_production_ranked_recipe_v1(bytes, &mut resolver, budget).map_err(|error| match error {
        ProductionRankedRecipeDecodeErrorV1::Wire(error) => E::RankedRecipeWire(error),
        ProductionRankedRecipeDecodeErrorV1::Resolver(error) => error,
    })
}

impl DecodedRecipes {
    fn new(
        inputs: &[NativeCompilerRankedRecipeRootV1<'_>],
        checked: &[CheckedRoot],
        budget: &mut Budget<'_>,
    ) -> Result<Self, E> {
        budget.charge_work(1)?;
        if inputs.len() != checked.len() {
            return Err(E::Mismatch("complete typed ranked root roster"));
        }
        let header = size_of::<Vec<ProductionRankedKernelV1>>();
        budget.reserve_storage(header)?;
        let (mut kernels, payload) = ranked_source::reserve_vec(inputs.len(), budget)?;
        let mut storage = header.checked_add(payload).ok_or(Resource::Arithmetic)?;
        for (input, root) in inputs.iter().zip(checked) {
            budget.charge_work(1)?;
            let (kernel, receipt) = decode_signed_recipe_v1(
                input.recipe_bytes,
                input.effect_receipts,
                &root.staging,
                root.signed.imported_proof().toolchain(),
                budget,
            )?;
            budget.reserve_storage(receipt.retained_storage())?;
            let nested = receipt
                .retained_storage()
                .checked_sub(size_of::<ProductionRankedKernelV1>())
                .ok_or(Resource::Accounting)?;
            kernels.push(kernel);
            // The prepaid vector slot now owns the kernel's inline header.
            budget.release_storage(size_of::<ProductionRankedKernelV1>())?;
            storage = storage.checked_add(nested).ok_or(Resource::Arithmetic)?;
        }
        Ok(Self { kernels, storage })
    }

    fn replay<T>(
        self,
        inputs: &[NativeCompilerRankedRecipeRootV1<'_>],
        budget: &mut Budget<'_>,
        replay: impl FnOnce(&[NativeCompilerRankedRootV1<'_>], &mut Budget<'_>) -> Result<T, E>,
    ) -> Result<T, E> {
        let header = size_of::<Vec<NativeCompilerRankedRootV1<'_>>>();
        budget.reserve_storage(header)?;
        let (mut roots, payload) = ranked_source::reserve_vec(inputs.len(), budget)?;
        for (input, kernel) in inputs.iter().zip(&self.kernels) {
            budget.charge_work(1)?;
            roots.push(NativeCompilerRankedRootV1 {
                candidate: NativeRankedSourceCandidateV1::from_untrusted_parts(
                    input.semantic_root,
                    input.launch_rank,
                    kernel,
                    input.access_sources,
                    input.executable_effect_sources,
                    input.ranked_ir,
                ),
                effect_receipts: input.effect_receipts,
            });
        }
        // The legacy continuation independently imports again and recompiles.
        let result = replay(&roots, budget);
        drop(roots);
        drop(self.kernels);
        budget.release_storage(
            header
                .checked_add(payload)
                .and_then(|n| n.checked_add(self.storage))
                .ok_or(Resource::Arithmetic)?,
        )?;
        result
    }
}

fn recipe_scope<T>(
    budget: &mut Budget<'_>,
    run: impl FnOnce(&mut Budget<'_>) -> Result<T, E>,
) -> Result<T, E> {
    budget.charge_work(8)?;
    let floor = budget.storage();
    let token = budget.work_ledger_identity_v1();
    let slot = budget as *const Budget<'_> as usize;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(budget)));
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

/// Validates the source packet once, reconstructs recipes using its checked
/// commitments, then independently imports, recompiles and replays correspondence.
/// Recipe transport adds no compiler-origin, artifact, or launch authority.
/// Returned storage excludes discarded decoded inputs; caller-owned auxiliary
/// rows and signatures remain borrowed. Success/error/unwind restore the floor.
pub fn validate_native_compiler_ranked_recipe_source_proof_v1(
    inputs: NativeCompilerRankedRecipeSourceProofInputsV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<
    (
        ValidatedNativeCompilerRankedSourceProofV1,
        NativeCompilerRankedSourceProofStorageV1,
    ),
    E,
> {
    recipe_scope(budget, |budget| {
        let (checked, storage) = validate_native_compiler_source_proof_v1(inputs.source, budget)?;
        budget.reserve_storage(storage.retained_storage())?;
        let recipes = DecodedRecipes::new(inputs.ranked_roots, &checked.roots, budget)?;
        recipes.replay(inputs.ranked_roots, budget, |roots, budget| {
            ranked_source::complete_ranked_source_proof_v1(checked, storage, roots, budget)
        })
    })
}

/// As above, but preserves original N and independently checks actual E under
/// the existing UnitLocal route. No erasure producer runs during reconstruction.
pub fn validate_native_compiler_unit_local_erased_recipe_source_proof_v1(
    inputs: NativeCompilerUnitLocalErasedRecipeSourceProofInputsV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<
    (
        ValidatedNativeCompilerUnitLocalErasedSourceProofV1,
        NativeCompilerUnitLocalErasedSourceProofStorageV1,
    ),
    E,
> {
    recipe_scope(budget, |budget| {
        let (checked, storage) = validate_native_compiler_source_packet_v1(
            inputs.original,
            SourceReplayRoute::UnitLocal,
            budget,
        )?;
        budget.reserve_storage(storage.retained_storage())?;
        let recipes = DecodedRecipes::new(inputs.ranked_roots, &checked.roots, budget)?;
        recipes.replay(inputs.ranked_roots, budget, |roots, budget| {
            unit_local_erased::complete_unit_local_erased_source_proof_v1(
                checked,
                storage,
                roots,
                inputs.erased,
                budget,
            )
        })
    })
}
