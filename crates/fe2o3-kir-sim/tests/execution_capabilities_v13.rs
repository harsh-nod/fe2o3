use std::collections::BTreeSet;
use std::fs;
use std::process::Command;

use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, AsyncCopyCompletionV1, AtomicKind, Axis, BasicBlock, BlockId,
    CollectiveCapabilityOperationV1, ComparePredicate, Constant, ExecutionAtomicKindV1,
    ExecutionCapabilityOpV1, ExecutionCapabilityOperationV1, ExecutionCapabilityProvenanceV1,
    ExecutionCapabilityRequirementV1, ExecutionCapabilityRoleV1, ExecutionCapabilitySignatureV1,
    ExecutionCapabilitySourceV1, ExecutionCapabilityTypeV1, ExecutionCollectiveKindV1,
    ExecutionDynamicExtentV1, ExecutionElementLayoutV1, ExecutionLdsStateV1,
    ExecutionMemoryAccessV1, ExecutionMemoryAddressSpaceV1, ExecutionMemoryExtentV1,
    ExecutionMemoryInitializationV1, ExecutionMemoryOrderingV1, ExecutionMemoryScopeV1,
    ExecutionMemorySemanticsV1, ExecutionMemorySpacesV1, ExecutionSafetyObligationsV1,
    ExecutionTypeIdentityV1, Function, FunctionId, IndexKind, IntrinsicKind, IntrinsicOperation,
    Kernel, KernelContextSourceIdentityV1, KernelContextTypeV1, LaunchDomain, LaunchExtent,
    MemoryAccess, MemoryOrdering, Module, NumericalModeV1, Operation, OperationKind,
    ResourceCapabilityRequirementV1, ScalarType, Signature, SynchronizationScope, TargetCapability,
    Terminator, Type, ValueDef, ValueId, VerifiedCanonicalKernelIrV13, WorkgroupSize,
    required_execution_obligations_v1,
};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, IncompleteExecutionCapabilityOperationV13,
    ScalarBitsV1, SimulationAdmissionErrorV1, SimulationArgumentV1,
    SimulationExecutionCapabilityFamilyV13, SimulationLimitsV1, SimulationRequestV1,
    SimulationTargetV1,
};

const UPPER_BOUND: u64 = 257;

fn identity(byte: u8) -> ExecutionTypeIdentityV1 {
    ExecutionTypeIdentityV1::new([byte; 32])
}

fn provenance() -> ExecutionCapabilityProvenanceV1 {
    ExecutionCapabilityProvenanceV1 {
        root: FunctionId::new("entry"),
        kernel_binding: [1; 32],
        frontend_unit: [2; 32],
        kernel_marker: [3; 32],
        target_brand: [4; 32],
        launch_brand: [5; 32],
        issuance: [6; 32],
    }
}

fn context_type() -> KernelContextTypeV1 {
    let provenance = provenance();
    KernelContextTypeV1::new(
        "entry",
        provenance.kernel_marker,
        provenance.target_brand,
        provenance.launch_brand,
    )
}

fn capability_type(
    source_type: ExecutionTypeIdentityV1,
    workgroup_brand: [u8; 32],
    epoch: [u8; 32],
    role: ExecutionCapabilityRoleV1,
) -> Type {
    Type::ExecutionCapability(ExecutionCapabilityTypeV1 {
        source_type,
        provenance: provenance(),
        workgroup_brand: Some(workgroup_brand),
        epoch: Some(epoch),
        role,
    })
}

fn workgroup_derivation(
    context: ValueId,
    result: ValueId,
    workgroup: ExecutionTypeIdentityV1,
    workgroup_brand: [u8; 32],
    epoch: [u8; 32],
    source: u8,
) -> Operation {
    let operation = ExecutionCapabilityOperationV1::WorkgroupDerive {
        context: identity(0xf0),
        workgroup,
    };
    Operation::new(
        vec![ValueDef::new(
            result,
            capability_type(
                workgroup,
                workgroup_brand,
                epoch,
                ExecutionCapabilityRoleV1::Workgroup,
            ),
        )],
        OperationKind::ExecutionCapability(ExecutionCapabilityOpV1 {
            operands: vec![context],
            signature: ExecutionCapabilitySignatureV1::new(&[identity(0xf0)], workgroup).unwrap(),
            provenance: provenance(),
            workgroup_brand: Some(workgroup_brand),
            epoch_before: Some(epoch),
            epoch_after: None,
            obligations: ExecutionSafetyObligationsV1::from_bits(
                required_execution_obligations_v1(&operation),
            ),
            source: ExecutionCapabilitySourceV1 {
                function: [0xf1; 32],
                operation: [source; 32],
                block: 0,
            },
            operation,
        }),
    )
}

// The fixture keeps each canonical carrier field explicit at call sites.
#[allow(clippy::too_many_arguments)]
fn subgroup_derivation(
    workgroup_value: ValueId,
    result: ValueId,
    workgroup: ExecutionTypeIdentityV1,
    subgroup: ExecutionTypeIdentityV1,
    workgroup_brand: [u8; 32],
    epoch: [u8; 32],
    width: u32,
    source: u8,
) -> Operation {
    let operation = ExecutionCapabilityOperationV1::SubgroupDerive {
        workgroup,
        subgroup,
        width,
    };
    Operation::new(
        vec![ValueDef::new(
            result,
            capability_type(
                subgroup,
                workgroup_brand,
                epoch,
                ExecutionCapabilityRoleV1::Subgroup { width },
            ),
        )],
        OperationKind::ExecutionCapability(ExecutionCapabilityOpV1 {
            operands: vec![workgroup_value],
            signature: ExecutionCapabilitySignatureV1::new(&[workgroup], subgroup).unwrap(),
            provenance: provenance(),
            workgroup_brand: Some(workgroup_brand),
            epoch_before: Some(epoch),
            epoch_after: None,
            obligations: ExecutionSafetyObligationsV1::from_bits(
                required_execution_obligations_v1(&operation),
            ),
            source: ExecutionCapabilitySourceV1 {
                function: [0xf2; 32],
                operation: [source; 32],
                block: 0,
            },
            operation,
        }),
    )
}

