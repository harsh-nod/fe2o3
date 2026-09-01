//! Independent validation of multi-root target-side compiler lineage.

use fe2o3_compiler_lineage::{
    DataLayoutTranscriptV3, InertProductionSemanticCapsuleV3, MultiRootTargetBindingTranscriptV2,
    MultiRootTargetWorkgroupV2, SemanticToLlvmAssociationTranscriptV3, TargetLineageIdentityV3,
    derive_semantic_target_layout_identity_v1,
};
use fe2o3_kernel_ir::Kernel;
use fe2o3_rustc_invocation::encode_descriptor_v3;
use fe2o3_verifier::ValidatedCompilerMultiRootProofInputsV1;

use crate::compiler_target_lineage_v1::{receipt_identity, require_identity_match};
use crate::{
    CompilerTargetLineageValidationErrorV1, ValidatedCompilerKirToLlvmReplayV1,
    validate_compiler_kir_to_llvm_replay_v1,
};

/// Move-only ownership of independently decoded multi-root target lineage and exact LLVM replay.
///
/// This owner establishes exact content association and deterministic replay through the
/// pre-descriptor LLVM module. It does not prove semantic refinement, LLVM-to-machine refinement,
/// producer authenticity, publication authority, or runtime safety.
///
/// ```compile_fail
/// use fe2o3_target_lineage::ValidatedCompilerMultiRootTargetLineageV1;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<ValidatedCompilerMultiRootTargetLineageV1>();
/// ```
#[derive(Debug)]
#[must_use = "dropping validated multi-root target lineage abandons exact target-side custody"]
pub struct ValidatedCompilerMultiRootTargetLineageV1 {
    target_binding: MultiRootTargetBindingTranscriptV2,
    data_layout: DataLayoutTranscriptV3,
    semantic_to_llvm: SemanticToLlvmAssociationTranscriptV3,
    replay: ValidatedCompilerKirToLlvmReplayV1,
    target_binding_receipt: TargetLineageIdentityV3,
    data_layout_receipt: TargetLineageIdentityV3,
    semantic_to_llvm_receipt: TargetLineageIdentityV3,
    final_llvm: TargetLineageIdentityV3,
    final_compiler_module_commitment: TargetLineageIdentityV3,
}

impl ValidatedCompilerMultiRootTargetLineageV1 {
    /// Returns the strictly decoded multi-root target-binding transcript.
    pub const fn target_binding(&self) -> &MultiRootTargetBindingTranscriptV2 {
        &self.target_binding
    }

    /// Returns the strictly decoded target data-layout transcript.
    pub const fn data_layout(&self) -> &DataLayoutTranscriptV3 {
        &self.data_layout
    }

    /// Returns the strictly decoded semantic-to-LLVM association transcript.
    pub const fn semantic_to_llvm(&self) -> &SemanticToLlvmAssociationTranscriptV3 {
        &self.semantic_to_llvm
    }

    /// Returns the independently replayed exact KIR-to-LLVM owner.
    pub const fn replay(&self) -> &ValidatedCompilerKirToLlvmReplayV1 {
        &self.replay
    }

    /// Returns the outer target-binding receipt coordinates checked by this owner.
    pub const fn target_binding_receipt_identity(&self) -> TargetLineageIdentityV3 {
        self.target_binding_receipt
    }

    /// Returns the outer data-layout receipt coordinates checked by this owner.
    pub const fn data_layout_receipt_identity(&self) -> TargetLineageIdentityV3 {
        self.data_layout_receipt
    }

    /// Returns the outer semantic-to-LLVM receipt coordinates checked by this owner.
    pub const fn semantic_to_llvm_receipt_identity(&self) -> TargetLineageIdentityV3 {
        self.semantic_to_llvm_receipt
    }

    /// Returns the exact final LLVM content coordinates associated by the capsule.
    pub const fn final_llvm_identity(&self) -> TargetLineageIdentityV3 {
        self.final_llvm
    }

    /// Returns the final-module commitment receipt coordinates in the association.
    pub const fn final_compiler_module_commitment_identity(&self) -> TargetLineageIdentityV3 {
        self.final_compiler_module_commitment
    }

    /// Reports that every target-side receipt and per-root workgroup was cross-bound.
    pub const fn has_exact_receipt_association(&self) -> bool {
        true
    }

