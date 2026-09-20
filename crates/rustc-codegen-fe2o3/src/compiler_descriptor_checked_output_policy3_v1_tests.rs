use super::*;
use crate::production_ranked_projection_v1::with_backend_checked_output_policy3_v1;
use fe2o3_artifacts::{
    BlockSize, Dimensions, PointerWidth, RustPhysicalComponentKindV1, RustPhysicalComponentV1,
    RustScalarElementTypeV1, RustSourceTypeShapeV1, RustTypeEvidenceV1,
};
use fe2o3_compiler_ffi::{
    CompilerFfiContractV1, CompilerFfiEnvelopeBuilderV1, CompilerFfiLinkRoleV1,
    CompilerFfiSourceOwnerV1, DeviceTargetV1,
};
use fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdV1;
use reserved_fe2o3_symbols::{
    DeviceFfiContractFieldsV1, DeviceFfiDirectionV1, derive_device_ffi_contract_id_v1,
};

#[path = "production_pipeline_checked_output_policy4_v1_tests.rs"]
mod native_handoff_tests;

#[path = "production_pipeline_erased_output_v1_tests.rs"]
mod erased_native_handoff_tests;
#[path = "production_native_checked_output_handoff_v1_tests.rs"]
mod native_output_binding_tests;
#[path = "production_pipeline_checked_output_policy5_v1_tests.rs"]
mod policy5_native_handoff_tests;
#[path = "production_pipeline_checked_output_policy6_v1_tests.rs"]
mod policy6_native_handoff_tests;
#[path = "production_pipeline_checked_output_policy7_v1_tests.rs"]
mod policy7_native_handoff_tests;
#[path = "production_policy8_native_k_v1_tests.rs"]
mod policy8_native_handoff_tests;

fn typed_roots(owner: &ProductionCheckedOutputOwnerPolicy3V1) -> Vec<TypedDescriptorRootV1> {
    typed_roots_for_source(owner.source_semantic_kir())
}

fn typed_roots_for_source(
    source: &fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1,
) -> Vec<TypedDescriptorRootV1> {
    let semantic = source.semantic().semantic();
    semantic
        .roots()
        .iter()
        .enumerate()
        .map(|(i, id)| {
            let function = &semantic.functions()[id.index() as usize];
            let entry = function.kernel_entry().unwrap();
            let layout = RustLayoutEvidenceV1::new(
                RustTypeEvidenceV1::new(RustSourceTypeShapeV1::scalar(
                    RustScalarElementTypeV1::U32,
                )),
                RustcAbiClassV1::Scalar,
                PointerWidth::Bits64,
                4,
                4,
                vec![
                    RustPhysicalComponentV1::new(
                        0,
                        4,
                        4,
                        RustPhysicalComponentKindV1::Scalar {
                            scalar: RustScalarElementTypeV1::U32,
                        },
                    )
                    .unwrap(),
                ],
            )
            .unwrap();
            let argument = TypedDescriptorArgumentV1 {
                name: "value".to_owned(),
                kind: DescriptorArgumentKindV1::Scalar(ScalarTypeV1::U32),
                access: AccessMode::ByValue,
                offset: 0,
                layout: Some(layout),
                source_size: 4,
                source_alignment: 4,
                rustc_abi_class: RustcAbiClassV1::Scalar,
                semantic_type_identity: semantic.types()
                    [SemanticTypeIdV1::from_index(1).index() as usize]
                    .identity(),
            };
            TypedDescriptorRootV1 {
                logical_name: format!("logical_{i}"),
                export_name: String::from_utf8(entry.export_symbol().as_bytes().to_vec()).unwrap(),
                kernel_binding: KernelBindingIdV1::from_bytes(
                    *entry.kernel_binding_identity().as_bytes(),
                ),
                arguments: TypedArgumentListV1::new(vec![argument]).unwrap(),
                explicit_argument_bytes: 4,
                kernarg_alignment_bytes: 8,
                source_launch: Some(
                    LaunchContract::new(
                        1,
                        BlockSize::Exact(Dimensions::new(64, 1, 1).unwrap()),
                        Dimensions::new(3, 1, 1).unwrap(),
                        0,
                        0,
                    )
                    .unwrap(),
                ),
            }
        })
        .collect()
}

