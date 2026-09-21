use fe2o3_kernel_descriptor::*;
use sha2::{Digest, Sha256};

fn free(_: usize) -> Result<(), &'static str> {
    Ok(())
}

fn with_input<T>(
    kinds: &[SourceTypeDescriptorV3],
    f: impl FnOnce(DeviceDescriptorTableInputV3<'_>) -> T,
) -> T {
    let compiler = CompilerIdentityV1::new(
        Text::new("rustc").unwrap(),
        Text::new("nightly").unwrap(),
        [7; 20],
    );
    let producer = ProducerIdentityV1::new(Text::new("fe2o3").unwrap(), Text::new("test").unwrap());
    let mut sources = kinds
        .iter()
        .map(|kind| SourceTypeRecordV3::new(*kind, &mut free).unwrap())
        .collect::<Vec<_>>();
    sources.sort_by_key(|r| r.identity());
    sources.dedup();
    let layouts_by_argument = kinds
        .iter()
        .map(|kind| {
            let d = match kind {
                SourceTypeDescriptorV3::SharedSlice(s) => {
                    DeviceLayoutDescriptorV1::shared_slice(*s)
                }
                SourceTypeDescriptorV3::DisjointSlice(s) => {
                    DeviceLayoutDescriptorV1::disjoint_slice(*s)
                }
                SourceTypeDescriptorV3::GlobalMutPointer(s) => {
                    DeviceLayoutDescriptorV1::global_mut_pointer(*s)
                }
                _ => DeviceLayoutDescriptorV1::scalar(kind.physical_scalar()),
            };
            device_layout_record_v3(d, &mut free).unwrap()
        })
        .collect::<Vec<_>>();
    let mut layouts = layouts_by_argument.clone();
    layouts.sort_by_key(|r| r.identity());
    layouts.dedup();
    let mut offset = 0u32;
    let mut max_alignment = 1u32;
    let components = kinds
        .iter()
        .map(|kind| {
            let mut result = Vec::new();
            let alignment = match kind {
                SourceTypeDescriptorV3::Scalar(s) => u32::from(s.alignment_bytes()),
                _ => 8,
            };
            max_alignment = max_alignment.max(alignment);
            offset = (offset + alignment - 1) & !(alignment - 1);
            match kind {
                SourceTypeDescriptorV3::SharedSlice(_)
                | SourceTypeDescriptorV3::DisjointSlice(_)
                | SourceTypeDescriptorV3::GlobalMutPointer(_) => {
                    let (access, alias) = if matches!(kind, SourceTypeDescriptorV3::SharedSlice(_))
                    {
                        (AccessMode::ReadOnly, AliasSemantics::SharedReadOnly)
                    } else {
                        (AccessMode::ReadWrite, AliasSemantics::Exclusive)
                    };
                    result.push(PhysicalComponentV3 {
                        kind: PhysicalAbiComponentKind::GlobalPointer,
                        offset,
                        size: 8,
                        alignment: 8,
                        access,
                        alias,
                    });
                    offset += 8;
                    if !matches!(kind, SourceTypeDescriptorV3::GlobalMutPointer(_)) {
                        result.push(PhysicalComponentV3 {
                            kind: PhysicalAbiComponentKind::SliceLengthU64,
                            offset,
                            size: 8,
                            alignment: 8,
                            access: AccessMode::ByValue,
                            alias: AliasSemantics::Value,
                        });
                        offset += 8;
                    }
                }
                _ => {
                    let s = kind.physical_scalar();
                    result.push(PhysicalComponentV3 {
                        kind: PhysicalAbiComponentKind::ScalarByValue(s),
                        offset,
                        size: s.size_bytes(),
                        alignment: s.alignment_bytes(),
                        access: AccessMode::ByValue,
                        alias: AliasSemantics::Value,
                    });
                    offset += u32::from(s.size_bytes());
                }
            }
            result
        })
        .collect::<Vec<_>>();
    offset = (offset + max_alignment - 1) & !(max_alignment - 1);
    let names = (0..kinds.len())
        .map(|i| format!("arg_{i}"))
        .collect::<Vec<_>>();
    let arguments = kinds
        .iter()
        .enumerate()
        .map(|(i, kind)| {
            let (ownership, access, alias) = match kind {
                SourceTypeDescriptorV3::SharedSlice(_) => (
                    OwnershipSemantics::SharedBorrow,
                    AccessMode::ReadOnly,
                    AliasSemantics::SharedReadOnly,
                ),
                SourceTypeDescriptorV3::DisjointSlice(_)
                | SourceTypeDescriptorV3::GlobalMutPointer(_) => (
                    OwnershipSemantics::UniqueBorrow,
                    AccessMode::ReadWrite,
                    AliasSemantics::Exclusive,
                ),
                _ => (
                    OwnershipSemantics::ByValue,
                    AccessMode::ByValue,
                    AliasSemantics::Value,
                ),
            };
            LogicalArgumentInputV3 {
                source_index: i as u16,
                name: &names[i],
                source_type: sources
                    .iter()
                    .find(|r| r.descriptor() == *kind)
                    .unwrap()
                    .identity(),
                device_layout: layouts_by_argument[i].identity(),
                ownership,
                access,
                alias,
                components: &components[i],
            }
        })
        .collect::<Vec<_>>();
    let launch = LaunchConstraintsV1::new(
        1,
        BlockSizeV1::Any,
        DimensionsV1::new(1024, 1, 1).unwrap(),
        256,
        0,
        0,
    )
    .unwrap();
    let evidence = BuildEvidenceV1::new(
        EvidenceIdentity::from_opaque_bytes([11; 32]),
        EvidenceDigest::from_sha256_bytes([12; 32]),
    );
    let id = KernelId::from_bytes([13; 32]);
    let kernels = [KernelDescriptorInputV3 {
        kernel_id: id,
        logical_name: "kernel",
        entry_name: "kernel",
        descriptor_symbol: "kernel.kd",
        source_evidence: evidence,
        executable_ir_evidence: evidence,
        capabilities: &[CapabilityV1::AmdWave],
        abi_layout: KernelAbiLayoutV1::new(offset, offset, 8).unwrap(),
        launch: &launch,
        arguments: &arguments,
    }];
    let requirements = [KernelTargetRequirementsV2::new(
        id,
        LdsRequirementsV2::new(0, 0).unwrap(),
        RequiredWavefrontWidthV2::Wave64,
        false,
        SynchronizationRequirementsV2::empty(),
        AtomicRequirementsV2::empty(),
    )];
    f(DeviceDescriptorTableInputV3 {
        canonical_code_object_digest: CanonicalCodeObjectDigest::from_bytes([0; 32]),
        code_object_version: CodeObjectVersion::V6,
        compiler: &compiler,
        producer: &producer,
        device_target: DeviceTargetV1::parse("gfx942:xnack-").unwrap(),
        type_records: &sources,
        layout_records: &layouts,
        kernels: &kernels,
        requirements: &requirements,
    })
}

const MIXED: &[SourceTypeDescriptorV3] = &[
    SourceTypeDescriptorV3::Usize,
    SourceTypeDescriptorV3::Isize,
    SourceTypeDescriptorV3::Scalar(ScalarTypeV1::U64),
    SourceTypeDescriptorV3::SharedSlice(ScalarTypeV1::U32),
];
fn encode(input: &DeviceDescriptorTableInputV3<'_>) -> Vec<u8> {
    let mut bytes = vec![0; encoded_device_descriptor_table_v3_len(input, &mut free).unwrap()];
    encode_device_descriptor_table_v3(input, &mut bytes, &mut free).unwrap();
    bytes
}
fn bytes() -> Vec<u8> {
    with_input(MIXED, |v| encode(&v))
}
fn types_start(bytes: &[u8]) -> usize {
    let mut p = 52;
    for _ in 0..2 {
        let n = u16::from_le_bytes(bytes[p..p + 2].try_into().unwrap()) as usize;
        p += 2 + n;
    }
    p += 20;
    for _ in 0..3 {
        let n = u16::from_le_bytes(bytes[p..p + 2].try_into().unwrap()) as usize;
        p += 2 + n;
    }
    p + 8
}

#[test]
fn nominal_v3_round_trip_keeps_nominal_sources_and_physical_layouts_separate() {
    for target in ["gfx942:xnack-", "gfx950:xnack-"] {
        with_input(MIXED, |mut input| {
            input.device_target = DeviceTargetV1::parse(target).unwrap();
            let bytes = encode(&input);
            let table = decode_device_descriptor_table_v3(&bytes, &mut free).unwrap();
            assert_eq!(table.device_target(), input.device_target);
            assert_eq!(table.kernel_count(), 1);
            assert_eq!(table.type_count(), 4);
            assert_eq!(table.layout_count(), 3);
            let k = table
                .find_kernel(input.kernels[0].kernel_id, &mut free)
                .unwrap();
            assert_eq!(k.argument_count(), 4);
            assert_eq!(k.component_count(), 5);
            assert_eq!(k.abi_layout().explicit_argument_size(), 40);
            let mut cursor = k.arguments();
            for (index, expected) in MIXED.iter().enumerate() {
                let arg = cursor.next(&mut free).unwrap().unwrap();
                assert_eq!(usize::from(arg.source_index()), index);
                let ty = table.source_type(arg.source_type(), &mut free).unwrap();
                assert_eq!(ty.descriptor(), *expected);
                assert_eq!(
                    arg.component(0, &mut free).unwrap().offset,
                    (index * 8) as u32
                );
                assert_eq!(
                    table
                        .device_layout(arg.device_layout(), &mut free)
                        .unwrap()
                        .identity(),
                    input.kernels[0].arguments[index].device_layout
                );
            }
            assert!(cursor.next(&mut free).unwrap().is_none());
            assert_eq!(
                table.requirement(0, &mut free).unwrap(),
                input.requirements[0]
            );
            assert_eq!(&bytes[16..48], &[0; 32]);
        });
    }
}

#[test]
fn nominal_v3_old_record_identities_remain_exact_and_nominal_ids_are_distinct() {
    for s in [ScalarTypeV1::U64, ScalarTypeV1::I64] {
        let old = SourceTypeRecordV1::new(SourceTypeDescriptorV1::scalar(s));
        let new = SourceTypeRecordV3::new(SourceTypeDescriptorV3::Scalar(s), &mut free).unwrap();
        assert_eq!(old.identity(), new.identity());
        let old = DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::scalar(s));
        let new = device_layout_record_v3(DeviceLayoutDescriptorV1::scalar(s), &mut free).unwrap();
        assert_eq!(old, new);
    }
    for (kind, payload, scalar) in [
        (
            SourceTypeDescriptorV3::Usize,
            [5, 0, 0, 0],
            ScalarTypeV1::U64,
        ),
        (
            SourceTypeDescriptorV3::Isize,
            [6, 0, 0, 0],
            ScalarTypeV1::I64,
        ),
    ] {
        let row = SourceTypeRecordV3::new(kind, &mut free).unwrap();
        let mut hash = Sha256::new();
        hash.update(b"FE2O3/RUST-TYPE/V3\0");
        hash.update(4u64.to_le_bytes());
        hash.update(payload);
        assert_eq!(
            row.identity().as_bytes(),
            &<[u8; 32]>::from(hash.finalize())
        );
        assert_ne!(
            row.identity(),
            SourceTypeRecordV1::new(SourceTypeDescriptorV1::scalar(scalar)).identity()
        );
    }
}

