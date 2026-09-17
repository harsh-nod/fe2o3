use super::*;
use crate::{
    BasicBlock, ComparePredicate, Constant, ExplicitLaunchExtent1d, Function, IntrinsicOperation,
    Kernel, KernelId, LaunchDomain, LaunchExtent, MemoryAccess, Module, Operation, OperationKind,
    ScalarType, Signature, Terminator, Type, ValueDef,
    derive_kernel_memory_obligations_from_verified, verify_module_ref,
};

fn op(id: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(id), ty), kind)
}
fn pointer() -> Type {
    Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    )
}
fn store(pointer: u32) -> Operation {
    Operation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(pointer),
            value: ValueId(1),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    )
}

// The same verified select-zero recipe used by guarded analysis, with no fabricated report owner.
fn module(load: bool) -> Module {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = vec![
        op(
            2,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        op(
            3,
            Type::INDEX,
            OperationKind::SliceLength { slice: ValueId(0) },
        ),
        op(
            4,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(2),
                rhs: ValueId(3),
            },
        ),
        op(5, Type::INDEX, OperationKind::Constant(Constant::Index(0))),
        op(
            6,
            Type::INDEX,
            OperationKind::Select {
                condition: ValueId(4),
                true_value: ValueId(2),
                false_value: ValueId(5),
            },
        ),
        op(7, pointer(), OperationKind::SliceData { slice: ValueId(0) }),
        op(
            8,
            pointer(),
            OperationKind::GetElementPointer {
                base: ValueId(7),
                offset: ValueId(6),
            },
        ),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(4),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    let mut yes = BasicBlock::new(BlockId(1));
    if load {
        entry.operations.push(op(
            10,
            Type::Scalar(ScalarType::U32),
            OperationKind::GuardedLoad {
                pointer: ValueId(8),
                predicate: ValueId(4),
                fallback: ValueId(1),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ));
    } else {
        yes.operations.push(store(8));
    }
    yes.terminator = Some(Terminator::Return { values: vec![] });
    let mut no = BasicBlock::new(BlockId(2));
    no.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("guarded-receipt");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(
            vec![
                Type::slice(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    AccessMode::ReadWrite,
                ),
                Type::Scalar(ScalarType::U32),
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1)],
        vec![entry, yes, no],
    ));
    module.kernels.push(Kernel::new(
        "kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}

fn report(module: &Module, width: FormalIndexWidth) -> FormalMemoryObligations {
    let verified = verify_module_ref(module).unwrap();
    let analysis = derive_kernel_memory_obligations_from_verified(
        verified,
        &KernelId::new("kernel"),
        ExplicitLaunchExtent1d::Exact(64),
        width,
    )
    .unwrap();
    assert!(analysis.is_complete(), "{analysis:?}");
    analysis.obligations().clone()
}
fn raw(report: &FormalMemoryObligations) -> Vec<u8> {
    encode_v3(
        report,
        &mut CodecMeter::new(default_work_limit().unwrap()).unwrap(),
    )
    .unwrap()
}
fn domain(report: &FormalMemoryObligations) -> FormalSliceBoundedDomainV1 {
    let FormalAccessDomainV1::SliceBounded(domain) = report.accesses[0].domain else {
        panic!("guarded")
    };
    domain
}
fn preamble(bytes: &[u8]) -> Reader<'_> {
    let mut reader = Reader::new(bytes);
    reader.fixed::<HEADER_BYTES>().unwrap();
    reader.text("kernel ID").unwrap();
    reader.text("entry function ID").unwrap();
    reader.fixed::<4>().unwrap();
    decode_optional_invocations(&mut reader).unwrap();
    reader
}
fn wire_accesses(bytes: &[u8]) -> (Vec<(AccessRecord, FormalAccessDomainV1)>, usize) {
    let mut reader = preamble(bytes);
    let count = reader.count("allocations").unwrap();
    for _ in 0..count {
        decode_allocation(&mut reader, 3).unwrap();
    }
    let count = reader.count("accesses").unwrap();
    let mut accesses = Vec::new();
    for _ in 0..count {
        accesses.push((
            decode_access(&mut reader).unwrap(),
            decode_domain(&mut reader).unwrap(),
        ));
    }
    (accesses, reader.offset)
}

#[test]
fn verified_store_and_load_roundtrip_complete_domains_and_metadata() {
    for load in [false, true] {
        {
            let width = FormalIndexWidth::Bits64;
            let report = report(&module(load), width);
            let receipt =
                InertCanonicalFormalMemoryObligationReceiptV3::from_obligations(&report).unwrap();
            assert_eq!(
                (receipt.kernel_id(), receipt.entry_id()),
                ("kernel", "entry")
            );
            assert_eq!(
                receipt.metadata().encoding(),
                FormalMemoryReceiptEncodingV3::GuardedV3
            );
            assert_eq!(receipt.metadata().encoding().extraction_policy(), 2);
            assert_eq!(receipt.metadata().index_width(), width);
            assert_eq!(
                receipt.metadata().invocations(),
                Some(InvocationRange1d::new(0, 64).unwrap())
            );
            assert_eq!(
                receipt.metadata().analysis_basis(),
                FormalMemoryAnalysisBasis::CompilerDerivedIrWithUnauthenticatedLaunchInputs
            );
            assert!(!receipt.grants_authority());
            assert_eq!(
                wire_accesses(receipt.canonical_bytes()).0[0].1,
                report.accesses[0].domain
            );
            assert_eq!(
                report.accesses[0].kind,
                if load {
                    FormalMemoryAccessKind::Read
                } else {
                    FormalMemoryAccessKind::Write
                }
            );
            assert_eq!(
                domain(&report).path,
                if load {
                    FormalGuardedPathV1::ExplicitPredicate
                } else {
                    FormalGuardedPathV1::TrueEdge {
                        source: BlockId(0),
                        ordinal: 0,
                        target: BlockId(1),
                    }
                }
            );
            for (index, len, expected) in [
                (0, 0, false),
                (0, 1, true),
                (1, 1, false),
                (63, 64, true),
                (64, 65, false),
            ] {
                assert_eq!(
                    domain(&report).may_access_untrusted_index(index, len, 64),
                    expected
                );
            }
            receipt.revalidate().unwrap();
            assert_eq!(
                InertCanonicalFormalMemoryObligationReceiptV3::from_canonical_bytes(
                    receipt.canonical_bytes().to_vec()
                )
                .unwrap(),
                receipt
            );
            let facade =
                InertFormalMemoryReceiptFormatV3::from_current_obligations(&report).unwrap();
            assert_eq!(facade.canonical_bytes(), receipt.canonical_bytes());
            assert_eq!(facade.identity_digest(), receipt.identity_digest());
            assert_eq!(facade.metadata(), receipt.metadata());
            facade.revalidate().unwrap();
            assert_eq!(
                InertFormalMemoryReceiptFormatV3::decode_current(facade.canonical_bytes().to_vec())
                    .unwrap(),
                facade
            );
            assert_eq!(
                InertCanonicalFormalMemoryObligationReceiptV1::from_obligations(&report),
                Err(FormalMemoryReceiptErrorV1::UnsupportedGuardedRepresentation)
            );
            assert_eq!(
                InertCanonicalFormalMemoryObligationReceiptV1::from_canonical_bytes(
                    receipt.into_canonical_bytes()
                ),
                Err(FormalMemoryReceiptErrorV1::UnknownVersion(3))
            );
        }
    }
}

#[test]
fn verified_explicit_guarded_store_and_inert_width_metadata_stay_distinct() {
    let mut module = module(false);
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks[1].operations.clear();
    body.blocks[0].operations.push(Operation::new(
        vec![],
        OperationKind::GuardedStore {
            pointer: ValueId(8),
            predicate: ValueId(4),
            value: ValueId(1),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    ));
    let mut report = report(&module, FormalIndexWidth::Bits64);
    assert_eq!(report.accesses[0].kind, FormalMemoryAccessKind::Write);
    assert_eq!(domain(&report).path, FormalGuardedPathV1::ExplicitPredicate);
    let genuine = InertCanonicalFormalMemoryObligationReceiptV3::from_obligations(&report).unwrap();
    genuine.revalidate().unwrap();
    // Width is inert metadata: this mutation does not constitute a Bits32 extraction.
    report.index_width = FormalIndexWidth::Bits32;
    let inert = InertCanonicalFormalMemoryObligationReceiptV3::from_obligations(&report).unwrap();
    assert_eq!(inert.metadata().index_width(), FormalIndexWidth::Bits32);
    assert!(!inert.grants_authority());
    let verified = verify_module_ref(&module).unwrap();
    let unsupported = derive_kernel_memory_obligations_from_verified(
        verified,
        &KernelId::new("kernel"),
        ExplicitLaunchExtent1d::Exact(64),
        FormalIndexWidth::Bits32,
    )
    .unwrap();
    assert!(!unsupported.is_complete());
    assert!(unsupported.obligations().accesses().is_empty());
}

#[test]
fn genuine_mixed_accesses_preserve_fixed_whole_alias_and_conflict_records() {
    let mut module = module(false);
    module.functions[0].signature.parameters.push(pointer());
    let body = module.functions[0].body.as_mut().unwrap();
    body.parameters.push(ValueId(20));
    body.blocks[1].operations.extend([store(7), store(20)]);
    let report = report(&module, FormalIndexWidth::Bits64);
    assert_eq!(report.accesses.len(), 3);
    assert!(!report.inter_invocation_conflicts.is_empty());
    let [alias] = report.runtime_alias_requirements.as_slice() else {
        panic!("one alias")
    };
    assert_eq!(
        alias.left_region(),
        FormalAliasRegionV1::WholeFormalAllocation
    );
    assert_eq!(alias.right_accessed_bytes().unwrap().end_exclusive(), 4);
    let receipt = InertCanonicalFormalMemoryObligationReceiptV3::from_obligations(&report).unwrap();
    let (accesses, offset) = wire_accesses(receipt.canonical_bytes());
    assert_eq!(accesses.len(), 3);
    assert_eq!(
        accesses
            .iter()
            .filter(|row| matches!(row.1, FormalAccessDomainV1::SliceBounded(_)))
            .count(),
        1
    );
    let mut reader = Reader::new(receipt.canonical_bytes());
    reader.offset = offset;
    let count = reader.count("bounds requirements").unwrap();
    assert_eq!(count, report.bounds_requirements.len());
    for _ in 0..count {
        decode_location(&mut reader).unwrap();
        reader.u32().unwrap();
        match reader.u8().unwrap() {
            0 => {
                reader.u64().unwrap();
            }
            1 => {
                decode_domain(&mut reader).unwrap();
            }
            _ => panic!("kind"),
        }
    }
    assert_eq!(reader.count("runtime alias requirements").unwrap(), 1);
    assert_eq!((reader.u32().unwrap(), reader.u32().unwrap()), (0, 2));
    assert_eq!(decode_region(&mut reader).unwrap(), alias.left_region());
    assert_eq!(decode_region(&mut reader).unwrap(), alias.right_region());
    assert_eq!(
        reader.count("inter-invocation conflicts").unwrap(),
        report.inter_invocation_conflicts.len()
    );
    for expected in &report.inter_invocation_conflicts {
        assert_eq!(
            decode_conflict(&mut reader).unwrap(),
            (
                (expected.left.block.0, expected.left.operation_index as u64),
                (
                    expected.right.block.0,
                    expected.right.operation_index as u64
                ),
                expected.allocation.parameter_index
            )
        );
    }
    assert!(reader.is_finished());
    receipt.revalidate().unwrap();
}

#[test]
fn two_verified_guarded_slices_retain_both_whole_alias_regions() {
    let mut module = module(false);
    module.functions[0].signature.parameters.push(Type::slice(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    ));
    let body = module.functions[0].body.as_mut().unwrap();
    body.parameters.push(ValueId(20));
    body.blocks[0].operations.extend([
        op(
            21,
            Type::INDEX,
            OperationKind::SliceLength { slice: ValueId(20) },
        ),
        op(
            22,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(2),
                rhs: ValueId(21),
            },
        ),
        op(
            23,
            Type::INDEX,
            OperationKind::Select {
                condition: ValueId(22),
                true_value: ValueId(2),
                false_value: ValueId(5),
            },
        ),
        op(
            24,
            pointer(),
            OperationKind::SliceData { slice: ValueId(20) },
        ),
        op(
            25,
            pointer(),
            OperationKind::GetElementPointer {
                base: ValueId(24),
                offset: ValueId(23),
            },
        ),
    ]);
    body.blocks[1].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(22),
        then_target: BlockId(3),
        then_arguments: vec![],
        else_target: BlockId(4),
        else_arguments: vec![],
    });
    let mut yes = BasicBlock::new(BlockId(3));
    yes.operations.push(store(25));
    yes.terminator = Some(Terminator::Return { values: vec![] });
    let mut no = BasicBlock::new(BlockId(4));
    no.terminator = Some(Terminator::Return { values: vec![] });
    body.blocks.extend([yes, no]);
    let report = report(&module, FormalIndexWidth::Bits64);
    assert_eq!(report.accesses.len(), 2);
    assert!(
        report
            .accesses
            .iter()
            .all(|row| matches!(row.domain, FormalAccessDomainV1::SliceBounded(_)))
    );
    let [alias] = report.runtime_alias_requirements.as_slice() else {
        panic!("one alias")
    };
    assert_eq!(
        alias.left_region(),
        FormalAliasRegionV1::WholeFormalAllocation
    );
    assert_eq!(
        alias.right_region(),
        FormalAliasRegionV1::WholeFormalAllocation
    );
    let receipt = InertCanonicalFormalMemoryObligationReceiptV3::from_obligations(&report).unwrap();
    let (_, offset) = wire_accesses(receipt.canonical_bytes());
    let mut reader = Reader::new(receipt.canonical_bytes());
    reader.offset = offset;
    assert_eq!(reader.count("bounds requirements").unwrap(), 2);
    for _ in 0..2 {
        decode_location(&mut reader).unwrap();
        reader.u32().unwrap();
        assert_eq!(reader.u8().unwrap(), 1);
        decode_domain(&mut reader).unwrap();
    }
    assert_eq!(reader.count("runtime alias requirements").unwrap(), 1);
    assert_eq!((reader.u32().unwrap(), reader.u32().unwrap()), (0, 2));
    assert_eq!(decode_region(&mut reader).unwrap(), alias.left_region());
    assert_eq!(decode_region(&mut reader).unwrap(), alias.right_region());
    receipt.revalidate().unwrap();
}

