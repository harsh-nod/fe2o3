use std::collections::BTreeSet;

use fe2o3_kernel_ir::*;

const WORKGROUP_BRAND: [u8; 32] = [0x71; 32];
const EPOCH_BEFORE: [u8; 32] = [0x72; 32];
const EPOCH_AFTER: [u8; 32] = [0x73; 32];
const ELEMENTS: u64 = 64;

fn id(byte: u8) -> ExecutionTypeIdentityV1 {
    ExecutionTypeIdentityV1::new([byte; 32])
}

fn provenance() -> ExecutionCapabilityProvenanceV1 {
    ExecutionCapabilityProvenanceV1 {
        root: FunctionId::new("entry"),
        kernel_binding: [0x61; 32],
        frontend_unit: [0x62; 32],
        kernel_marker: [0x63; 32],
        target_brand: [0x64; 32],
        launch_brand: [0x65; 32],
        issuance: [0x66; 32],
    }
}

fn layout() -> ExecutionElementLayoutV1 {
    ExecutionElementLayoutV1 {
        byte_size: 4,
        byte_alignment: 4,
    }
}

fn workgroup_semantics() -> ExecutionMemorySemanticsV1 {
    ExecutionMemorySemanticsV1 {
        scope: ExecutionMemoryScopeV1::Workgroup,
        ordering: ExecutionMemoryOrderingV1::AcquireRelease,
        spaces: ExecutionMemorySpacesV1::Workgroup,
    }
}

fn subgroup_semantics() -> ExecutionMemorySemanticsV1 {
    ExecutionMemorySemanticsV1 {
        scope: ExecutionMemoryScopeV1::Subgroup,
        ordering: ExecutionMemoryOrderingV1::AcquireRelease,
        spaces: ExecutionMemorySpacesV1::Workgroup,
    }
}

fn dynamic_extent() -> ExecutionDynamicExtentV1 {
    ExecutionDynamicExtentV1 {
        operand: 2,
        source_argument: 2,
        source_type: id(0x13),
        value_type: ScalarType::Index,
        upper_bound: 64,
        bound_check_operand: 3,
        nonnegative_check_operand: None,
    }
}

fn catalog() -> Vec<ExecutionCapabilityOperationV1> {
    vec![
        ExecutionCapabilityOperationV1::WorkgroupDerive {
            context: id(0x10),
            workgroup: id(0x11),
        },
        ExecutionCapabilityOperationV1::SubgroupDerive {
            workgroup: id(0x11),
            subgroup: id(0x12),
            width: 64,
        },
        ExecutionCapabilityOperationV1::LdsAllocate {
            workgroup: id(0x11),
            lds: id(0x20),
            element: id(0x21),
            layout: layout(),
            elements: ELEMENTS,
        },
        ExecutionCapabilityOperationV1::LdsInitializeByInvocation {
            input_lds: id(0x20),
            workgroup: id(0x11),
            output_lds: id(0x22),
            element: id(0x21),
            layout: layout(),
            elements: ELEMENTS,
        },
        ExecutionCapabilityOperationV1::LdsPublish {
            input_workgroup: id(0x11),
            input_lds: id(0x22),
            output_lds: id(0x23),
            transition: id(0x24),
            element: id(0x21),
            layout: layout(),
            elements: ELEMENTS,
        },
        ExecutionCapabilityOperationV1::LdsReadPublished {
            lds_reference: id(0x25),
            lds: id(0x23),
            workgroup: id(0x11),
            index: id(0x14),
            option: id(0x26),
            element: id(0x21),
            layout: layout(),
            elements: ELEMENTS,
        },
        ExecutionCapabilityOperationV1::WorkgroupBarrier {
            input_workgroup: id(0x11),
            output_workgroup: id(0x27),
            semantics: workgroup_semantics(),
        },
        ExecutionCapabilityOperationV1::SubgroupBarrier {
            input_workgroup: id(0x11),
            semantics: subgroup_semantics(),
            subgroup: id(0x12),
            transition: id(0x28),
            width: 64,
        },
        ExecutionCapabilityOperationV1::WorkgroupFence {
            workgroup: id(0x11),
            result: id(0x29),
            semantics: workgroup_semantics(),
        },
        ExecutionCapabilityOperationV1::SubgroupFence {
            semantics: subgroup_semantics(),
            subgroup_reference: id(0x2a),
            subgroup: id(0x12),
            epoch: id(0x2b),
            result: id(0x2c),
            width: 64,
        },
        ExecutionCapabilityOperationV1::Atomic {
            kind: ExecutionAtomicKindV1::BindGlobalView,
            authority: id(0x10),
            location_input: id(0x30),
            location: id(0x31),
            element: id(0x15),
            operand: None,
            replacement: None,
            result: id(0x31),
            value_type: ScalarType::U32,
            address_space: ExecutionMemoryAddressSpaceV1::Global,
            scope: ExecutionMemoryScopeV1::Device,
            success: None,
            failure: None,
        },
        ExecutionCapabilityOperationV1::WorkgroupCollective {
            kind: ExecutionCollectiveKindV1::ReduceSum,
            input_workgroup: id(0x11),
            scratch: id(0x20),
            element: id(0x15),
            transition: id(0x32),
            value_type: ScalarType::U32,
            layout: layout(),
            elements: ELEMENTS,
        },
        ExecutionCapabilityOperationV1::SubgroupCollective {
            kind: ExecutionCollectiveKindV1::ReduceSum,
            subgroup_reference: id(0x2a),
            subgroup: id(0x12),
            epoch: id(0x2b),
            element: id(0x15),
            value_type: ScalarType::U32,
            width: 64,
        },
        ExecutionCapabilityOperationV1::MatrixAccess {
            subgroup: id(0x12),
            epoch: id(0x2b),
            matrix: id(0x33),
            subgroup_brand: [0x74; 32],
            width: 64,
        },
        ExecutionCapabilityOperationV1::AsyncCopy {
            workgroup: id(0x11),
            source_reference: id(0x34),
            source: id(0x35),
            index: id(0x14),
            destination: id(0x20),
            pending: id(0x36),
            element: id(0x21),
            layout: layout(),
            elements: ELEMENTS,
        },
        ExecutionCapabilityOperationV1::AsyncWait {
            input_workgroup: id(0x11),
            pending: id(0x36),
            output_lds: id(0x23),
            transition: id(0x37),
            element: id(0x21),
            layout: layout(),
            elements: ELEMENTS,
        },
        ExecutionCapabilityOperationV1::RawMemoryBind {
            authority: id(0x10),
            pointer: id(0x38),
            length: id(0x13),
            extent: dynamic_extent(),
            view: id(0x39),
            element: id(0x15),
            layout: layout(),
            space: ExecutionMemoryAddressSpaceV1::Private,
            access: ExecutionMemoryAccessV1::ReadOnly,
            index_space: None,
            atomic_scope: None,
            unsafe_obligation: id(0x3a),
        },
        ExecutionCapabilityOperationV1::PrivateMemoryAllocate {
            context: id(0x10),
            view: id(0x3b),
            element: id(0x15),
            layout: layout(),
            elements: ELEMENTS,
        },
        ExecutionCapabilityOperationV1::WorkgroupMemoryIndex {
            workgroup: id(0x11),
            witness: id(0x3c),
        },
        ExecutionCapabilityOperationV1::WorkgroupMemoryAllocate {
            workgroup: id(0x11),
            view: id(0x3d),
            element: id(0x15),
            layout: layout(),
            elements: ELEMENTS,
            index_space: id(0x3e),
        },
        ExecutionCapabilityOperationV1::WorkgroupMemoryPublish {
            input_workgroup: id(0x11),
            input_view: id(0x3d),
            output_view: id(0x3f),
            transition: id(0x40),
            element: id(0x15),
            layout: layout(),
        },
        ExecutionCapabilityOperationV1::MemoryLoad {
            view: id(0x39),
            workgroup: None,
            index: id(0x14),
            option: id(0x41),
            element: id(0x15),
            layout: layout(),
            space: ExecutionMemoryAddressSpaceV1::Private,
            access: ExecutionMemoryAccessV1::ReadOnly,
        },
        ExecutionCapabilityOperationV1::MemoryStore {
            view: id(0x42),
            workgroup: None,
            index: id(0x14),
            element: id(0x15),
            layout: layout(),
            result: id(0x43),
            space: ExecutionMemoryAddressSpaceV1::Private,
            access: ExecutionMemoryAccessV1::ExclusiveReadWrite,
        },
    ]
}