#[test]
fn nominal_v3_old_versions_and_unknown_tags_remain_closed() {
    let bytes = bytes();
    assert!(matches!(
        decode_device_descriptor_table_v1(&bytes),
        Err(DecodeError::UnknownVersion(3))
    ));
    assert!(matches!(
        decode_device_descriptor_table_v2(&bytes),
        Err(DecodeError::UnknownVersion(3))
    ));
    for version in [1u16, 2, 4, u16::MAX] {
        let mut mutant = bytes.clone();
        mutant[8..10].copy_from_slice(&version.to_le_bytes());
        assert!(
            matches!(decode_device_descriptor_table_v3(&mutant, &mut free), Err(DescriptorWireErrorV3::Decode(DecodeError::UnknownVersion(v))) if v == version)
        );
    }
    let mut mutant = bytes.clone();
    let start = types_start(&mutant);
    mutant[start + 32] = 255;
    assert!(matches!(
        decode_device_descriptor_table_v3(&mutant, &mut free),
        Err(DescriptorWireErrorV3::Decode(
            DecodeError::UnknownTag { .. }
        ))
    ));
}

#[test]
fn nominal_v3_every_truncation_and_trailing_byte_is_rejected() {
    let bytes = bytes();
    for n in 0..bytes.len() {
        assert!(
            decode_device_descriptor_table_v3(&bytes[..n], &mut free).is_err(),
            "length {n}"
        );
        if n >= 16 {
            let mut inner_cut = bytes[..n].to_vec();
            inner_cut[12..16].copy_from_slice(&(n as u32).to_le_bytes());
            assert!(
                decode_device_descriptor_table_v3(&inner_cut, &mut free).is_err(),
                "inner cut {n}"
            );
        }
    }
    let mut extended = bytes.clone();
    extended.push(0);
    let n = extended.len() as u32;
    extended[12..16].copy_from_slice(&n.to_le_bytes());
    assert!(matches!(
        decode_device_descriptor_table_v3(&extended, &mut free),
        Err(DescriptorWireErrorV3::Decode(DecodeError::TrailingBytes))
    ));
}