    /// Reports deterministic reconstruction of target-bound KIR and pre-descriptor LLVM.
    pub const fn has_exact_kir_to_llvm_replay(&self) -> bool {
        self.replay.has_exact_target_binding_replay() && self.replay.has_exact_kir_to_llvm_replay()
    }

    /// Reports that association and replay are not a semantic-refinement proof.
    pub const fn establishes_semantic_refinement(&self) -> bool {
        false
    }

    /// Reports that this owner establishes no LLVM-to-machine refinement.
    pub const fn establishes_llvm_to_machine_refinement(&self) -> bool {
        false
    }

    /// Reports that this owner authenticates no compiler or verifier deployment.
    pub const fn authenticates_producer(&self) -> bool {
        false
    }

    /// Reports that this owner grants no publication, load, or launch authority.
    pub const fn grants_runtime_authority(&self) -> bool {
        false
    }
}

/// Independently decodes and cross-checks one multi-root capsule's complete target-side lineage.
pub fn validate_compiler_multi_root_target_lineage_v1(
    capsule: &InertProductionSemanticCapsuleV3,
    proof_inputs: &ValidatedCompilerMultiRootProofInputsV1,
) -> Result<ValidatedCompilerMultiRootTargetLineageV1, CompilerTargetLineageValidationErrorV1> {
    let receipts = capsule.receipts();
    if proof_inputs.receipt_identity() != receipts.proof_binding().identity()
        || proof_inputs.semantic_mir().canonical_encoding()
            != receipts.semantic_mir().canonical_preimage()
        || proof_inputs.kernel_ir().canonical_bytes() != receipts.kernel_ir().canonical_preimage()
    {
        return Err(CompilerTargetLineageValidationErrorV1::ProofInputMismatch);
    }

    let target_binding =
        MultiRootTargetBindingTranscriptV2::decode(receipts.target_binding().canonical_preimage())
            .map_err(CompilerTargetLineageValidationErrorV1::TargetLineage)?;
    let data_layout = DataLayoutTranscriptV3::decode(receipts.data_layout().canonical_preimage())
        .map_err(CompilerTargetLineageValidationErrorV1::TargetLineage)?;
    let semantic_to_llvm = SemanticToLlvmAssociationTranscriptV3::decode(
        receipts.semantic_to_llvm().canonical_preimage(),
    )
    .map_err(CompilerTargetLineageValidationErrorV1::TargetLineage)?;
    let replay =
        validate_compiler_kir_to_llvm_replay_v1(receipts.kernel_ir(), receipts.amdgpu_lowering())
            .map_err(CompilerTargetLineageValidationErrorV1::Replay)?;

    let invocation_bytes = encode_descriptor_v3(capsule.invocation())
        .map_err(CompilerTargetLineageValidationErrorV1::Invocation)?;
    let protected_invocation = TargetLineageIdentityV3::new(
        capsule.invocation_digest().into_bytes(),
        u64::try_from(invocation_bytes.len())
            .map_err(|_| CompilerTargetLineageValidationErrorV1::LengthOverflow)?,
    )
    .map_err(CompilerTargetLineageValidationErrorV1::TargetLineage)?;

    let data_layout_inputs = data_layout
        .inputs()
        .map_err(CompilerTargetLineageValidationErrorV1::TargetLineage)?;
    let association_inputs = semantic_to_llvm
        .inputs()
        .map_err(CompilerTargetLineageValidationErrorV1::TargetLineage)?;

    let semantic_mir = receipt_identity(
        receipts.semantic_mir().identity().sha256(),
        receipts.semantic_mir().identity().byte_len(),
    )?;
    let target_binding_receipt = receipt_identity(
        receipts.target_binding().identity().sha256(),
        receipts.target_binding().identity().byte_len(),
    )?;
    let data_layout_receipt = receipt_identity(
        receipts.data_layout().identity().sha256(),
        receipts.data_layout().identity().byte_len(),
    )?;
    let semantic_to_llvm_receipt = receipt_identity(
        receipts.semantic_to_llvm().identity().sha256(),
        receipts.semantic_to_llvm().identity().byte_len(),
    )?;

    let replay_evidence = replay.replay().evidence();
    let neutral_kir = TargetLineageIdentityV3::new(
        replay_evidence.neutral_kernel_ir_identity().sha256(),
        replay_evidence.neutral_kernel_ir_identity().byte_len(),
    )
    .map_err(CompilerTargetLineageValidationErrorV1::TargetLineage)?;
    let target_kir = TargetLineageIdentityV3::new(
        replay_evidence.target_bound_kernel_ir_identity().sha256(),
        replay_evidence.target_bound_kernel_ir_identity().byte_len(),
    )
    .map_err(CompilerTargetLineageValidationErrorV1::TargetLineage)?;
    let configured_target = capsule.target().to_string();
    for (matches, field) in [
        (
            target_binding.protected_rustc_invocation() == protected_invocation,
            "protected rustc invocation",
        ),
        (
            target_binding.semantic_mir() == semantic_mir,
            "semantic MIR",
        ),
        (
            target_binding.target_neutral_kir() == neutral_kir,
            "target-neutral Kernel IR",
        ),
        (
            target_binding.target_bound_kir() == target_kir,
            "target-bound Kernel IR",
        ),
        (
            target_binding.configured_target() == configured_target,
            "configured target",
        ),
        (
            replay_evidence.profile().device_target() == configured_target,
            "replayed target profile",
        ),
        (
            target_binding.roster_identity() == proof_inputs.middle_end_roster().roster_identity(),
            "compiler roster",
        ),
        (
            target_binding.root_count() == proof_inputs.roots().len(),
            "target root count",
        ),
    ] {
        require_identity_match(matches, field)?;
    }
    let target_bound_module = replay.replay().target_bound_module();
    require_identity_match(
        target_bound_module.kernels.len() == proof_inputs.roots().len(),
        "replayed target-bound root count",
    )?;
    for (index, root) in proof_inputs.roots().iter().enumerate() {
        let target = target_binding.workgroup(index).ok_or(
            CompilerTargetLineageValidationErrorV1::IdentityMismatch {
                field: "target workgroup root",
            },
        )?;
        let kernel = target_bound_module.kernels.get(index).ok_or(
            CompilerTargetLineageValidationErrorV1::IdentityMismatch {
                field: "replayed target-bound root",
            },
        )?;
        require_exact_multi_root_target_v1(
            target,
            kernel,
            root.kernel_id(),
            root.source_rank(),
            root.workgroup(),
        )?;
    }

    let semantic_layout = derive_semantic_target_layout_identity_v1(
        target_binding.rustc_llvm_target(),
        data_layout_inputs.live_rustc_data_layout,
        data_layout_inputs.default_pointer_width_bits,
        target_binding.target_cpu(),
        target_binding.target_features(),
    )
    .map_err(CompilerTargetLineageValidationErrorV1::TargetLineage)?;
    for (matches, field) in [
        (
            data_layout_inputs.semantic_mir == semantic_mir,
            "data-layout semantic MIR",
        ),
        (
            data_layout_inputs.target_binding == target_binding_receipt,
            "data-layout target binding",
        ),
        (
            data_layout_inputs.semantic_layout == semantic_layout,
            "semantic target layout",
        ),
        (
            proof_inputs
                .semantic_mir()
                .target_layout_identity()
                .as_bytes()
                == &semantic_layout.sha256(),
            "semantic-MIR target layout",
        ),
    ] {
        require_identity_match(matches, field)?;
    }

    let expected_associations = [
        (
            semantic_mir,
            association_inputs.semantic_mir,
            "semantic MIR",
        ),
        (
            receipt_identity(
                receipts.middle_end().identity().sha256(),
                receipts.middle_end().identity().byte_len(),
            )?,
            association_inputs.middle_end,
            "middle end",
        ),
        (
            receipt_identity(
                receipts.kernel_ir().identity().sha256(),
                receipts.kernel_ir().identity().byte_len(),
            )?,
            association_inputs.kernel_ir,
            "Kernel IR",
        ),
        (
            receipt_identity(
                receipts.mir_to_kir_correspondence().identity().sha256(),
                receipts.mir_to_kir_correspondence().identity().byte_len(),
            )?,
            association_inputs.mir_to_kir_correspondence,
            "MIR-to-KIR correspondence",
        ),
        (
            receipt_identity(
                receipts.formal_memory().identity().sha256(),
                receipts.formal_memory().identity().byte_len(),
            )?,
            association_inputs.formal_memory,
            "formal memory",
        ),
        (
            receipt_identity(
                receipts.proof_binding().identity().sha256(),
                receipts.proof_binding().identity().byte_len(),
            )?,
            association_inputs.proof_binding,
            "proof binding",
        ),
        (
            target_binding_receipt,
            association_inputs.target_binding,
            "target binding",
        ),
        (
            data_layout_receipt,
            association_inputs.data_layout,
            "data layout",
        ),
        (
            receipt_identity(
                receipts.abi().identity().sha256(),
                receipts.abi().identity().byte_len(),
            )?,
            association_inputs.abi,
            "ABI",
        ),
        (
            receipt_identity(
                receipts.export_manifest().identity().sha256(),
                receipts.export_manifest().identity().byte_len(),
            )?,
            association_inputs.export_manifest,
            "export manifest",
        ),
        (
            receipt_identity(
                receipts.amdgpu_lowering().identity().sha256(),
                receipts.amdgpu_lowering().identity().byte_len(),
            )?,
            association_inputs.amdgpu_lowering,
            "AMDGPU lowering",
        ),
        (
            receipt_identity(
                receipts
                    .final_compiler_module_commitment()
                    .identity()
                    .sha256(),
                receipts
                    .final_compiler_module_commitment()
                    .identity()
                    .byte_len(),
            )?,
            association_inputs.final_compiler_module_commitment,
            "final compiler module commitment",
        ),
    ];
    for (actual, associated, field) in expected_associations {
        require_identity_match(actual == associated, field)?;
    }

    Ok(ValidatedCompilerMultiRootTargetLineageV1 {
        target_binding,
        data_layout,
        semantic_to_llvm,
        replay,
        target_binding_receipt,
        data_layout_receipt,
        semantic_to_llvm_receipt,
        final_llvm: association_inputs.final_llvm,
        final_compiler_module_commitment: association_inputs.final_compiler_module_commitment,
    })
}