#[test]
fn all_legacy_formats_keep_exact_old_bytes_identity_and_policy() {
    let mut module = module(false);
    module.functions[0].body.as_mut().unwrap().blocks[0].operations[6].kind =
        OperationKind::GetElementPointer {
            base: ValueId(7),
            offset: ValueId(2),
        };
    let original = report(&module, FormalIndexWidth::Bits64);
    for write_only in [false, true] {
        // The write-only variation is an inert encoding component, not a changed source owner.
        let mut report = original.clone();
        if write_only {
            report.allocations[0].access = AccessMode::WriteOnly;
        }
        let old = InertCanonicalFormalMemoryObligationReceiptV1::from_obligations(&report).unwrap();
        let current = InertFormalMemoryReceiptFormatV3::from_current_obligations(&report).unwrap();
        assert_eq!(current.canonical_bytes(), old.canonical_bytes());
        assert_eq!(current.identity_digest(), old.identity().digest());
        assert_eq!(
            current.metadata().encoding(),
            if write_only {
                FormalMemoryReceiptEncodingV3::LegacyV2
            } else {
                FormalMemoryReceiptEncodingV3::LegacyV1
            }
        );
        assert_eq!(current.metadata().encoding().extraction_policy(), 1);
        assert_eq!(
            InertFormalMemoryReceiptFormatV3::decode_current(old.into_canonical_bytes()).unwrap(),
            current
        );
        assert_eq!(
            InertCanonicalFormalMemoryObligationReceiptV3::from_obligations(&report),
            Err(FormalMemoryReceiptErrorV1::NonCanonicalVersion { version: 3 })
        );
    }
}