#[test]
fn nominal_v3_mutated_identity_order_and_nominal_scalar_payload_are_rejected() {
    let base = bytes();
    let t = types_start(&base);
    let mut b = base.clone();
    b[t] ^= 1;
    assert!(matches!(
        decode_device_descriptor_table_v3(&b, &mut free),
        Err(DescriptorWireErrorV3::Decode(DecodeError::Validation(
            ValidationError::IdentityMismatch { field: "Rust type" }
        )))
    ));
    let mut b = base.clone();
    let first = b[t..t + 36].to_vec();
    let second = b[t + 36..t + 72].to_vec();
    b[t..t + 36].copy_from_slice(&second);
    b[t + 36..t + 72].copy_from_slice(&first);
    assert!(matches!(
        decode_device_descriptor_table_v3(&b, &mut free),
        Err(DescriptorWireErrorV3::Decode(DecodeError::Validation(
            ValidationError::NonCanonicalOrder { .. }
        )))
    ));
    let mut b = base;
    let row = (0..4)
        .find(|i| matches!(b[t + i * 36 + 32], 5 | 6))
        .unwrap();
    b[t + row * 36 + 33] = 8;
    assert!(matches!(
        decode_device_descriptor_table_v3(&b, &mut free),
        Err(DescriptorWireErrorV3::Decode(
            DecodeError::NonzeroReserved {
                field: "nominal scalar"
            }
        ))
    ));
}

