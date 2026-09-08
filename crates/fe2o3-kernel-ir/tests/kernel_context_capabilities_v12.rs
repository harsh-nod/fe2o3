use std::collections::BTreeSet;

use fe2o3_kernel_ir::*;

type Encoder = fn(&Module) -> Result<Vec<u8>, KernelIrEncodeError>;
const V12_GOLDEN_HEX: &str = include_str!("fixtures/kernel_context_capabilities_v12.hex");

fn from_hex(text: &str) -> Vec<u8> {
    let compact = text
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect::<Vec<_>>();
    assert_eq!(compact.len() % 2, 0);
    compact
        .chunks_exact(2)
        .map(|pair| {
            let digit = |value: u8| match value {
                b'0'..=b'9' => value - b'0',
                b'a'..=b'f' => value - b'a' + 10,
                _ => panic!("invalid golden hex"),
            };
            (digit(pair[0]) << 4) | digit(pair[1])
        })
        .collect()
}

fn unique_offset(bytes: &[u8], marker: &[u8]) -> usize {
    let offsets = bytes
        .windows(marker.len())
        .enumerate()
        .filter_map(|(offset, window)| (window == marker).then_some(offset))
        .collect::<Vec<_>>();
    assert_eq!(offsets.len(), 1, "marker must occur exactly once");
    offsets[0]
}

fn context_type(root: &str) -> KernelContextTypeV1 {
    KernelContextTypeV1::new(root, [1; 32], [2; 32], [3; 32])
}

fn source_identity() -> KernelContextSourceIdentityV1 {
    KernelContextSourceIdentityV1::new([4; 32], [5; 32], [6; 32], [7; 32])
}