#[test]
fn policies_headers_and_malformed_prefixes_never_fall_back() {
    let bytes = raw(&report(&module(false), FormalIndexWidth::Bits64));
    for end in 0..bytes.len() {
        assert!(
            InertFormalMemoryReceiptFormatV3::decode_current(bytes[..end].to_vec()).is_err(),
            "prefix {end}"
        );
    }
    for (offset, value, error) in [
        (10, 1u16, FormalMemoryReceiptErrorV1::UnknownPolicy(1)),
        (10, 3, FormalMemoryReceiptErrorV1::UnknownPolicy(3)),
        (12, 1, FormalMemoryReceiptErrorV1::UnsupportedFlags(1)),
        (
            14,
            1,
            FormalMemoryReceiptErrorV1::ReservedNonZero {
                field: "receipt header",
            },
        ),
        (8, 4, FormalMemoryReceiptErrorV1::UnknownVersion(4)),
        (8, 1, FormalMemoryReceiptErrorV1::UnknownPolicy(2)),
        (8, 2, FormalMemoryReceiptErrorV1::UnknownPolicy(2)),
    ] {
        let mut bad = bytes.clone();
        bad[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
        assert_eq!(
            InertFormalMemoryReceiptFormatV3::decode_current(bad),
            Err(error)
        );
    }
    let mut trailing = bytes;
    trailing.push(0);
    assert_eq!(
        InertFormalMemoryReceiptFormatV3::decode_current(trailing),
        Err(FormalMemoryReceiptErrorV1::TrailingBytes)
    );
}

#[test]
fn symbolic_bounds_require_complete_same_subject_domain_and_unique_location() {
    let original = report(&module(false), FormalIndexWidth::Bits64);
    for mutation in 0..10 {
        let mut report = original.clone();
        match mutation {
            0 => report.bounds_requirements.clear(),
            1 => report
                .bounds_requirements
                .push(report.bounds_requirements[0]),
            2 => report.bounds_requirements[0].location.operation_index += 1,
            3 => report.bounds_requirements[0].allocation.parameter_index = 1,
            4 => report.bounds_requirements[0].kind = FormalBoundsKindV1::FixedMinimumBytes(256),
            _ => {
                let mut domain = domain(&report);
                match mutation {
                    5 => domain.slice = ValueId(90),
                    6 => domain.predicate = ValueId(91),
                    7 => domain.selected_offset = ValueId(92),
                    8 => {
                        domain.path = FormalGuardedPathV1::TrueEdge {
                            source: BlockId(0),
                            ordinal: 1,
                            target: BlockId(1),
                        }
                    }
                    _ => domain.element_bytes = 8,
                }
                report.bounds_requirements[0].kind =
                    FormalBoundsKindV1::SliceElementAtGuardedIndex(domain);
            }
        }
        assert!(
            InertCanonicalFormalMemoryObligationReceiptV3::from_obligations(&report).is_err(),
            "mutation {mutation}"
        );
        assert!(
            InertCanonicalFormalMemoryObligationReceiptV3::from_canonical_bytes(raw(&report))
                .is_err(),
            "wire {mutation}"
        );
    }
}

#[test]
fn guarded_recipe_tags_allocations_widths_and_alias_regions_are_closed() {
    let mut module = module(false);
    module.functions[0].signature.parameters.push(pointer());
    let body = module.functions[0].body.as_mut().unwrap();
    body.parameters.push(ValueId(20));
    body.blocks[1].operations.push(store(20));
    let original = report(&module, FormalIndexWidth::Bits64);
    for mutation in 0..12 {
        let mut report = original.clone();
        match mutation {
            0 => report.allocations[0].kind = FormalParameterKind::Pointer,
            1 => report.allocations[0].value = ValueId(99),
            2 => report.accesses[0].byte_width = 0,
            3 => report.accesses[0].byte_width = 8,
            4 => report.accesses[0].alignment = 3,
            5 => {
                report.accesses[0].byte_offset = ByteExpression::Affine {
                    constant: 1,
                    invocation_coefficient: 4,
                }
            }
            6 => report.accesses[0].kind = FormalMemoryAccessKind::Atomic,
            7 => report.index_width = FormalIndexWidth::Unknown,
            8 => {
                report.runtime_alias_requirements[0].left_accessed_bytes =
                    FormalAliasRegionV1::FixedBytes(super::super::super::FormalByteRange {
                        start: 0,
                        end_exclusive: 256,
                    })
            }
            9 => {
                report.runtime_alias_requirements[0].right_accessed_bytes =
                    FormalAliasRegionV1::WholeFormalAllocation
            }
            10 => {
                report.runtime_alias_requirements[0].right_accessed_bytes =
                    FormalAliasRegionV1::FixedBytes(super::super::super::FormalByteRange {
                        start: 4,
                        end_exclusive: 4,
                    })
            }
            _ => {
                let mut domain = domain(&report);
                domain.pointer = domain.selected_offset;
                report.accesses[0].domain = FormalAccessDomainV1::SliceBounded(domain);
            }
        }
        assert!(
            InertCanonicalFormalMemoryObligationReceiptV3::from_obligations(&report).is_err(),
            "mutation {mutation}"
        );
    }
    let bytes = raw(&original);
    let mut reader = preamble(&bytes);
    let count = reader.count("allocations").unwrap();
    for _ in 0..count {
        decode_allocation(&mut reader, 3).unwrap();
    }
    reader.count("accesses").unwrap();
    let first_access = reader.offset;
    for (offset, tag) in [(first_access + 70, 2u8), (first_access + 70 + 37, 2)] {
        let mut bad = bytes.clone();
        bad[offset] = tag;
        assert!(matches!(
            InertCanonicalFormalMemoryObligationReceiptV3::from_canonical_bytes(bad),
            Err(FormalMemoryReceiptErrorV1::UnknownTag { .. })
        ));
    }
}

#[test]
fn duplicate_and_noncanonical_rows_are_not_coalesced() {
    let original = report(&module(false), FormalIndexWidth::Bits64);
    for mutation in 0..3 {
        let mut report = original.clone();
        match mutation {
            0 => report.allocations.push(report.allocations[0].clone()),
            1 => report.accesses.push(report.accesses[0].clone()),
            _ => report
                .bounds_requirements
                .push(report.bounds_requirements[0]),
        }
        assert!(matches!(
            InertCanonicalFormalMemoryObligationReceiptV3::from_obligations(&report),
            Err(FormalMemoryReceiptErrorV1::SemanticKeyConflict { .. })
        ));
    }
    let mut module = module(false);
    module.functions[0].signature.parameters.push(pointer());
    let body = module.functions[0].body.as_mut().unwrap();
    body.parameters.push(ValueId(20));
    body.blocks[1].operations.push(store(20));
    let mut bytes = raw(&report(&module, FormalIndexWidth::Bits64));
    let mut reader = preamble(&bytes);
    assert_eq!(reader.count("allocations").unwrap(), 2);
    let offset = reader.offset;
    let first = bytes[offset..offset + 12].to_vec();
    let second = bytes[offset + 12..offset + 24].to_vec();
    bytes[offset..offset + 12].copy_from_slice(&second);
    bytes[offset + 12..offset + 24].copy_from_slice(&first);
    assert!(matches!(
        InertCanonicalFormalMemoryObligationReceiptV3::from_canonical_bytes(bytes),
        Err(FormalMemoryReceiptErrorV1::NonCanonicalOrder { .. })
    ));
}

#[test]
fn ranges_conflict_joins_and_text_remain_closed() {
    let mut module = module(false);
    module.functions[0].body.as_mut().unwrap().blocks[1]
        .operations
        .push(store(7));
    let original = report(&module, FormalIndexWidth::Bits64);
    assert!(!original.inter_invocation_conflicts.is_empty());
    for mutation in 0..5 {
        let mut report = original.clone();
        match mutation {
            0 => report.invocations = None,
            1 => report.accesses[0].invocations = InvocationRange1d::new(1, 64).unwrap(),
            2 => {
                report.inter_invocation_conflicts[0]
                    .allocation
                    .parameter_index = 1
            }
            3 => report.inter_invocation_conflicts[0].right.operation_index = 99,
            _ => report
                .inter_invocation_conflicts
                .push(report.inter_invocation_conflicts[0]),
        }
        assert!(
            InertCanonicalFormalMemoryObligationReceiptV3::from_obligations(&report).is_err(),
            "mutation {mutation}"
        );
    }
    let mut bytes = raw(&original);
    bytes[HEADER_BYTES + 4] = 0xff;
    assert!(matches!(
        InertCanonicalFormalMemoryObligationReceiptV3::from_canonical_bytes(bytes),
        Err(FormalMemoryReceiptErrorV1::InvalidUtf8 { .. })
    ));
}

#[test]
fn genuine_body_order_conflicts_keep_their_original_endpoint_orientation() {
    let mut module = module(false);
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks[0].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(4),
        then_target: BlockId(40),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    body.blocks[1].id = BlockId(40);
    body.blocks[1].terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![],
    });
    let mut last = BasicBlock::new(BlockId(1));
    last.operations.push(store(7));
    last.terminator = Some(Terminator::Return { values: vec![] });
    body.blocks.push(last);
    let original = report(&module, FormalIndexWidth::Bits64);
    let reversed = original
        .inter_invocation_conflicts
        .iter()
        .copied()
        .find(|row| row.left > row.right)
        .expect("body order differs from numeric location order");
    for component in [false, true] {
        let mut report = original.clone();
        if component {
            // Opposite ordered keys remain separate inert rows, never silently coalesced.
            let mut opposite = reversed;
            std::mem::swap(&mut opposite.left, &mut opposite.right);
            report.inter_invocation_conflicts.push(opposite);
        }
        let receipt =
            InertCanonicalFormalMemoryObligationReceiptV3::from_obligations(&report).unwrap();
        let (_, offset) = wire_accesses(receipt.canonical_bytes());
        let mut reader = Reader::new(receipt.canonical_bytes());
        reader.offset = offset;
        let count = reader.count("bounds requirements").unwrap();
        for _ in 0..count {
            decode_location(&mut reader).unwrap();
            reader.u32().unwrap();
            match reader.u8().unwrap() {
                0 => {
                    reader.u64().unwrap();
                }
                1 => {
                    decode_domain(&mut reader).unwrap();
                }
                _ => panic!("kind"),
            }
        }
        assert_eq!(reader.count("runtime alias requirements").unwrap(), 0);
        assert_eq!(
            reader.count("inter-invocation conflicts").unwrap(),
            report.inter_invocation_conflicts.len()
        );
        let mut expected = report.inter_invocation_conflicts.clone();
        expected.sort_by_key(|row| (row.left, row.right));
        for row in expected {
            assert_eq!(
                decode_conflict(&mut reader).unwrap(),
                (
                    (row.left.block.0, row.left.operation_index as u64),
                    (row.right.block.0, row.right.operation_index as u64),
                    row.allocation.parameter_index
                )
            );
        }
        assert!(reader.is_finished());
        receipt.revalidate().unwrap();
    }
}