#[test]
fn nominal_v3_all_encoder_callback_denials_preserve_the_entire_output() {
    with_input(MIXED, |input| {
        let n = encoded_device_descriptor_table_v3_len(&input, &mut free).unwrap();
        let mut calls = 0usize;
        let mut full = vec![0; n];
        encode_device_descriptor_table_v3(&input, &mut full, &mut |_| {
            calls += 1;
            Ok::<_, usize>(())
        })
        .unwrap();
        for denied in 0..calls {
            let mut output = vec![0xa5; n];
            let mut seen = 0;
            let result = encode_device_descriptor_table_v3(&input, &mut output, &mut |_| {
                let current = seen;
                seen += 1;
                if current == denied {
                    Err(denied)
                } else {
                    Ok(())
                }
            });
            assert!(matches!(result, Err(DescriptorWireErrorV3::Work(value)) if value == denied));
            assert_eq!(seen, denied + 1);
            assert_eq!(output, vec![0xa5; n]);
        }
        let mut short = vec![0xa5; n - 1];
        assert!(
            matches!(encode_device_descriptor_table_v3(&input, &mut short, &mut free), Err(DescriptorWireErrorV3::OutputLength { expected, actual }) if expected == n && actual == n - 1)
        );
        assert_eq!(short, vec![0xa5; n - 1]);
    });
}

#[test]
fn nominal_v3_exact_work_boundary_and_all_decode_denials_are_typed() {
    let bytes = bytes();
    let mut work = 0usize;
    let mut calls = 0usize;
    decode_device_descriptor_table_v3(&bytes, &mut |n| {
        work += n;
        calls += 1;
        Ok::<_, usize>(())
    })
    .unwrap();
    for limit in [work - 1, work] {
        let mut used = 0;
        let value = decode_device_descriptor_table_v3(&bytes, &mut |n| {
            let next = used + n;
            if next > limit {
                Err((next, limit))
            } else {
                used = next;
                Ok(())
            }
        });
        if limit == work {
            assert!(value.is_ok());
            assert_eq!(used, work);
        } else {
            assert!(
                matches!(value, Err(DescriptorWireErrorV3::Work((actual, denied))) if actual > denied && denied == limit)
            );
        }
    }
    for denied in 0..calls {
        let mut seen = 0;
        let value = decode_device_descriptor_table_v3(&bytes, &mut |_| {
            let current = seen;
            seen += 1;
            if current == denied {
                Err(denied)
            } else {
                Ok(())
            }
        });
        assert!(matches!(value, Err(DescriptorWireErrorV3::Work(value)) if value == denied));
        assert_eq!(seen, denied + 1);
    }
}