fn require_exact_multi_root_target_v1(
    target: MultiRootTargetWorkgroupV2<'_>,
    kernel: &Kernel,
    expected_kernel: &str,
    expected_source_rank: u8,
    expected_workgroup: [u32; 3],
) -> Result<(), CompilerTargetLineageValidationErrorV1> {
    let workgroup =
        kernel
            .workgroup_size
            .ok_or(CompilerTargetLineageValidationErrorV1::IdentityMismatch {
                field: "replayed target-bound workgroup",
            })?;
    require_identity_match(
        target.kernel() == expected_kernel
            && target.workgroup() == expected_workgroup
            && kernel.id.as_str() == expected_kernel
            && kernel.entry.as_str() == expected_kernel
            && kernel.domain.rank() == expected_source_rank
            && [workgroup.x, workgroup.y, workgroup.z] == target.workgroup(),
        "per-root target workgroup",
    )
}

#[cfg(test)]
mod tests {
    use fe2o3_compiler_lineage::{MultiRootTargetBindingInputsV2, MultiRootTargetWorkgroupInputV2};
    use fe2o3_kernel_ir::{LaunchDomain, LaunchExtent, WorkgroupSize};

    use super::*;

    fn identity(seed: u8, byte_len: u64) -> TargetLineageIdentityV3 {
        TargetLineageIdentityV3::new([seed; 32], byte_len).unwrap()
    }