fn name(operation: &ExecutionCapabilityOperationV1) -> &'static str {
    match operation {
        ExecutionCapabilityOperationV1::WorkgroupDerive { .. } => "WorkgroupDerive",
        ExecutionCapabilityOperationV1::SubgroupDerive { .. } => "SubgroupDerive",
        ExecutionCapabilityOperationV1::LdsAllocate { .. } => "LdsAllocate",
        ExecutionCapabilityOperationV1::LdsInitializeByInvocation { .. } => {
            "LdsInitializeByInvocation"
        }
        ExecutionCapabilityOperationV1::LdsPublish { .. } => "LdsPublish",
        ExecutionCapabilityOperationV1::LdsReadPublished { .. } => "LdsReadPublished",
        ExecutionCapabilityOperationV1::WorkgroupBarrier { .. } => "WorkgroupBarrier",
        ExecutionCapabilityOperationV1::SubgroupBarrier { .. } => "SubgroupBarrier",
        ExecutionCapabilityOperationV1::WorkgroupFence { .. } => "WorkgroupFence",
        ExecutionCapabilityOperationV1::SubgroupFence { .. } => "SubgroupFence",
        ExecutionCapabilityOperationV1::Atomic { .. } => "Atomic",
        ExecutionCapabilityOperationV1::WorkgroupCollective { .. } => "WorkgroupCollective",
        ExecutionCapabilityOperationV1::SubgroupCollective { .. } => "SubgroupCollective",
        ExecutionCapabilityOperationV1::MatrixAccess { .. } => "MatrixAccess",
        ExecutionCapabilityOperationV1::AsyncCopy { .. } => "AsyncCopy",
        ExecutionCapabilityOperationV1::AsyncWait { .. } => "AsyncWait",
        ExecutionCapabilityOperationV1::RawMemoryBind { .. } => "RawMemoryBind",
        ExecutionCapabilityOperationV1::PrivateMemoryAllocate { .. } => "PrivateMemoryAllocate",
        ExecutionCapabilityOperationV1::WorkgroupMemoryIndex { .. } => "WorkgroupMemoryIndex",
        ExecutionCapabilityOperationV1::WorkgroupMemoryAllocate { .. } => "WorkgroupMemoryAllocate",
        ExecutionCapabilityOperationV1::WorkgroupMemoryPublish { .. } => "WorkgroupMemoryPublish",
        ExecutionCapabilityOperationV1::MemoryLoad { .. } => "MemoryLoad",
        ExecutionCapabilityOperationV1::MemoryStore { .. } => "MemoryStore",
    }
}

fn signature(operation: &ExecutionCapabilityOperationV1) -> ExecutionCapabilitySignatureV1 {
    use ExecutionCapabilityOperationV1 as Op;
    let (arguments, output) = match operation {
        Op::WorkgroupDerive { context, workgroup } => (vec![*context], *workgroup),
        Op::SubgroupDerive {
            workgroup,
            subgroup,
            ..
        } => (vec![*workgroup], *subgroup),
        Op::LdsAllocate { workgroup, lds, .. } => (vec![*workgroup], *lds),
        Op::LdsInitializeByInvocation {
            input_lds,
            workgroup,
            output_lds,
            element,
            ..
        } => (vec![*input_lds, *workgroup, *element], *output_lds),
        Op::LdsPublish {
            input_workgroup,
            input_lds,
            transition,
            ..
        } => (vec![*input_workgroup, *input_lds], *transition),
        Op::LdsReadPublished {
            lds_reference,
            workgroup,
            index,
            option,
            ..
        } => (vec![*lds_reference, *workgroup, *index], *option),
        Op::WorkgroupBarrier {
            input_workgroup,
            output_workgroup,
            ..
        } => (vec![*input_workgroup], *output_workgroup),
        Op::SubgroupBarrier {
            input_workgroup,
            subgroup,
            transition,
            ..
        } => (vec![*input_workgroup, *subgroup], *transition),
        Op::WorkgroupFence {
            workgroup, result, ..
        } => (vec![*workgroup], *result),
        Op::SubgroupFence {
            subgroup_reference,
            epoch,
            result,
            ..
        } => (vec![*subgroup_reference, *epoch], *result),
        Op::Atomic {
            kind,
            authority,
            location_input,
            operand,
            replacement,
            result,
            ..
        } => {
            let mut arguments = vec![*authority, *location_input];
            if !matches!(
                kind,
                ExecutionAtomicKindV1::BindGlobalView | ExecutionAtomicKindV1::Load
            ) {
                arguments.push(operand.expect("catalog atomic operand"));
            }
            if matches!(kind, ExecutionAtomicKindV1::CompareExchange) {
                arguments.push(replacement.expect("catalog atomic replacement"));
            }
            (arguments, *result)
        }
        Op::WorkgroupCollective {
            input_workgroup,
            scratch,
            element,
            transition,
            ..
        } => (vec![*input_workgroup, *scratch, *element], *transition),
        Op::SubgroupCollective {
            subgroup_reference,
            epoch,
            element,
            ..
        } => (vec![*subgroup_reference, *epoch, *element], *element),
        Op::MatrixAccess {
            subgroup,
            epoch,
            matrix,
            ..
        } => (vec![*subgroup, *epoch], *matrix),
        Op::AsyncCopy {
            workgroup,
            source_reference,
            index,
            destination,
            pending,
            ..
        } => (
            vec![*workgroup, *source_reference, *index, *destination],
            *pending,
        ),
        Op::AsyncWait {
            input_workgroup,
            pending,
            transition,
            ..
        } => (vec![*input_workgroup, *pending], *transition),
        Op::RawMemoryBind {
            authority,
            pointer,
            length,
            view,
            unsafe_obligation,
            ..
        } => (
            vec![*authority, *pointer, *length, *unsafe_obligation],
            *view,
        ),
        Op::PrivateMemoryAllocate { context, view, .. } => (vec![*context], *view),
        Op::WorkgroupMemoryIndex { workgroup, witness } => (vec![*workgroup], *witness),
        Op::WorkgroupMemoryAllocate {
            workgroup, view, ..
        } => (vec![*workgroup], *view),
        Op::WorkgroupMemoryPublish {
            input_workgroup,
            input_view,
            transition,
            ..
        } => (vec![*input_workgroup, *input_view], *transition),
        Op::MemoryLoad {
            view,
            workgroup,
            index,
            option,
            ..
        } => {
            let mut arguments = vec![*view];
            arguments.extend(workgroup);
            arguments.push(*index);
            (arguments, *option)
        }
        Op::MemoryStore {
            view,
            workgroup,
            index,
            element,
            result,
            ..
        } => {
            let mut arguments = vec![*view];
            arguments.extend(workgroup);
            arguments.extend([*index, *element]);
            (arguments, *result)
        }
    };
    ExecutionCapabilitySignatureV1::new(&arguments, output).unwrap()
}

fn capability_type(
    source_type: ExecutionTypeIdentityV1,
    role: ExecutionCapabilityRoleV1,
    operation: &ExecutionCapabilityOperationV1,
) -> Type {
    let kernel_scoped = operation.is_kernel_scoped();
    Type::ExecutionCapability(ExecutionCapabilityTypeV1 {
        source_type,
        provenance: provenance(),
        workgroup_brand: (!kernel_scoped).then_some(WORKGROUP_BRAND),
        epoch: (!kernel_scoped).then_some(if operation.transitions_epoch() {
            EPOCH_AFTER
        } else {
            EPOCH_BEFORE
        }),
        role,
    })
}

#[allow(dead_code)]
fn legacy_result_type(operation: &ExecutionCapabilityOperationV1) -> Type {
    use ExecutionCapabilityOperationV1 as Op;
    let (source_type, role) = match operation {
        Op::WorkgroupDerive { workgroup, .. } => {
            (*workgroup, Some(ExecutionCapabilityRoleV1::Workgroup))
        }
        Op::SubgroupDerive {
            subgroup, width, ..
        } => (
            *subgroup,
            Some(ExecutionCapabilityRoleV1::Subgroup { width: *width }),
        ),
        Op::LdsAllocate {
            lds,
            element,
            layout,
            elements,
            ..
        } => (
            *lds,
            Some(ExecutionCapabilityRoleV1::Lds {
                element: *element,
                layout: *layout,
                elements: *elements,
                state: ExecutionLdsStateV1::Uninitialized,
            }),
        ),
        Op::LdsInitializeByInvocation {
            output_lds,
            element,
            layout,
            elements,
            ..
        } => (
            *output_lds,
            Some(ExecutionCapabilityRoleV1::Lds {
                element: *element,
                layout: *layout,
                elements: *elements,
                state: ExecutionLdsStateV1::InvocationInitialized,
            }),
        ),
        Op::LdsPublish {
            transition,
            element,
            layout,
            elements,
            ..
        }
        | Op::AsyncWait {
            transition,
            element,
            layout,
            elements,
            ..
        } => (
            *transition,
            Some(ExecutionCapabilityRoleV1::Lds {
                element: *element,
                layout: *layout,
                elements: *elements,
                state: ExecutionLdsStateV1::Published,
            }),
        ),
        Op::MatrixAccess {
            matrix,
            subgroup_brand,
            width,
            ..
        } => (
            *matrix,
            Some(ExecutionCapabilityRoleV1::Matrix {
                subgroup_brand: *subgroup_brand,
                width: *width,
            }),
        ),
        Op::AsyncCopy {
            pending,
            element,
            layout,
            elements,
            ..
        } => (
            *pending,
            Some(ExecutionCapabilityRoleV1::PendingAsyncCopy {
                element: *element,
                layout: *layout,
                elements: *elements,
            }),
        ),
        Op::RawMemoryBind {
            view,
            extent,
            element,
            layout,
            space,
            access,
            index_space,
            atomic_scope,
            ..
        } => (
            *view,
            Some(ExecutionCapabilityRoleV1::MemoryView {
                element: *element,
                layout: *layout,
                space: *space,
                access: *access,
                extent: ExecutionMemoryExtentV1::Dynamic(*extent),
                initialization: ExecutionMemoryInitializationV1::FullyInitialized,
                index_space: *index_space,
                atomic_scope: *atomic_scope,
            }),
        ),
        Op::PrivateMemoryAllocate {
            view,
            element,
            layout,
            elements,
            ..
        } => (
            *view,
            Some(ExecutionCapabilityRoleV1::MemoryView {
                element: *element,
                layout: *layout,
                space: ExecutionMemoryAddressSpaceV1::Private,
                access: ExecutionMemoryAccessV1::ExclusiveReadWrite,
                extent: ExecutionMemoryExtentV1::Static(*elements),
                initialization: ExecutionMemoryInitializationV1::FullyInitialized,
                index_space: None,
                atomic_scope: None,
            }),
        ),
        Op::WorkgroupMemoryIndex { witness, .. } => (
            *witness,
            Some(ExecutionCapabilityRoleV1::WorkgroupMemoryIndex),
        ),
        Op::WorkgroupMemoryPublish { transition, .. } => (
            *transition,
            Some(ExecutionCapabilityRoleV1::EpochTransition),
        ),
        Op::Atomic {
            kind: ExecutionAtomicKindV1::BindGlobalView,
            result,
            element,
            scope,
            ..
        } => (
            *result,
            Some(ExecutionCapabilityRoleV1::MemoryView {
                element: *element,
                layout: layout(),
                space: ExecutionMemoryAddressSpaceV1::Global,
                access: ExecutionMemoryAccessV1::AtomicReadWrite,
                extent: ExecutionMemoryExtentV1::Static(ELEMENTS),
                initialization: ExecutionMemoryInitializationV1::FullyInitialized,
                index_space: None,
                atomic_scope: Some(*scope),
            }),
        ),
        Op::Atomic { .. } => unreachable!("catalog uses atomic-view binding"),
        Op::LdsReadPublished { .. } | Op::MemoryLoad { .. } => {
            return Type::Scalar(ScalarType::U32);
        }
        Op::WorkgroupCollective { .. } | Op::SubgroupCollective { .. } => {
            return Type::Scalar(ScalarType::U32);
        }
        Op::MemoryStore { .. } => return Type::BOOL,
        Op::WorkgroupBarrier { .. }
        | Op::SubgroupBarrier { .. }
        | Op::WorkgroupFence { .. }
        | Op::SubgroupFence { .. }
        | Op::WorkgroupMemoryAllocate { .. } => return Type::Unit,
    };
    capability_type(source_type, role.unwrap(), operation)
}

