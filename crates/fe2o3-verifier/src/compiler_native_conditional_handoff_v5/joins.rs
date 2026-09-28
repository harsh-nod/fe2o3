//! Exact outer coordinates inside the single source/F/text visit. These content
//! joins cannot establish original rustc, nominal, registration or launch origin.
use super::*;
use crate::compiler_native_conditional_source_proof_v2::final_replay::manifest::check_conditional_native_manifest_v5;
use fe2o3_amdgcn_model::ReplayedNativeV12TextDescriptorRelationV5 as Relation;
use fe2o3_compiler_lineage::{
    InertNativeNeutralSubjectV1 as Subject, NativeConditionalTargetLayoutRefV1 as Layout,
    NativeLoweringAssociationInputsV1 as Lowering, TargetLineageIdentityV3 as Coordinate,
    derive_semantic_target_layout_identity_v1,
};
use fe2o3_lower_mir_kernel::ReplayedNativeSourceV1;
use sha2::{Digest, Sha256};

pub(super) fn check(
    handoff: &Handoff,
    source: &ReplayedNativeSourceV1,
    relation: &Relation<'_, '_, '_, '_, '_>,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    let capsule = handoff.capsule();
    let native = handoff.module_handoff();
    check_conditional_native_manifest_v5(relation, native, budget)
        .map_err(|e| Error(Cause::Manifest(e)))?;
    // Pay the existing capsule getter's fixed parser/view and UTF8 visit before
    // borrowing it. No second metadata codec or copied transcript is introduced.
    const VIEW: usize = size_of::<Layout<'_>>() + size_of::<[&[u8]; 6]>() + size_of::<[usize; 8]>();
    budget.reserve_storage(VIEW)?;
    budget.charge_work(
        capsule
            .semantic_target_layout_bytes()
            .len()
            .checked_add(6 * 8 + 128)
            .ok_or(Resource::Arithmetic)?,
    )?;
    layout(
        capsule.semantic_target_layout(),
        capsule.semantic_target_layout_bytes().len(),
        source
            .source()
            .semantic_ssa()
            .source_semantic()
            .target_layout_identity()
            .as_bytes(),
        capsule.invocation().amd_target(),
        relation.profile(),
        budget,
    )?;
    budget.release_storage(VIEW)?;
    // The immutable FFI V5 decoder already compared the complete compact final
    // commitment with this exact V2 and checked invocation/module target equality.
    // Inventory/preflight intentionally remain inert preimages, not fake live facts.
    const FIXED: usize = size_of::<Account>()
        + 2 * size_of::<Lowering>()
        + size_of::<Subject>()
        + 5 * size_of::<Coordinate>()
        + size_of::<Sha256>();
    account::temporary_using(budget, FIXED, |budget| {
        budget.charge_work(1024)?;
        let f = relation.output().canonical().identity();
        let subject = Subject::new(
            *f.digest(),
            f.canonical_length(),
            *relation.catalog().digest(),
            u64::try_from(relation.catalog().canonical_bytes().len())
                .map_err(|_| Resource::Arithmetic)?,
        )
        .map_err(|e| Error(Cause::Subject(e)))?;
        let carrier = capsule.carrier_identity();
        let llvm = native.module_identity();
        let module = native.identity();
        let expected = Lowering {
            final_native: subject,
            carrier: coordinate(*carrier.sha256(), carrier.byte_len())?,
            // Both raw coordinates intentionally differ from descriptor receipt
            // domains; this is the original emitter's pre-suffix text coordinate.
            descriptor: raw(relation.descriptors().canonical_bytes(), budget)?,
            pre_descriptor_llvm: raw(relation.pre_descriptor_llvm().as_bytes(), budget)?,
            final_llvm: coordinate(*llvm.sha256(), llvm.byte_len())?,
            module_handoff: coordinate(*module.sha256(), module.byte_len())?,
            profile: relation.profile(),
        };
        lowering(capsule.native_lowering().inputs(), expected, budget)
    })
}

fn coordinate(digest: [u8; 32], length: u64) -> Result<Coordinate, Error> {
    Coordinate::new(digest, length).map_err(|e| Error(Cause::TargetLineage(e)))
}

fn raw(bytes: &[u8], budget: &mut Budget<'_>) -> Result<Coordinate, Error> {
    budget.charge_work(bytes.len().checked_add(160).ok_or(Resource::Arithmetic)?)?;
    coordinate(
        Sha256::digest(bytes).into(),
        u64::try_from(bytes.len()).map_err(|_| Resource::Arithmetic)?,
    )
}

fn lowering(actual: Lowering, expected: Lowering, budget: &mut Budget<'_>) -> Result<(), Error> {
    budget.charge_work(size_of::<Lowering>())?;
    if actual != expected {
        return Err(Error::mismatch(
            "exact conditional F/carrier/ABI/prefix/LLVM/module/profile",
        ));
    }
    Ok(())
}

fn layout(
    view: Layout<'_>,
    preimage_len: usize,
    source_layout: &[u8; 32],
    invocation_target: &str,
    profile: Profile,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    // Existing lineage builder retains its bounded allocation domain. Two
    // preimage extents cover Vec -> boxed-slice relocation until both retire.
    let scratch = sum(&[
        size_of::<Account>(),
        size_of::<Layout<'_>>(),
        size_of::<Coordinate>(),
        size_of::<Sha256>(),
        2 * size_of::<Vec<u8>>(),
        size_of::<[&[u8]; 6]>(),
        preimage_len,
        preimage_len,
    ])?;
    account::temporary_using(budget, scratch, |budget| {
        budget.charge_work(
            preimage_len
                .checked_mul(4)
                .and_then(|n| n.checked_add(invocation_target.len()))
                .and_then(|n| n.checked_add(512))
                .ok_or(Resource::Arithmetic)?,
        )?;
        if invocation_target != profile.device_target()
            || view.rustc_llvm_target != profile.rustc_target()
            || view.target_cpu != profile.cpu()
            || view.target_features != profile.rustc_features()
            || view.default_pointer_width_bits != 64
        {
            return Err(Error::mismatch(
                "exact conditional invocation/semantic layout/profile",
            ));
        }
        let identity = derive_semantic_target_layout_identity_v1(
            view.rustc_llvm_target,
            view.live_rustc_data_layout,
            view.default_pointer_width_bits,
            view.target_cpu,
            view.target_features,
        )
        .map_err(|e| Error(Cause::TargetLineage(e)))?;
        // The capsule view borrows the same already-decoded preimage. Length
        // comparison additionally pins framing to the existing canonical codec.
        if identity.byte_len() != u64::try_from(preimage_len).map_err(|_| Resource::Arithmetic)?
            || &identity.sha256() != source_layout
        {
            return Err(Error::mismatch(
                "exact reconstructed source target-layout identity",
            ));
        }
        Ok(())
    })
}

#[cfg(test)]
#[path = "joins_tests.rs"]
mod tests;