#[test]
fn nominal_v3_argument_and_capability_cursors_rollback_on_each_callback_denial() {
    let bytes = bytes();
    let v = decode_device_descriptor_table_v3(&bytes, &mut free).unwrap();
    let k = v.kernel(0, &mut free).unwrap();
    let mut count = 0;
    k.arguments()
        .next(&mut |_| {
            count += 1;
            Ok::<_, usize>(())
        })
        .unwrap();
    for denied in 0..count {
        let mut cursor = k.arguments();
        let mut seen = 0;
        assert!(
            matches!(cursor.next(&mut |_| { let index = seen; seen += 1; if index == denied { Err(denied) } else { Ok(()) } }), Err(DescriptorWireErrorV3::Work(n)) if n == denied)
        );
        assert_eq!(cursor.next(&mut free).unwrap().unwrap().source_index(), 0);
    }
    let mut count = 0;
    k.capabilities()
        .next(&mut |_| {
            count += 1;
            Ok::<_, usize>(())
        })
        .unwrap();
    for denied in 0..count {
        let mut cursor = k.capabilities();
        let mut seen = 0;
        assert!(
            matches!(cursor.next(&mut |_| { let index = seen; seen += 1; if index == denied { Err(denied) } else { Ok(()) } }), Err(DescriptorWireErrorV3::Work(n)) if n == denied)
        );
        assert_eq!(cursor.next(&mut free).unwrap(), Some(CapabilityV1::AmdWave));
    }
}

#[test]
fn nominal_v3_requirement_closure_checks_ids_lds_wave_caps_and_reserved_bits() {
    let base = bytes();
    let start = base.len() - 48;
    for (offset, value) in [
        (start, 99),
        (start + 32, 1),
        (start + 40, 1),
        (start + 42, 1),
        (start + 44, 1),
        (start + 46, 1),
    ] {
        let mut b = base.clone();
        b[offset] = value;
        assert!(
            decode_device_descriptor_table_v3(&b, &mut free).is_err(),
            "offset {offset}"
        );
    }
    with_input(MIXED, |mut input| {
        input.requirements = &[];
        assert!(matches!(
            encoded_device_descriptor_table_v3_len(&input, &mut free),
            Err(DescriptorWireErrorV3::Decode(DecodeError::Validation(
                ValidationError::InvalidValue {
                    field: "kernel target requirement closure"
                }
            )))
        ));
    });
}

#[test]
fn nominal_v3_empty_kernel_abi_and_all_pointer_shapes_remain_representable() {
    for kinds in [
        vec![],
        vec![
            SourceTypeDescriptorV3::SharedSlice(ScalarTypeV1::U8),
            SourceTypeDescriptorV3::DisjointSlice(ScalarTypeV1::U32),
            SourceTypeDescriptorV3::GlobalMutPointer(ScalarTypeV1::I64),
        ],
    ] {
        with_input(&kinds, |input| {
            let b = encode(&input);
            let v = decode_device_descriptor_table_v3(&b, &mut free).unwrap();
            assert_eq!(
                v.kernel(0, &mut free).unwrap().argument_count(),
                kinds.len()
            );
        });
    }
}

#[test]
fn nominal_v3_headers_include_full_index_and_queries_reject_out_of_range() {
    assert_eq!(
        DESCRIPTOR_TABLE_VIEW_STORAGE_V3,
        size_of::<DeviceDescriptorTableV3<'static>>()
    );
    assert!(DESCRIPTOR_TABLE_VIEW_STORAGE_V3 >= size_of::<[usize; MAX_KERNELS]>());
    assert!(
        DESCRIPTOR_READER_SCRATCH_STORAGE_V3
            >= MAX_TYPE_RECORDS + MAX_LAYOUT_RECORDS + size_of::<[&str; MAX_KERNELS * 3]>()
    );
    assert!(DESCRIPTOR_ENCODER_SCRATCH_STORAGE_V3 > DESCRIPTOR_READER_SCRATCH_STORAGE_V3);
    let b = bytes();
    let v = decode_device_descriptor_table_v3(&b, &mut free).unwrap();
    assert!(matches!(
        v.kernel(1, &mut free),
        Err(DescriptorWireErrorV3::Index {
            field: "kernel",
            index: 1,
            count: 1
        })
    ));
    assert!(matches!(
        v.requirement(1, &mut free),
        Err(DescriptorWireErrorV3::Index {
            field: "requirement",
            index: 1,
            count: 1
        })
    ));
    let k = v.kernel(0, &mut free).unwrap();
    let a = k.arguments().next(&mut free).unwrap().unwrap();
    assert!(matches!(
        a.component(1, &mut free),
        Err(DescriptorWireErrorV3::Index {
            field: "component",
            index: 1,
            count: 1
        })
    ));
    assert!(matches!(
        v.find_kernel(KernelId::from_bytes([99; 32]), &mut free),
        Err(DescriptorWireErrorV3::Decode(DecodeError::Validation(
            ValidationError::DanglingReference { field: "kernel" }
        )))
    ));
}