#[allow(dead_code)]
fn legacy_physical_inputs(operation: &ExecutionCapabilityOperationV1) -> (Vec<Type>, Vec<ValueId>) {
    match operation {
        ExecutionCapabilityOperationV1::LdsInitializeByInvocation { .. }
        | ExecutionCapabilityOperationV1::WorkgroupCollective { .. }
        | ExecutionCapabilityOperationV1::SubgroupCollective { .. } => {
            (vec![Type::Scalar(ScalarType::U32)], vec![ValueId(0)])
        }
        ExecutionCapabilityOperationV1::LdsReadPublished { .. }
        | ExecutionCapabilityOperationV1::AsyncCopy { .. }
        | ExecutionCapabilityOperationV1::MemoryLoad { .. } => {
            (vec![Type::INDEX], vec![ValueId(0)])
        }
        ExecutionCapabilityOperationV1::MemoryStore { .. } => (
            vec![Type::INDEX, Type::Scalar(ScalarType::U32)],
            vec![ValueId(0), ValueId(1)],
        ),
        ExecutionCapabilityOperationV1::RawMemoryBind { .. } => {
            (vec![Type::INDEX], vec![ValueId(0), ValueId(2)])
        }
        _ => (Vec::new(), Vec::new()),
    }
}

#[allow(dead_code)]
fn legacy_module_for(operation: ExecutionCapabilityOperationV1, ordinal: u8) -> Module {
    let requirements = expected_capabilities(&operation);
    let (parameters, operands) = legacy_physical_inputs(&operation);
    let parameter_values = (0..parameters.len())
        .map(|value| ValueId(value as u32))
        .collect();
    let mut block = BasicBlock::new(BlockId(0));
    if matches!(
        operation,
        ExecutionCapabilityOperationV1::RawMemoryBind { .. }
    ) {
        block.operations.extend([
            Operation::effect_free(
                ValueDef::new(ValueId(1), Type::INDEX),
                OperationKind::Constant(Constant::Index(64)),
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(2), Type::BOOL),
                OperationKind::Compare {
                    predicate: ComparePredicate::LessThanOrEqual,
                    lhs: ValueId(0),
                    rhs: ValueId(1),
                },
            ),
        ]);
    }
    let result_id = ValueId(if block.operations.is_empty() {
        parameters.len() as u32
    } else {
        3
    });
    let contract = ExecutionCapabilityOpV1 {
        operands,
        signature: signature(&operation),
        provenance: provenance(),
        workgroup_brand: (!operation.is_kernel_scoped()).then_some(WORKGROUP_BRAND),
        epoch_before: (!operation.is_kernel_scoped()).then_some(EPOCH_BEFORE),
        epoch_after: operation.transitions_epoch().then_some(EPOCH_AFTER),
        obligations: ExecutionSafetyObligationsV1::from_bits(required_execution_obligations_v1(
            &operation,
        )),
        source: ExecutionCapabilitySourceV1 {
            function: [0x67; 32],
            operation: [ordinal.wrapping_add(1); 32],
            block: 0,
        },
        operation: operation.clone(),
    };
    block.operations.push(Operation::effect_free(
        ValueDef::new(result_id, legacy_result_type(&operation)),
        OperationKind::ExecutionCapability(contract),
    ));
    block.terminator = Some(Terminator::Return { values: vec![] });

    let mut function = Function::kernel_entry(
        "entry",
        Signature::new(parameters, vec![]),
        parameter_values,
        vec![block],
    );
    function.required_capabilities = requirements.clone();

    let mut module = Module::new(format!("v13-catalog-{}", name(&operation)));
    module.required_capabilities = requirements;
    module.functions.push(function);
    module.kernels.push(Kernel::new(
        "entry-kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(64),
        },
    ));
    module
}

fn capability_type_at(
    source_type: ExecutionTypeIdentityV1,
    role: ExecutionCapabilityRoleV1,
    operation: &ExecutionCapabilityOperationV1,
    epoch_before: [u8; 32],
) -> Type {
    Type::ExecutionCapability(ExecutionCapabilityTypeV1 {
        source_type,
        provenance: provenance(),
        workgroup_brand: (!operation.is_kernel_scoped()).then_some(WORKGROUP_BRAND),
        epoch: (!operation.is_kernel_scoped()).then_some(if operation.transitions_epoch() {
            EPOCH_AFTER
        } else {
            epoch_before
        }),
        role,
    })
}