fn envelope(
    profile: ProductionAmdTargetProfileV1,
    version: CodeObjectVersion,
) -> CompilerFfiEnvelopeV1 {
    // Descriptor fixtures use the existing inert envelope contract. This
    // independent import is not a claim of an admitted linked module closure.
    let abi = "C(mut_ptr<global,u32>[size=8,align=8,as=global])->unit[size=0,align=1]";
    let target = DeviceTargetV1::parse(profile.device_target()).unwrap();
    let semantic_identity = [0x55; 32];
    let version_number = match version {
        CodeObjectVersion::V4 => 4,
        CodeObjectVersion::V5 => 5,
        CodeObjectVersion::V6 => 6,
    };
    let id = derive_device_ffi_contract_id_v1(DeviceFfiContractFieldsV1 {
        direction: DeviceFfiDirectionV1::Import.tag(),
        symbol: "external_test",
        calling_convention: "C",
        code_object_version: version_number,
        target: profile.device_target(),
        physical_abi: abi,
        effects: "read_global",
        semantic_identity: &"55".repeat(32),
    });
    let contract = CompilerFfiContractV1::new(
        id,
        DeviceFfiDirectionV1::Import,
        CompilerFfiLinkRoleV1::RequiresExternalDefinition,
        target,
        version,
        CompilerFfiSourceOwnerV1::new(
            "descriptor_test",
            "descriptor_test::external_test",
            [0x44; 16],
            "_RNvCs1234_descriptor_test13external_test",
        )
        .unwrap(),
        "external_test",
        abi,
        "read_global",
        semantic_identity,
    )
    .unwrap();
    let mut builder = CompilerFfiEnvelopeBuilderV1::new(target, version, 1).unwrap();
    builder.push(contract).unwrap();
    builder.finish().unwrap()
}

fn llvm(
    owner: &ProductionCheckedOutputOwnerPolicy3V1,
    profile: ProductionAmdTargetProfileV1,
) -> InertCompilerModuleTextV1 {
    let text = match profile {
        ProductionAmdTargetProfileV1::Gfx942 => dialect_amdgcn::lower_canonical_v12_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(owner.output()),
        ProductionAmdTargetProfileV1::Gfx950 => dialect_amdgcn::lower_canonical_v12_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1(owner.output()),
    }.unwrap();
    crate::kernel_ir_codegen::retain_production_compiler_module_text_v1(
        owner.output().module(),
        text,
    )
    .unwrap()
}

#[test]
fn exact_output_descriptor_uses_real_two_root_ranked_policy3_custody_on_both_targets() {
    for profile in [
        ProductionAmdTargetProfileV1::Gfx942,
        ProductionAmdTargetProfileV1::Gfx950,
    ] {
        with_backend_checked_output_policy3_v1(profile, |owner, budget| {
            assert!(std::ptr::eq(owner.output(), owner.checked_output().owner()));
            assert!(!std::ptr::eq(
                owner.output(),
                owner.source_semantic_kir().pre_ranked_executable().unwrap()
            ));
            assert_eq!(owner.kernels().len(), 2);
            assert!(!owner.grants_artifact_or_launch_authority());
            let roots = typed_roots(owner);
            let llvm = llvm(owner, profile);
            let envelope = envelope(profile, CodeObjectVersion::V6);
            let floor = budget.storage();
            let first = construct_checked_output_policy3_descriptor_source_v1(
                &envelope, &llvm, &roots, owner, budget,
            )
            .unwrap();
            let second = construct_checked_output_policy3_descriptor_source_v1(
                &envelope, &llvm, &roots, owner, budget,
            )
            .unwrap();
            assert_eq!(first.canonical_bytes(), second.canonical_bytes());
            assert_eq!(first.table().kernels().len(), 2);
            assert!(!first.authenticates_compiler_origin());
            assert!(!first.grants_link_authority());
            assert!(!first.grants_load_authority());
            assert!(!first.grants_launch_authority());
            assert_eq!(budget.storage(), floor);
        });
    }
}

