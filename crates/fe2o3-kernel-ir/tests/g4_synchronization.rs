use std::collections::BTreeSet;

use fe2o3_kernel_ir::*;

const V2_GOLDEN_HEX: &str = include_str!("fixtures/g4_sync_v2.hex");

fn op(result: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(result), ty), kind)
}

fn pointer(scalar: ScalarType, address_space: AddressSpace) -> Type {
    Type::pointer(Type::Scalar(scalar), address_space, AccessMode::ReadWrite)
}

fn module(parameters: Vec<Type>, operations: Vec<Operation>) -> Module {
    let parameter_values = (0..parameters.len())
        .map(|value| ValueId(value as u32))
        .collect();
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = operations;
    block.terminator = Some(Terminator::Return { values: vec![] });
    let function = Function::definition(
        "kernel_impl",
        Signature::new(parameters, vec![]),
        parameter_values,
        vec![block],
    );
    let mut module = Module::new("g4::synchronization");
    module.functions.push(function);
    module
}

fn atomic(
    kind: AtomicKind,
    address_space: AddressSpace,
    scope: SynchronizationScope,
    ordering: MemoryOrdering,
) -> Atomic {
    Atomic {
        kind,
        pointer: ValueId(0),
        value: (kind != AtomicKind::Load).then_some(ValueId(1)),
        compare: (kind == AtomicKind::CompareExchange).then_some(ValueId(1)),
        access: MemoryAccess::new(address_space, 4),
        scope,
        ordering,
        failure_ordering: (kind == AtomicKind::CompareExchange).then_some(MemoryOrdering::Acquire),
    }
}

fn valid_target_capabilities() -> BTreeSet<TargetCapability> {
    [
        TargetCapability::WorkgroupMemory,
        TargetCapability::DynamicWorkgroupMemory,
        TargetCapability::WorkgroupBarrier,
        TargetCapability::Atomic {
            width_bits: 32,
            address_space: AddressSpace::Global,
            max_scope: SynchronizationScope::System,
        },
        TargetCapability::WaveWidth(WaveWidth::Wave64),
    ]
    .into_iter()
    .collect()
}

#[test]
fn verifies_scoped_synchronization_lds_and_wave_requirements() {
    let global_u32 = pointer(ScalarType::U32, AddressSpace::Global);
    let workgroup_u32 = pointer(ScalarType::U32, AddressSpace::Workgroup);
    let operations = vec![
        op(
            2,
            workgroup_u32.clone(),
            OperationKind::WorkgroupMemory(WorkgroupMemory {
                element: Type::Scalar(ScalarType::U32),
                extent: WorkgroupMemoryExtent::Static(64),
                alignment: 16,
            }),
        ),
        op(
            3,
            workgroup_u32,
            OperationKind::WorkgroupMemory(WorkgroupMemory {
                element: Type::Scalar(ScalarType::U32),
                extent: WorkgroupMemoryExtent::Dynamic,
                alignment: 16,
            }),
        ),
        Operation::new(
            vec![],
            OperationKind::Fence(Fence {
                memory_scope: SynchronizationScope::Device,
                semantics: BarrierSemantics::new(MemoryOrdering::Release, [AddressSpace::Global]),
            }),
        ),
        Operation::new(
            vec![],
            OperationKind::WorkgroupBarrier(WorkgroupBarrier {
                memory_scope: SynchronizationScope::Workgroup,
                semantics: BarrierSemantics::new(
                    MemoryOrdering::AcquireRelease,
                    [AddressSpace::Workgroup, AddressSpace::Global],
                ),
                convergence: Convergence::uniform(SynchronizationScope::Workgroup),
            }),
        ),
        op(
            4,
            Type::Scalar(ScalarType::U32),
            OperationKind::Atomic(atomic(
                AtomicKind::Add,
                AddressSpace::Global,
                SynchronizationScope::Device,
                MemoryOrdering::AcquireRelease,
            )),
        ),
    ];
    let mut module = module(vec![global_u32, Type::Scalar(ScalarType::U32)], operations);
    module
        .required_capabilities
        .insert(TargetCapability::WaveWidth(WaveWidth::Wave64));

    verify_module_with_capabilities(&module, &valid_target_capabilities())
        .expect("the target's system-scope atomic support subsumes device scope");

    let derived = module.derived_capabilities();
    assert!(derived.contains(&TargetCapability::WorkgroupMemory));
    assert!(derived.contains(&TargetCapability::DynamicWorkgroupMemory));
    assert!(derived.contains(&TargetCapability::WorkgroupBarrier));
    assert!(derived.contains(&TargetCapability::Atomic {
        width_bits: 32,
        address_space: AddressSpace::Global,
        max_scope: SynchronizationScope::Device,
    }));

    let operations = &module.functions[0].body.as_ref().unwrap().blocks[0].operations;
    assert!(
        operations[0]
            .effect_summary()
            .effects()
            .contains(&MemoryEffect::Allocate(AddressSpace::Workgroup))
    );
    assert!(matches!(
        operations[2].memory_effects().as_slice(),
        [MemoryEffect::Fence {
            memory_scope: SynchronizationScope::Device,
            ordering: MemoryOrdering::Release,
            ..
        }]
    ));
    assert!(matches!(
        operations[3].memory_effects().as_slice(),
        [MemoryEffect::Synchronize {
            execution_scope: SynchronizationScope::Workgroup,
            ..
        }]
    ));
}