fn result_types(operation: &ExecutionCapabilityOperationV1, epoch_before: [u8; 32]) -> Vec<Type> {
    use ExecutionCapabilityOperationV1 as Op;
    let cap = |source, role| capability_type_at(source, role, operation, epoch_before);
    let lds = |source, element, layout, elements, state| {
        cap(
            source,
            ExecutionCapabilityRoleV1::Lds {
                element,
                layout,
                elements,
                state,
            },
        )
    };
    match operation {
        Op::WorkgroupDerive { workgroup, .. } => {
            vec![cap(*workgroup, ExecutionCapabilityRoleV1::Workgroup)]
        }
        Op::SubgroupDerive {
            subgroup, width, ..
        } => vec![cap(
            *subgroup,
            ExecutionCapabilityRoleV1::Subgroup { width: *width },
        )],
        Op::LdsAllocate {
            lds: source,
            element,
            layout,
            elements,
            ..
        } => vec![lds(
            *source,
            *element,
            *layout,
            *elements,
            ExecutionLdsStateV1::Uninitialized,
        )],
        Op::LdsInitializeByInvocation {
            output_lds,
            element,
            layout,
            elements,
            ..
        } => vec![lds(
            *output_lds,
            *element,
            *layout,
            *elements,
            ExecutionLdsStateV1::InvocationInitialized,
        )],
        Op::LdsPublish {
            output_lds,
            transition,
            element,
            layout,
            elements,
            ..
        } => vec![
            cap(*transition, ExecutionCapabilityRoleV1::Workgroup),
            lds(
                *output_lds,
                *element,
                *layout,
                *elements,
                ExecutionLdsStateV1::Published,
            ),
        ],
        Op::LdsReadPublished { .. } | Op::MemoryLoad { .. } => {
            vec![Type::Scalar(ScalarType::U32), Type::BOOL]
        }
        Op::WorkgroupBarrier {
            output_workgroup, ..
        } => vec![cap(*output_workgroup, ExecutionCapabilityRoleV1::Workgroup)],
        Op::SubgroupBarrier {
            transition, width, ..
        } => vec![
            cap(*transition, ExecutionCapabilityRoleV1::Workgroup),
            cap(
                *transition,
                ExecutionCapabilityRoleV1::Subgroup { width: *width },
            ),
        ],
        Op::WorkgroupFence { .. } | Op::SubgroupFence { .. } => Vec::new(),
        Op::Atomic {
            kind,
            location,
            result,
            element,
            value_type,
            scope,
            ..
        } => match kind {
            ExecutionAtomicKindV1::BindGlobalLocation => vec![
                cap(
                    *location,
                    ExecutionCapabilityRoleV1::ScopedAtomic {
                        element: *element,
                        space: ExecutionMemoryAddressSpaceV1::Global,
                        scope: *scope,
                    },
                ),
                Type::BOOL,
            ],
            ExecutionAtomicKindV1::BindGlobalView => vec![cap(
                *result,
                ExecutionCapabilityRoleV1::MemoryView {
                    element: *element,
                    layout: layout(),
                    space: ExecutionMemoryAddressSpaceV1::Global,
                    access: ExecutionMemoryAccessV1::AtomicReadWrite,
                    extent: ExecutionMemoryExtentV1::Static(ELEMENTS),
                    initialization: ExecutionMemoryInitializationV1::FullyInitialized,
                    index_space: None,
                    atomic_scope: Some(*scope),
                },
            )],
            ExecutionAtomicKindV1::Load | ExecutionAtomicKindV1::FetchAdd => {
                vec![Type::Scalar(*value_type)]
            }
            ExecutionAtomicKindV1::Store => Vec::new(),
            ExecutionAtomicKindV1::CompareExchange => {
                vec![Type::Scalar(*value_type), Type::BOOL]
            }
        },
        Op::WorkgroupCollective {
            transition,
            element,
            value_type,
            layout,
            elements,
            ..
        } => vec![
            cap(*transition, ExecutionCapabilityRoleV1::Workgroup),
            lds(
                *transition,
                *element,
                *layout,
                *elements,
                ExecutionLdsStateV1::Uninitialized,
            ),
            Type::Scalar(*value_type),
        ],
        Op::SubgroupCollective { value_type, .. } => vec![Type::Scalar(*value_type)],
        Op::MatrixAccess {
            matrix,
            subgroup_brand,
            width,
            ..
        } => vec![cap(
            *matrix,
            ExecutionCapabilityRoleV1::Matrix {
                subgroup_brand: *subgroup_brand,
                width: *width,
            },
        )],
        Op::AsyncCopy {
            pending,
            element,
            layout,
            elements,
            ..
        } => vec![cap(
            *pending,
            ExecutionCapabilityRoleV1::PendingAsyncCopy {
                element: *element,
                layout: *layout,
                elements: *elements,
            },
        )],
        Op::AsyncWait {
            output_lds,
            transition,
            element,
            layout,
            elements,
            ..
        } => vec![
            cap(*transition, ExecutionCapabilityRoleV1::Workgroup),
            lds(
                *output_lds,
                *element,
                *layout,
                *elements,
                ExecutionLdsStateV1::Published,
            ),
        ],
        Op::RawMemoryBind {
            view,
            extent,
            element,
            layout,
            space,
            access,
            index_space,
            atomic_scope,
            ..
        } => vec![cap(
            *view,
            ExecutionCapabilityRoleV1::MemoryView {
                element: *element,
                layout: *layout,
                space: *space,
                access: *access,
                extent: ExecutionMemoryExtentV1::Dynamic(*extent),
                initialization: ExecutionMemoryInitializationV1::FullyInitialized,
                index_space: *index_space,
                atomic_scope: *atomic_scope,
            },
        )],
        Op::PrivateMemoryAllocate {
            view,
            element,
            layout,
            elements,
            ..
        } => vec![cap(
            *view,
            ExecutionCapabilityRoleV1::MemoryView {
                element: *element,
                layout: *layout,
                space: ExecutionMemoryAddressSpaceV1::Private,
                access: ExecutionMemoryAccessV1::ExclusiveReadWrite,
                extent: ExecutionMemoryExtentV1::Static(*elements),
                initialization: ExecutionMemoryInitializationV1::FullyInitialized,
                index_space: None,
                atomic_scope: None,
            },
        )],
        Op::WorkgroupMemoryIndex { witness, .. } => vec![cap(
            *witness,
            ExecutionCapabilityRoleV1::WorkgroupMemoryIndex,
        )],
        Op::WorkgroupMemoryAllocate {
            view,
            element,
            layout,
            elements,
            index_space,
            ..
        } => vec![cap(
            *view,
            ExecutionCapabilityRoleV1::MemoryView {
                element: *element,
                layout: *layout,
                space: ExecutionMemoryAddressSpaceV1::Workgroup,
                access: ExecutionMemoryAccessV1::DisjointWrite,
                extent: ExecutionMemoryExtentV1::Static(*elements),
                initialization: ExecutionMemoryInitializationV1::Uninitialized,
                index_space: Some(*index_space),
                atomic_scope: None,
            },
        )],
        Op::WorkgroupMemoryPublish {
            output_view,
            transition,
            element,
            layout,
            ..
        } => vec![
            cap(*transition, ExecutionCapabilityRoleV1::Workgroup),
            cap(
                *output_view,
                ExecutionCapabilityRoleV1::MemoryView {
                    element: *element,
                    layout: *layout,
                    space: ExecutionMemoryAddressSpaceV1::Workgroup,
                    access: ExecutionMemoryAccessV1::ReadOnly,
                    extent: ExecutionMemoryExtentV1::Static(ELEMENTS),
                    initialization: ExecutionMemoryInitializationV1::Published,
                    index_space: None,
                    atomic_scope: None,
                },
            ),
        ],
        Op::MemoryStore { .. } => vec![Type::BOOL],
    }
}

struct CatalogModuleBuilder {
    block: BasicBlock,
    next_value: u32,
    next_source: u8,
    requirements: BTreeSet<TargetCapability>,
    context: ValueId,
}

impl CatalogModuleBuilder {
    fn new() -> Self {
        let context = ValueId(16);
        let mut block = BasicBlock::new(BlockId(0));
        block.operations.push(Operation::kernel_context_issue(
            context,
            KernelContextTypeV1::new(
                "entry",
                provenance().kernel_marker,
                provenance().target_brand,
                provenance().launch_brand,
            ),
            KernelContextSourceIdentityV1::new([0x81; 32], [0x82; 32], [0x83; 32], [0x84; 32]),
        ));
        Self {
            block,
            next_value: 17,
            next_source: 1,
            requirements: BTreeSet::new(),
            context,
        }
    }

    fn fresh(&mut self) -> ValueId {
        let value = ValueId(self.next_value);
        self.next_value += 1;
        value
    }

    fn constant(&mut self, ty: Type, constant: Constant) -> ValueId {
        let value = self.fresh();
        self.block.operations.push(Operation::effect_free(
            ValueDef::new(value, ty),
            OperationKind::Constant(constant),
        ));
        value
    }

    fn emit(
        &mut self,
        operation: ExecutionCapabilityOperationV1,
        operands: Vec<ValueId>,
        epoch_before: [u8; 32],
    ) -> Vec<ValueId> {
        self.requirements.extend(operation.required_capabilities());
        let results = result_types(&operation, epoch_before)
            .into_iter()
            .map(|ty| ValueDef::new(self.fresh(), ty))
            .collect::<Vec<_>>();
        let values = results.iter().map(|result| result.id).collect::<Vec<_>>();
        let contract = ExecutionCapabilityOpV1 {
            operands,
            signature: signature(&operation),
            provenance: provenance(),
            workgroup_brand: (!operation.is_kernel_scoped()).then_some(WORKGROUP_BRAND),
            epoch_before: (!operation.is_kernel_scoped()).then_some(epoch_before),
            epoch_after: operation.transitions_epoch().then_some(EPOCH_AFTER),
            obligations: ExecutionSafetyObligationsV1::from_bits(
                required_execution_obligations_v1(&operation),
            ),
            source: ExecutionCapabilitySourceV1 {
                function: [0x67; 32],
                operation: [self.next_source; 32],
                block: 0,
            },
            operation,
        };
        self.next_source = self.next_source.wrapping_add(1);
        self.block.operations.push(Operation::new(
            results,
            OperationKind::ExecutionCapability(contract),
        ));
        values
    }

    fn workgroup(&mut self, source: ExecutionTypeIdentityV1, epoch: [u8; 32]) -> ValueId {
        self.emit(
            ExecutionCapabilityOperationV1::WorkgroupDerive {
                context: id(0x10),
                workgroup: source,
            },
            vec![self.context],
            epoch,
        )[0]
    }

    fn subgroup(
        &mut self,
        workgroup_source: ExecutionTypeIdentityV1,
        subgroup_source: ExecutionTypeIdentityV1,
        width: u32,
        epoch: [u8; 32],
    ) -> ValueId {
        let workgroup = self.workgroup(workgroup_source, epoch);
        self.emit(
            ExecutionCapabilityOperationV1::SubgroupDerive {
                workgroup: workgroup_source,
                subgroup: subgroup_source,
                width,
            },
            vec![workgroup],
            epoch,
        )[0]
    }

    fn lds_uninitialized(
        &mut self,
        workgroup_source: ExecutionTypeIdentityV1,
        lds_source: ExecutionTypeIdentityV1,
        element: ExecutionTypeIdentityV1,
        epoch: [u8; 32],
    ) -> (ValueId, ValueId) {
        let workgroup = self.workgroup(workgroup_source, epoch);
        let lds = self.emit(
            ExecutionCapabilityOperationV1::LdsAllocate {
                workgroup: workgroup_source,
                lds: lds_source,
                element,
                layout: layout(),
                elements: ELEMENTS,
            },
            vec![workgroup],
            epoch,
        )[0];
        (workgroup, lds)
    }

