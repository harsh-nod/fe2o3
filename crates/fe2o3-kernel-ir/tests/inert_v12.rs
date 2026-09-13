use std::collections::BTreeSet;

use fe2o3_kernel_ir::*;

type Encoder = fn(&Module) -> Result<Vec<u8>, KernelIrEncodeError>;
const OLD_ENCODERS: [Encoder; 11] = [
    encode_module_v1,
    encode_module_v2,
    encode_module_v3,
    encode_module_v4,
    encode_module_v5,
    encode_module_v6,
    encode_module_v7,
    encode_module_v8,
    encode_module_v9,
    encode_module_v10,
    encode_module_v11,
];
const REJECTION: &str =
    "Kernel IR V12 vector and verification-contract carriers are not admitted by this verifier";
const VECTOR_GOLDEN: &str = "4645324f334b49000c000000b9000000000000000a000000766563746f722d76313201000000000000000000000009000000726f756e647472697001000000030302020d00000000010100000000000000010000000000000000000000030000000100000001000000050d0400011b01000000000d0400010310000000000100000002000000050d04000202001d01000000020200000000001c0100000000020000000d040002020003100000000001040000000000000000";

fn vector() -> FixedVectorTypeV12 {
    FixedVectorTypeV12::new(ScalarType::F32, 4, VectorLayoutV12::Contiguous)
}

fn marker(kind: WorkgroupPipelineEventKindV12) -> OperationKind {
    OperationKind::VerificationContract(VerificationContractOperationV12::WorkgroupPipelineEvent {
        contract: VerificationContractKeyV12::new(17),
        kind,
        storage: ValueId(3),
        epoch: ValueId(8),
    })
}

fn events() -> [WorkgroupPipelineEventKindV12; 6] {
    use WorkgroupPipelineEventKindV12::*;
    [Stage, Commit, Wait, Consume, Discard, Release]
}

fn carriers() -> Vec<(OperationKind, &'static str, u8)> {
    let access = VectorMemoryAccessV12::new(vector(), MemoryAccess::new(AddressSpace::Global, 16));
    let mut operations = vec![
        (
            OperationKind::VectorLoad(VectorLoadOperationV12::new(ValueId(3), access)),
            "fixed-vector load",
            27,
        ),
        (
            OperationKind::VectorStore(VectorStoreOperationV12::new(
                ValueId(3),
                ValueId(8),
                access,
            )),
            "fixed-vector store",
            28,
        ),
        (
            OperationKind::VectorLayoutConvert(VectorLayoutConversionV12::new(
                ValueId(8),
                VectorLayoutV12::Interleaved { factor: 2 },
            )),
            "fixed-vector layout conversion",
            29,
        ),
    ];
    operations.extend(events().map(|kind| (marker(kind), "verification contract", 30)));
    operations
}