#[test]
fn computed_record_widths_match_preallocated_emission() {
    fn check(width: usize, emit: impl FnOnce(&mut Writer)) {
        let mut writer = Writer {
            bytes: Vec::with_capacity(width),
            aggregate_records: 0,
        };
        let capacity = writer.bytes.capacity();
        emit(&mut writer);
        assert_eq!(writer.bytes.len(), width);
        assert_eq!(writer.bytes.capacity(), capacity);
    }
    for load in [false, true] {
        let mut report = report(&module(load), FormalIndexWidth::Bits64);
        let access = report.accesses[0].clone();
        let guarded_width = if load { 108 } else { 124 };
        check(guarded_width, |writer| {
            encode_access(writer, &access).unwrap();
            encode_domain(writer, access.domain).unwrap();
        });
        let bound = report.bounds_requirements[0];
        let FormalBoundsKindV1::SliceElementAtGuardedIndex(domain) = bound.kind else {
            panic!("symbolic bound")
        };
        check(if load { 55 } else { 71 }, |writer| {
            encode_location(writer, bound.location).unwrap();
            writer.u32(bound.allocation.parameter_index).unwrap();
            writer.u8(1).unwrap();
            encode_domain(writer, FormalAccessDomainV1::SliceBounded(domain)).unwrap();
        });
        // These numeric-only components check the remaining fixed record widths.
        let mut fixed = access;
        fixed.domain = FormalAccessDomainV1::LaunchEnvelope;
        check(71, |writer| {
            encode_access(writer, &fixed).unwrap();
            encode_domain(writer, fixed.domain).unwrap();
        });
        check(25, |writer| {
            encode_location(writer, bound.location).unwrap();
            writer.u32(bound.allocation.parameter_index).unwrap();
            writer.u8(0).unwrap();
            writer.u64(256).unwrap();
        });
        for length in [1, 97] {
            report.kernel = KernelId::new("k".repeat(length));
            let mut meter = CodecMeter::new(default_work_limit().unwrap()).unwrap();
            let expected = exact_bytes(&report, &mut meter).unwrap();
            let actual = encode_v3(&report, &mut meter).unwrap();
            assert_eq!(actual.len(), expected);
        }
    }
    let fixed = FormalAliasRegionV1::FixedBytes(super::super::super::FormalByteRange {
        start: 4,
        end_exclusive: 8,
    });
    for (left, right, width) in [
        (fixed, fixed, 42),
        (FormalAliasRegionV1::WholeFormalAllocation, fixed, 26),
        (fixed, FormalAliasRegionV1::WholeFormalAllocation, 26),
        (
            FormalAliasRegionV1::WholeFormalAllocation,
            FormalAliasRegionV1::WholeFormalAllocation,
            10,
        ),
    ] {
        check(width, |writer| {
            writer.u32(0).unwrap();
            writer.u32(1).unwrap();
            encode_region(writer, left).unwrap();
            encode_region(writer, right).unwrap();
        });
    }
}

