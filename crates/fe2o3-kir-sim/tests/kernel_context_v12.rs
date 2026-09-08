use std::collections::BTreeSet;

use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, AsyncCopyCompletionV1, Axis, BasicBlock, BinaryOp, BlockId,
    ExecutionCapabilityRequirementV1, Function, GlobalCapabilityTypeV1,
    GlobalDisjointIndexContractV1, GlobalDisjointIndexSpaceV1, IndexKind, IntrinsicKind,
    IntrinsicOperation, Kernel, KernelContextSourceIdentityV1, KernelContextTypeV1, LaunchDomain,
    LaunchExtent, MemoryAccess, Module, Operation, OperationKind, ScalarType, Signature,
    TargetCapability, Terminator, Type, ValueDef, ValueId, VerifiedCanonicalKernelIrErrorV12,
    VerifiedCanonicalKernelIrV11, VerifiedCanonicalKernelIrV12,
};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, SimulationAdmissionErrorV1, SimulationArgumentV1,
    SimulationLimitsV1, SimulationRequestV1, SimulationTargetV1,
};

fn context_type() -> KernelContextTypeV1 {
    KernelContextTypeV1::new("entry", [1; 32], [2; 32], [3; 32])
}

fn source_identity() -> KernelContextSourceIdentityV1 {
    KernelContextSourceIdentityV1::new([4; 32], [5; 32], [6; 32], [7; 32])
}

fn output_index_contract() -> GlobalDisjointIndexContractV1 {
    GlobalDisjointIndexContractV1::new([8; 32], GlobalDisjointIndexSpaceV1::Index1d)
}

fn op(result: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(result), ty), kind)
}