#[test]
fn nominal_v3_argument_limit_and_all_declared_table_caps_are_enforced() {
    with_input(
        &vec![SourceTypeDescriptorV3::Usize; MAX_ARGUMENTS_PER_KERNEL],
        |v| {
            let b = encode(&v);
            assert_eq!(
                decode_device_descriptor_table_v3(&b, &mut free)
                    .unwrap()
                    .kernel(0, &mut free)
                    .unwrap()
                    .argument_count(),
                MAX_ARGUMENTS_PER_KERNEL
            );
        },
    );
    with_input(
        &vec![SourceTypeDescriptorV3::Usize; MAX_ARGUMENTS_PER_KERNEL + 1],
        |v| {
            assert!(
                matches!(encoded_device_descriptor_table_v3_len(&v, &mut free), Err(DescriptorWireErrorV3::Decode(DecodeError::CountOutOfRange { field: "kernel arguments", count, max })) if count == (MAX_ARGUMENTS_PER_KERNEL + 1) as u64 && max == MAX_ARGUMENTS_PER_KERNEL)
            );
        },
    );
    let base = bytes();
    let start = types_start(&base);
    for (slot, cap) in [
        (0, MAX_TYPE_RECORDS),
        (1, MAX_LAYOUT_RECORDS),
        (2, MAX_KERNELS),
        (3, MAX_KERNELS),
    ] {
        let mut b = base.clone();
        b[start - 8 + slot * 2..start - 6 + slot * 2]
            .copy_from_slice(&((cap + 1) as u16).to_le_bytes());
        assert!(
            matches!(decode_device_descriptor_table_v3(&b, &mut free), Err(DescriptorWireErrorV3::Decode(DecodeError::CountOutOfRange { count, max, .. })) if count == (cap + 1) as u64 && max == cap)
        );
    }
    assert!(matches!(
        decode_device_descriptor_table_v3(&vec![0; MAX_DESCRIPTOR_TABLE_BYTES + 1], &mut free),
        Err(DescriptorWireErrorV3::Decode(DecodeError::TooLarge {
            max: MAX_DESCRIPTOR_TABLE_BYTES
        }))
    ));
}

#[test]
fn nominal_v3_extra_valid_record_is_not_silently_accepted() {
    with_input(MIXED, |v| {
        let mut types = v.type_records.to_vec();
        types.push(
            SourceTypeRecordV3::new(SourceTypeDescriptorV3::Scalar(ScalarTypeV1::I64), &mut free)
                .unwrap(),
        );
        types.sort_by_key(|r| r.identity());
        let v = DeviceDescriptorTableInputV3 {
            type_records: &types,
            ..v
        };
        assert!(matches!(
            encoded_device_descriptor_table_v3_len(&v, &mut free),
            Err(DescriptorWireErrorV3::Decode(DecodeError::Validation(
                ValidationError::UnreachableRecord { field: "Rust type" }
            )))
        ));
    });
}

