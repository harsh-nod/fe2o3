//! Durable recipe reconstruction feeding the same independent typed replay.

use super::*;
use crate::InertFunctionalRefinementReceiptSignatureV2;
use fe2o3_functional_proof::{ImportedFunctionalRefinementProofV2, VerusToolchainIdentityV2};
use fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12;
use fe2o3_lower_mir_kernel::{
    NativeRankedSourceCandidateV1, ProductionRankedSourceRowsV1,
    decode_production_ranked_source_rows_v1,
};
use fe2o3_pliron::{
    ProductionRankedKernelV1, ProductionRankedRecipeDecodeErrorV1,
    ProductionRankedRecipeProofClaimV1, ProductionRankedRecipeProofResolverV1,
    ProductionRankedRecipeResolverWorkV1, ProductionRankedRecipeStorageV1,
    ProductionReferenceProofV2, decode_production_ranked_recipe_v1,
};
use std::mem::size_of;

/// Borrowed inert recipe and source-row frames with ordered replay metadata.
/// The complete source packet codec transports these inputs and signatures;
/// neither this view nor its wire framing grants native artifact authority.
#[derive(Clone, Copy)]
pub struct NativeCompilerRankedRecipeRootV1<'a> {
    pub semantic_root: u32,
    pub launch_rank: u8,
    pub recipe_bytes: &'a [u8],
    pub source_rows_bytes: &'a [u8],
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