    fn lds_initialized(
        &mut self,
        workgroup_source: ExecutionTypeIdentityV1,
        output_source: ExecutionTypeIdentityV1,
        element: ExecutionTypeIdentityV1,
        epoch: [u8; 32],
    ) -> (ValueId, ValueId) {
        let (workgroup, input) = self.lds_uninitialized(workgroup_source, id(0xa1), element, epoch);
        let output = self.emit(
            ExecutionCapabilityOperationV1::LdsInitializeByInvocation {
                input_lds: id(0xa1),
                workgroup: workgroup_source,
                output_lds: output_source,
                element,
                layout: layout(),
                elements: ELEMENTS,
            },
            vec![input, workgroup, ValueId(0)],
            epoch,
        )[0];
        (workgroup, output)
    }

    fn lds_published(
        &mut self,
        workgroup_source: ExecutionTypeIdentityV1,
        output_source: ExecutionTypeIdentityV1,
        element: ExecutionTypeIdentityV1,
    ) -> (ValueId, ValueId) {
        let (workgroup, input) =
            self.lds_initialized(workgroup_source, id(0xa2), element, EPOCH_BEFORE);
        let results = self.emit(
            ExecutionCapabilityOperationV1::LdsPublish {
                input_workgroup: workgroup_source,
                input_lds: id(0xa2),
                output_lds: output_source,
                transition: workgroup_source,
                element,
                layout: layout(),
                elements: ELEMENTS,
            },
            vec![workgroup, input],
            EPOCH_BEFORE,
        );
        (results[0], results[1])
    }

    fn private_view(
        &mut self,
        source: ExecutionTypeIdentityV1,
        element: ExecutionTypeIdentityV1,
    ) -> ValueId {
        self.emit(
            ExecutionCapabilityOperationV1::PrivateMemoryAllocate {
                context: id(0x10),
                view: source,
                element,
                layout: layout(),
                elements: ELEMENTS,
            },
            vec![self.context],
            EPOCH_BEFORE,
        )[0]
    }

    fn workgroup_view(
        &mut self,
        workgroup_source: ExecutionTypeIdentityV1,
        source: ExecutionTypeIdentityV1,
        element: ExecutionTypeIdentityV1,
    ) -> (ValueId, ValueId) {
        let workgroup = self.workgroup(workgroup_source, EPOCH_BEFORE);
        let view = self.emit(
            ExecutionCapabilityOperationV1::WorkgroupMemoryAllocate {
                workgroup: workgroup_source,
                view: source,
                element,
                layout: layout(),
                elements: ELEMENTS,
                index_space: id(0x3e),
            },
            vec![workgroup],
            EPOCH_BEFORE,
        )[0];
        (workgroup, view)
    }

    fn bound_proof(&mut self, value: ValueId, upper_bound: u64) -> ValueId {
        let bound = self.constant(Type::INDEX, Constant::Index(upper_bound));
        let proof = self.fresh();
        self.block.operations.push(Operation::effect_free(
            ValueDef::new(proof, Type::BOOL),
            OperationKind::Compare {
                predicate: ComparePredicate::LessThanOrEqual,
                lhs: value,
                rhs: bound,
            },
        ));
        proof
    }

    fn raw_view(
        &mut self,
        authority_source: ExecutionTypeIdentityV1,
        view_source: ExecutionTypeIdentityV1,
        element: ExecutionTypeIdentityV1,
        space: ExecutionMemoryAddressSpaceV1,
        access: ExecutionMemoryAccessV1,
        atomic_scope: Option<ExecutionMemoryScopeV1>,
    ) -> ValueId {
        let authority = if space == ExecutionMemoryAddressSpaceV1::Private {
            self.context
        } else {
            self.workgroup(authority_source, EPOCH_BEFORE)
        };
        let pointer = match space {
            ExecutionMemoryAddressSpaceV1::Global => ValueId(3),
            ExecutionMemoryAddressSpaceV1::Private => ValueId(4),
            ExecutionMemoryAddressSpaceV1::Workgroup => ValueId(5),
        };
        let proof = self.bound_proof(ValueId(1), ELEMENTS);
        let extent = ExecutionDynamicExtentV1 {
            operand: 2,
            source_argument: 2,
            source_type: id(0x13),
            value_type: ScalarType::Index,
            upper_bound: ELEMENTS,
            bound_check_operand: 3,
            nonnegative_check_operand: None,
        };
        self.emit(
            ExecutionCapabilityOperationV1::RawMemoryBind {
                authority: authority_source,
                pointer: id(0x38),
                length: id(0x13),
                extent,
                view: view_source,
                element,
                layout: layout(),
                space,
                access,
                index_space: None,
                atomic_scope,
                unsafe_obligation: id(0x3a),
            },
            vec![authority, pointer, ValueId(1), proof],
            EPOCH_BEFORE,
        )[0]
    }

    fn target_operands(
        &mut self,
        operation: &ExecutionCapabilityOperationV1,
    ) -> (Vec<ValueId>, [u8; 32]) {
        use ExecutionCapabilityOperationV1 as Op;
        match operation {
            Op::WorkgroupDerive { .. } => (vec![self.context], EPOCH_BEFORE),
            Op::SubgroupDerive { workgroup, .. }
            | Op::LdsAllocate { workgroup, .. }
            | Op::WorkgroupBarrier {
                input_workgroup: workgroup,
                ..
            }
            | Op::WorkgroupFence { workgroup, .. }
            | Op::WorkgroupMemoryIndex { workgroup, .. }
            | Op::WorkgroupMemoryAllocate { workgroup, .. } => {
                (vec![self.workgroup(*workgroup, EPOCH_BEFORE)], EPOCH_BEFORE)
            }
            Op::LdsInitializeByInvocation {
                input_lds,
                workgroup,
                element,
                ..
            } => {
                let (workgroup_value, lds) =
                    self.lds_uninitialized(*workgroup, *input_lds, *element, EPOCH_BEFORE);
                (vec![lds, workgroup_value, ValueId(0)], EPOCH_BEFORE)
            }
            Op::LdsPublish {
                input_workgroup,
                input_lds,
                element,
                ..
            } => {
                let (workgroup, lds) =
                    self.lds_initialized(*input_workgroup, *input_lds, *element, EPOCH_BEFORE);
                (vec![workgroup, lds], EPOCH_BEFORE)
            }
            Op::LdsReadPublished {
                lds,
                workgroup,
                element,
                ..
            } => {
                let (workgroup, lds) = self.lds_published(*workgroup, *lds, *element);
                (vec![lds, workgroup, ValueId(1)], EPOCH_AFTER)
            }
            Op::SubgroupBarrier {
                input_workgroup,
                subgroup,
                width,
                ..
            } => {
                let workgroup = self.workgroup(*input_workgroup, EPOCH_BEFORE);
                let subgroup = self.emit(
                    Op::SubgroupDerive {
                        workgroup: *input_workgroup,
                        subgroup: *subgroup,
                        width: *width,
                    },
                    vec![workgroup],
                    EPOCH_BEFORE,
                )[0];
                (vec![workgroup, subgroup], EPOCH_BEFORE)
            }
            Op::SubgroupFence {
                subgroup, width, ..
            }
            | Op::MatrixAccess {
                subgroup, width, ..
            } => (
                vec![self.subgroup(id(0x11), *subgroup, *width, EPOCH_BEFORE)],
                EPOCH_BEFORE,
            ),
            Op::Atomic {
                kind: ExecutionAtomicKindV1::BindGlobalView,
                ..
            } => (vec![self.context, ValueId(2)], EPOCH_BEFORE),
            Op::Atomic {
                kind,
                authority,
                location_input,
                location,
                element,
                scope,
                ..
            } => {
                let workgroup = self.workgroup(*authority, EPOCH_BEFORE);
                let view = self.raw_view(
                    *authority,
                    *location_input,
                    *element,
                    ExecutionMemoryAddressSpaceV1::Global,
                    ExecutionMemoryAccessV1::AtomicReadWrite,
                    Some(*scope),
                );
                if *kind == ExecutionAtomicKindV1::BindGlobalLocation {
                    return (vec![workgroup, view, ValueId(1)], EPOCH_BEFORE);
                }
                let bind = Op::Atomic {
                    kind: ExecutionAtomicKindV1::BindGlobalLocation,
                    authority: *authority,
                    location_input: *location_input,
                    location: *location,
                    element: *element,
                    operand: Some(id(0x14)),
                    replacement: None,
                    result: id(0xb1),
                    value_type: ScalarType::U32,
                    address_space: ExecutionMemoryAddressSpaceV1::Global,
                    scope: *scope,
                    success: None,
                    failure: None,
                };
                let location = self.emit(bind, vec![workgroup, view, ValueId(1)], EPOCH_BEFORE)[0];
                let mut operands = vec![workgroup, location];
                match kind {
                    ExecutionAtomicKindV1::Load => {}
                    ExecutionAtomicKindV1::Store | ExecutionAtomicKindV1::FetchAdd => {
                        operands.push(ValueId(0));
                    }
                    ExecutionAtomicKindV1::CompareExchange => {
                        operands.extend([ValueId(0), ValueId(0)]);
                    }
                    ExecutionAtomicKindV1::BindGlobalLocation
                    | ExecutionAtomicKindV1::BindGlobalView => unreachable!(),
                }
                (operands, EPOCH_BEFORE)
            }
            Op::WorkgroupCollective {
                input_workgroup,
                scratch,
                element,
                ..
            } => {
                let (workgroup, scratch) =
                    self.lds_uninitialized(*input_workgroup, *scratch, *element, EPOCH_BEFORE);
                (vec![workgroup, scratch, ValueId(0)], EPOCH_BEFORE)
            }
            Op::SubgroupCollective {
                subgroup, width, ..
            } => (
                vec![
                    self.subgroup(id(0x11), *subgroup, *width, EPOCH_BEFORE),
                    ValueId(0),
                ],
                EPOCH_BEFORE,
            ),
            Op::AsyncCopy {
                workgroup,
                source,
                destination,
                element,
                ..
            } => {
                let workgroup_value = self.workgroup(*workgroup, EPOCH_BEFORE);
                let source = self.raw_view(
                    *workgroup,
                    *source,
                    *element,
                    ExecutionMemoryAddressSpaceV1::Global,
                    ExecutionMemoryAccessV1::ReadOnly,
                    None,
                );
                let (_, destination) =
                    self.lds_uninitialized(*workgroup, *destination, *element, EPOCH_BEFORE);
                (
                    vec![workgroup_value, source, ValueId(1), destination],
                    EPOCH_BEFORE,
                )
            }
            Op::AsyncWait {
                input_workgroup,
                pending,
                element,
                ..
            } => {
                let copy = Op::AsyncCopy {
                    workgroup: *input_workgroup,
                    source_reference: id(0xb2),
                    source: id(0xb3),
                    index: id(0x14),
                    destination: id(0xb4),
                    pending: *pending,
                    element: *element,
                    layout: layout(),
                    elements: ELEMENTS,
                };
                let (operands, _) = self.target_operands(&copy);
                let pending = self.emit(copy, operands, EPOCH_BEFORE)[0];
                let workgroup = self.workgroup(*input_workgroup, EPOCH_BEFORE);
                (vec![workgroup, pending], EPOCH_BEFORE)
            }
            Op::RawMemoryBind {
                authority,
                extent,
                space,
                ..
            } => {
                let authority = if *space == ExecutionMemoryAddressSpaceV1::Private {
                    self.context
                } else {
                    self.workgroup(*authority, EPOCH_BEFORE)
                };
                let pointer = match space {
                    ExecutionMemoryAddressSpaceV1::Global => ValueId(3),
                    ExecutionMemoryAddressSpaceV1::Private => ValueId(4),
                    ExecutionMemoryAddressSpaceV1::Workgroup => ValueId(5),
                };
                let proof = self.bound_proof(ValueId(1), extent.upper_bound);
                (vec![authority, pointer, ValueId(1), proof], EPOCH_BEFORE)
            }
            Op::PrivateMemoryAllocate { .. } => (vec![self.context], EPOCH_BEFORE),
            Op::WorkgroupMemoryPublish {
                input_workgroup,
                input_view,
                element,
                ..
            } => {
                let (workgroup, view) =
                    self.workgroup_view(*input_workgroup, *input_view, *element);
                (vec![workgroup, view], EPOCH_BEFORE)
            }
            Op::MemoryLoad { view, element, .. } => {
                let raw_view = self.raw_view(
                    id(0x10),
                    *view,
                    *element,
                    ExecutionMemoryAddressSpaceV1::Private,
                    ExecutionMemoryAccessV1::ReadOnly,
                    None,
                );
                (vec![raw_view, ValueId(1)], EPOCH_BEFORE)
            }
            Op::MemoryStore { view, element, .. } => (
                vec![self.private_view(*view, *element), ValueId(1), ValueId(0)],
                EPOCH_BEFORE,
            ),
        }
    }