#[test]
fn nominal_v3_duplicate_names_and_mismatched_requirements_refuse_before_writes() {
    with_input(MIXED, |input| {
        let old = &input.kernels[0];
        let kernels = [
            KernelDescriptorInputV3 {
                kernel_id: KernelId::from_bytes([1; 32]),
                logical_name: old.logical_name,
                entry_name: old.entry_name,
                descriptor_symbol: old.descriptor_symbol,
                source_evidence: old.source_evidence,
                executable_ir_evidence: old.executable_ir_evidence,
                capabilities: old.capabilities,
                abi_layout: old.abi_layout,
                launch: old.launch,
                arguments: old.arguments,
            },
            KernelDescriptorInputV3 {
                kernel_id: KernelId::from_bytes([2; 32]),
                logical_name: old.logical_name,
                entry_name: old.entry_name,
                descriptor_symbol: old.descriptor_symbol,
                source_evidence: old.source_evidence,
                executable_ir_evidence: old.executable_ir_evidence,
                capabilities: old.capabilities,
                abi_layout: old.abi_layout,
                launch: old.launch,
                arguments: old.arguments,
            },
        ];
        let req = input.requirements[0];
        let requirements = [1, 2].map(|id| {
            KernelTargetRequirementsV2::new(
                KernelId::from_bytes([id; 32]),
                req.lds(),
                req.wavefront_width(),
                req.cooperative_launch(),
                req.synchronization(),
                req.atomics(),
            )
        });
        let input = DeviceDescriptorTableInputV3 {
            kernels: &kernels,
            requirements: &requirements,
            ..input
        };
        let mut output = [0xa5; 16];
        assert!(matches!(
            encode_device_descriptor_table_v3(&input, &mut output, &mut free),
            Err(DescriptorWireErrorV3::Decode(DecodeError::Validation(
                ValidationError::Duplicate {
                    field: "kernel name or symbol"
                }
            )))
        ));
        assert_eq!(output, [0xa5; 16]);
    });
}

#[test]
fn nominal_v3_standalone_hash_queries_charge_exact_domains_and_preserve_errors() {
    for kind in [
        SourceTypeDescriptorV3::Usize,
        SourceTypeDescriptorV3::Isize,
        SourceTypeDescriptorV3::Scalar(ScalarTypeV1::U64),
    ] {
        let domain = if matches!(
            kind,
            SourceTypeDescriptorV3::Usize | SourceTypeDescriptorV3::Isize
        ) {
            b"FE2O3/RUST-TYPE/V3\0"
        } else {
            b"FE2O3/RUST-TYPE/V1\0"
        };
        let mut work = 0;
        SourceTypeRecordV3::new(kind, &mut |n| {
            work += n;
            Ok::<_, usize>(())
        })
        .unwrap();
        assert_eq!(work, domain.len() + 8 + 4);
        assert!(
            matches!(SourceTypeRecordV3::new(kind, &mut |n| Err(n)), Err(DescriptorWireErrorV3::Work(n)) if n == work)
        );
    }
    let b = bytes();
    let v = decode_device_descriptor_table_v3(&b, &mut free).unwrap();
    let mut work = 0;
    let digest = v
        .table_digest(&mut |n| {
            work += n;
            Ok::<_, usize>(())
        })
        .unwrap();
    assert_eq!(
        work,
        b"FE2O3/DEVICE-DESCRIPTOR-TABLE/V3\0".len() + 8 + b.len()
    );
    let mut hash = Sha256::new();
    hash.update(b"FE2O3/DEVICE-DESCRIPTOR-TABLE/V3\0");
    hash.update((b.len() as u64).to_le_bytes());
    hash.update(&b);
    assert_eq!(digest.as_bytes(), &<[u8; 32]>::from(hash.finalize()));
    assert!(
        matches!(v.table_digest(&mut |n| Err(n)), Err(DescriptorWireErrorV3::Work(n)) if n == work)
    );
}