fn context_module() -> Module {
    let context = context_type("entry");

    let mut helper_block = BasicBlock::new(BlockId(0));
    helper_block.terminator = Some(Terminator::Return { values: vec![] });
    let helper = Function::internal_helper(
        "helper",
        Signature::new(vec![Type::KernelContext(context.clone())], vec![]),
        vec![ValueId(0)],
        vec![helper_block],
    );

    let mut entry_block = BasicBlock::new(BlockId(0));
    entry_block.operations.push(Operation::kernel_context_issue(
        ValueId(0),
        context,
        source_identity(),
    ));
    entry_block.operations.push(Operation::new(
        vec![],
        OperationKind::Call {
            callee: FunctionId::new("helper"),
            arguments: vec![ValueId(0)],
        },
    ));
    entry_block.terminator = Some(Terminator::Return { values: vec![] });
    let entry = Function::kernel_entry(
        "entry",
        Signature::new(vec![], vec![]),
        vec![],
        vec![entry_block],
    );

    let mut module = Module::new("kernel-context-v12");
    module.functions = vec![entry, helper];
    module.kernels.push(Kernel::new(
        "kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}

fn execution_requirements() -> BTreeSet<TargetCapability> {
    [
        ExecutionCapabilityRequirementV1::AddressSpace {
            address_space: AddressSpace::Global,
            access: AccessMode::ReadOnly,
        },
        ExecutionCapabilityRequirementV1::Atomic {
            value_type: ScalarType::U32,
            operation: AtomicKind::Add,
            ordering: MemoryOrdering::AcquireRelease,
            failure_ordering: None,
            scope: SynchronizationScope::Device,
            address_space: AddressSpace::Global,
        },
        ExecutionCapabilityRequirementV1::Barrier {
            execution_scope: SynchronizationScope::Workgroup,
            memory_scope: SynchronizationScope::Workgroup,
            ordering: MemoryOrdering::AcquireRelease,
            address_spaces: BTreeSet::from([AddressSpace::Workgroup]),
        },
        ExecutionCapabilityRequirementV1::Collective {
            execution_scope: SynchronizationScope::Subgroup,
            operation: CollectiveCapabilityOperationV1::ReduceAdd,
            value_type: ScalarType::F32,
            participants: 32,
        },
        ExecutionCapabilityRequirementV1::Matrix {
            m: 16,
            n: 16,
            k: 16,
            input_type: ScalarType::F16,
            accumulator_type: ScalarType::F32,
        },
        ExecutionCapabilityRequirementV1::AsyncCopy {
            source: AddressSpace::Global,
            destination: AddressSpace::Workgroup,
            bytes: 256,
            alignment: 16,
            completion: AsyncCopyCompletionV1::ExplicitWaitGroups {
                maximum_pending_groups: 4,
            },
        },
        ExecutionCapabilityRequirementV1::Numerical {
            value_type: ScalarType::F32,
            mode: NumericalModeV1::StrictIeee,
        },
        ExecutionCapabilityRequirementV1::Resource(
            ResourceCapabilityRequirementV1::WorkgroupInvocationsAtMost(256),
        ),
    ]
    .into_iter()
    .map(TargetCapability::Execution)
    .collect()
}

fn full_module() -> Module {
    let mut module = context_module();
    module.required_capabilities = execution_requirements();
    module
}

fn only_capability(requirement: ExecutionCapabilityRequirementV1) -> Module {
    let mut module = Module::new("execution-requirement-v12");
    module
        .required_capabilities
        .insert(TargetCapability::Execution(requirement));
    module
}

fn older_encoders() -> [(u16, Encoder); 11] {
    [
        (KERNEL_IR_VERSION_V1, encode_module_v1),
        (KERNEL_IR_VERSION_V2, encode_module_v2),
        (KERNEL_IR_VERSION_V3, encode_module_v3),
        (KERNEL_IR_VERSION_V4, encode_module_v4),
        (KERNEL_IR_VERSION_V5, encode_module_v5),
        (KERNEL_IR_VERSION_V6, encode_module_v6),
        (KERNEL_IR_VERSION_V7, encode_module_v7),
        (KERNEL_IR_VERSION_V8, encode_module_v8),
        (KERNEL_IR_VERSION_V9, encode_module_v9),
        (KERNEL_IR_VERSION_V10, encode_module_v10),
        (KERNEL_IR_VERSION_V11, encode_module_v11),
    ]
}

#[test]
fn v12_context_and_requirements_round_trip_and_verify() {
    let module = full_module();
    verify_module(&module).unwrap();
    let encoded = encode_module_v12(&module).unwrap();
    assert_eq!(encoded, from_hex(V12_GOLDEN_HEX));
    assert_eq!(&encoded[8..10], &KERNEL_IR_VERSION_V12.to_le_bytes());
    assert_eq!(decode_module_v12(&encoded).unwrap(), module);
    assert_eq!(encode_module_v12(&module).unwrap(), encoded);

    let owner = VerifiedCanonicalKernelIrV12::from_module(module).unwrap();
    owner.revalidate().unwrap();
    assert_eq!(owner.identity().canonical_length(), encoded.len() as u64);
    assert_eq!(owner.canonical_bytes(), encoded);
}

#[test]
fn v12_canonical_identity_is_deterministic_and_policy_separated() {
    let first = VerifiedCanonicalKernelIrV12::from_module(full_module()).unwrap();
    let second = VerifiedCanonicalKernelIrV12::from_module(full_module()).unwrap();
    assert_eq!(first.identity(), second.identity());
    assert_eq!(
        first.identity().digest(),
        &[
            221, 83, 145, 92, 222, 122, 208, 32, 141, 22, 108, 107, 20, 23, 124, 163, 64, 164, 110,
            126, 10, 141, 102, 118, 209, 16, 114, 110, 152, 232, 252, 172,
        ]
    );
    assert_ne!(
        VERIFIED_CANONICAL_KERNEL_IR_V12_IDENTITY_DOMAIN_V1,
        VERIFIED_CANONICAL_KERNEL_IR_V11_IDENTITY_DOMAIN_V1
    );
}

#[test]
fn prior_versions_reject_context_and_execution_requirements() {
    let context_only = context_module();
    let capability_only = only_capability(ExecutionCapabilityRequirementV1::AddressSpace {
        address_space: AddressSpace::Global,
        access: AccessMode::ReadOnly,
    });
    for (version, encode) in older_encoders() {
        assert_eq!(
            encode(&context_only),
            Err(KernelIrEncodeError::UnsupportedInVersion {
                version,
                feature: "logical kernel-context type",
            })
        );
        assert_eq!(
            encode(&capability_only),
            Err(KernelIrEncodeError::UnsupportedInVersion {
                version,
                feature: "portable execution-capability requirement",
            })
        );
    }
}

#[test]
fn v11_decoder_rejects_forged_v12_context_and_requirement_tags() {
    let mut context = encode_module_v12(&context_module()).unwrap();
    context[8..10].copy_from_slice(&KERNEL_IR_VERSION_V11.to_le_bytes());
    assert!(matches!(
        decode_module_v11(&context),
        Err(KernelIrDecodeError::UnknownTag {
            kind: "type",
            tag: 5,
        })
    ));

    let mut requirement = encode_module_v12(&only_capability(
        ExecutionCapabilityRequirementV1::AddressSpace {
            address_space: AddressSpace::Global,
            access: AccessMode::ReadOnly,
        },
    ))
    .unwrap();
    requirement[8..10].copy_from_slice(&KERNEL_IR_VERSION_V11.to_le_bytes());
    assert!(matches!(
        decode_module_v11(&requirement),
        Err(KernelIrDecodeError::UnknownTag {
            kind: "target capability",
            tag: 13,
        })
    ));
}

#[test]
fn v12_decoder_rejects_unknown_new_tags_and_noncanonical_nested_sets() {
    let barrier = ExecutionCapabilityRequirementV1::Barrier {
        execution_scope: SynchronizationScope::Workgroup,
        memory_scope: SynchronizationScope::Workgroup,
        ordering: MemoryOrdering::AcquireRelease,
        address_spaces: BTreeSet::from([AddressSpace::Workgroup, AddressSpace::Global]),
    };
    let encoded = encode_module_v12(&only_capability(barrier)).unwrap();
    let marker = [13, 3, 3, 3, 4, 2, 0, 0, 0, 2, 3];
    let requirement = unique_offset(&encoded, &marker);

    let mut unknown_requirement = encoded.clone();
    unknown_requirement[requirement + 1] = 255;
    assert!(matches!(
        decode_module_v12(&unknown_requirement),
        Err(KernelIrDecodeError::UnknownTag {
            kind: "execution capability requirement",
            tag: 255,
        })
    ));

    let mut reordered_spaces = encoded;
    reordered_spaces.swap(requirement + 9, requirement + 10);
    assert_eq!(
        decode_module_v12(&reordered_spaces),
        Err(KernelIrDecodeError::NonCanonical)
    );

    let mut unknown_operation = encode_module_v12(&context_module()).unwrap();
    let operation = unique_offset(&unknown_operation, &[27, 4, 4, 4, 4]);
    unknown_operation[operation] = 255;
    assert!(matches!(
        decode_module_v12(&unknown_operation),
        Err(KernelIrDecodeError::UnknownTag {
            kind: "operation",
            tag: 255,
        })
    ));
}

#[test]
fn verifier_rejects_incomplete_or_misbound_context_issuance() {
    let mut missing_source = context_module();
    let entry = missing_source.functions.first_mut().unwrap();
    let operation = &mut entry.body.as_mut().unwrap().blocks[0].operations[0];
    operation.kind = OperationKind::KernelContextIssue(KernelContextIssueV1::new(
        KernelContextSourceIdentityV1::new([0; 32], [5; 32], [6; 32], [7; 32]),
    ));
    assert!(
        verify_module(&missing_source)
            .unwrap_err()
            .contains(DiagnosticCode::InvalidKernelContext)
    );

    let mut wrong_root = context_module();
    let result =
        &mut wrong_root.functions[0].body.as_mut().unwrap().blocks[0].operations[0].results[0];
    result.ty = Type::KernelContext(context_type("another-root"));
    assert!(
        verify_module(&wrong_root)
            .unwrap_err()
            .contains(DiagnosticCode::InvalidKernelContext)
    );

    let mut incomplete_brand = context_module();
    let result = &mut incomplete_brand.functions[0].body.as_mut().unwrap().blocks[0].operations[0]
        .results[0];
    result.ty = Type::KernelContext(KernelContextTypeV1::new("entry", [0; 32], [2; 32], [3; 32]));
    assert!(
        verify_module(&incomplete_brand)
            .unwrap_err()
            .contains(DiagnosticCode::InvalidKernelContext)
    );
}

#[test]
fn verifier_rejects_context_fabrication_escape_and_duplicate_issuance() {
    let context = context_type("entry");
    let mut physical_abi = context_module();
    physical_abi.functions[0]
        .signature
        .parameters
        .push(Type::KernelContext(context.clone()));
    physical_abi.functions[0]
        .body
        .as_mut()
        .unwrap()
        .parameters
        .push(ValueId(9));
    assert!(
        verify_module(&physical_abi)
            .unwrap_err()
            .contains(DiagnosticCode::InvalidKernelContext)
    );

    let mut fabricated = context_module();
    fabricated.functions[0].body.as_mut().unwrap().blocks[0].operations[1] = Operation::new(
        vec![ValueDef::new(
            ValueId(8),
            Type::KernelContext(context.clone()),
        )],
        OperationKind::Select {
            condition: ValueId(0),
            true_value: ValueId(0),
            false_value: ValueId(0),
        },
    );
    assert!(
        verify_module(&fabricated)
            .unwrap_err()
            .contains(DiagnosticCode::InvalidKernelContext)
    );

    let mut duplicate = context_module();
    duplicate.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .insert(
            1,
            Operation::kernel_context_issue(ValueId(8), context, source_identity()),
        );
    assert!(
        verify_module(&duplicate)
            .unwrap_err()
            .contains(DiagnosticCode::InvalidKernelContext)
    );

    let mut entry_parameter = context_module();
    entry_parameter.functions[0].body.as_mut().unwrap().blocks[0]
        .parameters
        .push(ValueDef::new(
            ValueId(9),
            Type::KernelContext(context_type("entry")),
        ));
    assert!(
        verify_module(&entry_parameter)
            .unwrap_err()
            .contains(DiagnosticCode::InvalidKernelContext)
    );
}

#[test]
fn context_issuance_must_dominate_root_capability_operations() {
    let mut module = context_module();
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .insert(
            0,
            Operation::new(
                vec![ValueDef::new(ValueId(9), Type::INDEX)],
                OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
            ),
        );

    let error = verify_module(&module).unwrap_err();
    assert!(error.contains(DiagnosticCode::InvalidKernelContext));
    assert!(
        error
            .to_string()
            .contains("must dominate every capability operation")
    );
}

#[test]
fn context_enabled_root_rejects_contextless_capability_helper() {
    let mut module = context_module();
    let helper = &mut module.functions[1];
    helper.signature.parameters.clear();
    helper.body.as_mut().unwrap().parameters.clear();
    helper.body.as_mut().unwrap().blocks[0]
        .operations
        .push(Operation::new(
            vec![ValueDef::new(ValueId(1), Type::INDEX)],
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ));
    let OperationKind::Call { arguments, .. } =
        &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    arguments.clear();

    let error = verify_module(&module).unwrap_err();
    assert!(error.contains(DiagnosticCode::InvalidKernelContext));
    assert!(
        error
            .to_string()
            .contains("without its exact context brand")
    );
}

#[test]
fn context_enabled_root_rejects_cross_root_helper_brand() {
    let mut module = context_module();
    module.functions[1].signature.parameters[0] = Type::KernelContext(context_type("another-root"));

    let error = verify_module(&module).unwrap_err();
    assert!(error.contains(DiagnosticCode::InvalidKernelContext));
    assert!(
        error
            .to_string()
            .contains("substitutes or omits the exact context brand")
    );
}

#[test]
fn malformed_execution_requirements_fail_closed() {
    let invalid = [
        ExecutionCapabilityRequirementV1::Collective {
            execution_scope: SynchronizationScope::Subgroup,
            operation: CollectiveCapabilityOperationV1::ReduceAdd,
            value_type: ScalarType::F32,
            participants: 0,
        },
        ExecutionCapabilityRequirementV1::Matrix {
            m: 0,
            n: 16,
            k: 16,
            input_type: ScalarType::F16,
            accumulator_type: ScalarType::F32,
        },
        ExecutionCapabilityRequirementV1::AsyncCopy {
            source: AddressSpace::Global,
            destination: AddressSpace::Workgroup,
            bytes: 256,
            alignment: 3,
            completion: AsyncCopyCompletionV1::ExplicitWaitGroups {
                maximum_pending_groups: 0,
            },
        },
        ExecutionCapabilityRequirementV1::Numerical {
            value_type: ScalarType::U32,
            mode: NumericalModeV1::StrictIeee,
        },
        ExecutionCapabilityRequirementV1::Resource(
            ResourceCapabilityRequirementV1::WorkgroupInvocationsAtMost(0),
        ),
    ];
    for requirement in invalid {
        assert!(
            verify_module(&only_capability(requirement))
                .unwrap_err()
                .contains(DiagnosticCode::InvalidCapability)
        );
    }
}

#[test]
fn target_support_is_exact_except_for_resource_capacity() {
    let required = only_capability(ExecutionCapabilityRequirementV1::Resource(
        ResourceCapabilityRequirementV1::WorkgroupInvocationsAtMost(256),
    ));
    let enough = BTreeSet::from([TargetCapability::Execution(
        ExecutionCapabilityRequirementV1::Resource(
            ResourceCapabilityRequirementV1::WorkgroupInvocationsAtMost(512),
        ),
    )]);
    verify_module_with_capabilities(&required, &enough).unwrap();

    let insufficient = BTreeSet::from([TargetCapability::Execution(
        ExecutionCapabilityRequirementV1::Resource(
            ResourceCapabilityRequirementV1::WorkgroupInvocationsAtMost(128),
        ),
    )]);
    assert!(
        verify_module_with_capabilities(&required, &insufficient)
            .unwrap_err()
            .contains(DiagnosticCode::UnsupportedCapability)
    );

    let wrong_axis = BTreeSet::from([TargetCapability::Execution(
        ExecutionCapabilityRequirementV1::AddressSpace {
            address_space: AddressSpace::Global,
            access: AccessMode::ReadWrite,
        },
    )]);
    let read_only = only_capability(ExecutionCapabilityRequirementV1::AddressSpace {
        address_space: AddressSpace::Global,
        access: AccessMode::ReadOnly,
    });
    assert!(
        verify_module_with_capabilities(&read_only, &wrong_axis)
            .unwrap_err()
            .contains(DiagnosticCode::UnsupportedCapability)
    );
}
