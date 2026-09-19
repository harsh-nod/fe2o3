use super::super::super::super::FormalByteRange;
use super::*;
use crate::{
    BasicBlock, ComparePredicate, ExplicitLaunchExtent1d, Function, FunctionId, Kernel, KernelId,
    LaunchDomain, LaunchExtent, MemoryAccess, Module, Operation, OperationKind, ScalarType,
    Signature, Terminator, Type, ValueDef, derive_kernel_memory_obligations_from_verified,
    verify_module_ref,
};

fn runtime_domain() -> FormalRuntimeSliceReadDomainV1 {
    FormalRuntimeSliceReadDomainV1 {
        allocation: FormalAllocationIdentity { parameter_index: 0 },
        slice: ValueId(0),
        index: ValueId(1),
        guard_index: ValueId(1),
        length: ValueId(2),
        predicate: ValueId(3),
        pointer: ValueId(5),
        element_bytes: 4,
        path: FormalGuardedPathV1::TrueEdge {
            source: BlockId(10),
            ordinal: 0,
            target: BlockId(20),
        },
    }
}

// Inert codec components are deliberately distinct from the genuine extractor fixture below.
fn component() -> FormalMemoryObligations {
    let domain = runtime_domain();
    let location = FunctionOperationLocation {
        block: BlockId(20),
        operation_index: 0,
    };
    FormalMemoryObligations {
        kernel: KernelId::new("k"),
        entry: FunctionId::new("e"),
        index_width: FormalIndexWidth::Bits64,
        invocations: Some(InvocationRange1d::new(0, 1).unwrap()),
        allocations: vec![FormalAllocationParameter {
            identity: domain.allocation,
            value: domain.slice,
            kind: FormalParameterKind::Slice,
            address_space: AddressSpace::Global,
            access: AccessMode::ReadOnly,
        }],
        accesses: vec![FormalMemoryAccess {
            location,
            allocation: domain.allocation,
            kind: FormalMemoryAccessKind::Read,
            address_space: AddressSpace::Global,
            byte_offset: ByteExpression::Unbounded,
            byte_width: 4,
            alignment: 4,
            invocations: InvocationRange1d::new(0, 1).unwrap(),
            domain: FormalAccessDomainV1::RuntimeSliceReadBounded(domain),
        }],
        bounds_requirements: vec![FormalBoundsRequirement {
            location,
            allocation: domain.allocation,
            kind: FormalBoundsKindV1::RuntimeSliceElementAtGuardedIndex(domain),
        }],
        runtime_alias_requirements: vec![],
        inter_invocation_conflicts: vec![],
    }
}

fn replace_domain(report: &mut FormalMemoryObligations, domain: FormalRuntimeSliceReadDomainV1) {
    report.accesses[0].domain = FormalAccessDomainV1::RuntimeSliceReadBounded(domain);
    report.bounds_requirements[0].kind =
        FormalBoundsKindV1::RuntimeSliceElementAtGuardedIndex(domain);
}

fn legacy(version: u16) -> FormalMemoryObligations {
    let mut report = component();
    if version < 3 {
        report.accesses.clear();
        report.bounds_requirements.clear();
        if version == 2 {
            report.allocations[0].access = AccessMode::WriteOnly;
        }
    } else {
        let runtime = runtime_domain();
        let domain = FormalSliceBoundedDomainV1 {
            allocation: runtime.allocation,
            slice: ValueId(0),
            index: ValueId(1),
            length: ValueId(2),
            predicate: ValueId(3),
            selected_offset: ValueId(4),
            pointer: ValueId(5),
            element_bytes: 4,
            path: runtime.path,
        };
        report.accesses[0].domain = FormalAccessDomainV1::SliceBounded(domain);
        report.accesses[0].byte_offset = ByteExpression::invocation_affine(0, 4);
        report.bounds_requirements[0].kind = FormalBoundsKindV1::SliceElementAtGuardedIndex(domain);
    }
    report
}

