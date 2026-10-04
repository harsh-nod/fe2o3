//! Joins while source/history/text-relation borrows are still live.
use super::*;
use fe2o3_amd_target::PRODUCTION_AMDHSA_LLVM22_WORKER_DATA_LAYOUT_V1;
use fe2o3_amdgcn_model::ReplayedNativeV12TextDescriptorRelationV1 as Relation;
use fe2o3_compiler_lineage::{
    DataLayoutTranscriptV3, InertNativeLoweringAssociationV1,
    MAX_PRODUCTION_TARGET_LINEAGE_TRANSCRIPT_BYTES_V3, MultiRootTargetBindingTranscriptV3,
    NATIVE_LOWERING_ASSOCIATION_WORK_V1, SemanticToLlvmAssociationTranscriptV3,
    TargetLineageIdentityV3 as Coordinate, derive_semantic_target_layout_identity_v1,
};
use fe2o3_rustc_invocation::{
    MAX_DESCRIPTOR_BYTES_V3, RustcInvocationDescriptorV3, encode_descriptor_v3,
};
use sha2::{Digest, Sha256};

fn coordinate(digest: [u8; 32], length: u64) -> R<Coordinate> {
    Coordinate::new(digest, length).map_err(E::TargetLineage)
}

macro_rules! identity {
    ($receipt:expr) => {{
        let id = $receipt.identity();
        coordinate(*id.sha256(), id.byte_len())?
    }};
}

// These codecs retain their existing bounded allocation domain. Allowances for
// coexisting target/layout records accumulate until their enclosing scope ends.
fn transcript<T>(budget: &mut Budget<'_>) -> R<()> {
    codec::<T>(MAX_PRODUCTION_TARGET_LINEAGE_TRANSCRIPT_BYTES_V3, budget)
}