#[test]
fn nominal_v3_literal_mixed_wire_golden_matches_independent_field_grammar() {
    let bytes = bytes();
    assert_eq!(bytes.len(), 1115);
    assert_eq!(
        Sha256::digest(&bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
        "d7e1ecee946926d645aea4e5592829d252097d4813a017159f35486d6e6e87f0"
    );
    let view = decode_device_descriptor_table_v3(&bytes, &mut free).unwrap();
    let digest = view.table_digest(&mut free).unwrap();
    let hex = digest
        .as_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    assert_eq!(
        hex,
        "fa018deddf5aa8c7d9166fdf1e91813c2a70f4ab45fd582c42762e47b6f4e4d1"
    );
}

#[test]
fn nominal_v3_all_fixed_scalars_and_pointer_shapes_round_trip_in_all_covs() {
    let scalars = [
        ScalarTypeV1::I8,
        ScalarTypeV1::U8,
        ScalarTypeV1::I16,
        ScalarTypeV1::U16,
        ScalarTypeV1::I32,
        ScalarTypeV1::U32,
        ScalarTypeV1::I64,
        ScalarTypeV1::U64,
        ScalarTypeV1::F16,
        ScalarTypeV1::F32,
        ScalarTypeV1::F64,
    ];
    for scalar in scalars {
        for kind in [
            SourceTypeDescriptorV3::Scalar(scalar),
            SourceTypeDescriptorV3::SharedSlice(scalar),
            SourceTypeDescriptorV3::DisjointSlice(scalar),
            SourceTypeDescriptorV3::GlobalMutPointer(scalar),
        ] {
            for cov in [
                CodeObjectVersion::V4,
                CodeObjectVersion::V5,
                CodeObjectVersion::V6,
            ] {
                for target in ["gfx942:xnack-", "gfx950:xnack-"] {
                    with_input(
                        &[
                            kind,
                            SourceTypeDescriptorV3::Usize,
                            SourceTypeDescriptorV3::Isize,
                        ],
                        |mut input| {
                            input.code_object_version = cov;
                            input.device_target = DeviceTargetV1::parse(target).unwrap();
                            let bytes = encode(&input);
                            let table =
                                decode_device_descriptor_table_v3(&bytes, &mut free).unwrap();
                            assert_eq!(table.code_object_version(), cov);
                            assert_eq!(table.device_target(), input.device_target);
                            let kernel = table.kernel(0, &mut free).unwrap();
                            let argument = kernel.arguments().next(&mut free).unwrap().unwrap();
                            assert_eq!(
                                table
                                    .source_type(argument.source_type(), &mut free)
                                    .unwrap()
                                    .descriptor(),
                                kind
                            );
                            let component = argument.component(0, &mut free).unwrap();
                            match kind {
                                SourceTypeDescriptorV3::Scalar(_) => {
                                    assert_eq!(
                                        component.kind,
                                        PhysicalAbiComponentKind::ScalarByValue(scalar)
                                    );
                                    assert_eq!(component.size, scalar.size_bytes());
                                    assert_eq!(component.alignment, scalar.alignment_bytes());
                                }
                                _ => assert_eq!(
                                    component.kind,
                                    PhysicalAbiComponentKind::GlobalPointer
                                ),
                            }
                            assert_eq!(kernel.abi_layout(), input.kernels[0].abi_layout);
                        },
                    );
                }
            }
        }
    }
}

#[test]
fn nominal_v3_three_kernel_index_boundaries_and_requirements_are_exact() {
    with_input(MIXED, |input| {
        let old = &input.kernels[0];
        let kernels = [
            (1, "alpha", "alpha.kd"),
            (7, "middle", "middle.kd"),
            (99, "omega", "omega.kd"),
        ]
        .map(|(id, name, symbol)| KernelDescriptorInputV3 {
            kernel_id: KernelId::from_bytes([id; 32]),
            logical_name: name,
            entry_name: name,
            descriptor_symbol: symbol,
            source_evidence: old.source_evidence,
            executable_ir_evidence: old.executable_ir_evidence,
            capabilities: old.capabilities,
            abi_layout: old.abi_layout,
            launch: old.launch,
            arguments: old.arguments,
        });
        let req = input.requirements[0];
        let requirements = [1, 7, 99].map(|id| {
            KernelTargetRequirementsV2::new(
                KernelId::from_bytes([id; 32]),
                req.lds(),
                req.wavefront_width(),
                req.cooperative_launch(),
                req.synchronization(),
                req.atomics(),
            )
        });
        let input = DeviceDescriptorTableInputV3 {
            kernels: &kernels,
            requirements: &requirements,
            ..input
        };
        let bytes = encode(&input);
        let view = decode_device_descriptor_table_v3(&bytes, &mut free).unwrap();
        assert_eq!(view.kernel_count(), 3);
        for (i, expected) in kernels.iter().enumerate() {
            let indexed = view.kernel(i, &mut free).unwrap();
            let found = view.find_kernel(expected.kernel_id, &mut free).unwrap();
            assert_eq!(indexed.kernel_id(), expected.kernel_id);
            assert_eq!(found.entry_name(), expected.entry_name);
            let mut arguments = found.arguments();
            for index in 0..4 {
                assert_eq!(
                    arguments.next(&mut free).unwrap().unwrap().source_index(),
                    index
                );
            }
            assert!(arguments.next(&mut free).unwrap().is_none());
            assert_eq!(
                view.requirement(i, &mut free).unwrap().kernel_id(),
                expected.kernel_id
            );
        }
        for id in [0, 2, 8, 100, 255] {
            assert!(matches!(
                view.find_kernel(KernelId::from_bytes([id; 32]), &mut free),
                Err(DescriptorWireErrorV3::Decode(DecodeError::Validation(
                    ValidationError::DanglingReference { field: "kernel" }
                )))
            ));
        }
    });
}