    fn target_binding() -> MultiRootTargetBindingTranscriptV2 {
        let workgroups = [
            MultiRootTargetWorkgroupInputV2 {
                kernel: "alpha_kernel",
                workgroup: [64, 1, 1],
            },
            MultiRootTargetWorkgroupInputV2 {
                kernel: "zeta_kernel",
                workgroup: [8, 4, 1],
            },
        ];
        MultiRootTargetBindingTranscriptV2::new(MultiRootTargetBindingInputsV2 {
            protected_rustc_invocation: identity(1, 101),
            semantic_mir: identity(2, 202),
            target_neutral_kir: identity(3, 303),
            target_bound_kir: identity(4, 404),
            configured_target: "gfx942:xnack-",
            rustc_llvm_target: "amdgcn-amd-amdhsa",
            target_cpu: "gfx942",
            target_features: "-wavefrontsize32,+wavefrontsize64,-xnack",
            roster_identity: [5; 32],
            code_object_version: 6,
            wave_width_bits: 64,
            workgroups: &workgroups,
        })
        .unwrap()
    }

    fn target_kernel(name: &str, rank: u8, workgroup: [u32; 3]) -> Kernel {
        let domain = match rank {
            1 => LaunchDomain::D1 {
                x: LaunchExtent::Static(1),
            },
            2 => LaunchDomain::D2 {
                x: LaunchExtent::Static(1),
                y: LaunchExtent::Static(1),
            },
            3 => LaunchDomain::D3 {
                x: LaunchExtent::Static(1),
                y: LaunchExtent::Static(1),
                z: LaunchExtent::Static(1),
            },
            _ => panic!("unsupported test rank"),
        };
        let mut kernel = Kernel::new(name, name, domain);
        kernel.workgroup_size = Some(WorkgroupSize::new(workgroup[0], workgroup[1], workgroup[2]));
        kernel
    }