impl Join<'_> {
    pub(in crate::compiler_refined_forwarding_output_v1) fn check(
        &self,
        frame: &Frame<'_>,
        history: &History<'_, '_>,
        inputs: &Inputs<'_>,
        descriptor: &Descriptor,
        relation: &Relation<'_, '_, '_, '_>,
        budget: &mut Budget<'_>,
    ) -> R<()> {
        scoped(budget, |budget| {
            let receipts = self.handoff.capsule().base().receipts();
            budget.reserve_storage(size_of::<[(&[u8], &[u8], &str); 8]>())?;
            for (actual, expected, name) in [
                (
                    receipts.semantic_mir().canonical_preimage(),
                    frame.field(Field::SemanticMir),
                    "V4 semantic MIR",
                ),
                (
                    receipts.middle_end().canonical_preimage(),
                    frame.field(Field::OriginalMiddleEnd),
                    "V4 original middle end",
                ),
                (
                    receipts.kernel_ir().canonical_preimage(),
                    frame.field(Field::OriginalNative),
                    "V4 original N/catalog",
                ),
                (
                    receipts.mir_to_kir_correspondence().canonical_preimage(),
                    frame.field(Field::OriginalCorrespondence),
                    "V4 original correspondence",
                ),
                (
                    receipts.formal_memory().canonical_preimage(),
                    frame.field(Field::OriginalFormalMemory),
                    "V4 original formal memory",
                ),
                (
                    receipts.proof_binding().canonical_preimage(),
                    frame.field(Field::OriginalInputV4),
                    "V4 original proof binding",
                ),
                (
                    receipts.abi().canonical_preimage(),
                    descriptor.canonical_bytes(),
                    "V4 descriptor ABI",
                ),
                (
                    receipts.export_manifest().canonical_preimage(),
                    self.handoff
                        .module_handoff()
                        .symbol_manifest()
                        .canonical_bytes(),
                    "V4 exact export manifest",
                ),
            ] {
                bytes(actual, expected, name, budget)?;
            }
            self.target_and_layout(history, inputs, relation.profile(), budget)?;
            self.lowering(descriptor, relation, budget)?;
            self.association(budget)
        })
    }

    fn target_and_layout(
        &self,
        history: &History<'_, '_>,
        inputs: &Inputs<'_>,
        profile: Profile,
        budget: &mut Budget<'_>,
    ) -> R<()> {
        scoped(budget, |budget| {
            let base = self.handoff.capsule().base();
            let receipts = base.receipts();
            transcript::<MultiRootTargetBindingTranscriptV3>(budget)?;
            let target = MultiRootTargetBindingTranscriptV3::decode(
                receipts.target_binding().canonical_preimage(),
            )
            .map_err(E::TargetLineage)?;
            codec::<RustcInvocationDescriptorV3>(4 * MAX_DESCRIPTOR_BYTES_V3, budget)?;
            let invocation = encode_descriptor_v3(base.invocation()).map_err(E::Invocation)?;
            budget.charge_work(
                target
                    .canonical_bytes()
                    .len()
                    .checked_add(2048)
                    .ok_or(Resource::Arithmetic)?,
            )?;
            let bound = history.graph(Role::B).canonical().identity();
            if target.protected_rustc_invocation()
                != coordinate(
                    base.invocation_digest().into_bytes(),
                    invocation.len() as u64,
                )?
                || target.semantic_mir() != identity!(receipts.semantic_mir())
                || target.native_neutral_subject()
                    != &super::super::joins::subject(inputs.original, inputs.catalog)?
                || target.target_bound_kir()
                    != coordinate(*bound.digest(), bound.canonical_length())?
                || target.configured_target() != profile.device_target()
                || target.rustc_llvm_target() != profile.rustc_target()
                || target.target_cpu() != profile.cpu()
                || target.target_features() != profile.rustc_features()
                || target.code_object_version() != 6
                || target.wave_width_bits() != 64
                || target.roster_identity() != inputs.middle.roster_identity()
                || target.root_count() != inputs.middle.root_count()
                || base.invocation().amd_target() != profile.device_target()
            {
                return Err(E::Mismatch("V4 exact N/B target coordinates"));
            }
            for ordinal in 0..target.root_count() {
                budget.charge_work(
                    inputs
                        .middle
                        .canonical_bytes()
                        .len()
                        .checked_add(history.graph(Role::B).canonical().canonical_bytes().len())
                        .and_then(|n| n.checked_add(target.canonical_bytes().len()))
                        .and_then(|n| n.checked_add(256))
                        .ok_or(Resource::Arithmetic)?,
                )?;
                let row = target
                    .workgroup(ordinal)
                    .ok_or(E::Mismatch("V4 target row"))?;
                let signed = inputs
                    .middle
                    .root(ordinal)
                    .ok_or(E::Mismatch("V4 signed target row"))?;
                let bound = history
                    .graph(Role::B)
                    .module()
                    .kernels
                    .iter()
                    .find(|kernel| kernel.id.as_str() == signed.export_symbol())
                    .ok_or(E::Mismatch("V4 bound target root"))?;
                if row.kernel() != signed.export_symbol()
                    || row.workgroup() != signed.workgroup()
                    || bound.workgroup_size.map(|w| [w.x, w.y, w.z]) != Some(row.workgroup())
                    || inputs
                        .launch
                        .roots()
                        .get(ordinal)
                        .and_then(|r| r.source_launch().exact_workgroup())
                        != Some(row.workgroup())
                {
                    return Err(E::Mismatch("V4 semantic-order target workgroups"));
                }
            }
            transcript::<DataLayoutTranscriptV3>(budget)?;
            let layout =
                DataLayoutTranscriptV3::decode(receipts.data_layout().canonical_preimage())
                    .map_err(E::TargetLineage)?;
            budget.charge_work(
                layout
                    .canonical_bytes()
                    .len()
                    .checked_add(512)
                    .ok_or(Resource::Arithmetic)?,
            )?;
            let data = layout.inputs().map_err(E::TargetLineage)?;
            transcript::<Coordinate>(budget)?;
            let semantic_layout = derive_semantic_target_layout_identity_v1(
                target.rustc_llvm_target(),
                data.live_rustc_data_layout,
                data.default_pointer_width_bits,
                target.target_cpu(),
                target.target_features(),
            )
            .map_err(E::TargetLineage)?;
            // The independent native text relation has already checked the
            // actual LLVM22 header, not merely a claimed layout string.
            if data.semantic_mir != identity!(receipts.semantic_mir())
                || data.target_binding != identity!(receipts.target_binding())
                || data.semantic_layout != semantic_layout
                || inputs.semantic.target_layout_identity().as_bytes() != &semantic_layout.sha256()
                || data.rustc_llvm_target != target.rustc_llvm_target()
                || data.final_llvm_data_layout != PRODUCTION_AMDHSA_LLVM22_WORKER_DATA_LAYOUT_V1
            {
                return Err(E::Mismatch("V4 exact semantic/LLVM22 layout"));
            }
            Ok(())
        })
    }

    fn lowering(
        &self,
        descriptor: &Descriptor,
        relation: &Relation<'_, '_, '_, '_>,
        budget: &mut Budget<'_>,
    ) -> R<()> {
        scoped(budget, |budget| {
            budget.reserve_storage(size_of::<InertNativeLoweringAssociationV1>() + HASH_STORAGE)?;
            budget.charge_work(NATIVE_LOWERING_ASSOCIATION_WORK_V1)?;
            let lowering = InertNativeLoweringAssociationV1::decode(
                self.handoff
                    .capsule()
                    .base()
                    .receipts()
                    .amdgpu_lowering()
                    .canonical_preimage(),
            )
            .map_err(E::NativeLowering)?;
            budget.charge_work(
                relation
                    .pre_descriptor_llvm()
                    .len()
                    .checked_add(1024)
                    .ok_or(Resource::Arithmetic)?,
            )?;
            let actual = lowering.inputs();
            let native = self.handoff.module_handoff();
            if actual.final_native
                != super::super::joins::subject(relation.output(), relation.catalog())?
                || actual.carrier != coordinate(*self.carrier.sha256(), self.carrier.byte_len())?
                || actual.descriptor != identity!(descriptor)
                || actual.pre_descriptor_llvm
                    != coordinate(
                        Sha256::digest(relation.pre_descriptor_llvm().as_bytes()).into(),
                        relation.pre_descriptor_llvm().len() as u64,
                    )?
                || actual.final_llvm
                    != coordinate(
                        *native.module_identity().sha256(),
                        native.module_identity().byte_len(),
                    )?
                || actual.module_handoff != identity!(native)
                || actual.profile != relation.profile()
            {
                return Err(E::Mismatch("V4 exact final-F native lowering"));
            }
            Ok(())
        })
    }

    fn association(&self, budget: &mut Budget<'_>) -> R<()> {
        scoped(budget, |budget| {
            let receipts = self.handoff.capsule().base().receipts();
            transcript::<SemanticToLlvmAssociationTranscriptV3>(budget)?;
            let transcript = SemanticToLlvmAssociationTranscriptV3::decode(
                receipts.semantic_to_llvm().canonical_preimage(),
            )
            .map_err(E::TargetLineage)?;
            budget.charge_work(
                transcript
                    .canonical_bytes()
                    .len()
                    .checked_add(2048)
                    .ok_or(Resource::Arithmetic)?,
            )?;
            budget.reserve_storage(size_of::<[(Coordinate, Coordinate); 13]>())?;
            let actual = transcript.inputs().map_err(E::TargetLineage)?;
            let native = self.handoff.module_handoff();
            // The immutable outer decoder already compared the full compact
            // final commitment against this exact V2. Bind its receipt here.
            for (actual, expected) in [
                (actual.semantic_mir, identity!(receipts.semantic_mir())),
                (actual.middle_end, identity!(receipts.middle_end())),
                (actual.kernel_ir, identity!(receipts.kernel_ir())),
                (
                    actual.mir_to_kir_correspondence,
                    identity!(receipts.mir_to_kir_correspondence()),
                ),
                (actual.formal_memory, identity!(receipts.formal_memory())),
                (actual.proof_binding, identity!(receipts.proof_binding())),
                (actual.target_binding, identity!(receipts.target_binding())),
                (actual.data_layout, identity!(receipts.data_layout())),
                (actual.abi, identity!(receipts.abi())),
                (
                    actual.export_manifest,
                    identity!(receipts.export_manifest()),
                ),
                (
                    actual.amdgpu_lowering,
                    identity!(receipts.amdgpu_lowering()),
                ),
                (
                    actual.final_llvm,
                    coordinate(
                        *native.module_identity().sha256(),
                        native.module_identity().byte_len(),
                    )?,
                ),
                (
                    actual.final_compiler_module_commitment,
                    identity!(receipts.final_compiler_module_commitment()),
                ),
            ] {
                if actual != expected {
                    return Err(E::Mismatch("V4 exact semantic-to-LLVM axis"));
                }
            }
            Ok(())
        })
    }
}
