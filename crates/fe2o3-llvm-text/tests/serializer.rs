#![allow(missing_docs)]

mod support;

use fe2o3_llvm_handoff::{
    ExecutableModuleV2, Gfx942HandoffV2, HandoffDiagnosticV2, InstructionKindV2, InstructionV2,
    ModuleFlagV1, ScalarTypeV1,
};
use fe2o3_llvm_text::{SerializeErrorV2, serialize_gfx942_handoff_v2};
use support::{Hostile, base_fixture, fixture, module_fixture};

#[test]
fn comprehensive_module_matches_the_golden_llvm_assembly() {
    let handoff = fixture(false, Hostile::None);
    let artifact = serialize_gfx942_handoff_v2(&handoff).unwrap();

    assert_eq!(artifact.as_str(), include_str!("golden/full.ll"));
    assert_eq!(artifact.source_identity(), handoff.identity());
    assert_eq!(artifact.len(), artifact.as_bytes().len());
    assert!(!artifact.is_empty());
    assert!(artifact.has_embedded_source_identity());
    assert!(
        artifact
            .as_str()
            .contains(&format!("sha256:{}", handoff.identity()))
    );
    assert_eq!(
        artifact.sha256().to_string(),
        "424e701afe634ea318278c3947ccea7518ba4082a725652999f088d73917a499"
    );
}

#[test]
fn canonical_collection_order_produces_identical_bytes_and_identity() {
    let ordered = serialize_gfx942_handoff_v2(&fixture(false, Hostile::None)).unwrap();
    let permuted = serialize_gfx942_handoff_v2(&fixture(true, Hostile::None)).unwrap();

    assert_eq!(ordered, permuted);
    assert_eq!(ordered.sha256(), permuted.sha256());
}

#[test]
fn serializer_rejects_cfg_gep_kernel_call_and_reserved_namespace_attacks() {
    assert_eq!(
        serialize_gfx942_handoff_v2(&fixture(false, Hostile::MultiIndexGep)),
        Err(SerializeErrorV2::UnsupportedGetElementPtr {
            function: fe2o3_llvm_handoff::FunctionIdV2::new(10),
            indices: 2,
        })
    );
    assert_eq!(
        serialize_gfx942_handoff_v2(&fixture(false, Hostile::UnreachableBlock)),
        Err(SerializeErrorV2::UnreachableBlock {
            function: fe2o3_llvm_handoff::FunctionIdV2::new(10),
            block: fe2o3_llvm_handoff::BlockIdV2::new(99),
        })
    );
    assert_eq!(
        serialize_gfx942_handoff_v2(&fixture(false, Hostile::EntryPredecessor)),
        Err(SerializeErrorV2::EntryBlockHasPredecessor {
            function: fe2o3_llvm_handoff::FunctionIdV2::new(10),
            predecessor: fe2o3_llvm_handoff::BlockIdV2::new(2),
        })
    );
    assert_eq!(
        serialize_gfx942_handoff_v2(&fixture(false, Hostile::KernelCall)),
        Err(SerializeErrorV2::KernelCall {
            caller: fe2o3_llvm_handoff::FunctionIdV2::new(10),
            callee: fe2o3_llvm_handoff::FunctionIdV2::new(10),
        })
    );
    assert_eq!(
        serialize_gfx942_handoff_v2(&fixture(false, Hostile::ReservedSymbol)),
        Err(SerializeErrorV2::ReservedLlvmSymbol)
    );
}

#[test]
fn validated_boundary_rejects_type_address_space_alignment_intrinsic_and_metadata_attacks() {
    let valid = fixture(false, Hostile::None);
    let bad_alignment = InstructionV2::new(
        None,
        InstructionKindV2::Store {
            pointer: fe2o3_llvm_handoff::ValueIdV2::new(1),
            value: fe2o3_llvm_handoff::ValueIdV2::new(2),
            value_type: ScalarTypeV1::I32,
            alignment: 3,
        },
        valid.module().functions()[0].evidence().clone(),
    );
    assert_eq!(bad_alignment, Err(HandoffDiagnosticV2::InvalidAlignment));

    let base = base_fixture();
    let module = module_fixture(&base, false, Hostile::None);
    let bad_helper = support::bad_global_address_helper(&base);
    let bad_address_space = ExecutableModuleV2::new(
        module.flags().to_vec(),
        module.named_metadata().to_vec(),
        module.globals().to_vec(),
        module.intrinsics().to_vec(),
        vec![bad_helper, module.functions()[1].clone()],
    );
    assert_eq!(
        bad_address_space,
        Err(HandoffDiagnosticV2::ValueTypeMismatch(
            fe2o3_llvm_handoff::ValueIdV2::new(1)
        ))
    );

    let missing_intrinsic = ExecutableModuleV2::new(
        module.flags().to_vec(),
        module.named_metadata().to_vec(),
        module.globals().to_vec(),
        vec![],
        module.functions().to_vec(),
    );
    assert_eq!(
        missing_intrinsic,
        Err(HandoffDiagnosticV2::MissingIntrinsicReference)
    );

    let metadata_changed = ExecutableModuleV2::new(
        vec![ModuleFlagV1::CodeObjectVersion6, ModuleFlagV1::PicLevel2],
        module.named_metadata().to_vec(),
        module.globals().to_vec(),
        module.intrinsics().to_vec(),
        module.functions().to_vec(),
    )
    .unwrap();
    assert_eq!(
        Gfx942HandoffV2::new(base, metadata_changed),
        Err(HandoffDiagnosticV2::MetadataMismatch)
    );
}

#[test]
fn llvm_text_has_explicit_def_use_types_alignments_intrinsics_and_strict_fp() {
    let text = serialize_gfx942_handoff_v2(&fixture(false, Hostile::None))
        .unwrap()
        .as_str()
        .to_owned();

    assert!(text.contains("%v9 = load float, ptr addrspace(4) @factor, align 4"));
    assert!(text.contains("define internal ccc float @scale(float %value)"));
    assert!(text.contains("%v13 = getelementptr float, ptr addrspace(1) %output, i64 %v11"));
    assert!(text.contains("%v12 = icmp ult i64 %v11, %length"));
    assert!(text.contains("store float %v14, ptr addrspace(1) %v13, align 4"));
    assert!(text.contains("call i32 @llvm.amdgcn.workitem.id.x()"));
    assert!(text.contains("call void @llvm.amdgcn.s.barrier()"));
    assert!(text.contains("fmul float %value, bitcast (i32 1056964608 to float)"));
    assert!(!text.contains(" fast "));
    assert!(!text.contains("contract f"));
    assert!(text.contains("\"fp-contract\"=\"off\""));
    assert!(text.contains("!\"amdhsa_code_object_version\", i32 600"));
    assert!(text.contains("!opencl.spir.version"));
}