#[test]
fn exact_output_descriptor_rejects_roster_identity_and_scalar_abi_substitutions() {
    let profile = ProductionAmdTargetProfileV1::Gfx942;
    with_backend_checked_output_policy3_v1(profile, |owner, budget| {
        let originals = typed_roots(owner);
        let llvm = llvm(owner, profile);
        let envelope = envelope(profile, CodeObjectVersion::V6);
        let floor = budget.storage();
        for case in 0..8 {
            let mut roots = originals.clone();
            match case {
                0 => roots.clear(),
                1 => {
                    roots.pop();
                }
                2 => roots.swap(0, 1),
                3 => roots[1].kernel_binding = roots[0].kernel_binding,
                4 => roots[1].export_name = roots[0].export_name.clone(),
                5..=7 => {
                    let mut arguments = roots[1].arguments.as_slice().to_vec();
                    match case {
                        5 => {
                            arguments[0].semantic_type_identity =
                                SemanticTypeIdentityV1::from_sha256([99; 32])
                        }
                        6 => arguments[0].source_size = 8,
                        _ => {
                            arguments[0].kind = DescriptorArgumentKindV1::Scalar(ScalarTypeV1::F32)
                        }
                    }
                    roots[1].arguments = TypedArgumentListV1::new(arguments).unwrap();
                }
                _ => unreachable!(),
            }
            assert!(
                construct_checked_output_policy3_descriptor_source_v1(
                    &envelope, &llvm, &roots, owner, budget,
                )
                .is_err(),
                "accepted substitution {case}"
            );
            assert_eq!(budget.storage(), floor);
        }
    });
}

#[test]
fn exact_output_descriptor_rejects_changed_launch_custody_and_target() {
    let profile = ProductionAmdTargetProfileV1::Gfx942;
    with_backend_checked_output_policy3_v1(profile, |owner, budget| {
        let originals = typed_roots(owner);
        let llvm = llvm(owner, profile);
        let exact = envelope(profile, CodeObjectVersion::V6);
        let floor = budget.storage();
        for launch in [
            None,
            Some(
                LaunchContract::new(
                    1,
                    BlockSize::Exact(Dimensions::new(32, 1, 1).unwrap()),
                    Dimensions::new(3, 1, 1).unwrap(),
                    0,
                    0,
                )
                .unwrap(),
            ),
            Some(
                LaunchContract::new(
                    1,
                    BlockSize::Exact(Dimensions::new(64, 1, 1).unwrap()),
                    Dimensions::new(4, 1, 1).unwrap(),
                    0,
                    0,
                )
                .unwrap(),
            ),
            Some(
                LaunchContract::new(
                    2,
                    BlockSize::Exact(Dimensions::new(64, 1, 1).unwrap()),
                    Dimensions::new(3, 1, 1).unwrap(),
                    0,
                    0,
                )
                .unwrap(),
            ),
        ] {
            let mut roots = originals.clone();
            roots[1].source_launch = launch;
            assert!(
                construct_checked_output_policy3_descriptor_source_v1(
                    &exact, &llvm, &roots, owner, budget,
                )
                .is_err()
            );
            assert_eq!(budget.storage(), floor);
        }
        assert!(matches!(
            construct_checked_output_policy3_descriptor_source_v1(
                &envelope(ProductionAmdTargetProfileV1::Gfx950, CodeObjectVersion::V6),
                &llvm,
                &originals,
                owner,
                budget,
            ),
            Err(CompilerDescriptorError::CheckedOutputTarget(_))
        ));
        assert!(matches!(
            construct_checked_output_policy3_descriptor_source_v1(
                &envelope(profile, CodeObjectVersion::V5),
                &llvm,
                &originals,
                owner,
                budget,
            ),
            Err(CompilerDescriptorError::UnsupportedCodeObjectVersion(
                CodeObjectVersion::V5
            ))
        ));
        assert_eq!(budget.storage(), floor);
    });
}