#[test]
fn rejects_illegal_atomic_type_order_scope_and_target_combinations() {
    let mut invalid_compare_exchange = atomic(
        AtomicKind::CompareExchange,
        AddressSpace::Global,
        SynchronizationScope::Device,
        MemoryOrdering::AcquireRelease,
    );
    invalid_compare_exchange.failure_ordering = Some(MemoryOrdering::Release);
    let cases = [
        (
            ScalarType::U32,
            AddressSpace::Workgroup,
            atomic(
                AtomicKind::Add,
                AddressSpace::Workgroup,
                SynchronizationScope::Device,
                MemoryOrdering::Relaxed,
            ),
        ),
        (
            ScalarType::U32,
            AddressSpace::Global,
            atomic(
                AtomicKind::Load,
                AddressSpace::Global,
                SynchronizationScope::Device,
                MemoryOrdering::Release,
            ),
        ),
        (
            ScalarType::F32,
            AddressSpace::Global,
            atomic(
                AtomicKind::Min,
                AddressSpace::Global,
                SynchronizationScope::Device,
                MemoryOrdering::Relaxed,
            ),
        ),
        (
            ScalarType::U32,
            AddressSpace::Global,
            invalid_compare_exchange,
        ),
    ];

    for (scalar, address_space, atomic) in cases {
        let results = match atomic.kind {
            AtomicKind::Store => vec![],
            AtomicKind::CompareExchange => vec![
                ValueDef::new(ValueId(2), Type::Scalar(scalar)),
                ValueDef::new(ValueId(3), Type::BOOL),
            ],
            _ => vec![ValueDef::new(ValueId(2), Type::Scalar(scalar))],
        };
        let module = module(
            vec![pointer(scalar, address_space), Type::Scalar(scalar)],
            vec![Operation::new(results, OperationKind::Atomic(atomic))],
        );
        let errors = verify_module(&module).unwrap_err();
        assert!(errors.contains(DiagnosticCode::InvalidAtomic));
    }

    let operation = op(
        2,
        Type::Scalar(ScalarType::U32),
        OperationKind::Atomic(atomic(
            AtomicKind::Add,
            AddressSpace::Global,
            SynchronizationScope::Device,
            MemoryOrdering::Relaxed,
        )),
    );
    let module = module(
        vec![
            pointer(ScalarType::U32, AddressSpace::Global),
            Type::Scalar(ScalarType::U32),
        ],
        vec![operation],
    );
    let supported = [TargetCapability::Atomic {
        width_bits: 32,
        address_space: AddressSpace::Global,
        max_scope: SynchronizationScope::Workgroup,
    }]
    .into_iter()
    .collect();
    let errors = verify_module_with_capabilities(&module, &supported).unwrap_err();
    assert!(errors.contains(DiagnosticCode::UnsupportedCapability));
}