    fn finish(mut self, operation: ExecutionCapabilityOperationV1) -> Module {
        let (operands, epoch_before) = self.target_operands(&operation);
        self.emit(operation.clone(), operands, epoch_before);
        self.block.terminator = Some(Terminator::Return { values: vec![] });

        let parameters = vec![
            Type::Scalar(ScalarType::U32),
            Type::INDEX,
            Type::slice(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Global,
                AccessMode::ReadOnly,
            ),
            Type::pointer(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Global,
                AccessMode::ReadWrite,
            ),
            Type::pointer(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Private,
                AccessMode::ReadWrite,
            ),
            Type::pointer(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Workgroup,
                AccessMode::ReadWrite,
            ),
        ];
        let mut function = Function::kernel_entry(
            "entry",
            Signature::new(parameters, vec![]),
            (0..6).map(ValueId).collect(),
            vec![self.block],
        );
        function.required_capabilities = self.requirements.clone();

        let mut module = Module::new(format!("v13-catalog-{}", name(&operation)));
        module.required_capabilities = self.requirements;
        module.functions.push(function);
        module.kernels.push(Kernel::new(
            "entry-kernel",
            "entry",
            LaunchDomain::D1 {
                x: LaunchExtent::Static(64),
            },
        ));
        module
    }
}

fn module_for(operation: ExecutionCapabilityOperationV1, _ordinal: u8) -> Module {
    CatalogModuleBuilder::new().finish(operation)
}

fn atomic_operation(kind: ExecutionAtomicKindV1) -> ExecutionCapabilityOperationV1 {
    let element = id(0x15);
    let (authority, operand, replacement, result, success, failure) = match kind {
        ExecutionAtomicKindV1::BindGlobalView => (id(0x10), None, None, id(0x31), None, None),
        ExecutionAtomicKindV1::BindGlobalLocation => {
            (id(0x11), Some(id(0x14)), None, id(0x44), None, None)
        }
        ExecutionAtomicKindV1::Load => (
            id(0x11),
            None,
            None,
            element,
            Some(ExecutionMemoryOrderingV1::Acquire),
            None,
        ),
        ExecutionAtomicKindV1::Store => (
            id(0x11),
            Some(element),
            None,
            id(0x45),
            Some(ExecutionMemoryOrderingV1::Release),
            None,
        ),
        ExecutionAtomicKindV1::FetchAdd => (
            id(0x11),
            Some(element),
            None,
            element,
            Some(ExecutionMemoryOrderingV1::AcquireRelease),
            None,
        ),
        ExecutionAtomicKindV1::CompareExchange => (
            id(0x11),
            Some(element),
            Some(element),
            id(0x46),
            Some(ExecutionMemoryOrderingV1::AcquireRelease),
            Some(ExecutionMemoryOrderingV1::Acquire),
        ),
    };
    ExecutionCapabilityOperationV1::Atomic {
        kind,
        authority,
        location_input: id(0x30),
        location: id(0x31),
        element,
        operand,
        replacement,
        result,
        value_type: ScalarType::U32,
        address_space: ExecutionMemoryAddressSpaceV1::Global,
        scope: ExecutionMemoryScopeV1::Device,
        success,
        failure,
    }
}

fn collective_operation(
    kind: ExecutionCollectiveKindV1,
    workgroup: bool,
) -> ExecutionCapabilityOperationV1 {
    if workgroup {
        ExecutionCapabilityOperationV1::WorkgroupCollective {
            kind,
            input_workgroup: id(0x11),
            scratch: id(0x20),
            element: id(0x15),
            transition: id(0x32),
            value_type: ScalarType::U32,
            layout: layout(),
            elements: ELEMENTS,
        }
    } else {
        ExecutionCapabilityOperationV1::SubgroupCollective {
            kind,
            subgroup_reference: id(0x2a),
            subgroup: id(0x12),
            epoch: id(0x2b),
            element: id(0x15),
            value_type: ScalarType::U32,
            width: 64,
        }
    }
}

fn capability(requirement: ExecutionCapabilityRequirementV1) -> TargetCapability {
    TargetCapability::Execution(requirement)
}

