use super::*;

#[allow(dead_code)]
mod compiler_proof_inputs_v3 {
    use crate as fe2o3_verifier;
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/support/compiler_proof_inputs_v3.rs"
    ));
}

#[test]
fn v4_function_binding_rejects_ordinary_singleton_and_pair_results() {
    let proof = compiler_proof_inputs_v3::canonical_compiler_proof_inputs_v4(0x20);
    let original = fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1::decode_current_production_canonical(
            proof.semantic_mir(), fe2o3_mir_model::semantic_mir_v1::SemanticMirLimitsV1::default()).unwrap();
    let (_, module) =
        fe2o3_kernel_ir::VerifiedCanonicalKernelIrV8::from_canonical_bytes_with_module(
            proof.kernel_ir().to_vec(),
        )
        .unwrap();
    let correspondence =
        InertCanonicalMirToKirCorrespondenceEvidenceV4::decode(proof.correspondence()).unwrap();
    validate_lossless_correspondence_v4(&original, &module, &correspondence).unwrap();
    for width in [1, 2] {
        let source = compiler_proof_inputs_v3::ordinary_aggregate_result_owner_v1(0x20, width);
        assert_eq!(
            source.semantic().functions()[0].abi().extern_abi(),
            fe2o3_mir_model::semantic_mir_v1::SemanticExternAbiV1::Rust
        );
        for role in [
            fe2o3_kernel_ir::FunctionRole::KernelEntry,
            fe2o3_kernel_ir::FunctionRole::InternalHelper,
        ] {
            let mut module = module.clone();
            module.functions[0].role = role;
            if role == fe2o3_kernel_ir::FunctionRole::InternalHelper {
                module.kernels.clear();
            }
            fe2o3_kernel_ir::verify_module(&module).unwrap();
            assert!(matches!(
                validate_lossless_correspondence_v4(source.semantic(), &module, &correspondence),
                Err(
                    CompilerProofInputValidationErrorV3::StructuralCorrespondence {
                        detail: "V4/V5 correspondence does not encode aggregate result components",
                    }
                )
            ));
        }
    }
}

#[test]
fn v4_parameter_envelope_rejects_singleton_projection_independently_of_rows() {
    let proof = compiler_proof_inputs_v3::canonical_compiler_proof_inputs_v4(0x20);
    let (_, module) =
        fe2o3_kernel_ir::VerifiedCanonicalKernelIrV8::from_canonical_bytes_with_module(
            proof.kernel_ir().to_vec(),
        )
        .unwrap();
    let correspondence =
        InertCanonicalMirToKirCorrespondenceEvidenceV4::decode(proof.correspondence()).unwrap();
    let source = compiler_proof_inputs_v3::ordinary_aggregate_parameter_owner_v1(0x20);
    assert!(
        fe2o3_lower_mir_kernel::legacy_correspondence_source_results_supported_v4(
            source.semantic()
        )
    );
    assert!(
        !fe2o3_lower_mir_kernel::legacy_correspondence_source_parameters_supported_v4(
            source.semantic()
        )
    );
    assert!(matches!(
        validate_lossless_correspondence_v4(source.semantic(), &module, &correspondence),
        Err(
            CompilerProofInputValidationErrorV3::StructuralCorrespondence {
                detail: "V4/V5 correspondence does not encode projected or ignored parameter components",
            }
        )
    ));
}