#[test]
fn rejects_invalid_fences_barriers_and_convergence_claims() {
    let invalid = vec![
        Operation::new(
            vec![],
            OperationKind::Fence(Fence {
                memory_scope: SynchronizationScope::Device,
                semantics: BarrierSemantics::new(MemoryOrdering::Relaxed, [AddressSpace::Global]),
            }),
        ),
        Operation::new(
            vec![],
            OperationKind::Fence(Fence {
                memory_scope: SynchronizationScope::Device,
                semantics: BarrierSemantics::new(
                    MemoryOrdering::Acquire,
                    [AddressSpace::Workgroup],
                ),
            }),
        ),
        Operation::new(
            vec![],
            OperationKind::WorkgroupBarrier(WorkgroupBarrier {
                memory_scope: SynchronizationScope::Subgroup,
                semantics: BarrierSemantics::new(
                    MemoryOrdering::AcquireRelease,
                    [AddressSpace::Global],
                ),
                convergence: Convergence::uniform(SynchronizationScope::Subgroup),
            }),
        ),
    ];
    let errors = verify_module(&module(vec![], invalid)).unwrap_err();
    assert!(errors.contains(DiagnosticCode::InvalidFence));
    assert!(errors.contains(DiagnosticCode::InvalidBarrier));
    assert!(errors.contains(DiagnosticCode::InvalidConvergence));
}

#[test]
fn rejects_malformed_and_duplicate_workgroup_memory_declarations() {
    let pointer = pointer(ScalarType::U32, AddressSpace::Workgroup);
    let operations = vec![
        op(
            0,
            pointer.clone(),
            OperationKind::WorkgroupMemory(WorkgroupMemory {
                element: Type::Scalar(ScalarType::U32),
                extent: WorkgroupMemoryExtent::Static(0),
                alignment: 3,
            }),
        ),
        op(
            1,
            pointer.clone(),
            OperationKind::WorkgroupMemory(WorkgroupMemory {
                element: Type::Scalar(ScalarType::U32),
                extent: WorkgroupMemoryExtent::Dynamic,
                alignment: 4,
            }),
        ),
        op(
            2,
            pointer,
            OperationKind::WorkgroupMemory(WorkgroupMemory {
                element: Type::Scalar(ScalarType::U32),
                extent: WorkgroupMemoryExtent::Dynamic,
                alignment: 4,
            }),
        ),
    ];
    let errors = verify_module(&module(vec![], operations)).unwrap_err();
    assert!(errors.contains(DiagnosticCode::InvalidAlignment));
    assert!(errors.contains(DiagnosticCode::InvalidWorkgroupMemory));
}

#[test]
fn rejects_conflicting_exact_wave_requirements() {
    let mut module = Module::new("g4::wave-conflict");
    module.required_capabilities.extend([
        TargetCapability::Subgroups,
        TargetCapability::SubgroupSize(64),
        TargetCapability::WaveWidth(WaveWidth::Wave32),
        TargetCapability::WaveWidth(WaveWidth::Wave64),
    ]);
    let errors = verify_module(&module).unwrap_err();
    assert!(errors.contains(DiagnosticCode::InvalidCapability));
}