fn vecadd_module(contextual: bool) -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let input = Type::slice(scalar.clone(), AddressSpace::Global, AccessMode::ReadOnly);
    let output = Type::slice(scalar.clone(), AddressSpace::Global, AccessMode::WriteOnly);
    let input_pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadOnly);
    let output_pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::WriteOnly);
    let read_capability = GlobalCapabilityTypeV1::read_only(scalar.clone(), context_type());
    let write_capability = GlobalCapabilityTypeV1::disjoint_write(
        scalar.clone(),
        context_type(),
        output_index_contract(),
    );
    let mut helper_parameters = vec![input.clone(), input.clone(), output.clone(), Type::INDEX];
    let mut helper_values = vec![ValueId(1), ValueId(2), ValueId(3), ValueId(4)];
    if contextual {
        helper_parameters.insert(0, Type::KernelContext(context_type()));
        helper_parameters[1] = Type::GlobalCapability(read_capability.clone());
        helper_parameters[2] = Type::GlobalCapability(read_capability.clone());
        helper_parameters[3] = Type::GlobalCapability(write_capability.clone());
        helper_values.insert(0, ValueId(0));
    }
    let mut helper_block = BasicBlock::new(BlockId(0));
    if contextual {
        helper_block.operations.extend([
            Operation::global_capability_index(ValueId(14), ValueId(1), ValueId(4), None),
            Operation::global_capability_index(ValueId(15), ValueId(2), ValueId(4), None),
            Operation::global_capability_index(
                ValueId(16),
                ValueId(3),
                ValueId(4),
                Some(output_index_contract()),
            ),
        ]);
    }
    let offsets = if contextual {
        [ValueId(14), ValueId(15), ValueId(16)]
    } else {
        [ValueId(4), ValueId(4), ValueId(4)]
    };
    helper_block.operations.extend([
        op(
            5,
            input_pointer.clone(),
            OperationKind::SliceData { slice: ValueId(1) },
        ),
        op(
            6,
            input_pointer.clone(),
            OperationKind::SliceData { slice: ValueId(2) },
        ),
        op(
            7,
            output_pointer.clone(),
            OperationKind::SliceData { slice: ValueId(3) },
        ),
        op(
            8,
            input_pointer.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(5),
                offset: offsets[0],
            },
        ),
        op(
            9,
            input_pointer,
            OperationKind::GetElementPointer {
                base: ValueId(6),
                offset: offsets[1],
            },
        ),
        op(
            10,
            output_pointer,
            OperationKind::GetElementPointer {
                base: ValueId(7),
                offset: offsets[2],
            },
        ),
        op(
            11,
            scalar.clone(),
            OperationKind::Load {
                pointer: ValueId(8),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
        op(
            12,
            scalar.clone(),
            OperationKind::Load {
                pointer: ValueId(9),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
        op(
            13,
            scalar,
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(11),
                rhs: ValueId(12),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(10),
                value: ValueId(13),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ]);
    helper_block.terminator = Some(Terminator::Return { values: vec![] });
    let helper = Function::internal_helper(
        "helper",
        Signature::new(helper_parameters, vec![]),
        helper_values,
        vec![helper_block],
    );

    let mut entry_block = BasicBlock::new(BlockId(0));
    if contextual {
        entry_block.operations.push(Operation::kernel_context_issue(
            ValueId(3),
            context_type(),
            source_identity(),
        ));
    }
    entry_block.operations.push(op(
        4,
        Type::INDEX,
        OperationKind::Intrinsic(IntrinsicOperation::new(
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Global,
                axis: Axis::X,
            },
            Type::INDEX,
        )),
    ));
    let mut call_arguments = vec![ValueId(0), ValueId(1), ValueId(2), ValueId(4)];
    if contextual {
        entry_block.operations.extend([
            Operation::global_capability_bind(
                ValueId(5),
                read_capability.clone(),
                ValueId(3),
                ValueId(0),
            ),
            Operation::global_capability_bind(ValueId(6), read_capability, ValueId(3), ValueId(1)),
            Operation::global_capability_bind(ValueId(7), write_capability, ValueId(3), ValueId(2)),
        ]);
        call_arguments = vec![ValueId(5), ValueId(6), ValueId(7), ValueId(4)];
        call_arguments.insert(0, ValueId(3));
    }
    entry_block.operations.push(Operation::new(
        vec![],
        OperationKind::Call {
            callee: "helper".into(),
            arguments: call_arguments,
        },
    ));
    entry_block.terminator = Some(Terminator::Return { values: vec![] });
    let entry = Function::kernel_entry(
        "entry",
        Signature::new(vec![input.clone(), input, output], vec![]),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![entry_block],
    );

    let mut module = Module::new("vecadd-v12");
    module.functions = vec![entry, helper];
    module.kernels.push(Kernel::new(
        "vecadd",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module.required_capabilities = BTreeSet::from([
        TargetCapability::Execution(ExecutionCapabilityRequirementV1::AddressSpace {
            address_space: AddressSpace::Global,
            access: AccessMode::ReadOnly,
        }),
        TargetCapability::Execution(ExecutionCapabilityRequirementV1::AddressSpace {
            address_space: AddressSpace::Global,
            access: AccessMode::WriteOnly,
        }),
    ]);
    module
}

fn u32_buffer(values: &[u32]) -> BufferArgumentV1 {
    let values = values
        .iter()
        .copied()
        .map(fe2o3_kir_sim::ScalarBitsV1::u32)
        .collect::<Vec<_>>();
    BufferArgumentV1::from_scalars(
        AccessMode::ReadWrite,
        4,
        &values,
        SimulationTargetV1::amdgpu_64(),
    )
    .unwrap()
}

fn request() -> SimulationRequestV1 {
    SimulationRequestV1::new(
        "vecadd",
        [4, 1, 1],
        [4, 1, 1],
        vec![
            SimulationArgumentV1::Buffer(u32_buffer(&[1, 2, 3, 4])),
            SimulationArgumentV1::Buffer(u32_buffer(&[10, 20, 30, 40])),
            SimulationArgumentV1::Buffer(u32_buffer(&[0, 0, 0, 0])),
        ],
    )
}

fn words(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks_exact(4)
        .map(|word| u32::from_le_bytes(word.try_into().unwrap()))
        .collect()
}

#[test]
fn contextual_vecadd_executes_as_its_logically_erased_equivalent() {
    let contextual = vecadd_module(true);
    let erased = vecadd_module(false);
    let contextual = AdmittedSimulationModuleV1::admit_v12(
        VerifiedCanonicalKernelIrV12::from_module(contextual).unwrap(),
        SimulationLimitsV1::default(),
    )
    .unwrap();
    let erased = AdmittedSimulationModuleV1::admit_v12(
        VerifiedCanonicalKernelIrV12::from_module(erased.clone()).unwrap(),
        SimulationLimitsV1::default(),
    )
    .unwrap();

    assert_eq!(contextual.identity().wire_version(), 12);
    assert_eq!(contextual.module(), erased.module());
    assert!(contextual.module().functions.iter().all(|function| {
        function
            .signature
            .parameters
            .iter()
            .all(|ty| !ty.contains_logical_capability())
    }));
    assert!(contextual.module().functions.iter().all(|function| {
        function
            .body
            .iter()
            .flat_map(|body| &body.blocks)
            .all(|block| {
                block.operations.iter().all(|operation| {
                    !matches!(
                        operation.kind,
                        OperationKind::KernelContextIssue(_)
                            | OperationKind::GlobalCapabilityBind(_)
                            | OperationKind::GlobalCapabilityIndex(_)
                    )
                })
            })
    }));

    let contextual = contextual
        .simulate(
            &request(),
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap();
    let erased = erased
        .simulate(
            &request(),
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap();
    assert_eq!(contextual.arguments(), erased.arguments());
    assert_eq!(
        words(contextual.buffer(2).unwrap().bytes()),
        [11, 22, 33, 44]
    );
}

#[test]
fn substituted_context_brand_is_rejected_before_simulator_admission() {
    let mut module = vecadd_module(true);
    module.functions[1].signature.parameters[0] =
        Type::KernelContext(KernelContextTypeV1::new("entry", [9; 32], [2; 32], [3; 32]));
    assert!(matches!(
        VerifiedCanonicalKernelIrV12::from_module(module),
        Err(VerifiedCanonicalKernelIrErrorV12::Verification(_))
    ));
}

#[test]
fn substituted_disjoint_index_authority_is_rejected_before_admission() {
    let mut module = vecadd_module(true);
    let OperationKind::GlobalCapabilityIndex(index) =
        &mut module.functions[1].body.as_mut().unwrap().blocks[0].operations[2].kind
    else {
        panic!("expected disjoint output index projection")
    };
    index.index_space = Some(GlobalDisjointIndexContractV1::new(
        [9; 32],
        GlobalDisjointIndexSpaceV1::Index1d,
    ));
    assert!(matches!(
        VerifiedCanonicalKernelIrV12::from_module(module),
        Err(VerifiedCanonicalKernelIrErrorV12::Verification(_))
    ));
}

#[test]
fn unsupported_execution_capability_fails_closed_at_v12_admission() {
    let mut module = vecadd_module(true);
    let unsupported = ExecutionCapabilityRequirementV1::AsyncCopy {
        source: AddressSpace::Global,
        destination: AddressSpace::Workgroup,
        bytes: 16,
        alignment: 16,
        completion: AsyncCopyCompletionV1::WorkgroupBarrier,
    };
    module
        .required_capabilities
        .insert(TargetCapability::Execution(unsupported.clone()));
    let canonical = VerifiedCanonicalKernelIrV12::from_module(module).unwrap();
    assert!(matches!(
        AdmittedSimulationModuleV1::admit_v12(canonical, SimulationLimitsV1::default()),
        Err(SimulationAdmissionErrorV1::UnsupportedExecutionCapability(found))
            if found == unsupported
    ));
}

#[test]
fn exact_v12_owner_rejects_an_older_erased_projection() {
    let mut projected = vecadd_module(false);
    projected.required_capabilities.clear();
    let projected = VerifiedCanonicalKernelIrV11::from_module(projected).unwrap();
    assert!(matches!(
        VerifiedCanonicalKernelIrV12::from_canonical_bytes(projected.into_canonical_bytes()),
        Err(VerifiedCanonicalKernelIrErrorV12::NotExactV12 { version: 11 })
    ));
}