#[test]
fn v4_function_binding_requires_an_actual_kernel_entry() {
    let proof = compiler_proof_inputs_v3::canonical_compiler_proof_inputs_v4(0x20);
    let semantic = AdmittedInertSemanticMirV1::decode_current_production_canonical(
        proof.semantic_mir(),
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    let (_, mut module) =
        VerifiedCanonicalKernelIrV8::from_canonical_bytes_with_module(proof.kernel_ir().to_vec())
            .unwrap();
    let correspondence =
        InertCanonicalMirToKirCorrespondenceEvidenceV4::decode(proof.correspondence()).unwrap();
    validate_lossless_correspondence_v4(&semantic, &module, &correspondence).unwrap();
    module.kernels.clear();
    module.functions[0].role = fe2o3_kernel_ir::FunctionRole::InternalHelper;
    fe2o3_kernel_ir::verify_module(&module).unwrap();
    assert!(matches!(
        validate_lossless_correspondence_v4(&semantic, &module, &correspondence),
        Err(
            CompilerProofInputValidationErrorV3::StructuralCorrespondence {
                detail: "correspondence entry is not the selected source kernel body",
            }
        )
    ));
}

#[test]
fn frozen_result_envelope_preserves_exact_wrappers_and_rejects_cross_body_calls() {
    use compiler_proof_inputs_v3::{ResultWrapperMutationV1 as Mutation, wrapped_result_owner_v1};
    use fe2o3_lower_mir_kernel::{
        ProductionSemanticKirLimitsV1, ProductionSemanticKirOwnerV1,
        legacy_correspondence_source_results_supported_v4,
    };
    let source = wrapped_result_owner_v1(0x20, Mutation::None);
    let selected = source.semantic().select_kernel_body_v1().unwrap();
    assert!(selected.has_transparent_result_wrapper());
    assert!(legacy_correspondence_source_results_supported_v4(
        source.semantic()
    ));
    let owner =
        ProductionSemanticKirOwnerV1::try_lower(source, ProductionSemanticKirLimitsV1::default())
            .unwrap();
    let semantic = owner.semantic().semantic();
    let report = analyze_semantic_u32_induction_no_overflow_v1(semantic, selected.root()).unwrap();
    let v4 =
        InertCanonicalMirToKirCorrespondenceEvidenceV4::from_live_owner(&owner, &report).unwrap();
    InertCanonicalMirToKirCorrespondenceEvidenceV5::from_live_owner(&owner, &report).unwrap();
    validate_lossless_correspondence_v4(semantic, owner.module(), &v4).unwrap();
    for mutation in [
        Mutation::Computes,
        Mutation::CrossCall,
        Mutation::CrossTailCall,
    ] {
        let source = wrapped_result_owner_v1(0x20, mutation);
        if !matches!(mutation, Mutation::Computes) {
            let bodies = source
                .semantic()
                .roots()
                .iter()
                .map(|root| {
                    source
                        .semantic()
                        .select_kernel_body_for_root_v1(*root)
                        .unwrap()
                        .body()
                        .index()
                })
                .collect::<Vec<_>>();
            assert_eq!(bodies, [0, 2]);
        }
        assert!(!legacy_correspondence_source_results_supported_v4(
            source.semantic()
        ));
    }
}

#[test]
fn v4_rejects_rust_call_even_when_expansion_has_no_parameters() {
    let proof = compiler_proof_inputs_v3::canonical_compiler_proof_inputs_v4(0x20);
    let semantic = fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1::decode_current_production_canonical(
            proof.semantic_mir(), fe2o3_mir_model::semantic_mir_v1::SemanticMirLimitsV1::default(),
        ).unwrap();
    let (_, module) =
        fe2o3_kernel_ir::VerifiedCanonicalKernelIrV8::from_canonical_bytes_with_module(
            proof.kernel_ir().to_vec(),
        )
        .unwrap();
    let body = module.functions[0].body.as_ref().unwrap();
    let ordinary = &semantic.functions()[0];
    let rust_call = compiler_proof_inputs_v3::rust_call_empty_helper_v28(0x20);
    let correspondence =
        InertCanonicalMirToKirCorrespondenceEvidenceV4::decode(proof.correspondence()).unwrap();
    let bodies = BTreeMap::from([(0, body)]);
    assert!(
        validate_parameter_bindings_v4(&correspondence, &BTreeMap::from([(0, ordinary)]), &bodies)
            .is_ok()
    );
    assert!(matches!(
        validate_parameter_bindings_v4(
            &correspondence,
            &BTreeMap::from([(0, &rust_call)]),
            &bodies
        ),
        Err(
            CompilerProofInputValidationErrorV3::StructuralCorrespondence {
                detail: "V4 parameter correspondence does not encode RustCall components",
            }
        )
    ));
}