fn expected_capabilities(operation: &ExecutionCapabilityOperationV1) -> BTreeSet<TargetCapability> {
    use ExecutionCapabilityOperationV1 as Op;
    let workgroup_memory = || {
        BTreeSet::from([
            TargetCapability::WorkgroupMemory,
            capability(ExecutionCapabilityRequirementV1::Resource(
                ResourceCapabilityRequirementV1::StaticWorkgroupMemoryBytesAtMost(256),
            )),
        ])
    };
    match operation {
        Op::WorkgroupDerive { .. }
        | Op::WorkgroupFence { .. }
        | Op::WorkgroupMemoryIndex { .. }
        | Op::WorkgroupMemoryPublish { .. } => BTreeSet::new(),
        Op::SubgroupDerive { .. } | Op::SubgroupFence { .. } | Op::MatrixAccess { .. } => {
            BTreeSet::from([
                TargetCapability::Subgroups,
                TargetCapability::SubgroupSize(64),
            ])
        }
        Op::LdsAllocate { .. }
        | Op::LdsInitializeByInvocation { .. }
        | Op::LdsPublish { .. }
        | Op::LdsReadPublished { .. }
        | Op::AsyncWait { .. }
        | Op::WorkgroupMemoryAllocate { .. } => workgroup_memory(),
        Op::WorkgroupBarrier { .. } => {
            BTreeSet::from([capability(ExecutionCapabilityRequirementV1::Barrier {
                execution_scope: SynchronizationScope::Workgroup,
                memory_scope: SynchronizationScope::Workgroup,
                ordering: MemoryOrdering::AcquireRelease,
                address_spaces: BTreeSet::from([AddressSpace::Workgroup]),
            })])
        }
        Op::SubgroupBarrier { .. } => BTreeSet::from([
            TargetCapability::Subgroups,
            TargetCapability::SubgroupSize(64),
            capability(ExecutionCapabilityRequirementV1::Barrier {
                execution_scope: SynchronizationScope::Subgroup,
                memory_scope: SynchronizationScope::Subgroup,
                ordering: MemoryOrdering::AcquireRelease,
                address_spaces: BTreeSet::from([AddressSpace::Workgroup]),
            }),
        ]),
        Op::Atomic { .. } => BTreeSet::new(),
        Op::WorkgroupCollective { .. } => {
            BTreeSet::from([capability(ExecutionCapabilityRequirementV1::Collective {
                execution_scope: SynchronizationScope::Workgroup,
                operation: CollectiveCapabilityOperationV1::ReduceAdd,
                value_type: ScalarType::U32,
                participants: 64,
            })])
        }
        Op::SubgroupCollective { .. } => BTreeSet::from([
            TargetCapability::Subgroups,
            TargetCapability::SubgroupSize(64),
            capability(ExecutionCapabilityRequirementV1::Collective {
                execution_scope: SynchronizationScope::Subgroup,
                operation: CollectiveCapabilityOperationV1::ReduceAdd,
                value_type: ScalarType::U32,
                participants: 64,
            }),
        ]),
        Op::AsyncCopy { .. } => {
            BTreeSet::from([capability(ExecutionCapabilityRequirementV1::AsyncCopy {
                source: AddressSpace::Global,
                destination: AddressSpace::Workgroup,
                bytes: 256,
                alignment: 4,
                completion: AsyncCopyCompletionV1::WorkgroupBarrier,
            })])
        }
        Op::RawMemoryBind { .. } | Op::MemoryLoad { .. } => {
            BTreeSet::from([capability(ExecutionCapabilityRequirementV1::AddressSpace {
                address_space: AddressSpace::Private,
                access: AccessMode::ReadOnly,
            })])
        }
        Op::PrivateMemoryAllocate { .. } => {
            BTreeSet::from([capability(ExecutionCapabilityRequirementV1::Resource(
                ResourceCapabilityRequirementV1::PrivateMemoryBytesPerInvocationAtMost(256),
            ))])
        }
        Op::MemoryStore { .. } => {
            BTreeSet::from([capability(ExecutionCapabilityRequirementV1::AddressSpace {
                address_space: AddressSpace::Private,
                access: AccessMode::ReadWrite,
            })])
        }
    }
}

fn expected_effects(operation: &ExecutionCapabilityOperationV1) -> Vec<MemoryEffect> {
    use ExecutionCapabilityOperationV1 as Op;
    match operation {
        Op::LdsAllocate { .. } | Op::WorkgroupMemoryAllocate { .. } => {
            vec![MemoryEffect::Allocate(AddressSpace::Workgroup)]
        }
        Op::PrivateMemoryAllocate { .. } => {
            vec![MemoryEffect::Allocate(AddressSpace::Private)]
        }
        Op::LdsInitializeByInvocation { .. }
        | Op::LdsPublish { .. }
        | Op::WorkgroupMemoryPublish { .. } => {
            vec![MemoryEffect::Write(AddressSpace::Workgroup)]
        }
        Op::LdsReadPublished { .. } => vec![MemoryEffect::Read(AddressSpace::Workgroup)],
        Op::WorkgroupBarrier { .. } => vec![MemoryEffect::Synchronize {
            execution_scope: SynchronizationScope::Workgroup,
            memory_scope: SynchronizationScope::Workgroup,
            address_spaces: BTreeSet::from([AddressSpace::Workgroup]),
        }],
        Op::SubgroupBarrier { .. } => vec![MemoryEffect::Synchronize {
            execution_scope: SynchronizationScope::Subgroup,
            memory_scope: SynchronizationScope::Subgroup,
            address_spaces: BTreeSet::from([AddressSpace::Workgroup]),
        }],
        Op::WorkgroupFence { .. } => vec![MemoryEffect::Fence {
            memory_scope: SynchronizationScope::Workgroup,
            ordering: MemoryOrdering::AcquireRelease,
            address_spaces: BTreeSet::from([AddressSpace::Workgroup]),
        }],
        Op::SubgroupFence { .. } => vec![MemoryEffect::Fence {
            memory_scope: SynchronizationScope::Subgroup,
            ordering: MemoryOrdering::AcquireRelease,
            address_spaces: BTreeSet::from([AddressSpace::Workgroup]),
        }],
        Op::Atomic { .. } => Vec::new(),
        Op::WorkgroupCollective { .. } => vec![
            MemoryEffect::Read(AddressSpace::Workgroup),
            MemoryEffect::Write(AddressSpace::Workgroup),
        ],
        Op::AsyncCopy { .. } => vec![
            MemoryEffect::Read(AddressSpace::Global),
            MemoryEffect::Write(AddressSpace::Workgroup),
        ],
        Op::MemoryLoad { .. } => vec![MemoryEffect::Read(AddressSpace::Private)],
        Op::MemoryStore { .. } => vec![MemoryEffect::Write(AddressSpace::Private)],
        Op::WorkgroupDerive { .. }
        | Op::SubgroupDerive { .. }
        | Op::SubgroupCollective { .. }
        | Op::MatrixAccess { .. }
        | Op::AsyncWait { .. }
        | Op::RawMemoryBind { .. }
        | Op::WorkgroupMemoryIndex { .. } => Vec::new(),
    }
}

fn target_contract(module: &Module) -> &ExecutionCapabilityOpV1 {
    let operation = target_operation(module);
    let OperationKind::ExecutionCapability(contract) = &operation.kind else {
        unreachable!()
    };
    contract
}

fn target_operation(module: &Module) -> &Operation {
    module.functions[0].body.as_ref().unwrap().blocks[0]
        .operations
        .last()
        .unwrap()
}

fn target_contract_mut(module: &mut Module) -> &mut ExecutionCapabilityOpV1 {
    let operation = target_operation_mut(module);
    let OperationKind::ExecutionCapability(contract) = &mut operation.kind else {
        unreachable!()
    };
    contract
}

fn target_operation_mut(module: &mut Module) -> &mut Operation {
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .last_mut()
        .unwrap()
}

fn assert_invalid_execution_capability(module: &Module, case: &str) {
    let error = verify_module(module).unwrap_err();
    assert!(
        error.contains(DiagnosticCode::InvalidExecutionCapability),
        "{case}: {error}"
    );
}

fn mutate_semantic_field(operation: &mut ExecutionCapabilityOperationV1) {
    use ExecutionCapabilityOperationV1 as Op;
    match operation {
        Op::WorkgroupDerive { workgroup, .. } => *workgroup = id(0xe0),
        Op::SubgroupDerive { width, .. } => *width = 32,
        Op::LdsAllocate { elements, .. }
        | Op::LdsInitializeByInvocation { elements, .. }
        | Op::LdsPublish { elements, .. }
        | Op::LdsReadPublished { elements, .. }
        | Op::WorkgroupCollective { elements, .. }
        | Op::AsyncCopy { elements, .. }
        | Op::AsyncWait { elements, .. }
        | Op::PrivateMemoryAllocate { elements, .. }
        | Op::WorkgroupMemoryAllocate { elements, .. } => *elements = ELEMENTS / 2,
        Op::WorkgroupBarrier { semantics, .. } | Op::WorkgroupFence { semantics, .. } => {
            semantics.spaces = ExecutionMemorySpacesV1::Global
        }
        Op::SubgroupBarrier { width, .. }
        | Op::SubgroupFence { width, .. }
        | Op::SubgroupCollective { width, .. }
        | Op::MatrixAccess { width, .. } => *width = 32,
        Op::Atomic { success, .. } => {
            *success = Some(ExecutionMemoryOrderingV1::SequentiallyConsistent)
        }
        Op::RawMemoryBind { layout, .. } => layout.byte_alignment = 8,
        Op::WorkgroupMemoryIndex { witness, .. } => *witness = id(0xe1),
        Op::WorkgroupMemoryPublish { output_view, .. } => *output_view = id(0xe2),
        Op::MemoryLoad { access, .. } => *access = ExecutionMemoryAccessV1::ExclusiveReadWrite,
        Op::MemoryStore { access, .. } => *access = ExecutionMemoryAccessV1::DisjointWrite,
    }
}

#[test]
fn roster_is_exactly_the_23_closed_execution_operation_families() {
    let actual = catalog().iter().map(name).collect::<BTreeSet<_>>();
    let expected = BTreeSet::from([
        "AsyncCopy",
        "AsyncWait",
        "Atomic",
        "LdsAllocate",
        "LdsInitializeByInvocation",
        "LdsPublish",
        "LdsReadPublished",
        "MatrixAccess",
        "MemoryLoad",
        "MemoryStore",
        "PrivateMemoryAllocate",
        "RawMemoryBind",
        "SubgroupBarrier",
        "SubgroupCollective",
        "SubgroupDerive",
        "SubgroupFence",
        "WorkgroupBarrier",
        "WorkgroupCollective",
        "WorkgroupDerive",
        "WorkgroupFence",
        "WorkgroupMemoryAllocate",
        "WorkgroupMemoryIndex",
        "WorkgroupMemoryPublish",
    ]);
    assert_eq!(actual, expected);
    assert_eq!(actual.len(), 23);
}