fn g4_wire_module() -> Module {
    let workgroup_pointer = pointer(ScalarType::U32, AddressSpace::Workgroup);
    let mut module = module(
        vec![],
        vec![
            op(
                0,
                workgroup_pointer,
                OperationKind::WorkgroupMemory(WorkgroupMemory {
                    element: Type::Scalar(ScalarType::U32),
                    extent: WorkgroupMemoryExtent::Static(32),
                    alignment: 16,
                }),
            ),
            Operation::new(
                vec![],
                OperationKind::Fence(Fence {
                    memory_scope: SynchronizationScope::Device,
                    semantics: BarrierSemantics::new(
                        MemoryOrdering::Release,
                        [AddressSpace::Global],
                    ),
                }),
            ),
            Operation::new(
                vec![],
                OperationKind::WorkgroupBarrier(WorkgroupBarrier {
                    memory_scope: SynchronizationScope::Workgroup,
                    semantics: BarrierSemantics::new(
                        MemoryOrdering::AcquireRelease,
                        [AddressSpace::Workgroup],
                    ),
                    convergence: Convergence::uniform(SynchronizationScope::Workgroup),
                }),
            ),
        ],
    );
    module
        .required_capabilities
        .insert(TargetCapability::WaveWidth(WaveWidth::Wave64));
    module
}

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

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
fn v2_has_stable_golden_bytes_and_reads_v1() {
    let module = g4_wire_module();
    let encoded = encode_module_v2(&module).unwrap();
    assert_eq!(to_hex(&encoded), to_hex(&from_hex(V2_GOLDEN_HEX)));
    assert_eq!(decode_module_v2(&encoded).unwrap(), module);

    let v1 = encode_module_v1(&Module::new("legacy")).unwrap();
    assert_eq!(decode_module_v2(&v1).unwrap(), Module::new("legacy"));
}

#[test]
fn v1_fails_closed_on_v2_only_nodes_and_capabilities() {
    let module = g4_wire_module();
    assert!(matches!(
        encode_module_v1(&module),
        Err(KernelIrEncodeError::UnsupportedInVersion { version: 1, .. })
    ));
    let encoded = encode_module_v2(&module).unwrap();
    assert_eq!(
        decode_module_v1(&encoded),
        Err(KernelIrDecodeError::UnknownVersion(2))
    );

    let mut forged_v1 = encoded;
    forged_v1[8..10].copy_from_slice(&KERNEL_IR_VERSION_V1.to_le_bytes());
    assert_eq!(
        decode_module_v2(&forged_v1),
        Err(KernelIrDecodeError::UnknownTag {
            kind: "target capability",
            tag: 12,
        })
    );
}

#[test]
fn v2_decoder_rejects_malformed_wave_extent_and_convergence_tags() {
    let mut wave_only = Module::new("m");
    wave_only
        .required_capabilities
        .insert(TargetCapability::WaveWidth(WaveWidth::Wave64));
    let mut encoded = encode_module_v2(&wave_only).unwrap();
    encoded[38] = 0xff;
    assert_eq!(
        decode_module_v2(&encoded),
        Err(KernelIrDecodeError::UnknownTag {
            kind: "wave width",
            tag: 0xff,
        })
    );

    let mut extent_module = module(
        vec![],
        vec![op(
            0,
            pointer(ScalarType::U32, AddressSpace::Workgroup),
            OperationKind::WorkgroupMemory(WorkgroupMemory {
                element: Type::Scalar(ScalarType::U32),
                extent: WorkgroupMemoryExtent::Dynamic,
                alignment: 4,
            }),
        )],
    );
    extent_module.id = ModuleId::new("m");
    extent_module.functions[0].id = FunctionId::new("f");
    let mut encoded = encode_module_v2(&extent_module).unwrap();
    encoded[87] = 0xff;
    assert_eq!(
        decode_module_v2(&encoded),
        Err(KernelIrDecodeError::UnknownTag {
            kind: "workgroup memory extent",
            tag: 0xff,
        })
    );

    let mut convergence_module = module(
        vec![],
        vec![Operation::new(
            vec![],
            OperationKind::WorkgroupBarrier(WorkgroupBarrier {
                memory_scope: SynchronizationScope::Workgroup,
                semantics: BarrierSemantics::new(
                    MemoryOrdering::AcquireRelease,
                    [AddressSpace::Workgroup],
                ),
                convergence: Convergence::uniform(SynchronizationScope::Workgroup),
            }),
        )],
    );
    convergence_module.id = ModuleId::new("m");
    convergence_module.functions[0].id = FunctionId::new("f");
    let mut encoded = encode_module_v2(&convergence_module).unwrap();
    encoded[83] = 0xff;
    assert_eq!(
        decode_module_v2(&encoded),
        Err(KernelIrDecodeError::UnknownTag {
            kind: "convergence",
            tag: 0xff,
        })
    );
}