    #[test]
    fn exact_root_coordinate_join_is_accepted() {
        let binding = target_binding();
        let target = binding.workgroup(1).unwrap();
        let kernel = target_kernel("zeta_kernel", 2, [8, 4, 1]);

        require_exact_multi_root_target_v1(target, &kernel, "zeta_kernel", 2, [8, 4, 1]).unwrap();
    }

    #[test]
    fn root_join_rejects_each_cross_spliced_axis() {
        let binding = target_binding();
        let target = binding.workgroup(1).unwrap();
        let exact_kernel = target_kernel("zeta_kernel", 2, [8, 4, 1]);
        let wrong_rank = target_kernel("zeta_kernel", 1, [8, 4, 1]);
        let wrong_kernel = target_kernel("alpha_kernel", 2, [8, 4, 1]);
        let mut wrong_entry = target_kernel("zeta_kernel", 2, [8, 4, 1]);
        wrong_entry.entry = "alpha_kernel".into();
        let wrong_replayed_workgroup = target_kernel("zeta_kernel", 2, [16, 2, 1]);

        for (axis, result) in [
            (
                "proof root order",
                require_exact_multi_root_target_v1(
                    target,
                    &exact_kernel,
                    "alpha_kernel",
                    2,
                    [8, 4, 1],
                ),
            ),
            (
                "replayed kernel",
                require_exact_multi_root_target_v1(
                    target,
                    &wrong_kernel,
                    "zeta_kernel",
                    2,
                    [8, 4, 1],
                ),
            ),
            (
                "source rank",
                require_exact_multi_root_target_v1(
                    target,
                    &wrong_rank,
                    "zeta_kernel",
                    2,
                    [8, 4, 1],
                ),
            ),
            (
                "replayed entry",
                require_exact_multi_root_target_v1(
                    target,
                    &wrong_entry,
                    "zeta_kernel",
                    2,
                    [8, 4, 1],
                ),
            ),
            (
                "proof workgroup",
                require_exact_multi_root_target_v1(
                    target,
                    &exact_kernel,
                    "zeta_kernel",
                    2,
                    [16, 2, 1],
                ),
            ),
            (
                "replayed workgroup",
                require_exact_multi_root_target_v1(
                    target,
                    &wrong_replayed_workgroup,
                    "zeta_kernel",
                    2,
                    [8, 4, 1],
                ),
            ),
        ] {
            assert!(
                matches!(
                    result,
                    Err(CompilerTargetLineageValidationErrorV1::IdentityMismatch {
                        field: "per-root target workgroup"
                    })
                ),
                "accepted cross-spliced {axis}"
            );
        }
    }

    #[test]
    fn root_join_rejects_absent_replayed_workgroup() {
        let binding = target_binding();
        let target = binding.workgroup(1).unwrap();
        let mut kernel = target_kernel("zeta_kernel", 2, [8, 4, 1]);
        kernel.workgroup_size = None;

        assert!(matches!(
            require_exact_multi_root_target_v1(target, &kernel, "zeta_kernel", 2, [8, 4, 1]),
            Err(CompilerTargetLineageValidationErrorV1::IdentityMismatch {
                field: "replayed target-bound workgroup"
            })
        ));
    }
}