#[test]
fn every_catalog_entry_has_exact_capabilities_effects_and_v13_round_trip() {
    for (ordinal, operation) in catalog().into_iter().enumerate() {
        let case_name = name(&operation);
        assert!(operation.is_well_formed(), "{case_name}");
        assert!(
            operation.signature_matches(signature(&operation)),
            "{case_name}"
        );
        assert_eq!(
            operation.required_capabilities(),
            expected_capabilities(&operation),
            "{case_name} target capabilities"
        );
        assert_eq!(
            operation.memory_effects(),
            expected_effects(&operation),
            "{case_name} memory effects"
        );

        let module = module_for(operation.clone(), ordinal as u8);
        verify_module(&module).unwrap_or_else(|error| panic!("{case_name}: {error}"));
        assert_eq!(target_contract(&module).operation, operation, "{case_name}");
        let encoded = encode_module_v13(&module).unwrap();
        assert_eq!(&encoded[8..10], &KERNEL_IR_VERSION_V13.to_le_bytes());
        assert_eq!(decode_module_v13(&encoded).unwrap(), module, "{case_name}");
        let (owner, decoded) =
            VerifiedCanonicalKernelIrV13::from_canonical_bytes_with_module(encoded).unwrap();
        owner.revalidate().unwrap();
        assert_eq!(decoded, module, "{case_name}");
        assert_eq!(
            owner.identity().canonical_length(),
            owner.canonical_bytes().len() as u64,
            "{case_name}"
        );
    }
}

#[test]
fn every_operation_semantic_mutation_is_rejected_or_changes_exact_identity() {
    for (ordinal, operation) in catalog().into_iter().enumerate() {
        let case_name = name(&operation);
        let baseline =
            VerifiedCanonicalKernelIrV13::from_module(module_for(operation.clone(), ordinal as u8))
                .unwrap_or_else(|error| panic!("{case_name} baseline: {error}"));
        let mut mutated = module_for(operation, ordinal as u8);
        mutate_semantic_field(&mut target_contract_mut(&mut mutated).operation);

        match VerifiedCanonicalKernelIrV13::from_module(mutated) {
            Ok(changed) => assert_ne!(
                changed.identity(),
                baseline.identity(),
                "{case_name} mutation retained canonical identity"
            ),
            Err(VerifiedCanonicalKernelIrErrorV13::Encode(KernelIrEncodeError::NonCanonical {
                ..
            }))
            | Err(VerifiedCanonicalKernelIrErrorV13::Verification(_)) => {}
            Err(error) => panic!("{case_name} unexpected mutation result: {error}"),
        }
    }
}

#[test]
fn every_family_rejects_missing_extra_and_reordered_ssa_shapes() {
    for (ordinal, operation) in catalog().into_iter().enumerate() {
        let case_name = name(&operation);
        let baseline = module_for(operation, ordinal as u8);

        let mut missing_operand = baseline.clone();
        target_contract_mut(&mut missing_operand).operands.pop();
        assert_invalid_execution_capability(
            &missing_operand,
            &format!("{case_name} missing operand"),
        );

        let mut extra_operand = baseline.clone();
        target_contract_mut(&mut extra_operand)
            .operands
            .push(ValueId(0));
        assert_invalid_execution_capability(&extra_operand, &format!("{case_name} extra operand"));

        if target_contract(&baseline).operands.len() > 1 {
            let mut reordered_operands = baseline.clone();
            target_contract_mut(&mut reordered_operands)
                .operands
                .swap(0, 1);
            assert_invalid_execution_capability(
                &reordered_operands,
                &format!("{case_name} reordered operands"),
            );
        }

        if !target_operation(&baseline).results.is_empty() {
            let mut missing_result = baseline.clone();
            target_operation_mut(&mut missing_result).results.pop();
            assert_invalid_execution_capability(
                &missing_result,
                &format!("{case_name} missing result"),
            );
        }

        let mut extra_result = baseline.clone();
        target_operation_mut(&mut extra_result)
            .results
            .push(ValueDef::new(
                ValueId(u32::MAX - ordinal as u32),
                Type::Unit,
            ));
        assert_invalid_execution_capability(&extra_result, &format!("{case_name} extra result"));

        if target_operation(&baseline).results.len() > 1 {
            let mut reordered_results = baseline;
            target_operation_mut(&mut reordered_results)
                .results
                .swap(0, 1);
            assert_invalid_execution_capability(
                &reordered_results,
                &format!("{case_name} reordered results"),
            );
        }
    }
}

#[test]
fn verifier_rejects_wrong_operand_type_result_type_and_result_role() {
    let mut wrong_operand = module_for(catalog()[22].clone(), 22);
    target_contract_mut(&mut wrong_operand).operands[1] = ValueId(0);
    assert_invalid_execution_capability(&wrong_operand, "MemoryStore wrong index type");

    let mut wrong_result_type = module_for(catalog()[21].clone(), 21);
    target_operation_mut(&mut wrong_result_type).results[0].ty = Type::Scalar(ScalarType::U64);
    assert_invalid_execution_capability(&wrong_result_type, "MemoryLoad wrong result type");

    let mut wrong_result_role = module_for(catalog()[19].clone(), 19);
    let Type::ExecutionCapability(capability) =
        &mut target_operation_mut(&mut wrong_result_role).results[0].ty
    else {
        unreachable!()
    };
    capability.role = ExecutionCapabilityRoleV1::Workgroup;
    assert_invalid_execution_capability(
        &wrong_result_role,
        "WorkgroupMemoryAllocate wrong result role",
    );
}

#[test]
fn every_nested_atomic_kind_has_an_exact_positive_construction() {
    let cases = [
        (ExecutionAtomicKindV1::BindGlobalLocation, 2),
        (ExecutionAtomicKindV1::Load, 1),
        (ExecutionAtomicKindV1::Store, 0),
        (ExecutionAtomicKindV1::FetchAdd, 1),
        (ExecutionAtomicKindV1::CompareExchange, 2),
        (ExecutionAtomicKindV1::BindGlobalView, 1),
    ];
    let actual = cases.iter().map(|(kind, _)| *kind).collect::<BTreeSet<_>>();
    assert_eq!(
        actual,
        BTreeSet::from([
            ExecutionAtomicKindV1::BindGlobalLocation,
            ExecutionAtomicKindV1::Load,
            ExecutionAtomicKindV1::Store,
            ExecutionAtomicKindV1::FetchAdd,
            ExecutionAtomicKindV1::CompareExchange,
            ExecutionAtomicKindV1::BindGlobalView,
        ])
    );
    assert_eq!(actual.len(), 6);

    for (ordinal, (kind, result_count)) in cases.into_iter().enumerate() {
        let operation = atomic_operation(kind);
        assert!(operation.is_well_formed(), "{kind:?}");
        assert!(
            operation.signature_matches(signature(&operation)),
            "{kind:?}"
        );
        let module = module_for(operation, ordinal as u8);
        verify_module(&module).unwrap_or_else(|error| panic!("{kind:?}: {error}"));
        assert_eq!(
            target_operation(&module).results.len(),
            result_count,
            "{kind:?}"
        );
        VerifiedCanonicalKernelIrV13::from_module(module)
            .unwrap_or_else(|error| panic!("{kind:?} canonical V13: {error}"));
    }
}

#[test]
fn every_nested_collective_kind_has_exact_workgroup_and_subgroup_constructions() {
    let kinds = [
        ExecutionCollectiveKindV1::ReduceSum,
        ExecutionCollectiveKindV1::InclusiveScanSum,
        ExecutionCollectiveKindV1::ExclusiveScanSum,
    ];
    let actual = kinds.into_iter().collect::<BTreeSet<_>>();
    assert_eq!(
        actual,
        BTreeSet::from([
            ExecutionCollectiveKindV1::ReduceSum,
            ExecutionCollectiveKindV1::InclusiveScanSum,
            ExecutionCollectiveKindV1::ExclusiveScanSum,
        ])
    );
    assert_eq!(actual.len(), 3);

    for (ordinal, kind) in kinds.into_iter().enumerate() {
        for (workgroup, result_count) in [(true, 3), (false, 1)] {
            let operation = collective_operation(kind, workgroup);
            assert!(operation.is_well_formed(), "{kind:?} workgroup={workgroup}");
            assert!(
                operation.signature_matches(signature(&operation)),
                "{kind:?} workgroup={workgroup}"
            );
            let module = module_for(operation, ordinal as u8);
            verify_module(&module)
                .unwrap_or_else(|error| panic!("{kind:?} workgroup={workgroup}: {error}"));
            assert_eq!(
                target_operation(&module).results.len(),
                result_count,
                "{kind:?} workgroup={workgroup}"
            );
            VerifiedCanonicalKernelIrV13::from_module(module).unwrap_or_else(|error| {
                panic!("{kind:?} workgroup={workgroup} canonical V13: {error}")
            });
        }
    }
}