fn module(operations: Vec<Operation>) -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = operations;
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("inert_v12");
    module.functions.push(Function::internal_helper(
        "helper",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    module
}

fn body(module: &mut Module) -> &mut FunctionBody {
    module.functions[0].body.as_mut().unwrap()
}

fn assert_rejected(module: &Module, block: Option<BlockId>, operation: Option<usize>) {
    assert_rejected_at(module, 0, block, operation);
}

fn assert_rejected_at(
    module: &Module,
    function: usize,
    block: Option<BlockId>,
    operation: Option<usize>,
) {
    let errors = [
        verify_module(module),
        verify_module_ref(module).map(|_| ()),
        verify_module_with_capabilities(module, &BTreeSet::new()),
    ]
    .map(Result::unwrap_err);
    assert_eq!(errors[0], errors[1]);
    assert_eq!(errors[1], errors[2]);
    assert_eq!(
        errors[0].diagnostics(),
        &[Diagnostic {
            location: DiagnosticLocation {
                module: module.id.clone(),
                function: Some(module.functions[function].id.clone()),
                kernel: None,
                block,
                operation,
            },
            code: DiagnosticCode::InvalidSemanticOperation,
            message: REJECTION.to_owned(),
        }]
    );
}

fn assert_roundtrip(module: &Module) -> Vec<u8> {
    let bytes = encode_module_v12(module).unwrap();
    assert_eq!(decode_module_v12(&bytes).unwrap(), *module);
    assert_eq!(encode_module_v12(module).unwrap(), bytes);
    bytes
}

#[test]
fn old_scalar_modules_remain_admitted_and_decode_with_v12() {
    let module = module(vec![Operation::effect_free(
        ValueDef::new(ValueId(0), Type::Scalar(ScalarType::U32)),
        OperationKind::Constant(Constant::U32(7)),
    )]);
    verify_module(&module).unwrap();
    verify_module_ref(&module).unwrap();
    verify_module_with_capabilities(&module, &BTreeSet::new()).unwrap();
    for encode in OLD_ENCODERS {
        assert_eq!(
            decode_module_v12(&encode(&module).unwrap()).unwrap(),
            module
        );
    }
    let bytes = assert_roundtrip(&module);
    assert_eq!(
        decode_module_v10(&bytes),
        Err(KernelIrDecodeError::UnknownVersion(12))
    );
    assert_eq!(
        decode_module_v11(&bytes),
        Err(KernelIrDecodeError::UnknownVersion(12))
    );
}

#[test]
fn operations_have_independent_version_guards_without_vector_results() {
    for (kind, feature, tag) in carriers() {
        let module = module(vec![Operation::new(vec![], kind)]);
        let mut bytes = assert_roundtrip(&module);
        assert_rejected(&module, Some(BlockId(0)), Some(0));
        for (index, encode) in OLD_ENCODERS.into_iter().enumerate() {
            assert_eq!(
                encode(&module),
                Err(KernelIrEncodeError::UnsupportedInVersion {
                    version: index as u16 + 1,
                    feature,
                })
            );
        }
        bytes[8..10].copy_from_slice(&11_u16.to_le_bytes());
        for decode in [decode_module_v11, decode_module_v12] {
            assert_eq!(
                decode(&bytes),
                Err(KernelIrDecodeError::UnknownTag {
                    kind: "operation",
                    tag,
                })
            );
        }
    }
}

#[test]
fn declarations_definitions_and_nested_signature_types_are_rejected() {
    let direct = Type::vector(vector());
    let pointer = Type::pointer(direct.clone(), AddressSpace::Global, AccessMode::ReadOnly);
    let slice = Type::slice(direct.clone(), AddressSpace::Global, AccessMode::ReadOnly);
    let nested = Type::pointer(
        Type::slice(pointer.clone(), AddressSpace::Global, AccessMode::ReadOnly),
        AddressSpace::Private,
        AccessMode::ReadWrite,
    );
    for ty in [direct, pointer, slice, nested] {
        for declaration in [false, true] {
            for result in [false, true] {
                let mut module = module(vec![]);
                if declaration {
                    module.functions[0].body = None;
                    module.functions[0].role = FunctionRole::ExternalImport;
                }
                if result {
                    module.functions[0].signature.results.push(ty.clone());
                } else {
                    module.functions[0].signature.parameters.push(ty.clone());
                }
                assert_rejected(&module, None, None);
                let mut bytes = assert_roundtrip(&module);
                for (index, encode) in OLD_ENCODERS.into_iter().enumerate() {
                    assert_eq!(
                        encode(&module),
                        Err(KernelIrEncodeError::UnsupportedInVersion {
                            version: index as u16 + 1,
                            feature: "fixed-lane vector type",
                        })
                    );
                }
                bytes[8..10].copy_from_slice(&11_u16.to_le_bytes());
                assert_eq!(
                    decode_module_v12(&bytes),
                    Err(KernelIrDecodeError::UnknownTag {
                        kind: "type",
                        tag: 5,
                    })
                );
            }
        }
    }
}

#[test]
fn dead_blocks_and_unused_results_do_not_hide_carriers() {
    for (kind, _, _) in carriers() {
        let mut module = module(vec![]);
        let mut dead = BasicBlock::new(BlockId(9));
        dead.operations.push(Operation::new(vec![], kind));
        dead.terminator = Some(Terminator::Return { values: vec![] });
        body(&mut module).blocks.push(dead);
        assert_rejected(&module, Some(BlockId(9)), Some(0));
        assert_roundtrip(&module);
    }
    let mut module = module(vec![]);
    let mut dead = BasicBlock::new(BlockId(9));
    dead.parameters
        .push(ValueDef::new(ValueId(4), Type::vector(vector())));
    dead.terminator = Some(Terminator::Return { values: vec![] });
    body(&mut module).blocks.push(dead);
    assert_rejected(&module, Some(BlockId(9)), None);
    body(&mut module).blocks[1].parameters.clear();
    body(&mut module).blocks[1].operations.push(Operation::new(
        vec![ValueDef::new(ValueId(4), Type::vector(vector()))],
        OperationKind::Constant(Constant::U32(0)),
    ));
    assert_rejected(&module, Some(BlockId(9)), Some(0));
}

#[test]
fn uncalled_helper_after_kernel_reports_its_exact_nonzero_operation() {
    for (kind, _, _) in carriers() {
        let mut module = module(vec![]);
        module.functions[0].role = FunctionRole::KernelEntry;
        module.kernels.push(Kernel::new(
            "entry",
            module.functions[0].id.clone(),
            LaunchDomain::D1 {
                x: LaunchExtent::Static(1),
            },
        ));
        let mut helper = module.functions[0].clone();
        helper.role = FunctionRole::InternalHelper;
        helper.id = FunctionId::new("uncalled");
        helper.body.as_mut().unwrap().blocks[0].operations = vec![
            Operation::effect_free(
                ValueDef::new(ValueId(1), Type::INDEX),
                OperationKind::Constant(Constant::Index(0)),
            ),
            Operation::new(vec![], kind),
        ];
        module.functions.push(helper);
        assert_rejected_at(&module, 1, Some(BlockId(0)), Some(1));
        assert_roundtrip(&module);
    }
}

#[test]
fn embedded_types_are_checked_even_without_vector_results() {
    let ty = Type::vector(vector());
    for kind in [
        OperationKind::Intrinsic(IntrinsicOperation::new(
            IntrinsicKind::LaunchExtent { axis: Axis::X },
            ty.clone(),
        )),
        OperationKind::Cast {
            kind: CastKind::Bitcast,
            value: ValueId(0),
            to: ty.clone(),
        },
        OperationKind::Alloca {
            element: ty.clone(),
            count: None,
            address_space: AddressSpace::Private,
            alignment: 16,
        },
        OperationKind::WorkgroupMemory(WorkgroupMemory {
            element: ty,
            extent: WorkgroupMemoryExtent::Static(4),
            alignment: 16,
        }),
    ] {
        let module = module(vec![Operation::new(vec![], kind)]);
        assert_rejected(&module, Some(BlockId(0)), Some(0));
        assert_roundtrip(&module);
    }
}

#[test]
fn descriptor_legality_is_separate_from_raw_wire_well_formedness() {
    for lanes in [0, 1, 2, 1024, 1025] {
        let ty = FixedVectorTypeV12::new(ScalarType::F32, lanes, VectorLayoutV12::Contiguous);
        assert_eq!(ty.validate().is_ok(), (2..=1024).contains(&lanes));
        assert_eq!(
            ty.byte_width(),
            (2..=1024).contains(&lanes).then_some(u32::from(lanes) * 4)
        );
        let mut module = module(vec![]);
        module.functions[0].signature.results.push(Type::vector(ty));
        if lanes <= 1024 {
            assert_roundtrip(&module);
            assert_rejected(&module, None, None);
        } else {
            assert_eq!(
                encode_module_v12(&module),
                Err(KernelIrEncodeError::LimitExceeded {
                    field: "fixed vector lanes",
                    actual: 1025,
                    max: 1024,
                })
            );
        }
    }
    for scalar in [ScalarType::Bool, ScalarType::Index] {
        assert!(
            FixedVectorTypeV12::new(scalar, 4, VectorLayoutV12::Contiguous)
                .validate()
                .is_err()
        );
    }
    for factor in [0, 1, 2, 3, 4, 8] {
        let ty = vector().with_layout(VectorLayoutV12::Interleaved { factor });
        assert_eq!(ty.validate().is_ok(), factor == 2);
        let mut module = module(vec![]);
        module.functions[0].signature.results.push(Type::vector(ty));
        assert_roundtrip(&module);
        assert_rejected(&module, None, None);
    }
}

#[test]
fn marker_effects_and_operand_visits_are_exact() {
    for (kind, _, _) in carriers() {
        let expected = match &kind {
            OperationKind::VectorLoad(_) => vec![ValueId(3)],
            OperationKind::VectorLayoutConvert(_) => vec![ValueId(8)],
            _ => vec![ValueId(3), ValueId(8)],
        };
        assert_eq!(kind.operands(), expected);
        assert_eq!(kind.operand_count(), expected.len());
        let mut visited = vec![];
        kind.visit_operands(|id| visited.push(id));
        assert_eq!(visited, expected);
        let mut count = 0;
        assert_eq!(
            kind.try_visit_operands(|_| {
                count += 1;
                Err("stop")
            }),
            Err("stop")
        );
        assert_eq!(count, 1);
        let operation = Operation::new(vec![], kind);
        if matches!(operation.kind, OperationKind::VerificationContract(_)) {
            assert!(operation.effect_summary().is_pure());
            assert!(!operation.combined_effect_summary_v12().is_pure());
            assert_eq!(
                operation.compiler_ordering_effects_v12().effect(),
                Some(CompilerOrderingEffectV12::OrderedVerificationContract)
            );
        } else {
            assert!(operation.compiler_ordering_effects_v12().is_empty());
            match operation.kind {
                OperationKind::VectorLoad(_) => assert_eq!(
                    operation.memory_effects(),
                    [MemoryEffect::Read(AddressSpace::Global)]
                ),
                OperationKind::VectorStore(_) => assert_eq!(
                    operation.memory_effects(),
                    [MemoryEffect::Write(AddressSpace::Global)]
                ),
                OperationKind::VectorLayoutConvert(_) => {
                    assert!(operation.combined_effect_summary_v12().is_pure())
                }
                _ => unreachable!(),
            }
        }
    }
    let repeated = OperationKind::VectorStore(VectorStoreOperationV12::new(
        ValueId(3),
        ValueId(3),
        VectorMemoryAccessV12::new(vector(), MemoryAccess::new(AddressSpace::Global, 16)),
    ));
    assert_eq!(repeated.operands(), [ValueId(3), ValueId(3)]);
}

#[test]
fn vector_golden_bytes_preserve_types_layouts_and_memory_metadata() {
    let bytes: Vec<_> = VECTOR_GOLDEN
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect();
    assert_eq!(bytes.len(), 185);
    let module = decode_module_v12(&bytes).unwrap();
    assert_eq!(module.id.as_str(), "vector-v12");
    assert_eq!(encode_module_v12(&module).unwrap(), bytes);
    let ops = &module.functions[0].body.as_ref().unwrap().blocks[0].operations;
    let contiguous = vector();
    let interleaved = contiguous.with_layout(VectorLayoutV12::Interleaved { factor: 2 });
    assert_eq!(
        ops,
        &[
            Operation::new(
                vec![ValueDef::new(ValueId(1), Type::vector(contiguous))],
                OperationKind::VectorLoad(VectorLoadOperationV12::new(
                    ValueId(0),
                    VectorMemoryAccessV12::new(
                        contiguous,
                        MemoryAccess::new(AddressSpace::Global, 16)
                    )
                ))
            ),
            Operation::new(
                vec![ValueDef::new(ValueId(2), Type::vector(interleaved))],
                OperationKind::VectorLayoutConvert(VectorLayoutConversionV12::new(
                    ValueId(1),
                    interleaved.layout
                ))
            ),
            Operation::new(
                vec![],
                OperationKind::VectorStore(VectorStoreOperationV12::new(
                    ValueId(0),
                    ValueId(2),
                    VectorMemoryAccessV12::new(
                        interleaved,
                        MemoryAccess::new(AddressSpace::Global, 16)
                    )
                ))
            ),
        ]
    );
    assert_rejected(&module, Some(BlockId(0)), Some(0));
}

fn unique_offset(bytes: &[u8], marker: &[u8]) -> usize {
    let offsets: Vec<_> = bytes
        .windows(marker.len())
        .enumerate()
        .filter_map(|(offset, window)| (window == marker).then_some(offset))
        .collect();
    assert_eq!(offsets.len(), 1);
    offsets[0]
}

#[test]
fn marker_payload_and_unknown_tags_are_closed() {
    for (index, kind) in events().into_iter().enumerate() {
        let module = module(vec![Operation::new(vec![], marker(kind))]);
        let bytes = assert_roundtrip(&module);
        let mut payload = vec![30, 1];
        payload.extend_from_slice(&17_u32.to_le_bytes());
        payload.push(index as u8 + 1);
        payload.extend_from_slice(&3_u32.to_le_bytes());
        payload.extend_from_slice(&8_u32.to_le_bytes());
        assert_eq!(payload.len(), 15);
        let offset = unique_offset(&bytes, &payload);
        for (relative, name) in [(1, "verification contract"), (6, "pipeline event")] {
            let mut bad = bytes.clone();
            bad[offset + relative] = 255;
            assert_eq!(
                decode_module_v12(&bad),
                Err(KernelIrDecodeError::UnknownTag {
                    kind: name,
                    tag: 255,
                })
            );
        }
        for end in 0..bytes.len() {
            assert!(decode_module_v12(&bytes[..end]).is_err());
        }
        for end in offset..offset + payload.len() {
            let mut truncated = bytes[..end].to_vec();
            truncated[12..16].copy_from_slice(&(end as u32).to_le_bytes());
            assert_eq!(
                decode_module_v12(&truncated),
                Err(KernelIrDecodeError::Truncated)
            );
        }
    }
}

#[test]
fn vector_wire_limits_and_tags_are_checked_before_semantic_admission() {
    let access = VectorMemoryAccessV12::new(
        vector(),
        MemoryAccess {
            address_space: AddressSpace::Workgroup,
            alignment: 32,
            volatile: true,
        },
    );
    let module = module(vec![Operation::new(
        vec![],
        OperationKind::VectorLoad(VectorLoadOperationV12::new(ValueId(0x11223344), access)),
    )]);
    let bytes = assert_roundtrip(&module);
    let offset = unique_offset(&bytes, &[27, 1, 0x44, 0x33, 0x22, 0x11]);
    for (relative, name) in [
        (1, "vector access provenance"),
        (6, "scalar type"),
        (9, "vector layout"),
    ] {
        let mut bad = bytes.clone();
        bad[offset + relative] = 255;
        assert_eq!(
            decode_module_v12(&bad),
            Err(KernelIrDecodeError::UnknownTag {
                kind: name,
                tag: 255,
            })
        );
    }
    let mut bad = bytes.clone();
    bad[offset + 7..offset + 9].copy_from_slice(&1025_u16.to_le_bytes());
    assert_eq!(
        decode_module_v12(&bad),
        Err(KernelIrDecodeError::LimitExceeded {
            field: "fixed vector lanes",
            actual: 1025,
            max: 1024,
        })
    );
    let mut bad = bytes;
    bad[offset + 15] = 2;
    assert_eq!(
        decode_module_v12(&bad),
        Err(KernelIrDecodeError::UnknownTag {
            kind: "volatile memory access",
            tag: 2,
        })
    );
}

#[test]
fn v12_preserves_legacy_role_rejections() {
    let mut module = module(vec![]);
    module.functions[0].role = FunctionRole::DeviceFfiExport;
    assert_eq!(
        encode_module_v12(&module),
        Err(KernelIrEncodeError::UnsupportedInVersion {
            version: 12,
            feature: "device-FFI export function roles",
        })
    );
    module.functions[0].role = FunctionRole::KernelEntry;
    assert_eq!(
        encode_module_v12(&module),
        Err(KernelIrEncodeError::NonCanonical {
            field: "function role does not match the V1/V2 body and kernel records",
        })
    );
}

#[test]
fn legacy_canonical_owners_cannot_admit_v12_carriers() {
    for (kind, feature, _) in carriers() {
        let module = module(vec![Operation::new(vec![], kind)]);
        assert_eq!(
            VerifiedCanonicalKernelIrV5::from_module(module.clone()).unwrap_err(),
            VerifiedCanonicalKernelIrErrorV5::Verification(verify_module(&module).unwrap_err())
        );
        assert_eq!(
            VerifiedCanonicalKernelIrV6::from_module(module.clone()).unwrap_err(),
            VerifiedCanonicalKernelIrErrorV6::Encode(KernelIrEncodeError::UnsupportedInVersion {
                version: 6,
                feature
            })
        );
        assert_eq!(
            VerifiedCanonicalKernelIrV7::from_module(module.clone()).unwrap_err(),
            VerifiedCanonicalKernelIrErrorV7::Encode(KernelIrEncodeError::UnsupportedInVersion {
                version: 7,
                feature
            })
        );
        assert_eq!(
            VerifiedCanonicalKernelIrV8::from_module(module.clone()).unwrap_err(),
            VerifiedCanonicalKernelIrErrorV8::Encode(KernelIrEncodeError::UnsupportedInVersion {
                version: 8,
                feature
            })
        );
        assert_eq!(
            VerifiedCanonicalKernelIrV9::from_module(module.clone()).unwrap_err(),
            VerifiedCanonicalKernelIrErrorV9::Encode(KernelIrEncodeError::UnsupportedInVersion {
                version: 9,
                feature
            })
        );
        assert_eq!(
            VerifiedCanonicalKernelIrV10::from_module(module.clone()).unwrap_err(),
            VerifiedCanonicalKernelIrErrorV10::Encode(KernelIrEncodeError::UnsupportedInVersion {
                version: 10,
                feature
            })
        );
        assert_eq!(
            VerifiedCanonicalKernelIrV11::from_module(module.clone()).unwrap_err(),
            VerifiedCanonicalKernelIrErrorV11::Encode(KernelIrEncodeError::UnsupportedInVersion {
                version: 11,
                feature
            })
        );
        let bytes = encode_module_v12(&module).unwrap();
        assert_eq!(
            VerifiedCanonicalKernelIrV10::from_canonical_bytes(bytes.clone()).unwrap_err(),
            VerifiedCanonicalKernelIrErrorV10::NotExactV10 { version: 12 }
        );
        assert_eq!(
            VerifiedCanonicalKernelIrV11::from_canonical_bytes(bytes.clone()).unwrap_err(),
            VerifiedCanonicalKernelIrErrorV11::NotExactV11 { version: 12 }
        );
        assert_eq!(
            VerifiedCanonicalKernelIrV10::from_canonical_bytes_with_module(bytes.clone())
                .unwrap_err(),
            VerifiedCanonicalKernelIrErrorV10::NotExactV10 { version: 12 }
        );
        assert_eq!(
            VerifiedCanonicalKernelIrV11::from_canonical_bytes_with_module(bytes).unwrap_err(),
            VerifiedCanonicalKernelIrErrorV11::NotExactV11 { version: 12 }
        );
    }
}