#[test]
fn exact_preflight_and_after_reservation_work_failures_preserve_prefixes() {
    let report = report(&module(false), FormalIndexWidth::Bits64);
    let rows = report.allocations.len() + report.accesses.len() + report.bounds_requirements.len();
    let preflight = 32 + 4 * rows;
    for limit in [preflight - 1, preflight] {
        let mut meter = CodecMeter::new(limit).unwrap();
        let initial = meter.allocations.charged_bytes;
        assert_eq!(exact_bytes(&report, &mut meter).is_ok(), limit == preflight);
        assert_eq!(meter.allocations.charged_bytes, initial);
        assert_eq!(
            meter.work.work(),
            if limit == preflight { preflight } else { 0 }
        );
    }
    let bytes = raw(&report);
    let prefix = 32 + 3 * bytes.len();
    let mut before = CodecMeter::new(prefix - 1).unwrap();
    let initial = before.allocations.charged_bytes;
    assert!(validate_v3(&bytes, &mut before).is_err());
    assert_eq!(before.work.work(), 0);
    assert_eq!(before.allocations.charged_bytes, initial);
    let mut after = CodecMeter::new(prefix).unwrap();
    assert!(matches!(
        validate_v3(&bytes, &mut after),
        Err(FormalMemoryReceiptErrorV1::LimitExceeded {
            field: "guarded receipt work",
            ..
        })
    ));
    assert_eq!(after.work.work(), prefix);
    assert!(
        after.allocations.charged_bytes >= initial + size_of::<AllocationV3>() + size_of::<u32>()
    );
}