fn raw_bind_module() -> Module {
    let extent = ExecutionDynamicExtentV1 {
        operand: 2,
        source_argument: 2,
        source_type: identity(9),
        value_type: ScalarType::Index,
        upper_bound: UPPER_BOUND,
        bound_check_operand: 3,
        nonnegative_check_operand: None,
    };
    let operation = ExecutionCapabilityOperationV1::RawMemoryBind {
        authority: identity(7),
        pointer: identity(8),
        length: identity(9),
        extent,
        view: identity(10),
        element: identity(11),
        layout: ExecutionElementLayoutV1 {
            byte_size: 4,
            byte_alignment: 4,
        },
        space: ExecutionMemoryAddressSpaceV1::Private,
        access: ExecutionMemoryAccessV1::ReadOnly,
        index_space: None,
        atomic_scope: None,
        unsafe_obligation: identity(12),
    };
    let result_type = Type::ExecutionCapability(ExecutionCapabilityTypeV1 {
        source_type: identity(10),
        provenance: provenance(),
        workgroup_brand: None,
        epoch: None,
        role: ExecutionCapabilityRoleV1::MemoryView {
            element: identity(11),
            layout: ExecutionElementLayoutV1 {
                byte_size: 4,
                byte_alignment: 4,
            },
            space: ExecutionMemoryAddressSpaceV1::Private,
            access: ExecutionMemoryAccessV1::ReadOnly,
            extent: ExecutionMemoryExtentV1::Dynamic(extent),
            initialization: ExecutionMemoryInitializationV1::FullyInitialized,
            index_space: None,
            atomic_scope: None,
        },
    });
    let requirements = operation.required_capabilities();
    let contract = ExecutionCapabilityOpV1 {
        operands: vec![ValueId(1), ValueId(4), ValueId(0), ValueId(3)],
        signature: ExecutionCapabilitySignatureV1::new(
            &[identity(7), identity(8), identity(9), identity(12)],
            identity(10),
        )
        .unwrap(),
        provenance: provenance(),
        workgroup_brand: None,
        epoch_before: None,
        epoch_after: None,
        obligations: ExecutionSafetyObligationsV1::from_bits(required_execution_obligations_v1(
            &operation,
        )),
        source: ExecutionCapabilitySourceV1 {
            function: [13; 32],
            operation: [14; 32],
            block: 0,
        },
        operation,
    };

    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::kernel_context_issue(
            ValueId(1),
            context_type(),
            KernelContextSourceIdentityV1::new([21; 32], [22; 32], [23; 32], [24; 32]),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(2), Type::INDEX),
            OperationKind::Constant(Constant::Index(UPPER_BOUND)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(3), Type::BOOL),
            OperationKind::Compare {
                predicate: ComparePredicate::LessThanOrEqual,
                lhs: ValueId(0),
                rhs: ValueId(2),
            },
        ),
        Operation::new(
            vec![ValueDef::new(
                ValueId(4),
                Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Private,
                    AccessMode::ReadWrite,
                ),
            )],
            OperationKind::Alloca {
                element: Type::Scalar(ScalarType::U32),
                count: Some(ValueId(2)),
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(5), result_type),
            OperationKind::ExecutionCapability(contract),
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut entry = Function::kernel_entry(
        "entry",
        Signature::new(vec![Type::INDEX], vec![]),
        vec![ValueId(0)],
        vec![block],
    );
    entry.required_capabilities = requirements.clone();
    let mut module = Module::new("sim-incomplete-execution-capability-v13");
    module.functions.push(entry);
    module.required_capabilities = requirements;
    module.kernels.push(Kernel::new(
        "entry-kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}

fn contextual_noop_module() -> Module {
    let context = KernelContextTypeV1::new("context-entry", [21; 32], [22; 32], [23; 32]);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(Operation::kernel_context_issue(
        ValueId(0),
        context,
        KernelContextSourceIdentityV1::new([24; 32], [25; 32], [26; 32], [27; 32]),
    ));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("sim-exact-context-v13");
    module.functions.push(Function::kernel_entry(
        "context-entry",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    module.kernels.push(Kernel::new(
        "context-kernel",
        "context-entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}

fn barrier_contract(before: u8, after: u8, source: u8) -> ExecutionCapabilityOpV1 {
    let semantics = ExecutionMemorySemanticsV1 {
        scope: ExecutionMemoryScopeV1::Workgroup,
        ordering: ExecutionMemoryOrderingV1::AcquireRelease,
        spaces: ExecutionMemorySpacesV1::GlobalAndWorkgroup,
    };
    let operation = ExecutionCapabilityOperationV1::WorkgroupBarrier {
        input_workgroup: identity(31),
        output_workgroup: identity(32),
        semantics,
    };
    ExecutionCapabilityOpV1 {
        operands: Vec::new(),
        signature: ExecutionCapabilitySignatureV1::new(&[identity(31)], identity(32)).unwrap(),
        provenance: provenance(),
        workgroup_brand: Some([30; 32]),
        epoch_before: Some([before; 32]),
        epoch_after: Some([after; 32]),
        obligations: ExecutionSafetyObligationsV1::from_bits(required_execution_obligations_v1(
            &operation,
        )),
        source: ExecutionCapabilitySourceV1 {
            function: [13; 32],
            operation: [source; 32],
            block: 0,
        },
        operation,
    }
}

fn barrier_module(contracts: Vec<ExecutionCapabilityOpV1>) -> Module {
    let requirements: BTreeSet<_> = contracts
        .iter()
        .flat_map(|contract| contract.operation.required_capabilities())
        .collect();
    let mut block = BasicBlock::new(BlockId(0));
    let first = contracts.first().expect("barrier fixture has a barrier");
    let ExecutionCapabilityOperationV1::WorkgroupBarrier {
        input_workgroup, ..
    } = &first.operation
    else {
        unreachable!();
    };
    let brand = first.workgroup_brand.unwrap();
    let epoch = first.epoch_before.unwrap();
    block.operations.push(Operation::kernel_context_issue(
        ValueId(0),
        context_type(),
        KernelContextSourceIdentityV1::new([43; 32], [44; 32], [45; 32], [46; 32]),
    ));
    block.operations.push(workgroup_derivation(
        ValueId(0),
        ValueId(1),
        *input_workgroup,
        brand,
        epoch,
        47,
    ));
    let mut workgroup_value = ValueId(1);
    for (next_value, mut contract) in (2_u32..).zip(contracts) {
        contract.operands = vec![workgroup_value];
        let ExecutionCapabilityOperationV1::WorkgroupBarrier {
            output_workgroup, ..
        } = contract.operation
        else {
            unreachable!();
        };
        let output = ValueId(next_value);
        block.operations.push(Operation::new(
            vec![ValueDef::new(
                output,
                capability_type(
                    output_workgroup,
                    contract.workgroup_brand.unwrap(),
                    contract.epoch_after.unwrap(),
                    ExecutionCapabilityRoleV1::Workgroup,
                ),
            )],
            OperationKind::ExecutionCapability(contract),
        ));
        workgroup_value = output;
    }
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut entry =
        Function::kernel_entry("entry", Signature::new(vec![], vec![]), vec![], vec![block]);
    entry.required_capabilities = requirements.clone();
    let mut kernel = Kernel::new(
        "barrier-kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(4, 1, 1));
    let mut module = Module::new("v13-capability-barrier");
    module.functions.push(entry);
    module.required_capabilities = requirements;
    module.kernels.push(kernel);
    module
}

fn collective_contract(
    operation: ExecutionCapabilityOperationV1,
    operands: Vec<ValueId>,
    source: u8,
    before: u8,
    after: Option<u8>,
) -> ExecutionCapabilityOpV1 {
    let (arguments, output) = match &operation {
        ExecutionCapabilityOperationV1::LdsAllocate { workgroup, lds, .. } => {
            (vec![*workgroup], *lds)
        }
        ExecutionCapabilityOperationV1::WorkgroupCollective {
            input_workgroup,
            scratch,
            element,
            transition,
            ..
        } => (vec![*input_workgroup, *scratch, *element], *transition),
        _ => unreachable!(),
    };
    ExecutionCapabilityOpV1 {
        operands,
        signature: ExecutionCapabilitySignatureV1::new(&arguments, output).unwrap(),
        provenance: provenance(),
        workgroup_brand: Some([70; 32]),
        epoch_before: Some([before; 32]),
        epoch_after: after.map(|epoch| [epoch; 32]),
        obligations: ExecutionSafetyObligationsV1::from_bits(required_execution_obligations_v1(
            &operation,
        )),
        source: ExecutionCapabilitySourceV1 {
            function: [71; 32],
            operation: [source; 32],
            block: 0,
        },
        operation,
    }
}

fn workgroup_collective_module(kind: ExecutionCollectiveKindV1) -> Module {
    let layout = ExecutionElementLayoutV1 {
        byte_size: 4,
        byte_alignment: 4,
    };
    let workgroup_type = identity(72);
    let lds_type = identity(73);
    let element_type = identity(74);
    let transition_type = identity(75);
    let lds_operation = ExecutionCapabilityOperationV1::LdsAllocate {
        workgroup: workgroup_type,
        lds: lds_type,
        element: element_type,
        layout,
        elements: 4,
    };
    let collective_operation = ExecutionCapabilityOperationV1::WorkgroupCollective {
        kind,
        input_workgroup: workgroup_type,
        scratch: lds_type,
        element: element_type,
        transition: transition_type,
        value_type: ScalarType::U32,
        layout,
        elements: 4,
    };
    let requirements: BTreeSet<_> = lds_operation
        .required_capabilities()
        .into_iter()
        .chain(collective_operation.required_capabilities())
        .collect();
    let lds_result = Type::ExecutionCapability(ExecutionCapabilityTypeV1 {
        source_type: lds_type,
        provenance: provenance(),
        workgroup_brand: Some([70; 32]),
        epoch: Some([76; 32]),
        role: ExecutionCapabilityRoleV1::Lds {
            element: element_type,
            layout,
            elements: 4,
            state: ExecutionLdsStateV1::Uninitialized,
        },
    });
    let output = Type::slice(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::WriteOnly,
    );
    let output_pointer = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::WriteOnly,
    );
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::kernel_context_issue(
            ValueId(2),
            context_type(),
            KernelContextSourceIdentityV1::new([0x41; 32], [0x42; 32], [0x43; 32], [0x44; 32]),
        ),
        workgroup_derivation(
            ValueId(2),
            ValueId(3),
            workgroup_type,
            [70; 32],
            [76; 32],
            0x45,
        ),
        Operation::new(
            vec![ValueDef::new(ValueId(4), lds_result)],
            OperationKind::ExecutionCapability(collective_contract(
                lds_operation,
                vec![ValueId(3)],
                77,
                76,
                None,
            )),
        ),
        Operation::new(
            vec![
                ValueDef::new(
                    ValueId(5),
                    capability_type(
                        transition_type,
                        [70; 32],
                        [79; 32],
                        ExecutionCapabilityRoleV1::Workgroup,
                    ),
                ),
                ValueDef::new(
                    ValueId(6),
                    capability_type(
                        transition_type,
                        [70; 32],
                        [79; 32],
                        ExecutionCapabilityRoleV1::Lds {
                            element: element_type,
                            layout,
                            elements: 4,
                            state: ExecutionLdsStateV1::Uninitialized,
                        },
                    ),
                ),
                ValueDef::new(ValueId(7), Type::Scalar(ScalarType::U32)),
            ],
            OperationKind::ExecutionCapability(collective_contract(
                collective_operation,
                vec![ValueId(3), ValueId(4), ValueId(0)],
                78,
                76,
                Some(79),
            )),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(8), Type::INDEX),
            OperationKind::Intrinsic(IntrinsicOperation::new(
                IntrinsicKind::InvocationIndex {
                    kind: IndexKind::Local,
                    axis: Axis::X,
                },
                Type::INDEX,
            )),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(9), output_pointer.clone()),
            OperationKind::SliceData { slice: ValueId(1) },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(10), output_pointer),
            OperationKind::GetElementPointer {
                base: ValueId(9),
                offset: ValueId(8),
            },
        ),
        Operation::new(
            Vec::new(),
            OperationKind::Store {
                pointer: ValueId(10),
                value: ValueId(7),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut entry = Function::kernel_entry(
        "entry",
        Signature::new(vec![Type::Scalar(ScalarType::U32), output], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    );
    entry.required_capabilities = requirements.clone();
    let mut kernel = Kernel::new(
        "reduce-kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(4, 1, 1));
    let mut module = Module::new("v13-workgroup-reduce");
    module.functions.push(entry);
    module.kernels.push(kernel);
    module.required_capabilities = requirements;
    module
}

fn subgroup_collective_module(kind: ExecutionCollectiveKindV1) -> Module {
    let workgroup = identity(79);
    let operation = ExecutionCapabilityOperationV1::SubgroupCollective {
        kind,
        subgroup_reference: identity(80),
        subgroup: identity(81),
        epoch: identity(82),
        element: identity(83),
        value_type: ScalarType::U32,
        width: 64,
    };
    let requirements = operation.required_capabilities();
    let contract = ExecutionCapabilityOpV1 {
        operands: vec![ValueId(0)],
        signature: ExecutionCapabilitySignatureV1::new(
            &[identity(80), identity(82), identity(83)],
            identity(83),
        )
        .unwrap(),
        provenance: provenance(),
        workgroup_brand: Some([84; 32]),
        epoch_before: Some([85; 32]),
        epoch_after: None,
        obligations: ExecutionSafetyObligationsV1::from_bits(required_execution_obligations_v1(
            &operation,
        )),
        source: ExecutionCapabilitySourceV1 {
            function: [86; 32],
            operation: [87; 32],
            block: 0,
        },
        operation,
    };
    let output = Type::slice(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::WriteOnly,
    );
    let output_pointer = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::WriteOnly,
    );
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::kernel_context_issue(
            ValueId(2),
            context_type(),
            KernelContextSourceIdentityV1::new([0x51; 32], [0x52; 32], [0x53; 32], [0x54; 32]),
        ),
        workgroup_derivation(ValueId(2), ValueId(3), workgroup, [84; 32], [85; 32], 0x55),
        subgroup_derivation(
            ValueId(3),
            ValueId(4),
            workgroup,
            identity(81),
            [84; 32],
            [85; 32],
            64,
            0x56,
        ),
        Operation::new(
            vec![ValueDef::new(ValueId(5), Type::Scalar(ScalarType::U32))],
            OperationKind::ExecutionCapability(ExecutionCapabilityOpV1 {
                operands: vec![ValueId(4), ValueId(0)],
                ..contract
            }),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(6), Type::INDEX),
            OperationKind::Intrinsic(IntrinsicOperation::new(
                IntrinsicKind::InvocationIndex {
                    kind: IndexKind::Local,
                    axis: Axis::X,
                },
                Type::INDEX,
            )),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(7), output_pointer.clone()),
            OperationKind::SliceData { slice: ValueId(1) },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(8), output_pointer),
            OperationKind::GetElementPointer {
                base: ValueId(7),
                offset: ValueId(6),
            },
        ),
        Operation::new(
            Vec::new(),
            OperationKind::Store {
                pointer: ValueId(8),
                value: ValueId(5),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut entry = Function::kernel_entry(
        "entry",
        Signature::new(vec![Type::Scalar(ScalarType::U32), output], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    );
    entry.required_capabilities = requirements.clone();
    let mut kernel = Kernel::new(
        "subgroup-kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    let mut module = Module::new("v13-subgroup-collective");
    module.functions.push(entry);
    module.kernels.push(kernel);
    module.required_capabilities = requirements;
    module
}

fn kernel_scoped_contract(
    operation: ExecutionCapabilityOperationV1,
    operands: Vec<ValueId>,
    source: u8,
) -> ExecutionCapabilityOpV1 {
    let (arguments, output) = match &operation {
        ExecutionCapabilityOperationV1::PrivateMemoryAllocate { context, view, .. } => {
            (vec![*context], *view)
        }
        ExecutionCapabilityOperationV1::MemoryStore {
            view,
            index,
            element,
            result,
            ..
        } => (vec![*view, *index, *element], *result),
        ExecutionCapabilityOperationV1::MemoryLoad {
            view,
            index,
            option,
            ..
        } => (vec![*view, *index], *option),
        _ => unreachable!(),
    };
    ExecutionCapabilityOpV1 {
        operands,
        signature: ExecutionCapabilitySignatureV1::new(&arguments, output).unwrap(),
        provenance: provenance(),
        workgroup_brand: None,
        epoch_before: None,
        epoch_after: None,
        obligations: ExecutionSafetyObligationsV1::from_bits(required_execution_obligations_v1(
            &operation,
        )),
        source: ExecutionCapabilitySourceV1 {
            function: [88; 32],
            operation: [source; 32],
            block: 0,
        },
        operation,
    }
}

fn private_memory_roundtrip_module() -> Module {
    let layout = ExecutionElementLayoutV1 {
        byte_size: 4,
        byte_alignment: 4,
    };
    let context_identity = identity(90);
    let view_type = identity(91);
    let index_type = identity(92);
    let element_type = identity(93);
    let bool_type = identity(94);
    let option_type = identity(95);
    let allocate = ExecutionCapabilityOperationV1::PrivateMemoryAllocate {
        context: context_identity,
        view: view_type,
        element: element_type,
        layout,
        elements: 4,
    };
    let store = ExecutionCapabilityOperationV1::MemoryStore {
        view: view_type,
        workgroup: None,
        index: index_type,
        element: element_type,
        layout,
        result: bool_type,
        space: ExecutionMemoryAddressSpaceV1::Private,
        access: ExecutionMemoryAccessV1::ExclusiveReadWrite,
    };
    let load = ExecutionCapabilityOperationV1::MemoryLoad {
        view: view_type,
        workgroup: None,
        index: index_type,
        option: option_type,
        element: element_type,
        layout,
        space: ExecutionMemoryAddressSpaceV1::Private,
        access: ExecutionMemoryAccessV1::ExclusiveReadWrite,
    };
    let requirements: BTreeSet<_> = allocate
        .required_capabilities()
        .into_iter()
        .chain(store.required_capabilities())
        .chain(load.required_capabilities())
        .collect();
    let view = Type::ExecutionCapability(ExecutionCapabilityTypeV1 {
        source_type: view_type,
        provenance: provenance(),
        workgroup_brand: None,
        epoch: None,
        role: ExecutionCapabilityRoleV1::MemoryView {
            element: element_type,
            layout,
            space: ExecutionMemoryAddressSpaceV1::Private,
            access: ExecutionMemoryAccessV1::ExclusiveReadWrite,
            extent: ExecutionMemoryExtentV1::Static(4),
            initialization: ExecutionMemoryInitializationV1::FullyInitialized,
            index_space: None,
            atomic_scope: None,
        },
    });
    let output = Type::slice(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::WriteOnly,
    );
    let output_pointer = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::WriteOnly,
    );
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::kernel_context_issue(
            ValueId(3),
            context_type(),
            KernelContextSourceIdentityV1::new([0x61; 32], [0x62; 32], [0x63; 32], [0x64; 32]),
        ),
        Operation::new(
            vec![ValueDef::new(ValueId(4), view)],
            OperationKind::ExecutionCapability(kernel_scoped_contract(
                allocate,
                vec![ValueId(3)],
                96,
            )),
        ),
        Operation::new(
            vec![ValueDef::new(ValueId(5), Type::BOOL)],
            OperationKind::ExecutionCapability(kernel_scoped_contract(
                store,
                vec![ValueId(4), ValueId(0), ValueId(1)],
                97,
            )),
        ),
        Operation::new(
            vec![
                ValueDef::new(ValueId(6), Type::Scalar(ScalarType::U32)),
                ValueDef::new(ValueId(7), Type::BOOL),
            ],
            OperationKind::ExecutionCapability(kernel_scoped_contract(
                load,
                vec![ValueId(4), ValueId(0)],
                98,
            )),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(8), output_pointer.clone()),
            OperationKind::SliceData { slice: ValueId(2) },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(9), Type::INDEX),
            OperationKind::Constant(Constant::Index(0)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(10), output_pointer),
            OperationKind::GetElementPointer {
                base: ValueId(8),
                offset: ValueId(9),
            },
        ),
        Operation::new(
            Vec::new(),
            OperationKind::Store {
                pointer: ValueId(10),
                value: ValueId(6),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut entry = Function::kernel_entry(
        "entry",
        Signature::new(
            vec![Type::INDEX, Type::Scalar(ScalarType::U32), output],
            vec![],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![block],
    );
    entry.required_capabilities = requirements.clone();
    let mut module = Module::new("v13-private-memory-roundtrip");
    module.functions.push(entry);
    module.kernels.push(Kernel::new(
        "private-kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module.required_capabilities = requirements;
    module
}

fn raw_memory_roundtrip_module() -> Module {
    let layout = ExecutionElementLayoutV1 {
        byte_size: 4,
        byte_alignment: 4,
    };
    let extent = ExecutionDynamicExtentV1 {
        operand: 2,
        source_argument: 2,
        source_type: identity(102),
        value_type: ScalarType::Index,
        upper_bound: 4,
        bound_check_operand: 3,
        nonnegative_check_operand: None,
    };
    let view_identity = identity(103);
    let element_identity = identity(104);
    let operation = ExecutionCapabilityOperationV1::RawMemoryBind {
        authority: identity(100),
        pointer: identity(101),
        length: identity(102),
        extent,
        view: view_identity,
        element: element_identity,
        layout,
        space: ExecutionMemoryAddressSpaceV1::Private,
        access: ExecutionMemoryAccessV1::ExclusiveReadWrite,
        index_space: None,
        atomic_scope: None,
        unsafe_obligation: identity(105),
    };
    let requirements = operation.required_capabilities();
    let view = Type::ExecutionCapability(ExecutionCapabilityTypeV1 {
        source_type: view_identity,
        provenance: provenance(),
        workgroup_brand: None,
        epoch: None,
        role: ExecutionCapabilityRoleV1::MemoryView {
            element: element_identity,
            layout,
            space: ExecutionMemoryAddressSpaceV1::Private,
            access: ExecutionMemoryAccessV1::ExclusiveReadWrite,
            extent: ExecutionMemoryExtentV1::Dynamic(extent),
            initialization: ExecutionMemoryInitializationV1::FullyInitialized,
            index_space: None,
            atomic_scope: None,
        },
    });
    let bind = ExecutionCapabilityOpV1 {
        operands: vec![ValueId(8), ValueId(4), ValueId(5), ValueId(7)],
        signature: ExecutionCapabilitySignatureV1::new(
            &[identity(100), identity(101), identity(102), identity(105)],
            view_identity,
        )
        .unwrap(),
        provenance: provenance(),
        workgroup_brand: None,
        epoch_before: None,
        epoch_after: None,
        obligations: ExecutionSafetyObligationsV1::from_bits(required_execution_obligations_v1(
            &operation,
        )),
        source: ExecutionCapabilitySourceV1 {
            function: [106; 32],
            operation: [107; 32],
            block: 0,
        },
        operation,
    };
    let store = ExecutionCapabilityOperationV1::MemoryStore {
        view: view_identity,
        workgroup: None,
        index: identity(108),
        element: element_identity,
        layout,
        result: identity(109),
        space: ExecutionMemoryAddressSpaceV1::Private,
        access: ExecutionMemoryAccessV1::ExclusiveReadWrite,
    };
    let load = ExecutionCapabilityOperationV1::MemoryLoad {
        view: view_identity,
        workgroup: None,
        index: identity(108),
        option: identity(110),
        element: element_identity,
        layout,
        space: ExecutionMemoryAddressSpaceV1::Private,
        access: ExecutionMemoryAccessV1::ExclusiveReadWrite,
    };
    let requirements: BTreeSet<_> = requirements
        .into_iter()
        .chain(store.required_capabilities())
        .chain(load.required_capabilities())
        .collect();
    let pointer = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Private,
        AccessMode::ReadWrite,
    );
    let output = Type::slice(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::WriteOnly,
    );
    let output_pointer = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::WriteOnly,
    );
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(3), Type::INDEX),
            OperationKind::Constant(Constant::Index(4)),
        ),
        Operation::new(
            vec![ValueDef::new(ValueId(4), pointer)],
            OperationKind::Alloca {
                element: Type::Scalar(ScalarType::U32),
                count: Some(ValueId(3)),
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(5), Type::INDEX),
            OperationKind::Constant(Constant::Index(4)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(6), Type::INDEX),
            OperationKind::Constant(Constant::Index(4)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(7), Type::BOOL),
            OperationKind::Compare {
                predicate: ComparePredicate::LessThanOrEqual,
                lhs: ValueId(5),
                rhs: ValueId(6),
            },
        ),
        Operation::kernel_context_issue(
            ValueId(8),
            context_type(),
            KernelContextSourceIdentityV1::new([0x65; 32], [0x66; 32], [0x67; 32], [0x68; 32]),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(9), view),
            OperationKind::ExecutionCapability(bind),
        ),
        Operation::new(
            vec![ValueDef::new(ValueId(10), Type::BOOL)],
            OperationKind::ExecutionCapability(kernel_scoped_contract(
                store,
                vec![ValueId(9), ValueId(0), ValueId(1)],
                111,
            )),
        ),
        Operation::new(
            vec![
                ValueDef::new(ValueId(11), Type::Scalar(ScalarType::U32)),
                ValueDef::new(ValueId(12), Type::BOOL),
            ],
            OperationKind::ExecutionCapability(kernel_scoped_contract(
                load,
                vec![ValueId(9), ValueId(0)],
                112,
            )),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(13), output_pointer.clone()),
            OperationKind::SliceData { slice: ValueId(2) },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(14), Type::INDEX),
            OperationKind::Constant(Constant::Index(0)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(15), output_pointer),
            OperationKind::GetElementPointer {
                base: ValueId(13),
                offset: ValueId(14),
            },
        ),
        Operation::new(
            Vec::new(),
            OperationKind::Store {
                pointer: ValueId(15),
                value: ValueId(11),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut entry = Function::kernel_entry(
        "entry",
        Signature::new(
            vec![Type::INDEX, Type::Scalar(ScalarType::U32), output],
            vec![],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![block],
    );
    entry.required_capabilities = requirements.clone();
    let mut module = Module::new("v13-raw-memory-roundtrip");
    module.functions.push(entry);
    module.kernels.push(Kernel::new(
        "raw-memory-kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module.required_capabilities = requirements;
    module
}

fn workgroup_contract(
    operation: ExecutionCapabilityOperationV1,
    operands: Vec<ValueId>,
    arguments: &[ExecutionTypeIdentityV1],
    output: ExecutionTypeIdentityV1,
    before: u8,
    after: Option<u8>,
    source: u8,
) -> ExecutionCapabilityOpV1 {
    ExecutionCapabilityOpV1 {
        operands,
        signature: ExecutionCapabilitySignatureV1::new(arguments, output).unwrap(),
        provenance: provenance(),
        workgroup_brand: Some([120; 32]),
        epoch_before: Some([before; 32]),
        epoch_after: after.map(|epoch| [epoch; 32]),
        obligations: ExecutionSafetyObligationsV1::from_bits(required_execution_obligations_v1(
            &operation,
        )),
        source: ExecutionCapabilitySourceV1 {
            function: [121; 32],
            operation: [source; 32],
            block: 0,
        },
        operation,
    }
}

fn async_copy_roundtrip_module() -> Module {
    let layout = ExecutionElementLayoutV1 {
        byte_size: 4,
        byte_alignment: 4,
    };
    let workgroup = identity(122);
    let source_reference = identity(123);
    let source_view = identity(124);
    let length = identity(125);
    let index = identity(126);
    let allocated_lds = identity(127);
    let pending = identity(128);
    let published_lds = identity(129);
    let transition = identity(130);
    let option = identity(131);
    let element = identity(132);
    let unsafe_obligation = identity(133);
    let extent = ExecutionDynamicExtentV1 {
        operand: 2,
        source_argument: 2,
        source_type: length,
        value_type: ScalarType::Index,
        upper_bound: 4,
        bound_check_operand: 3,
        nonnegative_check_operand: None,
    };
    let bind_source = ExecutionCapabilityOperationV1::RawMemoryBind {
        authority: workgroup,
        pointer: source_reference,
        length,
        extent,
        view: source_view,
        element,
        layout,
        space: ExecutionMemoryAddressSpaceV1::Global,
        access: ExecutionMemoryAccessV1::ReadOnly,
        index_space: None,
        atomic_scope: None,
        unsafe_obligation,
    };
    let allocate = ExecutionCapabilityOperationV1::LdsAllocate {
        workgroup,
        lds: allocated_lds,
        element,
        layout,
        elements: 4,
    };
    let copy = ExecutionCapabilityOperationV1::AsyncCopy {
        workgroup,
        source_reference,
        source: source_view,
        index,
        destination: allocated_lds,
        pending,
        element,
        layout,
        elements: 4,
    };
    let wait = ExecutionCapabilityOperationV1::AsyncWait {
        input_workgroup: workgroup,
        pending,
        output_lds: published_lds,
        transition,
        element,
        layout,
        elements: 4,
    };
    let read = ExecutionCapabilityOperationV1::LdsReadPublished {
        lds_reference: transition,
        lds: published_lds,
        workgroup: transition,
        index,
        option,
        element,
        layout,
        elements: 4,
    };
    let requirements: BTreeSet<_> = bind_source
        .required_capabilities()
        .into_iter()
        .chain(allocate.required_capabilities())
        .chain(copy.required_capabilities())
        .chain(wait.required_capabilities())
        .chain(read.required_capabilities())
        .collect();
    let lds_type = |source_type, state, epoch| {
        capability_type(
            source_type,
            [120; 32],
            [epoch; 32],
            ExecutionCapabilityRoleV1::Lds {
                element,
                layout,
                elements: 4,
                state,
            },
        )
    };
    let pending_type = capability_type(
        pending,
        [120; 32],
        [131; 32],
        ExecutionCapabilityRoleV1::PendingAsyncCopy {
            element,
            layout,
            elements: 4,
        },
    );
    let source_view_type = capability_type(
        source_view,
        [120; 32],
        [131; 32],
        ExecutionCapabilityRoleV1::MemoryView {
            element,
            layout,
            space: ExecutionMemoryAddressSpaceV1::Global,
            access: ExecutionMemoryAccessV1::ReadOnly,
            extent: ExecutionMemoryExtentV1::Dynamic(extent),
            initialization: ExecutionMemoryInitializationV1::FullyInitialized,
            index_space: None,
            atomic_scope: None,
        },
    );
    let source = Type::slice(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    );
    let output = Type::slice(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::WriteOnly,
    );
    let output_pointer = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::WriteOnly,
    );
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::kernel_context_issue(
            ValueId(2),
            context_type(),
            KernelContextSourceIdentityV1::new([0x69; 32], [0x6a; 32], [0x6b; 32], [0x6c; 32]),
        ),
        workgroup_derivation(
            ValueId(2),
            ValueId(3),
            workgroup,
            [120; 32],
            [131; 32],
            0x6d,
        ),
        Operation::effect_free(
            ValueDef::new(
                ValueId(4),
                Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    AccessMode::ReadWrite,
                ),
            ),
            OperationKind::SliceData { slice: ValueId(0) },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(5), Type::INDEX),
            OperationKind::SliceLength { slice: ValueId(0) },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(6), Type::INDEX),
            OperationKind::Constant(Constant::Index(4)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(7), Type::BOOL),
            OperationKind::Compare {
                predicate: ComparePredicate::LessThanOrEqual,
                lhs: ValueId(5),
                rhs: ValueId(6),
            },
        ),
        Operation::new(
            vec![ValueDef::new(ValueId(8), source_view_type)],
            OperationKind::ExecutionCapability(workgroup_contract(
                bind_source,
                vec![ValueId(3), ValueId(4), ValueId(5), ValueId(7)],
                &[workgroup, source_reference, length, unsafe_obligation],
                source_view,
                131,
                None,
                132,
            )),
        ),
        Operation::new(
            vec![ValueDef::new(
                ValueId(9),
                lds_type(allocated_lds, ExecutionLdsStateV1::Uninitialized, 131),
            )],
            OperationKind::ExecutionCapability(workgroup_contract(
                allocate,
                vec![ValueId(3)],
                &[workgroup],
                allocated_lds,
                131,
                None,
                133,
            )),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(10), Type::INDEX),
            OperationKind::Constant(Constant::Index(1)),
        ),
        Operation::new(
            vec![ValueDef::new(ValueId(11), pending_type)],
            OperationKind::ExecutionCapability(workgroup_contract(
                copy,
                vec![ValueId(3), ValueId(8), ValueId(10), ValueId(9)],
                &[workgroup, source_reference, index, allocated_lds],
                pending,
                131,
                None,
                134,
            )),
        ),
        Operation::new(
            vec![
                ValueDef::new(
                    ValueId(12),
                    capability_type(
                        transition,
                        [120; 32],
                        [134; 32],
                        ExecutionCapabilityRoleV1::Workgroup,
                    ),
                ),
                ValueDef::new(
                    ValueId(13),
                    lds_type(published_lds, ExecutionLdsStateV1::Published, 134),
                ),
            ],
            OperationKind::ExecutionCapability(workgroup_contract(
                wait,
                vec![ValueId(3), ValueId(11)],
                &[workgroup, pending],
                transition,
                131,
                Some(134),
                135,
            )),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(14), Type::INDEX),
            OperationKind::Intrinsic(IntrinsicOperation::new(
                IntrinsicKind::InvocationIndex {
                    kind: IndexKind::Local,
                    axis: Axis::X,
                },
                Type::INDEX,
            )),
        ),
        Operation::new(
            vec![
                ValueDef::new(ValueId(15), Type::Scalar(ScalarType::U32)),
                ValueDef::new(ValueId(16), Type::BOOL),
            ],
            OperationKind::ExecutionCapability(workgroup_contract(
                read,
                vec![ValueId(13), ValueId(12), ValueId(14)],
                &[transition, transition, index],
                option,
                134,
                None,
                136,
            )),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(17), output_pointer.clone()),
            OperationKind::SliceData { slice: ValueId(1) },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(18), output_pointer),
            OperationKind::GetElementPointer {
                base: ValueId(17),
                offset: ValueId(14),
            },
        ),
        Operation::new(
            Vec::new(),
            OperationKind::Store {
                pointer: ValueId(18),
                value: ValueId(15),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut entry = Function::kernel_entry(
        "entry",
        Signature::new(vec![source, output], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    );
    entry.required_capabilities = requirements.clone();
    let mut kernel = Kernel::new(
        "async-copy-kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(4, 1, 1));
    let mut module = Module::new("v13-async-copy-roundtrip");
    module.functions.push(entry);
    module.kernels.push(kernel);
    module.required_capabilities = requirements;
    module
}

fn scoped_atomic_fetch_add_module() -> Module {
    let layout = ExecutionElementLayoutV1 {
        byte_size: 4,
        byte_alignment: 4,
    };
    let extent = ExecutionDynamicExtentV1 {
        operand: 2,
        source_argument: 2,
        source_type: identity(141),
        value_type: ScalarType::Index,
        upper_bound: 4,
        bound_check_operand: 3,
        nonnegative_check_operand: None,
    };
    let authority = identity(139);
    let pointer = identity(140);
    let view_identity = identity(142);
    let unsafe_obligation = identity(143);
    let element = identity(144);
    let index = identity(145);
    let option = identity(146);
    let raw = ExecutionCapabilityOperationV1::RawMemoryBind {
        authority,
        pointer,
        length: identity(141),
        extent,
        view: view_identity,
        element,
        layout,
        space: ExecutionMemoryAddressSpaceV1::Global,
        access: ExecutionMemoryAccessV1::AtomicReadWrite,
        index_space: None,
        atomic_scope: Some(ExecutionMemoryScopeV1::Workgroup),
        unsafe_obligation,
    };
    let bind = ExecutionCapabilityOperationV1::Atomic {
        kind: ExecutionAtomicKindV1::BindGlobalLocation,
        authority,
        location_input: view_identity,
        location: identity(147),
        element,
        operand: Some(index),
        replacement: None,
        result: option,
        value_type: ScalarType::U32,
        address_space: ExecutionMemoryAddressSpaceV1::Global,
        scope: ExecutionMemoryScopeV1::Workgroup,
        success: None,
        failure: None,
    };
    let fetch_add = ExecutionCapabilityOperationV1::Atomic {
        kind: ExecutionAtomicKindV1::FetchAdd,
        authority,
        location_input: identity(147),
        location: identity(147),
        element,
        operand: Some(element),
        replacement: None,
        result: element,
        value_type: ScalarType::U32,
        address_space: ExecutionMemoryAddressSpaceV1::Global,
        scope: ExecutionMemoryScopeV1::Workgroup,
        success: Some(ExecutionMemoryOrderingV1::AcquireRelease),
        failure: None,
    };
    let requirements: BTreeSet<_> = raw
        .required_capabilities()
        .into_iter()
        .chain(bind.required_capabilities())
        .chain(fetch_add.required_capabilities())
        .collect();
    let view_type = capability_type(
        view_identity,
        [120; 32],
        [150; 32],
        ExecutionCapabilityRoleV1::MemoryView {
            element,
            layout,
            space: ExecutionMemoryAddressSpaceV1::Global,
            access: ExecutionMemoryAccessV1::AtomicReadWrite,
            extent: ExecutionMemoryExtentV1::Dynamic(extent),
            initialization: ExecutionMemoryInitializationV1::FullyInitialized,
            index_space: None,
            atomic_scope: Some(ExecutionMemoryScopeV1::Workgroup),
        },
    );
    let location_type = capability_type(
        identity(147),
        [120; 32],
        [150; 32],
        ExecutionCapabilityRoleV1::ScopedAtomic {
            element,
            space: ExecutionMemoryAddressSpaceV1::Global,
            scope: ExecutionMemoryScopeV1::Workgroup,
        },
    );
    let buffer = Type::slice(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    );
    let buffer_pointer = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    );
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::kernel_context_issue(
            ValueId(1),
            context_type(),
            KernelContextSourceIdentityV1::new([0x6e; 32], [0x6f; 32], [0x70; 32], [0x71; 32]),
        ),
        workgroup_derivation(
            ValueId(1),
            ValueId(2),
            authority,
            [120; 32],
            [150; 32],
            0x72,
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(3), buffer_pointer),
            OperationKind::SliceData { slice: ValueId(0) },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(4), Type::INDEX),
            OperationKind::SliceLength { slice: ValueId(0) },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(5), Type::INDEX),
            OperationKind::Constant(Constant::Index(4)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(6), Type::BOOL),
            OperationKind::Compare {
                predicate: ComparePredicate::LessThanOrEqual,
                lhs: ValueId(4),
                rhs: ValueId(5),
            },
        ),
        Operation::new(
            vec![ValueDef::new(ValueId(7), view_type)],
            OperationKind::ExecutionCapability(workgroup_contract(
                raw,
                vec![ValueId(2), ValueId(3), ValueId(4), ValueId(6)],
                &[authority, pointer, identity(141), unsafe_obligation],
                view_identity,
                150,
                None,
                151,
            )),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(8), Type::INDEX),
            OperationKind::Constant(Constant::Index(0)),
        ),
        Operation::new(
            vec![
                ValueDef::new(ValueId(9), location_type),
                ValueDef::new(ValueId(10), Type::BOOL),
            ],
            OperationKind::ExecutionCapability(workgroup_contract(
                bind,
                vec![ValueId(2), ValueId(7), ValueId(8)],
                &[authority, view_identity, index],
                option,
                150,
                None,
                152,
            )),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(11), Type::Scalar(ScalarType::U32)),
            OperationKind::Constant(Constant::U32(1)),
        ),
        Operation::new(
            vec![ValueDef::new(ValueId(12), Type::Scalar(ScalarType::U32))],
            OperationKind::ExecutionCapability(workgroup_contract(
                fetch_add,
                vec![ValueId(2), ValueId(9), ValueId(11)],
                &[authority, identity(147), element],
                element,
                150,
                None,
                153,
            )),
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut entry = Function::kernel_entry(
        "entry",
        Signature::new(vec![buffer], vec![]),
        vec![ValueId(0)],
        vec![block],
    );
    entry.required_capabilities = requirements.clone();
    let mut kernel = Kernel::new(
        "atomic-kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(4, 1, 1));
    let mut module = Module::new("v13-scoped-atomic-fetch-add");
    module.functions.push(entry);
    module.kernels.push(kernel);
    module.required_capabilities = requirements;
    module
}

fn lds_epoch_roundtrip_module() -> Module {
    let layout = ExecutionElementLayoutV1 {
        byte_size: 4,
        byte_alignment: 4,
    };
    let workgroup = identity(160);
    let uninitialized = identity(161);
    let initialized = identity(162);
    let published = identity(163);
    let transition = identity(164);
    let element = identity(165);
    let index = identity(166);
    let option = identity(167);
    let allocate = ExecutionCapabilityOperationV1::LdsAllocate {
        workgroup,
        lds: uninitialized,
        element,
        layout,
        elements: 4,
    };
    let initialize = ExecutionCapabilityOperationV1::LdsInitializeByInvocation {
        input_lds: uninitialized,
        workgroup,
        output_lds: initialized,
        element,
        layout,
        elements: 4,
    };
    let publish = ExecutionCapabilityOperationV1::LdsPublish {
        input_workgroup: workgroup,
        input_lds: initialized,
        output_lds: published,
        transition,
        element,
        layout,
        elements: 4,
    };
    let read = ExecutionCapabilityOperationV1::LdsReadPublished {
        lds_reference: transition,
        lds: published,
        workgroup: transition,
        index,
        option,
        element,
        layout,
        elements: 4,
    };
    let requirements = [&allocate, &initialize, &publish, &read]
        .into_iter()
        .flat_map(|operation| operation.required_capabilities())
        .collect::<BTreeSet<_>>();
    let lds = |source_type, epoch, state| {
        capability_type(
            source_type,
            [120; 32],
            [epoch; 32],
            ExecutionCapabilityRoleV1::Lds {
                element,
                layout,
                elements: 4,
                state,
            },
        )
    };
    let output = Type::slice(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::WriteOnly,
    );
    let output_pointer = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::WriteOnly,
    );
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::kernel_context_issue(
            ValueId(2),
            context_type(),
            KernelContextSourceIdentityV1::new([0x91; 32], [0x92; 32], [0x93; 32], [0x94; 32]),
        ),
        workgroup_derivation(
            ValueId(2),
            ValueId(3),
            workgroup,
            [120; 32],
            [168; 32],
            0x95,
        ),
        Operation::new(
            vec![ValueDef::new(
                ValueId(4),
                lds(uninitialized, 168, ExecutionLdsStateV1::Uninitialized),
            )],
            OperationKind::ExecutionCapability(workgroup_contract(
                allocate,
                vec![ValueId(3)],
                &[workgroup],
                uninitialized,
                168,
                None,
                0x96,
            )),
        ),
        Operation::new(
            vec![ValueDef::new(
                ValueId(5),
                lds(initialized, 168, ExecutionLdsStateV1::InvocationInitialized),
            )],
            OperationKind::ExecutionCapability(workgroup_contract(
                initialize,
                vec![ValueId(4), ValueId(3), ValueId(0)],
                &[uninitialized, workgroup, element],
                initialized,
                168,
                None,
                0x97,
            )),
        ),
        Operation::new(
            vec![
                ValueDef::new(
                    ValueId(6),
                    capability_type(
                        transition,
                        [120; 32],
                        [169; 32],
                        ExecutionCapabilityRoleV1::Workgroup,
                    ),
                ),
                ValueDef::new(
                    ValueId(7),
                    lds(published, 169, ExecutionLdsStateV1::Published),
                ),
            ],
            OperationKind::ExecutionCapability(workgroup_contract(
                publish,
                vec![ValueId(3), ValueId(5)],
                &[workgroup, initialized],
                transition,
                168,
                Some(169),
                0x98,
            )),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(8), Type::INDEX),
            OperationKind::Intrinsic(IntrinsicOperation::new(
                IntrinsicKind::InvocationIndex {
                    kind: IndexKind::Local,
                    axis: Axis::X,
                },
                Type::INDEX,
            )),
        ),
        Operation::new(
            vec![
                ValueDef::new(ValueId(9), Type::Scalar(ScalarType::U32)),
                ValueDef::new(ValueId(10), Type::BOOL),
            ],
            OperationKind::ExecutionCapability(workgroup_contract(
                read,
                vec![ValueId(7), ValueId(6), ValueId(8)],
                &[transition, transition, index],
                option,
                169,
                None,
                0x99,
            )),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(11), output_pointer.clone()),
            OperationKind::SliceData { slice: ValueId(1) },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(12), output_pointer),
            OperationKind::GetElementPointer {
                base: ValueId(11),
                offset: ValueId(8),
            },
        ),
        Operation::new(
            Vec::new(),
            OperationKind::Store {
                pointer: ValueId(12),
                value: ValueId(9),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut entry = Function::kernel_entry(
        "entry",
        Signature::new(vec![Type::Scalar(ScalarType::U32), output], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    );
    entry.required_capabilities = requirements.clone();
    let mut kernel = Kernel::new(
        "lds-epoch-kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(4, 1, 1));
    let mut module = Module::new("v13-lds-epoch-roundtrip");
    module.functions.push(entry);
    module.kernels.push(kernel);
    module.required_capabilities = requirements;
    module
}

fn synchronization_carrier_module() -> Module {
    let workgroup = identity(180);
    let subgroup = identity(181);
    let transition = identity(182);
    let workgroup_fence = ExecutionCapabilityOperationV1::WorkgroupFence {
        workgroup,
        result: identity(183),
        semantics: ExecutionMemorySemanticsV1 {
            scope: ExecutionMemoryScopeV1::Workgroup,
            ordering: ExecutionMemoryOrderingV1::AcquireRelease,
            spaces: ExecutionMemorySpacesV1::Workgroup,
        },
    };
    let subgroup_fence = ExecutionCapabilityOperationV1::SubgroupFence {
        semantics: ExecutionMemorySemanticsV1 {
            scope: ExecutionMemoryScopeV1::Subgroup,
            ordering: ExecutionMemoryOrderingV1::AcquireRelease,
            spaces: ExecutionMemorySpacesV1::Workgroup,
        },
        subgroup_reference: subgroup,
        subgroup,
        epoch: identity(184),
        result: identity(185),
        width: 64,
    };
    let subgroup_barrier = ExecutionCapabilityOperationV1::SubgroupBarrier {
        input_workgroup: workgroup,
        semantics: ExecutionMemorySemanticsV1 {
            scope: ExecutionMemoryScopeV1::Subgroup,
            ordering: ExecutionMemoryOrderingV1::AcquireRelease,
            spaces: ExecutionMemorySpacesV1::Workgroup,
        },
        subgroup,
        transition,
        width: 64,
    };
    let requirements = [&workgroup_fence, &subgroup_fence, &subgroup_barrier]
        .into_iter()
        .flat_map(|operation| operation.required_capabilities())
        .collect::<BTreeSet<_>>();
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::kernel_context_issue(
            ValueId(0),
            context_type(),
            KernelContextSourceIdentityV1::new([0xa1; 32], [0xa2; 32], [0xa3; 32], [0xa4; 32]),
        ),
        workgroup_derivation(
            ValueId(0),
            ValueId(1),
            workgroup,
            [120; 32],
            [186; 32],
            0xa5,
        ),
        Operation::new(
            Vec::new(),
            OperationKind::ExecutionCapability(workgroup_contract(
                workgroup_fence,
                vec![ValueId(1)],
                &[workgroup],
                identity(183),
                186,
                None,
                0xa6,
            )),
        ),
        subgroup_derivation(
            ValueId(1),
            ValueId(2),
            workgroup,
            subgroup,
            [120; 32],
            [186; 32],
            64,
            0xa7,
        ),
        Operation::new(
            Vec::new(),
            OperationKind::ExecutionCapability(workgroup_contract(
                subgroup_fence,
                vec![ValueId(2)],
                &[subgroup, identity(184)],
                identity(185),
                186,
                None,
                0xa8,
            )),
        ),
        Operation::new(
            vec![
                ValueDef::new(
                    ValueId(3),
                    capability_type(
                        transition,
                        [120; 32],
                        [187; 32],
                        ExecutionCapabilityRoleV1::Workgroup,
                    ),
                ),
                ValueDef::new(
                    ValueId(4),
                    capability_type(
                        transition,
                        [120; 32],
                        [187; 32],
                        ExecutionCapabilityRoleV1::Subgroup { width: 64 },
                    ),
                ),
            ],
            OperationKind::ExecutionCapability(workgroup_contract(
                subgroup_barrier,
                vec![ValueId(1), ValueId(2)],
                &[workgroup, subgroup],
                transition,
                186,
                Some(187),
                0xa9,
            )),
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut entry =
        Function::kernel_entry("entry", Signature::new(vec![], vec![]), vec![], vec![block]);
    entry.required_capabilities = requirements.clone();
    let mut kernel = Kernel::new(
        "synchronization-kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    let mut module = Module::new("v13-synchronization-carriers");
    module.functions.push(entry);
    module.kernels.push(kernel);
    module.required_capabilities = requirements;
    module
}

fn workgroup_memory_roundtrip_module() -> Module {
    let layout = ExecutionElementLayoutV1 {
        byte_size: 4,
        byte_alignment: 4,
    };
    let workgroup = identity(190);
    let view = identity(191);
    let witness = identity(192);
    let index_space = identity(193);
    let published = identity(194);
    let transition = identity(195);
    let element = identity(196);
    let store_result = identity(197);
    let load_option = identity(198);
    let allocate = ExecutionCapabilityOperationV1::WorkgroupMemoryAllocate {
        workgroup,
        view,
        element,
        layout,
        elements: 4,
        index_space,
    };
    let index = ExecutionCapabilityOperationV1::WorkgroupMemoryIndex { workgroup, witness };
    let store = ExecutionCapabilityOperationV1::MemoryStore {
        view,
        workgroup: Some(workgroup),
        index: witness,
        element,
        layout,
        result: store_result,
        space: ExecutionMemoryAddressSpaceV1::Workgroup,
        access: ExecutionMemoryAccessV1::DisjointWrite,
    };
    let publish = ExecutionCapabilityOperationV1::WorkgroupMemoryPublish {
        input_workgroup: workgroup,
        input_view: view,
        output_view: published,
        transition,
        element,
        layout,
    };
    let load = ExecutionCapabilityOperationV1::MemoryLoad {
        view: published,
        workgroup: Some(transition),
        index: identity(199),
        option: load_option,
        element,
        layout,
        space: ExecutionMemoryAddressSpaceV1::Workgroup,
        access: ExecutionMemoryAccessV1::ReadOnly,
    };
    let requirements = [&allocate, &index, &store, &publish, &load]
        .into_iter()
        .flat_map(|operation| operation.required_capabilities())
        .collect::<BTreeSet<_>>();
    let write_view = capability_type(
        view,
        [120; 32],
        [200; 32],
        ExecutionCapabilityRoleV1::MemoryView {
            element,
            layout,
            space: ExecutionMemoryAddressSpaceV1::Workgroup,
            access: ExecutionMemoryAccessV1::DisjointWrite,
            extent: ExecutionMemoryExtentV1::Static(4),
            initialization: ExecutionMemoryInitializationV1::Uninitialized,
            index_space: Some(index_space),
            atomic_scope: None,
        },
    );
    let read_view = capability_type(
        published,
        [120; 32],
        [201; 32],
        ExecutionCapabilityRoleV1::MemoryView {
            element,
            layout,
            space: ExecutionMemoryAddressSpaceV1::Workgroup,
            access: ExecutionMemoryAccessV1::ReadOnly,
            extent: ExecutionMemoryExtentV1::Static(4),
            initialization: ExecutionMemoryInitializationV1::Published,
            index_space: None,
            atomic_scope: None,
        },
    );
    let output = Type::slice(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::WriteOnly,
    );
    let output_pointer = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::WriteOnly,
    );
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::kernel_context_issue(
            ValueId(2),
            context_type(),
            KernelContextSourceIdentityV1::new([0xb1; 32], [0xb2; 32], [0xb3; 32], [0xb4; 32]),
        ),
        workgroup_derivation(
            ValueId(2),
            ValueId(3),
            workgroup,
            [120; 32],
            [200; 32],
            0xb5,
        ),
        Operation::new(
            vec![ValueDef::new(ValueId(4), write_view)],
            OperationKind::ExecutionCapability(workgroup_contract(
                allocate,
                vec![ValueId(3)],
                &[workgroup],
                view,
                200,
                None,
                0xb6,
            )),
        ),
        Operation::new(
            vec![ValueDef::new(
                ValueId(5),
                capability_type(
                    witness,
                    [120; 32],
                    [200; 32],
                    ExecutionCapabilityRoleV1::WorkgroupMemoryIndex,
                ),
            )],
            OperationKind::ExecutionCapability(workgroup_contract(
                index,
                vec![ValueId(3)],
                &[workgroup],
                witness,
                200,
                None,
                0xb7,
            )),
        ),
        Operation::new(
            vec![ValueDef::new(ValueId(6), Type::BOOL)],
            OperationKind::ExecutionCapability(workgroup_contract(
                store,
                vec![ValueId(4), ValueId(3), ValueId(5), ValueId(0)],
                &[view, workgroup, witness, element],
                store_result,
                200,
                None,
                0xb8,
            )),
        ),
        Operation::new(
            vec![
                ValueDef::new(
                    ValueId(7),
                    capability_type(
                        transition,
                        [120; 32],
                        [201; 32],
                        ExecutionCapabilityRoleV1::Workgroup,
                    ),
                ),
                ValueDef::new(ValueId(8), read_view),
            ],
            OperationKind::ExecutionCapability(workgroup_contract(
                publish,
                vec![ValueId(3), ValueId(4)],
                &[workgroup, view],
                transition,
                200,
                Some(201),
                0xb9,
            )),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(9), Type::INDEX),
            OperationKind::Intrinsic(IntrinsicOperation::new(
                IntrinsicKind::InvocationIndex {
                    kind: IndexKind::Local,
                    axis: Axis::X,
                },
                Type::INDEX,
            )),
        ),
        Operation::new(
            vec![
                ValueDef::new(ValueId(10), Type::Scalar(ScalarType::U32)),
                ValueDef::new(ValueId(11), Type::BOOL),
            ],
            OperationKind::ExecutionCapability(workgroup_contract(
                load,
                vec![ValueId(8), ValueId(7), ValueId(9)],
                &[published, transition, identity(199)],
                load_option,
                201,
                None,
                0xba,
            )),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(12), output_pointer.clone()),
            OperationKind::SliceData { slice: ValueId(1) },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(13), output_pointer),
            OperationKind::GetElementPointer {
                base: ValueId(12),
                offset: ValueId(9),
            },
        ),
        Operation::new(
            Vec::new(),
            OperationKind::Store {
                pointer: ValueId(13),
                value: ValueId(10),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut entry = Function::kernel_entry(
        "entry",
        Signature::new(vec![Type::Scalar(ScalarType::U32), output], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    );
    entry.required_capabilities = requirements.clone();
    let mut kernel = Kernel::new(
        "workgroup-memory-kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(4, 1, 1));
    let mut module = Module::new("v13-workgroup-memory-roundtrip");
    module.functions.push(entry);
    module.kernels.push(kernel);
    module.required_capabilities = requirements;
    module
}

fn non_matrix_execution_modules() -> Vec<Module> {
    vec![
        lds_epoch_roundtrip_module(),
        synchronization_carrier_module(),
        barrier_module(vec![barrier_contract(40, 41, 42)]),
        workgroup_collective_module(ExecutionCollectiveKindV1::ReduceSum),
        subgroup_collective_module(ExecutionCollectiveKindV1::ReduceSum),
        private_memory_roundtrip_module(),
        raw_memory_roundtrip_module(),
        async_copy_roundtrip_module(),
        scoped_atomic_fetch_add_module(),
        workgroup_memory_roundtrip_module(),
    ]
}

fn execution_families(module: &Module) -> BTreeSet<SimulationExecutionCapabilityFamilyV13> {
    module
        .functions
        .iter()
        .filter_map(|function| function.body.as_ref())
        .flat_map(|body| &body.blocks)
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match &operation.kind {
            OperationKind::ExecutionCapability(contract) => Some(
                SimulationExecutionCapabilityFamilyV13::of(&contract.operation),
            ),
            _ => None,
        })
        .collect()
}

fn assert_every_provenance_substitution_is_rejected(
    module: Module,
) -> BTreeSet<SimulationExecutionCapabilityFamilyV13> {
    let coordinates = module
        .functions
        .iter()
        .enumerate()
        .filter_map(|(function, definition)| definition.body.as_ref().map(|body| (function, body)))
        .flat_map(|(function, body)| {
            body.blocks
                .iter()
                .enumerate()
                .flat_map(move |(block, definition)| {
                    definition.operations.iter().enumerate().filter_map(
                        move |(operation, definition)| match &definition.kind {
                            OperationKind::ExecutionCapability(contract) => Some((
                                function,
                                block,
                                operation,
                                SimulationExecutionCapabilityFamilyV13::of(&contract.operation),
                            )),
                            _ => None,
                        },
                    )
                })
        })
        .collect::<Vec<_>>();
    for &(function, block, operation, family) in &coordinates {
        let mut substituted = module.clone();
        let OperationKind::ExecutionCapability(contract) = &mut substituted.functions[function]
            .body
            .as_mut()
            .unwrap()
            .blocks[block]
            .operations[operation]
            .kind
        else {
            unreachable!();
        };
        contract.provenance.kernel_binding[0] ^= 0x80;
        assert!(
            VerifiedCanonicalKernelIrV13::from_module(substituted).is_err(),
            "{family:?} accepted a one-axis provenance substitution"
        );
    }
    coordinates
        .into_iter()
        .map(|(_, _, _, family)| family)
        .collect()
}

fn with_requirement(mut module: Module, requirement: ExecutionCapabilityRequirementV1) -> Module {
    let capability = TargetCapability::Execution(requirement);
    module.required_capabilities.insert(capability.clone());
    module.functions[0]
        .required_capabilities
        .insert(capability.clone());
    module.kernels[0].required_capabilities.insert(capability);
    module
}

fn write_fresh_process_observation(path: &std::path::Path) {
    let canonical =
        VerifiedCanonicalKernelIrV13::from_module(workgroup_memory_roundtrip_module()).unwrap();
    let admitted =
        AdmittedSimulationModuleV1::admit_v13(canonical, SimulationLimitsV1::default()).unwrap();
    let output = BufferArgumentV1::from_scalars(
        AccessMode::WriteOnly,
        4,
        &[ScalarBitsV1::u32(0); 4],
        SimulationTargetV1::amdgpu_64(),
    )
    .unwrap();
    let execution = admitted
        .simulate(
            &SimulationRequestV1::new(
                "workgroup-memory-kernel",
                [4, 1, 1],
                [4, 1, 1],
                vec![
                    SimulationArgumentV1::Scalar(ScalarBitsV1::u32(77)),
                    SimulationArgumentV1::Buffer(output),
                ],
            ),
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap();
    let mut observation = format!(
        "{:?}\n",
        admitted
            .capability_projection_receipt_v13()
            .unwrap()
            .coordinates()
    )
    .into_bytes();
    observation.extend_from_slice(execution.schedule_transcript_identity());
    observation.extend_from_slice(execution.buffer(1).unwrap().bytes());
    fs::write(path, observation).unwrap();
}

#[test]
fn v13_fresh_process_child() {
    let Some(path) = std::env::var_os("FE2O3_V13_FRESH_PROCESS_OUTPUT") else {
        return;
    };
    write_fresh_process_observation(std::path::Path::new(&path));
}

#[test]
fn v13_projection_receipt_records_every_non_matrix_family_definition_and_use() {
    use SimulationExecutionCapabilityFamilyV13 as Family;
    let expected = BTreeSet::from([
        Family::WorkgroupDerive,
        Family::SubgroupDerive,
        Family::LdsAllocate,
        Family::LdsInitializeByInvocation,
        Family::LdsPublish,
        Family::LdsReadPublished,
        Family::WorkgroupBarrier,
        Family::SubgroupBarrier,
        Family::WorkgroupFence,
        Family::SubgroupFence,
        Family::Atomic,
        Family::WorkgroupCollective,
        Family::SubgroupCollective,
        Family::AsyncCopy,
        Family::AsyncWait,
        Family::RawMemoryBind,
        Family::PrivateMemoryAllocate,
        Family::WorkgroupMemoryIndex,
        Family::WorkgroupMemoryAllocate,
        Family::WorkgroupMemoryPublish,
        Family::MemoryLoad,
        Family::MemoryStore,
    ]);
    let mut observed = BTreeSet::new();
    for module in non_matrix_execution_modules() {
        let expected_module_families = execution_families(&module);
        let first = AdmittedSimulationModuleV1::admit_v13(
            VerifiedCanonicalKernelIrV13::from_module(module.clone()).unwrap(),
            SimulationLimitsV1::default(),
        )
        .unwrap();
        let second = AdmittedSimulationModuleV1::admit_v13(
            VerifiedCanonicalKernelIrV13::from_module(module).unwrap(),
            SimulationLimitsV1::default(),
        )
        .unwrap();
        let first_receipt = first.capability_projection_receipt_v13().unwrap();
        let second_receipt = second.capability_projection_receipt_v13().unwrap();
        assert_eq!(first_receipt, second_receipt);
        assert!(first_receipt.definitions() != 0);
        assert!(first_receipt.uses() != 0);
        let receipt_families = first_receipt
            .coordinates()
            .iter()
            .filter_map(|coordinate| coordinate.execution_family())
            .collect::<BTreeSet<_>>();
        assert_eq!(receipt_families, expected_module_families);
        observed.extend(receipt_families);
    }
    assert_eq!(observed, expected);
}

#[test]
fn v13_every_non_matrix_family_rejects_one_axis_provenance_substitution() {
    let expected = non_matrix_execution_modules()
        .iter()
        .flat_map(execution_families)
        .collect::<BTreeSet<_>>();
    let observed = non_matrix_execution_modules()
        .into_iter()
        .flat_map(assert_every_provenance_substitution_is_rejected)
        .collect::<BTreeSet<_>>();
    assert_eq!(observed, expected);
}

#[test]
fn v13_execution_and_projection_are_deterministic_across_fresh_processes() {
    let base = std::env::temp_dir().join(format!("fe2o3-v13-fresh-process-{}", std::process::id()));
    let paths = [base.with_extension("first"), base.with_extension("second")];
    for path in &paths {
        let _ = fs::remove_file(path);
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "v13_fresh_process_child", "--nocapture"])
            .env("FE2O3_V13_FRESH_PROCESS_OUTPUT", path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "fresh simulator process failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let first = fs::read(&paths[0]).unwrap();
    let second = fs::read(&paths[1]).unwrap();
    for path in &paths {
        fs::remove_file(path).unwrap();
    }
    assert_eq!(first, second);
}

#[test]
fn v13_unsupported_execution_target_requirements_fail_closed_one_axis_at_a_time() {
    let requirements = [
        ExecutionCapabilityRequirementV1::AddressSpace {
            address_space: AddressSpace::Generic,
            access: AccessMode::ReadWrite,
        },
        ExecutionCapabilityRequirementV1::Atomic {
            value_type: ScalarType::F32,
            operation: AtomicKind::Add,
            ordering: MemoryOrdering::Relaxed,
            failure_ordering: None,
            scope: SynchronizationScope::Device,
            address_space: AddressSpace::Global,
        },
        ExecutionCapabilityRequirementV1::Barrier {
            execution_scope: SynchronizationScope::Device,
            memory_scope: SynchronizationScope::Device,
            ordering: MemoryOrdering::SequentiallyConsistent,
            address_spaces: BTreeSet::from([AddressSpace::Global]),
        },
        ExecutionCapabilityRequirementV1::Collective {
            execution_scope: SynchronizationScope::Subgroup,
            operation: CollectiveCapabilityOperationV1::ReduceAdd,
            value_type: ScalarType::U32,
            participants: 16,
        },
        ExecutionCapabilityRequirementV1::Matrix {
            m: 8,
            n: 16,
            k: 16,
            input_type: ScalarType::Bf16,
            accumulator_type: ScalarType::F32,
        },
        ExecutionCapabilityRequirementV1::AsyncCopy {
            source: AddressSpace::Constant,
            destination: AddressSpace::Workgroup,
            bytes: 16,
            alignment: 16,
            completion: AsyncCopyCompletionV1::WorkgroupBarrier,
        },
        ExecutionCapabilityRequirementV1::Numerical {
            value_type: ScalarType::F32,
            mode: NumericalModeV1::AllowContraction,
        },
        ExecutionCapabilityRequirementV1::Resource(
            ResourceCapabilityRequirementV1::WorkgroupInvocationsAtMost(1_025),
        ),
    ];
    for requirement in requirements {
        let canonical = VerifiedCanonicalKernelIrV13::from_module(with_requirement(
            contextual_noop_module(),
            requirement.clone(),
        ))
        .unwrap();
        assert!(matches!(
            AdmittedSimulationModuleV1::admit_v13(canonical, SimulationLimitsV1::default()),
            Err(SimulationAdmissionErrorV1::UnsupportedExecutionCapability(found))
                if found == requirement
        ));
    }
}

#[test]
fn exact_v13_context_graph_executes_without_identity_downgrade() {
    let canonical = VerifiedCanonicalKernelIrV13::from_module(contextual_noop_module()).unwrap();
    let expected_digest = *canonical.identity().digest();
    let admitted =
        AdmittedSimulationModuleV1::admit_v13(canonical, SimulationLimitsV1::default()).unwrap();
    assert_eq!(admitted.identity().wire_version(), 13);
    assert_eq!(admitted.identity().digest(), &expected_digest);

    let result = admitted
        .simulate(
            &SimulationRequestV1::new("context-kernel", [2, 1, 1], [1, 1, 1], vec![]),
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap();
    assert_eq!(result.invocations_executed(), 2);
}

#[test]
fn v13_lds_initialize_publish_and_read_execute_exact_epoch_carriers() {
    let canonical =
        VerifiedCanonicalKernelIrV13::from_module(lds_epoch_roundtrip_module()).unwrap();
    let admitted =
        AdmittedSimulationModuleV1::admit_v13(canonical, SimulationLimitsV1::default()).unwrap();
    let output = BufferArgumentV1::from_scalars(
        AccessMode::WriteOnly,
        4,
        &[ScalarBitsV1::u32(0); 4],
        SimulationTargetV1::amdgpu_64(),
    )
    .unwrap();
    let request = SimulationRequestV1::new(
        "lds-epoch-kernel",
        [4, 1, 1],
        [4, 1, 1],
        vec![
            SimulationArgumentV1::Scalar(ScalarBitsV1::u32(33)),
            SimulationArgumentV1::Buffer(output),
        ],
    );
    let first = admitted
        .simulate(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap();
    let replay = admitted
        .simulate(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap();
    assert_eq!(first, replay);
    assert_eq!(
        first.buffer(1).unwrap().bytes(),
        [33_u32; 4]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>()
    );
}

#[test]
fn v13_subgroup_barrier_and_fences_replay_deterministically() {
    let canonical =
        VerifiedCanonicalKernelIrV13::from_module(synchronization_carrier_module()).unwrap();
    let admitted =
        AdmittedSimulationModuleV1::admit_v13(canonical, SimulationLimitsV1::default()).unwrap();
    let request =
        SimulationRequestV1::new("synchronization-kernel", [64, 1, 1], [64, 1, 1], vec![]);
    let first = admitted
        .simulate(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap();
    let replay = admitted
        .simulate(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap();
    assert_eq!(first, replay);
    assert_eq!(first.invocations_executed(), 64);
}

#[test]
fn v13_workgroup_memory_allocate_index_publish_and_load_roundtrip() {
    let canonical =
        VerifiedCanonicalKernelIrV13::from_module(workgroup_memory_roundtrip_module()).unwrap();
    let admitted =
        AdmittedSimulationModuleV1::admit_v13(canonical, SimulationLimitsV1::default()).unwrap();
    let output = BufferArgumentV1::from_scalars(
        AccessMode::WriteOnly,
        4,
        &[ScalarBitsV1::u32(0); 4],
        SimulationTargetV1::amdgpu_64(),
    )
    .unwrap();
    let request = SimulationRequestV1::new(
        "workgroup-memory-kernel",
        [4, 1, 1],
        [4, 1, 1],
        vec![
            SimulationArgumentV1::Scalar(ScalarBitsV1::u32(77)),
            SimulationArgumentV1::Buffer(output),
        ],
    );
    let first = admitted
        .simulate(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap();
    let replay = admitted
        .simulate(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap();
    assert_eq!(first, replay);
    assert_eq!(
        first.buffer(1).unwrap().bytes(),
        [77_u32; 4]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>()
    );

    let mut substituted = workgroup_memory_roundtrip_module();
    let witness = substituted.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .iter_mut()
        .find(|operation| {
            matches!(
                &operation.kind,
                OperationKind::ExecutionCapability(ExecutionCapabilityOpV1 {
                    operation: ExecutionCapabilityOperationV1::WorkgroupMemoryIndex { .. },
                    ..
                })
            )
        })
        .unwrap();
    let Type::ExecutionCapability(capability) = &mut witness.results[0].ty else {
        unreachable!();
    };
    capability.role = ExecutionCapabilityRoleV1::Workgroup;
    assert!(VerifiedCanonicalKernelIrV13::from_module(substituted).is_err());
}

#[test]
fn v13_workgroup_barrier_projects_to_the_physical_scheduler_deterministically() {
    let canonical =
        VerifiedCanonicalKernelIrV13::from_module(barrier_module(vec![barrier_contract(
            40, 41, 42,
        )]))
        .unwrap();
    let expected_identity = *canonical.identity();
    let admitted =
        AdmittedSimulationModuleV1::admit_v13(canonical, SimulationLimitsV1::default()).unwrap();
    assert_eq!(admitted.identity().wire_version(), 13);
    assert_eq!(admitted.identity().digest(), expected_identity.digest());
    assert!(matches!(
        admitted.module().functions[0].body.as_ref().unwrap().blocks[0].operations[0].kind,
        OperationKind::WorkgroupBarrier(_)
    ));

    let request = SimulationRequestV1::new("barrier-kernel", [4, 1, 1], [4, 1, 1], vec![]);
    let first = admitted
        .simulate(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap();
    let replay = admitted
        .simulate(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap();
    assert_eq!(first, replay);
    assert_eq!(first.invocations_executed(), 4);
    assert_ne!(first.schedule_transcript_identity(), &[0; 32]);
}

#[test]
fn v13_workgroup_reduce_executes_through_physical_lds_and_barriers() {
    let canonical = VerifiedCanonicalKernelIrV13::from_module(workgroup_collective_module(
        ExecutionCollectiveKindV1::ReduceSum,
    ))
    .unwrap();
    let identity = *canonical.identity();
    let admitted =
        AdmittedSimulationModuleV1::admit_v13(canonical, SimulationLimitsV1::default()).unwrap();
    let output = BufferArgumentV1::from_scalars(
        AccessMode::WriteOnly,
        4,
        &[ScalarBitsV1::u32(0); 4],
        SimulationTargetV1::amdgpu_64(),
    )
    .unwrap();
    let request = SimulationRequestV1::new(
        "reduce-kernel",
        [4, 1, 1],
        [4, 1, 1],
        vec![
            SimulationArgumentV1::Scalar(ScalarBitsV1::u32(7)),
            SimulationArgumentV1::Buffer(output),
        ],
    );
    let first = admitted
        .simulate(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap();
    let replay = admitted
        .simulate(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap();
    assert_eq!(first, replay);
    assert_eq!(first.identity().digest(), identity.digest());
    let words = first
        .buffer(1)
        .unwrap()
        .bytes()
        .chunks_exact(4)
        .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
        .collect::<Vec<_>>();
    assert_eq!(words, vec![28; 4]);

    let limited = SimulationLimitsV1 {
        max_reachable_operations: 4,
        ..SimulationLimitsV1::default()
    };
    assert!(
        admitted
            .simulate(&request, SimulationTargetV1::amdgpu_64(), limited)
            .is_err()
    );
}

#[test]
fn v13_workgroup_scans_match_lane_order_and_replay() {
    for (kind, expected) in [
        (
            ExecutionCollectiveKindV1::InclusiveScanSum,
            vec![7, 14, 21, 28],
        ),
        (
            ExecutionCollectiveKindV1::ExclusiveScanSum,
            vec![0, 7, 14, 21],
        ),
    ] {
        let canonical =
            VerifiedCanonicalKernelIrV13::from_module(workgroup_collective_module(kind)).unwrap();
        let admitted =
            AdmittedSimulationModuleV1::admit_v13(canonical, SimulationLimitsV1::default())
                .unwrap();
        let output = BufferArgumentV1::from_scalars(
            AccessMode::WriteOnly,
            4,
            &[ScalarBitsV1::u32(0); 4],
            SimulationTargetV1::amdgpu_64(),
        )
        .unwrap();
        let request = SimulationRequestV1::new(
            "reduce-kernel",
            [4, 1, 1],
            [4, 1, 1],
            vec![
                SimulationArgumentV1::Scalar(ScalarBitsV1::u32(7)),
                SimulationArgumentV1::Buffer(output),
            ],
        );
        let first = admitted
            .simulate(
                &request,
                SimulationTargetV1::amdgpu_64(),
                SimulationLimitsV1::default(),
            )
            .unwrap();
        let replay = admitted
            .simulate(
                &request,
                SimulationTargetV1::amdgpu_64(),
                SimulationLimitsV1::default(),
            )
            .unwrap();
        assert_eq!(first, replay);
        let words = first
            .buffer(1)
            .unwrap()
            .bytes()
            .chunks_exact(4)
            .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
            .collect::<Vec<_>>();
        assert_eq!(words, expected);
    }
}

#[test]
fn v13_wave64_reduce_and_scans_execute_with_exact_lane_order() {
    for (kind, expected) in [
        (ExecutionCollectiveKindV1::ReduceSum, vec![64; 64]),
        (
            ExecutionCollectiveKindV1::InclusiveScanSum,
            (1..=64).collect::<Vec<u32>>(),
        ),
        (
            ExecutionCollectiveKindV1::ExclusiveScanSum,
            (0..64).collect::<Vec<u32>>(),
        ),
    ] {
        let canonical =
            VerifiedCanonicalKernelIrV13::from_module(subgroup_collective_module(kind)).unwrap();
        let admitted =
            AdmittedSimulationModuleV1::admit_v13(canonical, SimulationLimitsV1::default())
                .unwrap();
        let output = BufferArgumentV1::from_scalars(
            AccessMode::WriteOnly,
            4,
            &[ScalarBitsV1::u32(0); 64],
            SimulationTargetV1::amdgpu_64(),
        )
        .unwrap();
        let request = SimulationRequestV1::new(
            "subgroup-kernel",
            [64, 1, 1],
            [64, 1, 1],
            vec![
                SimulationArgumentV1::Scalar(ScalarBitsV1::u32(1)),
                SimulationArgumentV1::Buffer(output),
            ],
        );
        let execution = admitted
            .simulate(
                &request,
                SimulationTargetV1::amdgpu_64(),
                SimulationLimitsV1::default(),
            )
            .unwrap();
        let words = execution
            .buffer(1)
            .unwrap()
            .bytes()
            .chunks_exact(4)
            .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
            .collect::<Vec<_>>();
        assert_eq!(words, expected);
    }
}

#[test]
fn v13_private_memory_load_store_preserve_bounds_and_value() {
    let canonical =
        VerifiedCanonicalKernelIrV13::from_module(private_memory_roundtrip_module()).unwrap();
    let admitted =
        AdmittedSimulationModuleV1::admit_v13(canonical, SimulationLimitsV1::default()).unwrap();
    let output = BufferArgumentV1::from_scalars(
        AccessMode::WriteOnly,
        4,
        &[ScalarBitsV1::u32(0)],
        SimulationTargetV1::amdgpu_64(),
    )
    .unwrap();
    let execution = admitted
        .simulate(
            &SimulationRequestV1::new(
                "private-kernel",
                [1, 1, 1],
                [1, 1, 1],
                vec![
                    SimulationArgumentV1::Scalar(
                        ScalarBitsV1::index(2, SimulationTargetV1::amdgpu_64()).unwrap(),
                    ),
                    SimulationArgumentV1::Scalar(ScalarBitsV1::u32(99)),
                    SimulationArgumentV1::Buffer(output),
                ],
            ),
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap();
    assert_eq!(execution.buffer(2).unwrap().bytes(), &99_u32.to_le_bytes());
}

#[test]
fn v13_raw_memory_dynamic_extent_guards_every_access() {
    let canonical =
        VerifiedCanonicalKernelIrV13::from_module(raw_memory_roundtrip_module()).unwrap();
    let admitted =
        AdmittedSimulationModuleV1::admit_v13(canonical, SimulationLimitsV1::default()).unwrap();
    for (index, expected) in [(3_u64, 77_u32), (4, 0)] {
        let output = BufferArgumentV1::from_scalars(
            AccessMode::WriteOnly,
            4,
            &[ScalarBitsV1::u32(u32::MAX)],
            SimulationTargetV1::amdgpu_64(),
        )
        .unwrap();
        let execution = admitted
            .simulate(
                &SimulationRequestV1::new(
                    "raw-memory-kernel",
                    [1, 1, 1],
                    [1, 1, 1],
                    vec![
                        SimulationArgumentV1::Scalar(
                            ScalarBitsV1::index(index, SimulationTargetV1::amdgpu_64()).unwrap(),
                        ),
                        SimulationArgumentV1::Scalar(ScalarBitsV1::u32(77)),
                        SimulationArgumentV1::Buffer(output),
                    ],
                ),
                SimulationTargetV1::amdgpu_64(),
                SimulationLimitsV1::default(),
            )
            .unwrap();
        assert_eq!(
            execution.buffer(2).unwrap().bytes(),
            &expected.to_le_bytes()
        );
    }
}

#[test]
fn v13_async_copy_wait_retains_pending_state_and_replays() {
    let canonical =
        VerifiedCanonicalKernelIrV13::from_module(async_copy_roundtrip_module()).unwrap();
    let admitted =
        AdmittedSimulationModuleV1::admit_v13(canonical, SimulationLimitsV1::default()).unwrap();
    let source = BufferArgumentV1::from_scalars(
        AccessMode::ReadWrite,
        4,
        &[
            ScalarBitsV1::u32(10),
            ScalarBitsV1::u32(20),
            ScalarBitsV1::u32(30),
            ScalarBitsV1::u32(40),
        ],
        SimulationTargetV1::amdgpu_64(),
    )
    .unwrap();
    let output = BufferArgumentV1::from_scalars(
        AccessMode::WriteOnly,
        4,
        &[ScalarBitsV1::u32(u32::MAX); 4],
        SimulationTargetV1::amdgpu_64(),
    )
    .unwrap();
    let request = SimulationRequestV1::new(
        "async-copy-kernel",
        [4, 1, 1],
        [4, 1, 1],
        vec![
            SimulationArgumentV1::Buffer(source),
            SimulationArgumentV1::Buffer(output),
        ],
    );
    let first = admitted
        .simulate(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap();
    let replay = admitted
        .simulate(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap();
    assert_eq!(first, replay);
    let words = first
        .buffer(1)
        .unwrap()
        .bytes()
        .chunks_exact(4)
        .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
        .collect::<Vec<_>>();
    assert_eq!(words, vec![20, 30, 40, 0]);

    let mut missing_wait = async_copy_roundtrip_module();
    let operations = &mut missing_wait.functions[0].body.as_mut().unwrap().blocks[0].operations;
    let after_copy = operations
        .iter()
        .position(|operation| {
            matches!(
                operation.kind,
                OperationKind::ExecutionCapability(ExecutionCapabilityOpV1 {
                    operation: ExecutionCapabilityOperationV1::AsyncCopy { .. },
                    ..
                })
            )
        })
        .unwrap()
        + 1;
    operations.truncate(after_copy);
    let missing_wait = VerifiedCanonicalKernelIrV13::from_module(missing_wait).unwrap();
    assert!(matches!(
        AdmittedSimulationModuleV1::admit_v13(missing_wait, SimulationLimitsV1::default()),
        Err(
            SimulationAdmissionErrorV1::IncompleteExecutionCapabilityV13(
                IncompleteExecutionCapabilityOperationV13::AsyncWait
            )
        )
    ));
}

#[test]
fn v13_scoped_atomic_fetch_add_is_deterministic_and_scope_bound() {
    let canonical =
        VerifiedCanonicalKernelIrV13::from_module(scoped_atomic_fetch_add_module()).unwrap();
    let admitted =
        AdmittedSimulationModuleV1::admit_v13(canonical, SimulationLimitsV1::default()).unwrap();
    let buffer = BufferArgumentV1::from_scalars(
        AccessMode::ReadWrite,
        4,
        &[ScalarBitsV1::u32(0)],
        SimulationTargetV1::amdgpu_64(),
    )
    .unwrap();
    let request = SimulationRequestV1::new(
        "atomic-kernel",
        [4, 1, 1],
        [4, 1, 1],
        vec![SimulationArgumentV1::Buffer(buffer)],
    );
    let first = admitted
        .simulate(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap();
    let replay = admitted
        .simulate(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap();
    assert_eq!(first, replay);
    assert_eq!(first.buffer(0).unwrap().bytes(), &4_u32.to_le_bytes());

    let mut substituted = scoped_atomic_fetch_add_module();
    let OperationKind::ExecutionCapability(contract) =
        &mut substituted.functions[0].body.as_mut().unwrap().blocks[0].operations[8].kind
    else {
        unreachable!();
    };
    let ExecutionCapabilityOperationV1::Atomic { scope, .. } = &mut contract.operation else {
        unreachable!();
    };
    *scope = ExecutionMemoryScopeV1::Device;
    assert!(VerifiedCanonicalKernelIrV13::from_module(substituted).is_err());
}

#[test]
fn v13_barrier_epoch_order_and_scope_fail_closed() {
    let stale = barrier_module(vec![
        barrier_contract(50, 51, 52),
        barrier_contract(50, 53, 54),
    ]);
    assert!(VerifiedCanonicalKernelIrV13::from_module(stale).is_err());

    let mut insufficient = barrier_module(vec![barrier_contract(60, 61, 62)]);
    let OperationKind::ExecutionCapability(contract) =
        &mut insufficient.functions[0].body.as_mut().unwrap().blocks[0]
            .operations
            .last_mut()
            .unwrap()
            .kind
    else {
        unreachable!();
    };
    let ExecutionCapabilityOperationV1::WorkgroupBarrier { semantics, .. } =
        &mut contract.operation
    else {
        unreachable!();
    };
    semantics.scope = ExecutionMemoryScopeV1::Subgroup;
    assert!(VerifiedCanonicalKernelIrV13::from_module(insufficient).is_err());
}

#[test]
fn v13_memory_address_role_substitution_fails_before_projection() {
    let mut substituted = raw_bind_module();
    let OperationKind::ExecutionCapability(contract) =
        &mut substituted.functions[0].body.as_mut().unwrap().blocks[0]
            .operations
            .last_mut()
            .unwrap()
            .kind
    else {
        unreachable!();
    };
    let ExecutionCapabilityOperationV1::RawMemoryBind { space, .. } = &mut contract.operation
    else {
        unreachable!();
    };
    *space = ExecutionMemoryAddressSpaceV1::Global;
    assert!(VerifiedCanonicalKernelIrV13::from_module(substituted).is_err());
}

#[test]
fn v13_raw_bind_is_supported_and_resource_bounded() {
    let canonical = VerifiedCanonicalKernelIrV13::from_module(raw_bind_module()).unwrap();
    AdmittedSimulationModuleV1::admit_v13(canonical, SimulationLimitsV1::default()).unwrap();

    let mut limits = SimulationLimitsV1::default();
    let canonical = VerifiedCanonicalKernelIrV13::from_module(raw_bind_module()).unwrap();
    limits.max_canonical_bytes = canonical.identity().canonical_length() as usize - 1;
    assert!(matches!(
        AdmittedSimulationModuleV1::admit_v13(canonical, limits),
        Err(SimulationAdmissionErrorV1::CanonicalBytesLimit { .. })
    ));
}

#[test]
fn incomplete_execution_capability_names_are_stable() {
    use IncompleteExecutionCapabilityOperationV13 as Operation;
    let operations = [
        Operation::WorkgroupDerive,
        Operation::SubgroupDerive,
        Operation::LdsAllocate,
        Operation::LdsInitializeByInvocation,
        Operation::LdsPublish,
        Operation::LdsReadPublished,
        Operation::WorkgroupBarrier,
        Operation::SubgroupBarrier,
        Operation::WorkgroupFence,
        Operation::SubgroupFence,
        Operation::Atomic,
        Operation::WorkgroupCollective,
        Operation::SubgroupCollective,
        Operation::MatrixAccess,
        Operation::AsyncCopy,
        Operation::AsyncWait,
        Operation::RawMemoryBind,
        Operation::PrivateMemoryAllocate,
        Operation::WorkgroupMemoryIndex,
        Operation::WorkgroupMemoryAllocate,
        Operation::WorkgroupMemoryPublish,
        Operation::MemoryLoad,
        Operation::MemoryStore,
    ];
    assert_eq!(
        operations.map(|operation| operation.to_string()),
        [
            "workgroup_derive",
            "subgroup_derive",
            "lds_allocate",
            "lds_initialize_by_invocation",
            "lds_publish",
            "lds_read_published",
            "workgroup_barrier",
            "subgroup_barrier",
            "workgroup_fence",
            "subgroup_fence",
            "atomic",
            "workgroup_collective",
            "subgroup_collective",
            "matrix_access",
            "async_copy",
            "async_wait",
            "raw_memory_bind",
            "private_memory_allocate",
            "workgroup_memory_index",
            "workgroup_memory_allocate",
            "workgroup_memory_publish",
            "memory_load",
            "memory_store",
        ]
    );
}