fn raw(report: &FormalMemoryObligations) -> Vec<u8> {
    encode_revision(
        report,
        WireRevision::RuntimeV4,
        &mut meter_v4(default_work_limit().unwrap()).unwrap(),
    )
    .unwrap()
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

fn access_offset(bytes: &[u8]) -> usize {
    let mut reader = preamble(bytes);
    let count = reader.count("allocations").unwrap();
    for _ in 0..count {
        decode_allocation(&mut reader, 4).unwrap();
    }
    assert!(reader.count("accesses").unwrap() > 0);
    reader.offset
}

fn assert_refused(report: &FormalMemoryObligations) {
    assert!(InertCanonicalFormalMemoryObligationReceiptV4::from_obligations(report).is_err());
    assert!(
        InertCanonicalFormalMemoryObligationReceiptV4::from_canonical_bytes(raw(report)).is_err()
    );
}

fn verified_runtime_report() -> FormalMemoryObligations {
    let pointer = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadOnly,
    );
    let op = |id, ty, kind| Operation::effect_free(ValueDef::new(ValueId(id), ty), kind);
    let mut entry = BasicBlock::new(BlockId(10));
    entry.operations = vec![
        op(
            2,
            Type::INDEX,
            OperationKind::SliceLength { slice: ValueId(0) },
        ),
        op(
            3,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(1),
                rhs: ValueId(2),
            },
        ),
        op(
            4,
            pointer.clone(),
            OperationKind::SliceData { slice: ValueId(0) },
        ),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(3),
        then_target: BlockId(20),
        then_arguments: vec![],
        else_target: BlockId(30),
        else_arguments: vec![],
    });
    let mut yes = BasicBlock::new(BlockId(20));
    yes.operations = vec![
        op(
            5,
            pointer,
            OperationKind::GetElementPointer {
                base: ValueId(4),
                offset: ValueId(1),
            },
        ),
        op(
            6,
            Type::Scalar(ScalarType::U32),
            OperationKind::Load {
                pointer: ValueId(5),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    yes.terminator = Some(Terminator::Return { values: vec![] });
    let mut no = BasicBlock::new(BlockId(30));
    no.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("runtime-receipt");
    module.functions.push(Function::kernel_entry(
        "e",
        Signature::new(
            vec![
                Type::slice(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    AccessMode::ReadOnly,
                ),
                Type::INDEX,
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1)],
        vec![entry, yes, no],
    ));
    module.kernels.push(Kernel::new(
        "k",
        "e",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    let verified = verify_module_ref(&module).unwrap();
    let analysis = derive_kernel_memory_obligations_from_verified(
        verified,
        &KernelId::new("k"),
        ExplicitLaunchExtent1d::Exact(64),
        FormalIndexWidth::Bits64,
    )
    .unwrap();
    assert!(analysis.is_complete(), "{analysis:?}");
    analysis.obligations().clone()
}

#[test]
fn actual_verified_runtime_read_roundtrips_without_new_authority() {
    let report = verified_runtime_report();
    assert_eq!(report.accesses.len(), 1);
    let FormalAccessDomainV1::RuntimeSliceReadBounded(domain) = report.accesses[0].domain else {
        panic!("runtime slice read expected");
    };
    assert_eq!(domain, runtime_domain());
    assert_eq!(report.accesses[0].byte_offset, ByteExpression::Unbounded);
    let receipt = InertCanonicalFormalMemoryObligationReceiptV4::from_obligations(&report).unwrap();
    assert_eq!((receipt.kernel_id(), receipt.entry_id()), ("k", "e"));
    assert_eq!(
        receipt.metadata().encoding(),
        FormalMemoryReceiptEncodingV4::RuntimeBoundedV4
    );
    assert_eq!(receipt.metadata().encoding().wire_version(), 4);
    assert_eq!(receipt.metadata().encoding().extraction_policy(), 3);
    assert_eq!(receipt.metadata().index_width(), FormalIndexWidth::Bits64);
    assert_eq!(
        receipt.metadata().invocations(),
        Some(InvocationRange1d::new(0, 64).unwrap())
    );
    assert_eq!(
        receipt.metadata().analysis_basis(),
        FormalMemoryAnalysisBasis::CompilerDerivedIrWithUnauthenticatedLaunchInputs
    );
    assert!(!receipt.grants_authority());
    receipt.revalidate().unwrap();
    assert_eq!(
        InertCanonicalFormalMemoryObligationReceiptV4::from_canonical_bytes(
            receipt.canonical_bytes().to_vec()
        )
        .unwrap(),
        receipt
    );
    let facade = InertFormalMemoryReceiptFormatV4::from_current_obligations(&report).unwrap();
    assert_eq!(facade.canonical_bytes(), receipt.canonical_bytes());
    assert_eq!(facade.identity_digest(), receipt.identity_digest());
    assert_eq!(facade.metadata(), receipt.metadata());
    assert!(!facade.grants_authority());
    facade.revalidate().unwrap();
    assert_eq!(
        InertFormalMemoryReceiptFormatV4::decode_current(facade.canonical_bytes().to_vec())
            .unwrap(),
        facade
    );
    let mut reader = Reader::new(receipt.canonical_bytes());
    reader.offset = access_offset(receipt.canonical_bytes());
    decode_access(&mut reader).unwrap();
    assert_eq!(
        decode_domain_revision(&mut reader, WireRevision::RuntimeV4).unwrap(),
        report.accesses[0].domain
    );
}

#[test]
fn legacy_encoders_and_decoders_explicitly_refuse_runtime_domains() {
    let report = component();
    assert_eq!(
        InertCanonicalFormalMemoryObligationReceiptV1::from_obligations(&report),
        Err(FormalMemoryReceiptErrorV1::UnsupportedGuardedRepresentation)
    );
    assert_eq!(
        InertCanonicalFormalMemoryObligationReceiptV3::from_obligations(&report),
        Err(FormalMemoryReceiptErrorV1::UnsupportedGuardedRepresentation)
    );
    assert_eq!(
        InertFormalMemoryReceiptFormatV3::from_current_obligations(&report),
        Err(FormalMemoryReceiptErrorV1::UnsupportedGuardedRepresentation)
    );
    let bytes = raw(&report);
    assert_eq!(
        InertCanonicalFormalMemoryObligationReceiptV1::from_canonical_bytes(bytes.clone()),
        Err(FormalMemoryReceiptErrorV1::UnknownVersion(4))
    );
    assert_eq!(
        InertFormalMemoryReceiptFormatV3::decode_current(bytes),
        Err(FormalMemoryReceiptErrorV1::UnknownVersion(4))
    );
    let mut orphan = legacy(1);
    orphan.bounds_requirements = report.bounds_requirements.clone();
    let mut meter = CodecMeter::new(default_work_limit().unwrap()).unwrap();
    let initial = meter.allocations.charged_bytes;
    assert_eq!(
        encode_v3(&orphan, &mut meter),
        Err(FormalMemoryReceiptErrorV1::UnsupportedGuardedRepresentation)
    );
    assert_eq!(meter.allocations.charged_bytes, initial);
}

fn hex(value: &str) -> Vec<u8> {
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

#[test]
fn legacy_v1_v2_v3_literal_goldens_and_identities_are_unchanged() {
    // Independent wire-spec goldens, not outputs generated by the encoder under test.
    let goldens = [
        (
            1,
            "4645324f33464d00010001000000000053000000010000006b01000000650201000001000000000000000001000000000000000100000000000000000000000203010000000000000000000000000000000000",
            "9d1aab540d7f96dcdc8de95917b3e0797678191a8dcf847a20e343c5a9bfc339",
        ),
        (
            2,
            "4645324f33464d00020001000000000053000000010000006b01000000650201000001000000000000000001000000000000000100000000000000000000000203030000000000000000000000000000000000",
            "2e171b670ffe62ec22a0811375176202f7a7ed3f4db3245a55361295074db458",
        ),
        (
            3,
            "4645324f33464d00030002000000000016010000010000006b010000006502010000010000000000000000010000000000000001000000000000000000000002030100010000001400000000000000000000000000000001030100000000000000000000000400000000000000040000000000000004000000000000000000000000000000010000000000000001000000000000000001000000020000000300000004000000050000000400000000000000010a00000000000000000000001400000001000000140000000000000000000000000000000101000000000000000001000000020000000300000004000000050000000400000000000000010a0000000000000000000000140000000000000000000000",
            "e93603dd06b5827642d59c4af4d6852accbe0a55b2d402dcf9bbcdcf4b862eaa",
        ),
    ];
    for (version, bytes, identity) in goldens {
        let bytes = hex(bytes);
        let expected_identity = hex(identity);
        let report = legacy(version);
        let old = InertFormalMemoryReceiptFormatV3::from_current_obligations(&report).unwrap();
        let new = InertFormalMemoryReceiptFormatV4::from_current_obligations(&report).unwrap();
        assert_eq!(old.canonical_bytes(), bytes);
        assert_eq!(old.identity_digest().as_slice(), expected_identity);
        assert_eq!(new.canonical_bytes(), bytes);
        assert_eq!(new.identity_digest().as_slice(), expected_identity);
        assert_eq!(new.metadata().encoding().wire_version(), version);
        assert_eq!(
            new.metadata().encoding().extraction_policy(),
            if version == 3 { 2 } else { 1 }
        );
        assert_eq!(
            InertFormalMemoryReceiptFormatV4::decode_current(bytes).unwrap(),
            new
        );
        assert_eq!(
            InertCanonicalFormalMemoryObligationReceiptV4::from_obligations(&report),
            Err(FormalMemoryReceiptErrorV1::NonCanonicalVersion { version: 4 })
        );
    }
}

#[test]
fn access_constraints_reject_writes_atomics_wrong_layout_and_nonruntime_offsets() {
    for mutation in 0..16 {
        let mut report = component();
        match mutation {
            0 => report.accesses[0].kind = FormalMemoryAccessKind::Write,
            1 => report.accesses[0].kind = FormalMemoryAccessKind::Atomic,
            2 => report.accesses[0].byte_offset = ByteExpression::constant(0),
            3 => report.accesses[0].byte_offset = ByteExpression::invocation_affine(0, 4),
            4 => report.accesses[0].alignment = 8,
            5 => report.accesses[0].alignment = 3,
            6 => report.accesses[0].alignment = 0,
            7 => report.accesses[0].byte_width = 8,
            8 => report.allocations[0].access = AccessMode::WriteOnly,
            9 => report.allocations[0].kind = FormalParameterKind::Pointer,
            10 => report.allocations[0].value = ValueId(99),
            11 => report.index_width = FormalIndexWidth::Bits32,
            12 => report.index_width = FormalIndexWidth::Unknown,
            13 => report.accesses[0].address_space = AddressSpace::Workgroup,
            14 => {
                report.accesses[0].address_space = AddressSpace::Workgroup;
                report.allocations[0].address_space = AddressSpace::Workgroup;
            }
            _ => report.accesses[0].allocation.parameter_index = 9,
        }
        assert_refused(&report);
    }
}

#[test]
fn supported_scalar_widths_and_readable_modes_roundtrip() {
    for width in [1, 2, 4, 8] {
        for access in [AccessMode::ReadOnly, AccessMode::ReadWrite] {
            let mut report = component();
            let mut domain = runtime_domain();
            domain.element_bytes = width;
            replace_domain(&mut report, domain);
            report.allocations[0].access = access;
            report.accesses[0].byte_width = width;
            report.accesses[0].alignment = 1;
            InertCanonicalFormalMemoryObligationReceiptV4::from_obligations(&report)
                .unwrap()
                .revalidate()
                .unwrap();
        }
    }
    for width in [0, 3, 16, u64::MAX] {
        let mut report = component();
        let mut domain = runtime_domain();
        domain.element_bytes = width;
        replace_domain(&mut report, domain);
        report.accesses[0].byte_width = width;
        assert_refused(&report);
    }
}

#[test]
fn exact_full_domain_is_required_at_each_symbolic_bound() {
    for mutation in 0..14 {
        let mut report = component();
        match mutation {
            0 => report.bounds_requirements.clear(),
            1 => report
                .bounds_requirements
                .push(report.bounds_requirements[0]),
            2 => report.bounds_requirements[0].location.operation_index += 1,
            3 => report.bounds_requirements[0].allocation.parameter_index += 1,
            4 => report.bounds_requirements[0].kind = FormalBoundsKindV1::FixedMinimumBytes(4),
            _ => {
                let mut domain = runtime_domain();
                match mutation {
                    5 => domain.allocation.parameter_index += 1,
                    6 => domain.slice = ValueId(90),
                    7 => domain.index = ValueId(91),
                    8 => domain.guard_index = ValueId(92),
                    9 => domain.length = ValueId(93),
                    10 => domain.predicate = ValueId(94),
                    11 => domain.pointer = ValueId(95),
                    12 => domain.element_bytes = 8,
                    _ => {
                        domain.path = FormalGuardedPathV1::TrueEdge {
                            source: BlockId(10),
                            ordinal: 1,
                            target: BlockId(20),
                        }
                    }
                }
                report.bounds_requirements[0].kind =
                    FormalBoundsKindV1::RuntimeSliceElementAtGuardedIndex(domain);
            }
        }
        assert_refused(&report);
    }
}

#[test]
fn numerically_consistent_foreign_graph_coordinates_remain_inert() {
    let original =
        InertCanonicalFormalMemoryObligationReceiptV4::from_obligations(&component()).unwrap();
    let mut report = component();
    let mut domain = runtime_domain();
    domain.index = ValueId(900);
    domain.predicate = ValueId(901);
    domain.path = FormalGuardedPathV1::TrueEdge {
        source: BlockId(777),
        ordinal: 0,
        target: BlockId(999),
    };
    replace_domain(&mut report, domain);
    let receipt = InertCanonicalFormalMemoryObligationReceiptV4::from_obligations(&report).unwrap();
    assert_ne!(receipt.identity_digest(), original.identity_digest());
    assert!(!receipt.grants_authority());
    assert_ne!(
        receipt.identity_digest(),
        &identity_v3(receipt.canonical_bytes())
    );
    assert_ne!(
        receipt.identity_digest(),
        receipt_identity(receipt.canonical_bytes()).digest()
    );
    receipt.revalidate().unwrap();
}

#[test]
fn corrupt_cached_identity_or_metadata_is_refused() {
    for identity in [false, true] {
        let mut receipt =
            InertCanonicalFormalMemoryObligationReceiptV4::from_obligations(&component()).unwrap();
        if identity {
            receipt.identity[0] ^= 1;
        } else {
            receipt.metadata.index_width = FormalIndexWidth::Bits32;
        }
        assert_eq!(
            receipt.revalidate(),
            Err(FormalMemoryReceiptErrorV1::IdentityMismatch)
        );
    }
}

#[test]
fn malformed_headers_prefixes_paths_and_unknown_tags_never_fall_back() {
    let bytes = raw(&component());
    for end in 0..bytes.len() {
        assert!(
            InertFormalMemoryReceiptFormatV4::decode_current(bytes[..end].to_vec()).is_err(),
            "prefix {end}"
        );
    }
    for (offset, value) in [(8, 5u16), (8, 3), (10, 2), (10, 4), (12, 1), (14, 1)] {
        let mut bad = bytes.clone();
        bad[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
        assert!(InertFormalMemoryReceiptFormatV4::decode_current(bad).is_err());
    }
    let access = access_offset(&bytes);
    for (offset, value) in [
        (access + 70, 3),
        (access + 70 + 37, 0),
        (access + 70 + 37, 2),
    ] {
        let mut bad = bytes.clone();
        bad[offset] = value;
        assert!(InertFormalMemoryReceiptFormatV4::decode_current(bad).is_err());
    }
    let mut bad = bytes.clone();
    bad[HEADER_BYTES + 4] = 0xff;
    assert!(InertFormalMemoryReceiptFormatV4::decode_current(bad).is_err());
    let mut trailing = bytes;
    trailing.push(0);
    assert_eq!(
        InertFormalMemoryReceiptFormatV4::decode_current(trailing),
        Err(FormalMemoryReceiptErrorV1::TrailingBytes)
    );
    let mut report = component();
    let mut domain = runtime_domain();
    domain.path = FormalGuardedPathV1::ExplicitPredicate;
    replace_domain(&mut report, domain);
    assert_eq!(
        InertCanonicalFormalMemoryObligationReceiptV4::from_obligations(&report),
        Err(invalid("runtime read requires a true edge"))
    );
}

fn with_fixed_write() -> FormalMemoryObligations {
    let mut report = component();
    let allocation = FormalAllocationIdentity { parameter_index: 1 };
    let location = FunctionOperationLocation {
        block: BlockId(20),
        operation_index: 1,
    };
    report.allocations.push(FormalAllocationParameter {
        identity: allocation,
        value: ValueId(100),
        kind: FormalParameterKind::Pointer,
        address_space: AddressSpace::Global,
        access: AccessMode::ReadWrite,
    });
    report.accesses.push(FormalMemoryAccess {
        location,
        allocation,
        kind: FormalMemoryAccessKind::Write,
        address_space: AddressSpace::Global,
        byte_offset: ByteExpression::constant(0),
        byte_width: 4,
        alignment: 4,
        invocations: InvocationRange1d::new(0, 1).unwrap(),
        domain: FormalAccessDomainV1::LaunchEnvelope,
    });
    report.bounds_requirements.push(FormalBoundsRequirement {
        location,
        allocation,
        kind: FormalBoundsKindV1::FixedMinimumBytes(4),
    });
    report
        .runtime_alias_requirements
        .push(RuntimeAliasRequirement {
            left: report.allocations[0].identity,
            right: allocation,
            left_accessed_bytes: FormalAliasRegionV1::WholeFormalAllocation,
            right_accessed_bytes: FormalAliasRegionV1::FixedBytes(FormalByteRange {
                start: 0,
                end_exclusive: 4,
            }),
        });
    report
}

#[test]
fn runtime_read_alias_regions_cover_whole_allocations_without_affine_claims() {
    let report = with_fixed_write();
    InertCanonicalFormalMemoryObligationReceiptV4::from_obligations(&report)
        .unwrap()
        .revalidate()
        .unwrap();
    for mutation in 0..5 {
        let mut bad = report.clone();
        match mutation {
            0 => {
                bad.runtime_alias_requirements[0].left_accessed_bytes =
                    FormalAliasRegionV1::FixedBytes(FormalByteRange {
                        start: 0,
                        end_exclusive: 4,
                    })
            }
            1 => {
                bad.runtime_alias_requirements[0].right_accessed_bytes =
                    FormalAliasRegionV1::WholeFormalAllocation
            }
            2 => bad.runtime_alias_requirements[0].right.parameter_index = 99,
            3 => bad
                .runtime_alias_requirements
                .push(bad.runtime_alias_requirements[0]),
            _ => bad.accesses[1].kind = FormalMemoryAccessKind::Read,
        }
        assert_refused(&bad);
    }
}

#[test]
fn runtime_and_guarded_and_fixed_domains_coexist_without_conversion() {
    let mut report = with_fixed_write();
    let mut guarded = legacy(3);
    let mut domain = match guarded.accesses[0].domain {
        FormalAccessDomainV1::SliceBounded(d) => d,
        _ => unreachable!(),
    };
    domain.allocation.parameter_index = 2;
    domain.slice = ValueId(200);
    guarded.allocations[0].identity = domain.allocation;
    guarded.allocations[0].value = domain.slice;
    guarded.accesses[0].allocation = domain.allocation;
    guarded.accesses[0].location.operation_index = 2;
    guarded.accesses[0].domain = FormalAccessDomainV1::SliceBounded(domain);
    guarded.bounds_requirements[0].allocation = domain.allocation;
    guarded.bounds_requirements[0].location.operation_index = 2;
    guarded.bounds_requirements[0].kind = FormalBoundsKindV1::SliceElementAtGuardedIndex(domain);
    report.allocations.extend(guarded.allocations);
    report.accesses.extend(guarded.accesses);
    report
        .bounds_requirements
        .extend(guarded.bounds_requirements);
    let receipt = InertCanonicalFormalMemoryObligationReceiptV4::from_obligations(&report).unwrap();
    receipt.revalidate().unwrap();
    let expected = receipt.canonical_bytes().to_vec();
    report.allocations.reverse();
    report.accesses.reverse();
    report.bounds_requirements.reverse();
    assert_eq!(
        InertCanonicalFormalMemoryObligationReceiptV4::from_obligations(&report)
            .unwrap()
            .canonical_bytes(),
        expected
    );
}

#[test]
fn duplicate_and_noncanonical_rows_stay_refused() {
    for mutation in 0..3 {
        let mut report = component();
        match mutation {
            0 => report.allocations.push(report.allocations[0].clone()),
            1 => report.accesses.push(report.accesses[0].clone()),
            _ => report
                .bounds_requirements
                .push(report.bounds_requirements[0]),
        }
        assert!(matches!(
            InertCanonicalFormalMemoryObligationReceiptV4::from_obligations(&report),
            Err(FormalMemoryReceiptErrorV1::SemanticKeyConflict { .. })
        ));
    }
    let mut bytes = raw(&with_fixed_write());
    let offset = preamble(&bytes).offset + 4;
    for index in 0..12 {
        bytes.swap(offset + index, offset + 12 + index);
    }
    assert!(matches!(
        InertCanonicalFormalMemoryObligationReceiptV4::from_canonical_bytes(bytes),
        Err(FormalMemoryReceiptErrorV1::NonCanonicalOrder { .. })
    ));
}

#[test]
fn runtime_domain_and_bounds_have_exact_fixed_wire_sizes() {
    let report = component();
    let mut meter = meter_v4(default_work_limit().unwrap()).unwrap();
    let expected = exact_bytes_revision(&report, WireRevision::RuntimeV4, &mut meter).unwrap();
    let bytes = encode_revision(&report, WireRevision::RuntimeV4, &mut meter).unwrap();
    assert_eq!(bytes.len(), expected);
    assert_eq!(bytes.len(), 278);
    let mut reader = Reader::new(&bytes);
    reader.offset = access_offset(&bytes);
    let start = reader.offset;
    decode_access(&mut reader).unwrap();
    decode_domain_revision(&mut reader, WireRevision::RuntimeV4).unwrap();
    assert_eq!(reader.offset - start, 124);
    assert_eq!(reader.count("bounds").unwrap(), 1);
    let start = reader.offset;
    decode_location(&mut reader).unwrap();
    reader.u32().unwrap();
    assert_eq!(reader.u8().unwrap(), 2);
    decode_domain_revision(&mut reader, WireRevision::RuntimeV4).unwrap();
    assert_eq!(reader.offset - start, 71);
}

#[test]
fn exact_preflight_and_decoder_prefix_work_boundaries_preserve_meter_state() {
    let report = component();
    for limit in [43, 44] {
        let mut meter = meter_v4(limit).unwrap();
        let initial = meter.allocations.charged_bytes;
        assert_eq!(
            exact_bytes_revision(&report, WireRevision::RuntimeV4, &mut meter).is_ok(),
            limit == 44
        );
        assert_eq!(meter.work.work(), if limit == 44 { 44 } else { 0 });
        assert_eq!(meter.allocations.charged_bytes, initial);
    }
    let bytes = raw(&report);
    let prefix = 32 + 3 * bytes.len();
    let mut before = meter_v4(prefix - 1).unwrap();
    let initial = before.allocations.charged_bytes;
    assert!(validate_revision(&bytes, WireRevision::RuntimeV4, &mut before).is_err());
    assert_eq!(before.work.work(), 0);
    assert_eq!(before.allocations.charged_bytes, initial);
    let mut after = meter_v4(prefix).unwrap();
    assert!(matches!(
        validate_revision(&bytes, WireRevision::RuntimeV4, &mut after),
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
fn measured_exact_and_one_short_complete_codec_work_limits() {
    let report = with_fixed_write();
    let mut measured = meter_v4(default_work_limit().unwrap()).unwrap();
    let bytes = encode_revision(&report, WireRevision::RuntimeV4, &mut measured).unwrap();
    validate_revision(&bytes, WireRevision::RuntimeV4, &mut measured).unwrap();
    let exact = measured.work.work();
    for limit in [exact - 1, exact] {
        let mut meter = meter_v4(limit).unwrap();
        let result = encode_revision(&report, WireRevision::RuntimeV4, &mut meter)
            .and_then(|bytes| validate_revision(&bytes, WireRevision::RuntimeV4, &mut meter));
        assert_eq!(result.is_ok(), limit == exact);
        assert!(meter.work.work() <= limit);
    }
}

#[test]
fn actual_vector_capacity_storage_and_global_caps_remain_bounded() {
    let mut meter = meter_v4(default_work_limit().unwrap()).unwrap();
    let baseline = meter.allocations.charged_bytes;
    let rows = meter.vector::<AccessV3>(3, "test").unwrap();
    let exact = baseline + rows.capacity() * size_of::<AccessV3>();
    assert_eq!(meter.allocations.charged_bytes, exact);
    drop(rows);
    assert_eq!(meter.allocations.charged_bytes, exact);
    meter.allocations.charged_bytes = MAX_FORMAL_MEMORY_DECODER_AUXILIARY_BYTES_V1;
    assert!(matches!(
        meter.vector::<u8>(1, "test"),
        Err(FormalMemoryReceiptErrorV1::LimitExceeded {
            field: "decoder auxiliary allocation",
            ..
        })
    ));
    assert_eq!(
        meter.allocations.charged_bytes,
        MAX_FORMAL_MEMORY_DECODER_AUXILIARY_BYTES_V1
    );
    assert!(matches!(
        meter.vector::<u64>(usize::MAX, "overflow"),
        Err(FormalMemoryReceiptErrorV1::Overflow { .. })
    ));
    let too_large = vec![0; MAX_FORMAL_MEMORY_RECEIPT_BYTES_V1 + 1];
    let mut empty_work = meter_v4(0).unwrap();
    assert!(matches!(
        validate_revision(&too_large, WireRevision::RuntimeV4, &mut empty_work),
        Err(FormalMemoryReceiptErrorV1::TooLarge { .. })
    ));
    assert_eq!(empty_work.work.work(), 0);
    let mut bytes = raw(&component());
    let offset = preamble(&bytes).offset;
    bytes[offset..offset + 4]
        .copy_from_slice(&((MAX_FORMAL_MEMORY_RECORDS_PER_KIND_V1 + 1) as u32).to_le_bytes());
    assert!(matches!(
        InertCanonicalFormalMemoryObligationReceiptV4::from_canonical_bytes(bytes),
        Err(FormalMemoryReceiptErrorV1::LimitExceeded {
            field: "allocations",
            ..
        })
    ));
}