#[test]
fn bounded_numeric_sort_and_search_have_exact_local_boundaries() {
    for limit in [7, 8] {
        let mut meter = CodecMeter::new(limit).unwrap();
        let mut rows = [2u32, 1];
        assert_eq!(meter.sort(&mut rows, 1, Ord::cmp).is_ok(), limit == 8);
        assert_eq!(rows, if limit == 8 { [1, 2] } else { [2, 1] });
        assert_eq!(meter.work.work(), if limit == 8 { 8 } else { 0 });
    }
    for limit in [4, 5] {
        let mut meter = CodecMeter::new(limit).unwrap();
        assert_eq!(
            meter
                .find(&[(0u32, 0u64)], 2, |row| row.cmp(&(0, 0)))
                .is_ok(),
            limit == 5
        );
        assert_eq!(meter.work.work(), if limit == 5 { 5 } else { 3 });
    }
}

#[test]
fn byte_record_and_actual_capacity_limits_reject_before_unbounded_growth() {
    let mut meter = CodecMeter::new(default_work_limit().unwrap()).unwrap();
    meter.allocations.charged_bytes = MAX_FORMAL_MEMORY_DECODER_AUXILIARY_BYTES_V1;
    assert!(matches!(
        meter.vector::<u8>(1, "test"),
        Err(FormalMemoryReceiptErrorV1::LimitExceeded {
            field: "decoder auxiliary allocation",
            ..
        })
    ));
    let mut meter = CodecMeter::new(default_work_limit().unwrap()).unwrap();
    let initial = meter.allocations.charged_bytes;
    let rows = meter.vector::<AccessV3>(3, "test").unwrap();
    assert_eq!(
        meter.allocations.charged_bytes,
        initial + rows.capacity() * size_of::<AccessV3>()
    );
    let capacity = rows.capacity();
    drop(rows);
    assert_eq!(
        meter.allocations.charged_bytes,
        initial + capacity * size_of::<AccessV3>()
    );
    assert!(matches!(
        meter.vector::<u64>(usize::MAX, "overflow"),
        Err(FormalMemoryReceiptErrorV1::Overflow { .. })
    ));
    let bytes = vec![0; MAX_FORMAL_MEMORY_RECEIPT_BYTES_V1 + 1];
    let mut meter = CodecMeter::new(0).unwrap();
    assert!(matches!(
        validate_v3(&bytes, &mut meter),
        Err(FormalMemoryReceiptErrorV1::TooLarge { .. })
    ));
    assert_eq!(meter.work.work(), 0);
    assert!(
        preflight_record_counts(ObligationRecordCountsV1 {
            allocations: MAX_FORMAL_MEMORY_RECORDS_V1,
            accesses: 1,
            bounds: 0,
            aliases: 0,
            conflicts: 0
        })
        .is_err()
    );
    let mut bytes = raw(&report(&module(false), FormalIndexWidth::Bits64));
    let offset = preamble(&bytes).offset;
    bytes[offset..offset + 4]
        .copy_from_slice(&((MAX_FORMAL_MEMORY_RECORDS_PER_KIND_V1 + 1) as u32).to_le_bytes());
    assert!(matches!(
        InertCanonicalFormalMemoryObligationReceiptV3::from_canonical_bytes(bytes),
        Err(FormalMemoryReceiptErrorV1::LimitExceeded {
            field: "allocations",
            ..
        })
    ));
}

#[test]
fn numerically_consistent_foreign_ids_are_inert_not_module_authentication() {
    let original = report(&module(false), FormalIndexWidth::Bits64);
    let baseline =
        InertCanonicalFormalMemoryObligationReceiptV3::from_obligations(&original).unwrap();
    let mut altered = original;
    let mut domain = domain(&altered);
    domain.predicate = ValueId(900);
    altered.accesses[0].domain = FormalAccessDomainV1::SliceBounded(domain);
    altered.bounds_requirements[0].kind = FormalBoundsKindV1::SliceElementAtGuardedIndex(domain);
    let inert = InertCanonicalFormalMemoryObligationReceiptV3::from_obligations(&altered).unwrap();
    assert_ne!(inert.identity_digest(), baseline.identity_digest());
    assert!(!inert.grants_authority());
    assert_ne!(
        inert.identity_digest(),
        receipt_identity(inert.canonical_bytes()).digest()
    );
    let mut corrupt = inert;
    corrupt.metadata.index_width = FormalIndexWidth::Unknown;
    assert_eq!(
        corrupt.revalidate(),
        Err(FormalMemoryReceiptErrorV1::IdentityMismatch)
    );
}
