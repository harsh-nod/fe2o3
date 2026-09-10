use super::{
    CanonicalCompilerProofInputsV3, ProductionSourceIsaKernelFamilyV1,
    refresh_original_induction_report_v4,
};

/// Genuine historical V8 lowering, with current induction replay over its original MIR.
#[allow(dead_code, reason = "shared legacy finalizer fixtures")]
pub(crate) fn historical_compiler_proof_inputs_v4(seed: u8) -> CanonicalCompilerProofInputsV3 {
    refresh_original_induction_report_v4(captured(seed, None))
}

#[allow(dead_code, reason = "shared legacy finalizer fixtures")]
pub(crate) fn historical_compiler_proof_inputs_v4_with_sourceful_induction(
    seed: u8,
) -> CanonicalCompilerProofInputsV3 {
    historical_compiler_proof_inputs_v4_with_sourceful_family(
        seed,
        ProductionSourceIsaKernelFamilyV1::Elementwise,
    )
}

#[allow(dead_code, reason = "shared legacy finalizer fixtures")]
pub(crate) fn historical_compiler_proof_inputs_v4_with_sourceful_family(
    seed: u8,
    family: ProductionSourceIsaKernelFamilyV1,
) -> CanonicalCompilerProofInputsV3 {
    refresh_original_induction_report_v4(captured(seed, Some(family)))
}

fn captured(
    seed: u8,
    family: Option<ProductionSourceIsaKernelFamilyV1>,
) -> CanonicalCompilerProofInputsV3 {
    macro_rules! bundle {
        ($case:literal) => {
            CanonicalCompilerProofInputsV3 {
                semantic_mir: include_bytes!(concat!($case, "/semantic-mir.bin")).to_vec(),
                middle_end: include_bytes!(concat!($case, "/middle-end.bin")).to_vec(),
                kernel_ir: include_bytes!(concat!($case, "/kernel-ir.bin")).to_vec(),
                correspondence: include_bytes!(concat!($case, "/correspondence.bin")).to_vec(),
                formal_memory: include_bytes!(concat!($case, "/formal-memory.bin")).to_vec(),
            }
        };
    }
    use ProductionSourceIsaKernelFamilyV1::{Elementwise, Tiled, WorkgroupCollective};
    match (seed, family) {
        (0x20, None) => bundle!("noop-20"),
        (0x40, None) => bundle!("noop-40"),
        (0x20, Some(Elementwise)) => bundle!("elementwise-20"),
        (0x21, Some(Elementwise)) => bundle!("elementwise-21"),
        (0x40, Some(Elementwise)) => bundle!("elementwise-40"),
        (0x20, Some(WorkgroupCollective)) => bundle!("collective-20"),
        (0x20, Some(Tiled)) => bundle!("tiled-20"),
        _ => panic!("uncaptured historical V4 fixture: seed={seed:#04x}, family={family:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::VerifiedCanonicalKernelIrV8;
    use fe2o3_lower_mir_kernel::InertCanonicalMirToKirCorrespondenceEvidenceV4;

    #[test]
    fn all_captures_preserve_source_kir_and_exact_induction_certificates() {
        use ProductionSourceIsaKernelFamilyV1::{Elementwise, Tiled, WorkgroupCollective};
        for (seed, family) in [
            (0x20, None),
            (0x40, None),
            (0x20, Some(Elementwise)),
            (0x21, Some(Elementwise)),
            (0x40, Some(Elementwise)),
            (0x20, Some(WorkgroupCollective)),
            (0x20, Some(Tiled)),
        ] {
            let raw = captured(seed, family);
            let current = match family {
                None => historical_compiler_proof_inputs_v4(seed),
                Some(Elementwise) => {
                    historical_compiler_proof_inputs_v4_with_sourceful_induction(seed)
                }
                Some(family) => {
                    historical_compiler_proof_inputs_v4_with_sourceful_family(seed, family)
                }
            };
            VerifiedCanonicalKernelIrV8::from_canonical_bytes_with_module(raw.kernel_ir.clone())
                .unwrap();
            assert_eq!(raw.semantic_mir(), current.semantic_mir());
            assert_eq!(raw.middle_end(), current.middle_end());
            assert_eq!(raw.kernel_ir(), current.kernel_ir());
            assert_eq!(raw.formal_memory(), current.formal_memory());

            let old = InertCanonicalMirToKirCorrespondenceEvidenceV4::decode(raw.correspondence())
                .unwrap();
            let new =
                InertCanonicalMirToKirCorrespondenceEvidenceV4::decode(current.correspondence())
                    .unwrap();
            let retained = old.semantic_u32_induction();
            let replay = new.semantic_u32_induction();
            assert_eq!(retained.certificates().is_empty(), family.is_none());
            assert_eq!(retained.certificates(), replay.certificates());
            assert_eq!(
                retained.checked_additions_examined(),
                replay.checked_additions_examined()
            );
            assert_eq!(retained.semantic_mir_sha256(), replay.semantic_mir_sha256());
            assert_eq!(retained.function(), replay.function());
            assert_eq!(retained.function_identity(), replay.function_identity());
            assert_eq!(raw.correspondence().len(), current.correspondence().len());
            let start = raw.correspondence().len() - retained.canonical_bytes().len();
            assert_eq!(
                &raw.correspondence()[..start],
                &current.correspondence()[..start]
            );
            assert_eq!(&current.correspondence()[start..], replay.canonical_bytes());
            assert!(!replay.grants_authority());

            // Refreshing a derivative cannot alter the embedded historical preimage.
            assert_eq!(
                raw.correspondence(),
                captured(seed, family).correspondence()
            );
        }
    }

    #[test]
    fn uncaptured_noop_seeds_are_rejected() {
        for seed in [0, 0x21, 0x41, 0xff] {
            assert!(
                std::panic::catch_unwind(|| historical_compiler_proof_inputs_v4(seed)).is_err()
            );
        }
    }

    #[test]
    fn uncaptured_sourceful_keys_are_rejected_without_family_fallback() {
        use ProductionSourceIsaKernelFamilyV1::{Elementwise, Tiled, WorkgroupCollective};
        for (seed, family) in [
            (0, Elementwise),
            (0x22, Elementwise),
            (0x21, WorkgroupCollective),
            (0x40, WorkgroupCollective),
            (0x21, Tiled),
            (0x40, Tiled),
        ] {
            assert!(
                std::panic::catch_unwind(|| {
                    historical_compiler_proof_inputs_v4_with_sourceful_family(seed, family)
                })
                .is_err()
            );
        }
    }
}
