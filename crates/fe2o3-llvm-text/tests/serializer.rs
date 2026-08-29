#![allow(missing_docs)]

mod support;

use std::{
    io::Write as _,
    process::{Command, Stdio},
};

use fe2o3_llvm_handoff::{
    AddressSpaceV1, BasicBlockV2, BinaryOperationV2, BlockIdV2, CallTargetV2, CallingConventionV2,
    ComparePredicateV2, ExecutableModuleV2, FunctionAttributeV2, FunctionIdV2, FunctionKindV2,
    FunctionParameterV2, FunctionV2, Gfx942HandoffV2, GlobalIdV2, GlobalV2, HandoffDiagnosticV2,
    InstructionKindV2, InstructionV2, IntegerBinaryOperationV2, IntrinsicReferenceV2, IntrinsicV2,
    KERNEL_DESCRIPTOR_SECTION_V2, ModuleFlagV1, ReturnTypeV2, ScalarConstantV2, ScalarTypeV1,
    TerminatorV2, TypedValueV2, ValueIdV2, ValueTypeV2,
};
use fe2o3_llvm_text::{SerializeErrorV2, serialize_gfx942_handoff_v2};
use support::{Hostile, base_fixture, fixture, module_fixture};

fn vector_lds_machine_helper(base: &fe2o3_llvm_handoff::Gfx942HandoffV1) -> FunctionV2 {
    let i1 = ValueTypeV2::Scalar(ScalarTypeV1::I1);
    let i16 = ValueTypeV2::Scalar(ScalarTypeV1::I16);
    let i32 = ValueTypeV2::Scalar(ScalarTypeV1::I32);
    let i64 = ValueTypeV2::Scalar(ScalarTypeV1::I64);
    let f32 = ValueTypeV2::Scalar(ScalarTypeV1::F32);
    let i16x4 = ValueTypeV2::fixed_vector(ScalarTypeV1::I16, 4).unwrap();
    let f32x4 = ValueTypeV2::fixed_vector(ScalarTypeV1::F32, 4).unwrap();
    let global_i16 = ValueTypeV2::Pointer {
        pointee: ScalarTypeV1::I16,
        address_space: AddressSpaceV1::Global,
    };
    let local_array =
        ValueTypeV2::array_pointer(ScalarTypeV1::I16, 256, AddressSpaceV1::Local).unwrap();
    let local_i16 = ValueTypeV2::Pointer {
        pointee: ScalarTypeV1::I16,
        address_space: AddressSpaceV1::Local,
    };
    let op = |result, kind| support::instruction(base, result, kind, false);
    let entry = BasicBlockV2::new(
        BlockIdV2::new(0),
        vec![
            op(
                Some(TypedValueV2::new(ValueIdV2::new(2), i64)),
                InstructionKindV2::Constant(ScalarConstantV2::new(ScalarTypeV1::I64, 0).unwrap()),
            ),
            op(
                Some(TypedValueV2::new(ValueIdV2::new(3), i64)),
                InstructionKindV2::Constant(ScalarConstantV2::new(ScalarTypeV1::I64, 1).unwrap()),
            ),
            op(
                Some(TypedValueV2::new(ValueIdV2::new(4), i64)),
                InstructionKindV2::Constant(ScalarConstantV2::new(ScalarTypeV1::I64, 2).unwrap()),
            ),
            op(
                Some(TypedValueV2::new(ValueIdV2::new(5), i32)),
                InstructionKindV2::Constant(ScalarConstantV2::new(ScalarTypeV1::I32, 0).unwrap()),
            ),
            op(
                Some(TypedValueV2::new(ValueIdV2::new(6), local_array)),
                InstructionKindV2::GlobalAddress(GlobalIdV2::new(20)),
            ),
            op(
                Some(TypedValueV2::new(ValueIdV2::new(7), local_i16)),
                InstructionKindV2::GetElementPtr {
                    base: ValueIdV2::new(6),
                    indices: vec![ValueIdV2::new(2), ValueIdV2::new(2)],
                },
            ),
            op(
                Some(TypedValueV2::new(ValueIdV2::new(8), i16)),
                InstructionKindV2::Load {
                    pointer: ValueIdV2::new(7),
                    value_type: ScalarTypeV1::I16,
                    alignment: 2,
                },
            ),
            op(
                Some(TypedValueV2::new(ValueIdV2::new(9), i16x4)),
                InstructionKindV2::VectorZero {
                    element_type: ScalarTypeV1::I16,
                },
            ),
            op(
                Some(TypedValueV2::new(ValueIdV2::new(10), i16x4)),
                InstructionKindV2::InsertElement {
                    vector: ValueIdV2::new(9),
                    element: ValueIdV2::new(8),
                    index: ValueIdV2::new(5),
                },
            ),
            op(
                Some(TypedValueV2::new(ValueIdV2::new(11), i16x4)),
                InstructionKindV2::VectorLoad4 {
                    pointer: ValueIdV2::new(1),
                    element_type: ScalarTypeV1::I16,
                    alignment: 8,
                },
            ),
            op(
                Some(TypedValueV2::new(ValueIdV2::new(12), f32x4)),
                InstructionKindV2::VectorZero {
                    element_type: ScalarTypeV1::F32,
                },
            ),
        ],
        TerminatorV2::Branch(BlockIdV2::new(1)),
    );
    let loop_header = BasicBlockV2::new(
        BlockIdV2::new(1),
        vec![
            op(
                Some(TypedValueV2::new(ValueIdV2::new(13), i64)),
                InstructionKindV2::Phi {
                    incoming: vec![
                        (ValueIdV2::new(2), BlockIdV2::new(0)),
                        (ValueIdV2::new(17), BlockIdV2::new(2)),
                    ],
                },
            ),
            op(
                Some(TypedValueV2::new(ValueIdV2::new(14), f32x4)),
                InstructionKindV2::Phi {
                    incoming: vec![
                        (ValueIdV2::new(12), BlockIdV2::new(0)),
                        (ValueIdV2::new(16), BlockIdV2::new(2)),
                    ],
                },
            ),
            op(
                Some(TypedValueV2::new(ValueIdV2::new(15), i1)),
                InstructionKindV2::Compare {
                    predicate: ComparePredicateV2::UnsignedLessThan,
                    left: ValueIdV2::new(13),
                    right: ValueIdV2::new(4),
                },
            ),
        ],
        TerminatorV2::ConditionalBranch {
            condition: ValueIdV2::new(15),
            then_block: BlockIdV2::new(2),
            else_block: BlockIdV2::new(3),
        },
    );
    let loop_body = BasicBlockV2::new(
        BlockIdV2::new(2),
        vec![
            op(
                Some(TypedValueV2::new(ValueIdV2::new(16), f32x4)),
                InstructionKindV2::Call {
                    target: CallTargetV2::Intrinsic(IntrinsicV2::AmdGpuMfmaF32_16x16x16Bf16_1k),
                    arguments: vec![
                        ValueIdV2::new(11),
                        ValueIdV2::new(10),
                        ValueIdV2::new(14),
                        ValueIdV2::new(5),
                        ValueIdV2::new(5),
                        ValueIdV2::new(5),
                    ],
                },
            ),
            op(
                Some(TypedValueV2::new(ValueIdV2::new(17), i64)),
                InstructionKindV2::Binary {
                    operation: BinaryOperationV2::Integer(IntegerBinaryOperationV2::Add),
                    left: ValueIdV2::new(13),
                    right: ValueIdV2::new(3),
                },
            ),
            op(
                None,
                InstructionKindV2::Call {
                    target: CallTargetV2::Intrinsic(IntrinsicV2::AmdGpuBarrier),
                    arguments: vec![],
                },
            ),
        ],
        TerminatorV2::Branch(BlockIdV2::new(1)),
    );
    let exit = BasicBlockV2::new(
        BlockIdV2::new(3),
        vec![op(
            Some(TypedValueV2::new(ValueIdV2::new(18), f32)),
            InstructionKindV2::ExtractElement {
                vector: ValueIdV2::new(14),
                index: ValueIdV2::new(5),
            },
        )],
        TerminatorV2::Return(Some(ValueIdV2::new(18))),
    );
    FunctionV2::new(
        FunctionIdV2::new(20),
        "vector_lds_machine_surface",
        FunctionKindV2::Helper,
        CallingConventionV2::C,
        ReturnTypeV2::Value(f32),
        vec![
            FunctionParameterV2::new(
                TypedValueV2::new(ValueIdV2::new(1), global_i16),
                "input",
                vec![],
            )
            .unwrap(),
        ],
        vec![
            FunctionAttributeV2::NoUnwind,
            FunctionAttributeV2::AlwaysInline,
        ],
        BlockIdV2::new(0),
        vec![entry, loop_header, loop_body, exit],
        support::evidence(base, false),
    )
    .unwrap()
}

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
        "e663dcedd0ed4208967b3bbf5397c85f42fe51503114dee8fbb61b0029b47f1e"
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
fn bounded_vector_and_local_array_shapes_round_trip_and_emit_typed_llvm() {
    let base = base_fixture();
    let existing = module_fixture(&base, false, Hostile::None);
    let mut globals = existing.globals().to_vec();
    globals.push(
        GlobalV2::new_local_array(
            GlobalIdV2::new(20),
            "shared_i16_lds",
            ScalarTypeV1::I16,
            256,
            16,
            support::evidence(&base, false),
        )
        .unwrap(),
    );
    globals.push(
        GlobalV2::new_private_constant_bytes(
            GlobalIdV2::new(22),
            "descriptor_source",
            KERNEL_DESCRIPTOR_SECTION_V2,
            vec![0, 1, 0x7f, 0xff],
            1,
            support::evidence(&base, false),
        )
        .unwrap(),
    );
    globals.push(
        GlobalV2::new_local_array(
            GlobalIdV2::new(21),
            "shared_f32_lds",
            ScalarTypeV1::F32,
            1024,
            32,
            support::evidence(&base, false),
        )
        .unwrap(),
    );
    let mut intrinsics = existing.intrinsics().to_vec();
    intrinsics.extend([
        IntrinsicReferenceV2::new(IntrinsicV2::Trap, support::evidence(&base, false)),
        IntrinsicReferenceV2::new(
            IntrinsicV2::AmdGpuMfmaF32_16x16x16Bf16_1k,
            support::evidence(&base, false),
        ),
    ]);
    let mut functions = existing.functions().to_vec();
    let kernel_index = functions
        .iter()
        .position(|function| function.kind() == FunctionKindV2::Kernel)
        .unwrap();
    let kernel = &functions[kernel_index];
    let mut kernel_attributes = kernel.attributes().to_vec();
    kernel_attributes.push(FunctionAttributeV2::RequiredWorkgroupSize([64, 1, 1]));
    functions[kernel_index] = FunctionV2::new(
        kernel.id(),
        kernel.symbol(),
        kernel.kind(),
        kernel.calling_convention(),
        kernel.return_type(),
        kernel.parameters().to_vec(),
        kernel_attributes,
        kernel.entry(),
        kernel.blocks().to_vec(),
        kernel.evidence().clone(),
    )
    .unwrap();
    functions.push(vector_lds_machine_helper(&base));
    let module = ExecutableModuleV2::new(
        existing.flags().to_vec(),
        existing.named_metadata().to_vec(),
        globals,
        intrinsics,
        functions,
    )
    .unwrap();
    let handoff = Gfx942HandoffV2::new(base, module).unwrap();
    let encoded = handoff.encode_canonical();
    assert_eq!(
        Gfx942HandoffV2::decode_canonical(encoded.as_bytes()).unwrap(),
        handoff
    );

    let assembly = serialize_gfx942_handoff_v2(&handoff).unwrap();
    let text = assembly.as_str();
    assert!(
        text.contains("@shared_i16_lds = internal addrspace(3) global [256 x i16] undef, align 16")
    );
    assert!(
        text.contains(
            "@shared_f32_lds = internal addrspace(3) global [1024 x float] undef, align 32"
        )
    );
    assert!(
        text.contains("getelementptr [256 x i16], ptr addrspace(3) @shared_i16_lds, i64 0, i64 0")
    );
    assert!(text.contains("load <4 x i16>, ptr addrspace(1) %input, align 8"));
    assert!(text.contains("phi <4 x float>"));
    assert!(text.contains("@llvm.amdgcn.mfma.f32.16x16x16bf16.1k"));
    assert!(text.contains("declare void @llvm.trap()"));
    assert!(text.contains("extractelement <4 x float>"));
    assert!(text.contains("!reqd_work_group_size"));
    assert!(text.contains("!{i32 64, i32 1, i32 1}"));
    assert!(text.contains("section \".fe2o3.kd.v1\", align 1"));
    assert!(text.contains("@llvm.compiler.used = appending global [1 x ptr]"));
    assert!(text.contains("@descriptor_source to ptr"));
    if let Some(llvm_as) = std::env::var_os("FE2O3_LLVM_AS") {
        let mut child = Command::new(llvm_as)
            .args(["-o", "/dev/null", "-"])
            .stdin(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(assembly.as_bytes())
            .unwrap();
        assert!(child.wait().unwrap().success());
    }
}

#[test]
fn standard_amdgpu_abi_attributes_serialize_canonically() {
    let existing = fixture(false, Hostile::None);
    let mut functions = existing.module().functions().to_vec();
    let helper_index = functions
        .iter()
        .position(|function| function.kind() == FunctionKindV2::Helper)
        .unwrap();
    let helper = functions[helper_index].clone();
    let mut attributes = helper.attributes().to_vec();
    attributes.extend([
        FunctionAttributeV2::NoCompletionAction,
        FunctionAttributeV2::NoDefaultQueue,
        FunctionAttributeV2::NoHeapPointer,
        FunctionAttributeV2::NoHostcallPointer,
        FunctionAttributeV2::NoMultigridSyncArgument,
        FunctionAttributeV2::NoQueuePointer,
    ]);
    functions[helper_index] = FunctionV2::new(
        helper.id(),
        helper.symbol(),
        helper.kind(),
        helper.calling_convention(),
        helper.return_type(),
        helper.parameters().to_vec(),
        attributes,
        helper.entry(),
        helper.blocks().to_vec(),
        helper.evidence().clone(),
    )
    .unwrap();
    let module = ExecutableModuleV2::new(
        existing.module().flags().to_vec(),
        existing.module().named_metadata().to_vec(),
        existing.module().globals().to_vec(),
        existing.module().intrinsics().to_vec(),
        functions,
    )
    .unwrap();
    let handoff = Gfx942HandoffV2::new(existing.base().clone(), module).unwrap();
    let assembly = serialize_gfx942_handoff_v2(&handoff).unwrap();
    let attribute_line = assembly
        .as_str()
        .lines()
        .find(|line| line.starts_with("attributes #0 ="))
        .unwrap();
    assert!(attribute_line.contains(
        "\"amdgpu-no-completion-action\" \"amdgpu-no-default-queue\" \
         \"amdgpu-no-heap-ptr\" \"amdgpu-no-hostcall-ptr\" \
         \"amdgpu-no-multigrid-sync-arg\" \"amdgpu-no-queue-ptr\""
    ));

    if let Some(llvm_as) = std::env::var_os("FE2O3_LLVM_AS") {
        let mut child = Command::new(llvm_as)
            .args(["-o", "/dev/null", "-"])
            .stdin(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(assembly.as_bytes())
            .unwrap();
        assert!(child.wait().unwrap().success());
    }
}

#[test]
fn serializer_rejects_gep_kernel_call_and_reserved_namespace_attacks() {
    assert_eq!(
        serialize_gfx942_handoff_v2(&fixture(false, Hostile::MultiIndexGep)),
        Err(SerializeErrorV2::UnsupportedGetElementPtr {
            function: fe2o3_llvm_handoff::FunctionIdV2::new(10),
            indices: 2,
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
