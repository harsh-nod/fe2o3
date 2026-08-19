//! Focused coverage for the live Pliron graph to typed LLVM handoff V2 boundary.

use fe2o3_amdgcn_pliron_llvm::{
    MAX_DIAGNOSTIC_BYTES_V1, ScalarKernelModuleV1, lower_scalar_kernel_v1, lower_scalar_kernel_v2,
};
use fe2o3_llvm_handoff::{
    AddressSpaceV1, BinaryOperationV2, CallingConventionV2, FloatBinaryOperationV2,
    FunctionAttributeV1, FunctionKindV2, IdentityV1, InstructionKindV2, ReturnTypeV2, ScalarTypeV1,
    StageIdentitiesV1, TargetFeatureV1, TerminatorV2, ValueIdV2, ValueTypeV2, WorkgroupSizeRangeV1,
};

fn request() -> ScalarKernelModuleV1 {
    ScalarKernelModuleV1::canonical(
        "scalar_module",
        "scalar_add",
        IdentityV1::new([0x41; 32]).unwrap(),
        StageIdentitiesV1::new([0x11; 32], [0x22; 32], [0x33; 32]).unwrap(),
    )
}

#[test]
fn exact_live_graph_extracts_to_validated_v2() {
    let lowered = lower_scalar_kernel_v1(&request()).unwrap();
    let handoff = lowered.extract_handoff_v2().unwrap();

    assert_eq!(handoff.base(), lowered.handoff());
    assert!(handoff.module().globals().is_empty());
    assert!(handoff.module().intrinsics().is_empty());
    let [function] = handoff.module().functions() else {
        panic!("expected exactly one extracted function")
    };
    assert_eq!(function.id().get(), 0);
    assert_eq!(function.symbol(), "scalar_add");
    assert_eq!(function.kind(), FunctionKindV2::Kernel);
    assert_eq!(
        function.calling_convention(),
        CallingConventionV2::AmdGpuKernel
    );
    assert_eq!(function.return_type(), ReturnTypeV2::Void);

    let pointer_type = ValueTypeV2::Pointer {
        pointee: ScalarTypeV1::F32,
        address_space: AddressSpaceV1::Global,
    };
    let f32_type = ValueTypeV2::Scalar(ScalarTypeV1::F32);
    assert_eq!(
        function
            .parameters()
            .iter()
            .map(|parameter| (
                parameter.value().id().get(),
                parameter.name(),
                parameter.value().value_type(),
                parameter.attributes()
            ))
            .collect::<Vec<_>>(),
        vec![
            (0, "input", pointer_type, &[][..]),
            (1, "output", pointer_type, &[][..]),
            (2, "addend", f32_type, &[][..]),
        ]
    );

    let [block] = function.blocks() else {
        panic!("expected exactly one extracted block")
    };
    assert_eq!(function.entry(), block.id());
    assert_eq!(block.id().get(), 0);
    assert_eq!(block.terminator(), &TerminatorV2::Return(None));
    let [load, add, store] = block.instructions() else {
        panic!("expected load, add, store")
    };
    assert_eq!(
        load.result()
            .map(|value| (value.id().get(), value.value_type())),
        Some((3, f32_type))
    );
    assert_eq!(
        load.kind(),
        &InstructionKindV2::Load {
            pointer: ValueIdV2::new(0),
            value_type: ScalarTypeV1::F32,
            alignment: 4,
        }
    );
    assert_eq!(
        add.result()
            .map(|value| (value.id().get(), value.value_type())),
        Some((4, f32_type))
    );
    assert_eq!(
        add.kind(),
        &InstructionKindV2::Binary {
            operation: BinaryOperationV2::Float(FloatBinaryOperationV2::Add),
            left: ValueIdV2::new(3),
            right: ValueIdV2::new(2),
        }
    );
    assert_eq!(store.result(), None);
    assert_eq!(
        store.kind(),
        &InstructionKindV2::Store {
            pointer: ValueIdV2::new(1),
            value: ValueIdV2::new(4),
            value_type: ScalarTypeV1::F32,
            alignment: 4,
        }
    );

    let origin = handoff.base().origins()[0].identity();
    let obligation_ids = handoff
        .base()
        .obligations()
        .iter()
        .map(|obligation| obligation.identity())
        .collect::<Vec<_>>();
    for evidence in [
        function.evidence(),
        load.evidence(),
        add.evidence(),
        store.evidence(),
    ] {
        assert_eq!(evidence.origin(), origin);
        assert_eq!(evidence.obligations(), obligation_ids);
    }
}

#[test]
fn typed_v2_boundary_is_deterministic_and_round_trips() {
    let input = request();
    let direct = lower_scalar_kernel_v2(&input).unwrap();
    let extracted = lower_scalar_kernel_v1(&input)
        .unwrap()
        .extract_handoff_v2()
        .unwrap();
    assert_eq!(direct, extracted);
    assert_eq!(direct.identity(), extracted.identity());

    let encoded = direct.encode_canonical();
    let decoded =
        fe2o3_llvm_handoff::Gfx942HandoffV2::decode_canonical(encoded.as_bytes()).unwrap();
    assert_eq!(decoded, direct);
    assert_eq!(decoded.encode_canonical(), encoded);
}

#[test]
fn v1_surface_and_receipt_remain_stable_after_v2_extraction() {
    let lowered = lower_scalar_kernel_v1(&request()).unwrap();
    let handoff_bytes = lowered.handoff().encode_canonical();
    let receipt = lowered.receipt().clone();
    let context_identity = lowered.context_identity();

    let _ = lowered.extract_handoff_v2().unwrap();
    assert_eq!(lowered.handoff().encode_canonical(), handoff_bytes);
    assert_eq!(lowered.receipt(), &receipt);
    assert_eq!(lowered.context_identity(), context_identity);
}

#[test]
fn diagnostics_remain_bounded_and_non_authoritative() {
    for diagnostic in [
        fe2o3_amdgcn_pliron_llvm::HandoffExtractionDiagnosticV2::DefUseMismatch,
        fe2o3_amdgcn_pliron_llvm::HandoffExtractionDiagnosticV2::EvidenceMismatch,
        fe2o3_amdgcn_pliron_llvm::HandoffExtractionDiagnosticV2::UpstreamPanicked,
    ] {
        assert!(diagnostic.to_string().len() <= MAX_DIAGNOSTIC_BYTES_V1);
        assert!(!diagnostic.to_string().contains("llvm.fadd"));
    }
}

#[test]
fn one_to_sixty_four_workgroup_range_is_preserved_with_wave64_policy() {
    let expected = WorkgroupSizeRangeV1::new(1, 64).unwrap();

    let handoff = lower_scalar_kernel_v2(&request()).unwrap();
    let flat_range = handoff.module().functions()[0]
        .attributes()
        .iter()
        .find_map(|attribute| match attribute {
            fe2o3_llvm_handoff::FunctionAttributeV2::FlatWorkgroupSize(range) => Some(*range),
            _ => None,
        })
        .expect("admitted kernel must preserve a flat workgroup range");
    assert_eq!(flat_range, expected);
    assert!(
        handoff.base().kernels()[0]
            .function_attributes()
            .contains(&FunctionAttributeV1::FlatWorkgroupSize(flat_range))
    );
    assert!(handoff.base().target().features().iter().any(|feature| {
        feature.feature() == TargetFeatureV1::WavefrontSize64 && feature.enabled()
    }));
    assert!(handoff.base().target().features().iter().any(|feature| {
        feature.feature() == TargetFeatureV1::WavefrontSize32 && !feature.enabled()
    }));
}