struct OrderedResolver<'a, Accept, Retain> {
    signatures: &'a [InertFunctionalRefinementReceiptSignatureV2],
    expected: &'a [NativeCompilerStagingCommitmentV1],
    toolchain: VerusToolchainIdentityV2,
    cursor: usize,
    accept: Accept,
    retain: Retain,
}
impl<'a, Accept, Retain> OrderedResolver<'a, Accept, Retain> {
    fn new(
        signatures: &'a [InertFunctionalRefinementReceiptSignatureV2],
        expected: &'a [NativeCompilerStagingCommitmentV1],
        toolchain: VerusToolchainIdentityV2,
        accept: Accept,
        retain: Retain,
    ) -> Result<Self, E> {
        if signatures.len() != expected.len() || expected.is_empty() {
            return Err(E::Mismatch("complete nonempty signed effect roster"));
        }
        Ok(Self {
            signatures,
            expected,
            toolchain,
            cursor: 0,
            accept,
            retain,
        })
    }
}
impl<Accept, Retain> ProductionRankedRecipeProofResolverV1 for OrderedResolver<'_, Accept, Retain>
where
    Accept: FnMut(
        &InertFunctionalRefinementReceiptSignatureV2,
        &NativeCompilerStagingCommitmentV1,
        &mut ProductionRankedRecipeResolverWorkV1<'_, '_>,
    ) -> Result<(), E>,
    Retain: FnMut(ImportedFunctionalRefinementProofV2),
{
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
        (self.accept)(signature, expected, work)?;
        let proof = ranked_source::import_ordered_effect_v1(
            claim.receipt_digest,
            claim.binding,
            signature,
            expected,
            self.toolchain,
            |amount| work.charge_work(amount),
        )?;
        self.cursor = self.cursor.checked_add(1).ok_or(Resource::Arithmetic)?;
        let request =
            ProductionReferenceProofV2::request_exact(proof.receipt_identity(), proof.binding());
        (self.retain)(proof);
        Ok(request)
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

struct DecodedRoot {
    kernel: ProductionRankedKernelV1,
    source_rows: ProductionRankedSourceRowsV1,
}

struct DecodedRecipes {
    roots: Vec<DecodedRoot>,
    storage: usize,
}

pub(super) fn decode_signed_recipe_v1(
    bytes: &[u8],
    signatures: &[InertFunctionalRefinementReceiptSignatureV2],
    expected: &[NativeCompilerStagingCommitmentV1],
    toolchain: VerusToolchainIdentityV2,
    budget: &mut Budget<'_>,
) -> Result<(ProductionRankedKernelV1, ProductionRankedRecipeStorageV1), E> {
    decode_signed_recipe_with_import_hooks_v1(
        bytes,
        signatures,
        expected,
        toolchain,
        budget,
        |_, _, _| Ok(()),
        drop,
    )
}

// V1 has no extra policy or retention work. Conditional replay uses these hooks
// to require externally accepted signers BEFORE import and move each proof once.
#[allow(clippy::too_many_arguments)]
pub(crate) fn decode_signed_recipe_with_import_hooks_v1(
    bytes: &[u8],
    signatures: &[InertFunctionalRefinementReceiptSignatureV2],
    expected: &[NativeCompilerStagingCommitmentV1],
    toolchain: VerusToolchainIdentityV2,
    budget: &mut Budget<'_>,
    accept: impl FnMut(
        &InertFunctionalRefinementReceiptSignatureV2,
        &NativeCompilerStagingCommitmentV1,
        &mut ProductionRankedRecipeResolverWorkV1<'_, '_>,
    ) -> Result<(), E>,
    retain: impl FnMut(ImportedFunctionalRefinementProofV2),
) -> Result<(ProductionRankedKernelV1, ProductionRankedRecipeStorageV1), E> {
    let mut resolver = OrderedResolver::new(signatures, expected, toolchain, accept, retain)?;
    decode_production_ranked_recipe_v1(bytes, &mut resolver, budget).map_err(|error| match error {
        ProductionRankedRecipeDecodeErrorV1::Wire(error) => E::RankedRecipeWire(error),
        ProductionRankedRecipeDecodeErrorV1::Resolver(error) => error,
    })
}

#[cfg(test)]
#[path = "compiler_native_ordered_resolver_v1_tests.rs"]
mod ordered_resolver_tests;

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
        let header = size_of::<Vec<DecodedRoot>>();
        budget.reserve_storage(header)?;
        let (mut roots, payload) = ranked_source::reserve_vec(inputs.len(), budget)?;
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
            let (source_rows, row_receipt) =
                decode_production_ranked_source_rows_v1(input.source_rows_bytes, budget)
                    .map_err(E::RankedSourceRowsWire)?;
            budget.reserve_storage(row_receipt.retained_storage())?;
            let inline = size_of::<ProductionRankedKernelV1>()
                .checked_add(size_of::<ProductionRankedSourceRowsV1>())
                .ok_or(Resource::Arithmetic)?;
            let nested = receipt
                .retained_storage()
                .checked_add(row_receipt.retained_storage())
                .ok_or(Resource::Arithmetic)?
                .checked_sub(inline)
                .ok_or(Resource::Accounting)?;
            roots.push(DecodedRoot {
                kernel,
                source_rows,
            });
            // The prepaid root slot now owns both inline headers (and padding).
            budget.release_storage(inline)?;
            storage = storage.checked_add(nested).ok_or(Resource::Arithmetic)?;
        }
        Ok(Self { roots, storage })
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
        for (input, root) in inputs.iter().zip(&self.roots) {
            budget.charge_work(1)?;
            roots.push(NativeCompilerRankedRootV1 {
                candidate: NativeRankedSourceCandidateV1::from_untrusted_parts(
                    input.semantic_root,
                    input.launch_rank,
                    &root.kernel,
                    root.source_rows.access_sources(),
                    root.source_rows.executable_effect_sources(),
                    input.ranked_ir,
                ),
                effect_receipts: input.effect_receipts,
            });
        }
        // The legacy continuation independently imports again and recompiles.
        let result = replay(&roots, budget);
        drop(roots);
        drop(self.roots);
        budget.release_storage(
            header
                .checked_add(payload)
                .and_then(|n| n.checked_add(self.storage))
                .ok_or(Resource::Arithmetic)?,
        )?;
        result
    }
}

pub(super) fn recipe_scope<T>(
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
/// Returned storage excludes discarded decoded recipes and source rows;
/// caller-owned signatures remain borrowed. Success/error/unwind restore the floor.
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
