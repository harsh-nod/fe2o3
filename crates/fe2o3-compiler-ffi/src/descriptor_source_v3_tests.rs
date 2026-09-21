use super::*;
use fe2o3_kernel_descriptor::*;

const TARGETS: [&str; 2] = ["gfx942:xnack-", "gfx950:xnack-"];
const MIXED: [SourceTypeDescriptorV3; 4] = [
    SourceTypeDescriptorV3::Usize,
    SourceTypeDescriptorV3::Isize,
    SourceTypeDescriptorV3::Scalar(ScalarTypeV1::U64),
    SourceTypeDescriptorV3::Scalar(ScalarTypeV1::I64),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Denied {
    index: usize,
    amount: usize,
}
impl fmt::Display for Denied {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "denied callback {} for {} units",
            self.index, self.amount
        )
    }
}
impl Error for Denied {}
fn free(_: usize) -> Result<(), Denied> {
    Ok(())
}

// This fixture encodes inert bytes only. Its opaque evidence is not a compiler,
// proof-runtime or signed-source fixture and cannot qualify those routes.
fn wire(target: &str, kinds: &[SourceTypeDescriptorV3]) -> Vec<u8> {
    assert!(!kinds.is_empty() && kinds.len() <= MIXED.len());
    let compiler = CompilerIdentityV1::new(
        Text::new("rustc").unwrap(),
        Text::new("nightly").unwrap(),
        [7; 20],
    );
    let producer = ProducerIdentityV1::new(Text::new("fe2o3").unwrap(), Text::new("test").unwrap());
    let argument_sources = kinds
        .iter()
        .map(|kind| SourceTypeRecordV3::new(*kind, &mut free).unwrap())
        .collect::<Vec<_>>();
    let mut sources = argument_sources.clone();
    sources.sort_by_key(|record| record.identity());
    sources.dedup();
    let argument_layouts = kinds
        .iter()
        .map(|kind| {
            device_layout_record_v3(
                DeviceLayoutDescriptorV1::scalar(kind.physical_scalar()),
                &mut free,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let mut layouts = argument_layouts.clone();
    layouts.sort_by_key(|record| record.identity());
    layouts.dedup();
    let components = kinds
        .iter()
        .enumerate()
        .map(|(index, kind)| {
            let scalar = kind.physical_scalar();
            assert_eq!(scalar.size_bytes(), 8);
            [PhysicalComponentV3 {
                kind: PhysicalAbiComponentKind::ScalarByValue(scalar),
                offset: u32::try_from(index * 8).unwrap(),
                size: 8,
                alignment: 8,
                access: AccessMode::ByValue,
                alias: AliasSemantics::Value,
            }]
        })
        .collect::<Vec<_>>();
    let names = ["a", "b", "c", "d"];
    let arguments = kinds
        .iter()
        .enumerate()
        .map(|(index, _)| LogicalArgumentInputV3 {
            source_index: u16::try_from(index).unwrap(),
            name: names[index],
            source_type: argument_sources[index].identity(),
            device_layout: argument_layouts[index].identity(),
            ownership: OwnershipSemantics::ByValue,
            access: AccessMode::ByValue,
            alias: AliasSemantics::Value,
            components: &components[index],
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
    let size = u32::try_from(kinds.len() * 8).unwrap();
    let kernels = [KernelDescriptorInputV3 {
        kernel_id: id,
        logical_name: "kernel",
        entry_name: "kernel",
        descriptor_symbol: "kernel.kd",
        source_evidence: evidence,
        executable_ir_evidence: evidence,
        capabilities: &[CapabilityV1::AmdWave],
        abi_layout: KernelAbiLayoutV1::new(size, size, 8).unwrap(),
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
    let input = DeviceDescriptorTableInputV3 {
        canonical_code_object_digest: CanonicalCodeObjectDigest::from_bytes([0; 32]),
        code_object_version: CodeObjectVersion::V6,
        compiler: &compiler,
        producer: &producer,
        device_target: DeviceTargetV1::parse(target).unwrap(),
        type_records: &sources,
        layout_records: &layouts,
        kernels: &kernels,
        requirements: &requirements,
    };
    let mut bytes = vec![0; encoded_device_descriptor_table_v3_len(&input, &mut free).unwrap()];
    encode_device_descriptor_table_v3(&input, &mut bytes, &mut free).unwrap();
    bytes
}

fn validation_storage(bytes: &Vec<u8>) -> usize {
    compiler_descriptor_source_validation_storage_v3(bytes.capacity()).unwrap()
}
fn own(bytes: Vec<u8>) -> CompilerDescriptorSourceV3 {
    let prepaid = validation_storage(&bytes);
    CompilerDescriptorSourceV3::from_owned_canonical_bytes(bytes, prepaid, &mut free).unwrap()
}
fn table_storage(source: &CompilerDescriptorSourceV3) -> usize {
    compiler_descriptor_source_table_storage_v3(source.canonical_bytes.capacity()).unwrap()
}
fn source_validation_storage(source: &CompilerDescriptorSourceV3) -> usize {
    validation_storage(&source.canonical_bytes)
}
fn reference_identity(bytes: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"FE2O3/COMPILER-DESCRIPTOR-SOURCE/V3\0");
    hash.update(u64::try_from(bytes.len()).unwrap().to_le_bytes());
    hash.update(bytes);
    hash.finalize().into()
}
fn assert_denied(error: CompilerDescriptorSourceErrorV3<Denied>, expected: Denied) {
    match error {
        CompilerDescriptorSourceErrorV3::Work(actual)
        | CompilerDescriptorSourceErrorV3::Wire(DescriptorWireErrorV3::Work(actual)) => {
            assert_eq!(actual, expected);
        }
        other => panic!("unexpected refusal: {other:?}"),
    }
}
fn deny_at(index: usize, seen: &mut Vec<usize>) -> impl FnMut(usize) -> Result<(), Denied> + '_ {
    move |amount| {
        let actual = seen.len();
        seen.push(amount);
        if actual == index {
            Err(Denied { index, amount })
        } else {
            Ok(())
        }
    }
}

#[test]
fn both_profiles_keep_nominal_and_fixed_types_without_authority() {
    for target in TARGETS {
        let bytes = wire(target, &MIXED);
        let pointer = bytes.as_ptr();
        let capacity = bytes.capacity();
        let expected = bytes.clone();
        let source = own(bytes);
        assert_eq!(source.canonical_bytes().as_ptr(), pointer);
        assert_eq!(source.canonical_bytes.capacity(), capacity);
        assert_eq!(source.canonical_bytes(), expected);
        assert_eq!(source.identity().byte_len(), expected.len() as u64);
        assert_eq!(*source.identity().sha256(), reference_identity(&expected));
        assert!(!source.authenticates_compiler_origin());
        assert!(!source.grants_link_authority());
        assert!(!source.grants_load_authority());
        assert!(!source.grants_launch_authority());
        let table = source.table(table_storage(&source), &mut free).unwrap();
        assert_eq!(table.canonical_bytes().as_ptr(), pointer);
        assert_eq!(
            table.device_target(),
            DeviceTargetV1::parse(target).unwrap()
        );
        assert_eq!(table.type_count(), 4);
        assert_eq!(table.layout_count(), 2);
        assert_eq!(table.kernel_count(), 1);
        // This test's query cursor/results have their own caller lifetime. The
        // production table() storage extent does not pay them on behalf of callers.
        let kernel = table.kernel(0, &mut free).unwrap();
        let mut args = kernel.arguments();
        for (index, kind) in MIXED.iter().enumerate() {
            let arg = args.next(&mut free).unwrap().unwrap();
            assert_eq!(usize::from(arg.source_index()), index);
            assert_eq!(
                table
                    .source_type(arg.source_type(), &mut free)
                    .unwrap()
                    .descriptor(),
                *kind
            );
            assert_eq!(
                arg.component(0, &mut free).unwrap().offset,
                (index * 8) as u32
            );
            assert_eq!(
                table
                    .device_layout(arg.device_layout(), &mut free)
                    .unwrap()
                    .identity(),
                device_layout_record_v3(
                    DeviceLayoutDescriptorV1::scalar(kind.physical_scalar()),
                    &mut free,
                )
                .unwrap()
                .identity()
            );
        }
        assert!(args.next(&mut free).unwrap().is_none());
        source
            .revalidate(source_validation_storage(&source), &mut free)
            .unwrap();
    }
}

#[test]
fn nominal_and_fixed_single_arguments_share_layout_not_identity() {
    for target in TARGETS {
        for (nominal, fixed) in [(MIXED[0], MIXED[2]), (MIXED[1], MIXED[3])] {
            let nominal_source = own(wire(target, &[nominal]));
            let fixed_source = own(wire(target, &[fixed]));
            assert_ne!(nominal_source.identity(), fixed_source.identity());
            let nominal_table = nominal_source
                .table(table_storage(&nominal_source), &mut free)
                .unwrap();
            let fixed_table = fixed_source
                .table(table_storage(&fixed_source), &mut free)
                .unwrap();
            let n = nominal_table
                .kernel(0, &mut free)
                .unwrap()
                .arguments()
                .next(&mut free)
                .unwrap()
                .unwrap();
            let f = fixed_table
                .kernel(0, &mut free)
                .unwrap()
                .arguments()
                .next(&mut free)
                .unwrap()
                .unwrap();
            assert_ne!(n.source_type(), f.source_type());
            assert_eq!(n.device_layout(), f.device_layout());
            assert_eq!(
                n.component(0, &mut free).unwrap(),
                f.component(0, &mut free).unwrap()
            );
        }
    }
}

#[test]
fn identity_domain_and_section_cannot_relabel_v1_or_table_digest() {
    assert_eq!(COMPILER_DESCRIPTOR_SECTION_NAME_V3, ".fe2o3.kd.v3");
    assert_ne!(
        COMPILER_DESCRIPTOR_SECTION_NAME_V3,
        crate::COMPILER_DESCRIPTOR_SECTION_NAME_V1
    );
    assert_eq!(
        COMPILER_DESCRIPTOR_SOURCE_DOMAIN_V3,
        b"FE2O3/COMPILER-DESCRIPTOR-SOURCE/V3\0"
    );
    let a = own(wire(TARGETS[0], &MIXED));
    let b = own(wire(TARGETS[1], &MIXED));
    assert_ne!(a.identity(), b.identity());
    for source in [a, b] {
        let raw_digest: [u8; 32] = Sha256::digest(source.canonical_bytes()).into();
        assert_ne!(*source.identity().sha256(), raw_digest);
        assert_eq!(
            *source.identity().sha256(),
            reference_identity(source.canonical_bytes())
        );
        let table = source.table(table_storage(&source), &mut free).unwrap();
        assert_ne!(
            source.identity().sha256(),
            table.table_digest(&mut free).unwrap().as_bytes()
        );
        assert!(crate::CompilerDescriptorSourceV1::decode(source.canonical_bytes()).is_err());
    }
}

#[test]
fn checked_storage_helpers_cover_actual_capacity_and_overflow() {
    for capacity in [0, 1, 1024, MAX_DESCRIPTOR_TABLE_BYTES] {
        let retained = size_of::<CompilerDescriptorSourceV3>() + capacity;
        assert_eq!(
            compiler_descriptor_source_retained_storage_v3(capacity)
                .unwrap()
                .retained_storage(),
            retained
        );
        assert_eq!(
            compiler_descriptor_source_table_storage_v3(capacity),
            Some(
                retained + DESCRIPTOR_TABLE_VIEW_STORAGE_V3 + DESCRIPTOR_READER_SCRATCH_STORAGE_V3
            )
        );
        assert_eq!(
            compiler_descriptor_source_validation_storage_v3(capacity),
            Some(
                retained
                    + DESCRIPTOR_TABLE_VIEW_STORAGE_V3
                    + DESCRIPTOR_READER_SCRATCH_STORAGE_V3
                    + COMPILER_DESCRIPTOR_SOURCE_HASH_STORAGE_V3
            )
        );
    }
    assert_eq!(
        compiler_descriptor_source_retained_storage_v3(usize::MAX),
        None
    );
    assert_eq!(
        compiler_descriptor_source_table_storage_v3(usize::MAX),
        None
    );
    assert_eq!(
        compiler_descriptor_source_validation_storage_v3(usize::MAX),
        None
    );
    assert_eq!(
        compiler_descriptor_source_validation_storage_v3(
            usize::MAX - COMPILER_DESCRIPTOR_SOURCE_HEADER_STORAGE_V3
        ),
        None
    );
    assert!(matches!(
        prepaid::<Denied>(None, usize::MAX),
        Err(CompilerDescriptorSourceErrorV3::Arithmetic)
    ));
}

#[test]
fn construction_preserves_spare_capacity_and_requires_full_prepaid_extent() {
    for target in TARGETS {
        let canonical = wire(target, &[MIXED[0]]);
        for short in [false, true] {
            let mut bytes = Vec::with_capacity(canonical.len() + 257);
            bytes.extend_from_slice(&canonical);
            let pointer = bytes.as_ptr();
            let capacity = bytes.capacity();
            assert!(capacity > canonical.len());
            let required = validation_storage(&bytes);
            let paid = if short { required - 1 } else { required };
            let mut calls = 0;
            let result =
                CompilerDescriptorSourceV3::from_owned_canonical_bytes(bytes, paid, &mut |_| {
                    calls += 1;
                    Ok::<(), Denied>(())
                });
            if short {
                assert!(
                    matches!(result, Err(CompilerDescriptorSourceErrorV3::Storage { required: r, prepaid: p }) if r == required && p == paid)
                );
                assert_eq!(calls, 0);
            } else {
                let source = result.unwrap();
                assert_eq!(source.canonical_bytes().as_ptr(), pointer);
                assert_eq!(source.canonical_bytes.capacity(), capacity);
                assert_eq!(
                    source.storage().retained_storage(),
                    COMPILER_DESCRIPTOR_SOURCE_HEADER_STORAGE_V3 + capacity
                );
                assert!(calls > 0);
            }
        }
        let mut bytes = Vec::with_capacity(canonical.len() + 257);
        bytes.extend_from_slice(&canonical);
        let length_only = compiler_descriptor_source_validation_storage_v3(bytes.len()).unwrap();
        let mut calls = 0;
        let result =
            CompilerDescriptorSourceV3::from_owned_canonical_bytes(bytes, length_only, &mut |_| {
                calls += 1;
                Ok::<(), Denied>(())
            });
        assert!(matches!(
            result,
            Err(CompilerDescriptorSourceErrorV3::Storage { .. })
        ));
        assert_eq!(calls, 0);
    }
}

#[test]
fn table_and_revalidation_storage_refusals_leave_whole_owner_unchanged() {
    for target in TARGETS {
        let source = own(wire(target, &MIXED));
        let before = source.identity();
        let bytes = source.canonical_bytes().to_vec();
        for table in [true, false] {
            let required = if table {
                table_storage(&source)
            } else {
                source_validation_storage(&source)
            };
            let mut calls = 0;
            let mut charge = |_| {
                calls += 1;
                Ok::<(), Denied>(())
            };
            let error = if table {
                source.table(required - 1, &mut charge).err().unwrap()
            } else {
                source.revalidate(required - 1, &mut charge).unwrap_err()
            };
            assert!(
                matches!(error, CompilerDescriptorSourceErrorV3::Storage { required: r, prepaid: p } if r == required && p == required - 1)
            );
            assert_eq!(calls, 0);
            assert_eq!(source.identity(), before);
            assert_eq!(source.canonical_bytes(), bytes);
        }
    }
}

fn decode_error(bytes: Vec<u8>) -> DecodeError {
    let paid = validation_storage(&bytes);
    match CompilerDescriptorSourceV3::from_owned_canonical_bytes(bytes, paid, &mut free)
        .unwrap_err()
    {
        CompilerDescriptorSourceErrorV3::Wire(DescriptorWireErrorV3::Decode(error)) => error,
        other => panic!("expected typed decode failure, got {other:?}"),
    }
}
fn types_start(bytes: &[u8]) -> usize {
    let mut offset = 52;
    for _ in 0..2 {
        let n = u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap()) as usize;
        offset += 2 + n;
    }
    offset += 20;
    for _ in 0..3 {
        let n = u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap()) as usize;
        offset += 2 + n;
    }
    offset + 8
}

#[test]
fn malformed_finalized_noncanonical_and_wrong_version_bytes_are_rejected() {
    for target in TARGETS {
        let canonical = wire(target, &MIXED);
        let mut bytes = canonical.clone();
        bytes[0] ^= 1;
        assert_eq!(decode_error(bytes), DecodeError::InvalidMagic);
        for version in [1u16, 2, 4, u16::MAX] {
            let mut bytes = canonical.clone();
            bytes[8..10].copy_from_slice(&version.to_le_bytes());
            assert_eq!(decode_error(bytes), DecodeError::UnknownVersion(version));
        }
        let mut bytes = canonical.clone();
        bytes[10..12].copy_from_slice(&1u16.to_le_bytes());
        assert_eq!(decode_error(bytes), DecodeError::UnsupportedFlags(1));
        let mut bytes = canonical.clone();
        bytes.push(0);
        assert_eq!(decode_error(bytes), DecodeError::NonCanonical);
        let mut bytes = canonical.clone();
        bytes.push(0);
        let length = u32::try_from(bytes.len()).unwrap();
        bytes[12..16].copy_from_slice(&length.to_le_bytes());
        assert_eq!(decode_error(bytes), DecodeError::TrailingBytes);
        let mut bytes = canonical.clone();
        bytes[CANONICAL_CODE_OBJECT_DIGEST_OFFSET_V3] = 1;
        let paid = validation_storage(&bytes);
        assert!(matches!(
            CompilerDescriptorSourceV3::from_owned_canonical_bytes(bytes, paid, &mut free),
            Err(CompilerDescriptorSourceErrorV3::FinalizedDigest)
        ));
        let mut bytes = canonical;
        let start = types_start(&bytes);
        for index in 0..36 {
            bytes.swap(start + index, start + 36 + index);
        }
        assert!(matches!(
            decode_error(bytes),
            DecodeError::Validation(ValidationError::NonCanonicalOrder { .. })
        ));
    }
    assert_eq!(
        decode_error(vec![0; MAX_DESCRIPTOR_TABLE_BYTES + 1]),
        DecodeError::TooLarge {
            max: MAX_DESCRIPTOR_TABLE_BYTES
        }
    );
}

fn construction_trace(bytes: Vec<u8>) -> Vec<usize> {
    let mut trace = Vec::new();
    let paid = validation_storage(&bytes);
    let _owner =
        CompilerDescriptorSourceV3::from_owned_canonical_bytes(bytes, paid, &mut |amount| {
            trace.push(amount);
            Ok::<(), Denied>(())
        })
        .unwrap();
    trace
}

#[test]
fn every_constructor_callback_refusal_is_typed_and_stops_immediately() {
    for target in TARGETS {
        let bytes = wire(target, &[MIXED[0]]);
        let trace = construction_trace(bytes.clone());
        for (index, amount) in trace.iter().copied().enumerate() {
            let candidate = bytes.clone();
            let paid = validation_storage(&candidate);
            let mut seen = Vec::new();
            let error = CompilerDescriptorSourceV3::from_owned_canonical_bytes(
                candidate,
                paid,
                &mut deny_at(index, &mut seen),
            )
            .unwrap_err();
            assert_denied(error, Denied { index, amount });
            assert_eq!(seen, trace[..=index]);
        }
    }
}

#[test]
fn every_table_and_revalidation_refusal_preserves_owner_and_identity() {
    for target in TARGETS {
        let source = own(wire(target, &[MIXED[1]]));
        let identity = source.identity();
        let bytes = source.canonical_bytes().to_vec();
        for table in [true, false] {
            let paid = if table {
                table_storage(&source)
            } else {
                source_validation_storage(&source)
            };
            let mut trace = Vec::new();
            let mut charge = |amount| {
                trace.push(amount);
                Ok::<(), Denied>(())
            };
            if table {
                let _view = source.table(paid, &mut charge).unwrap();
            } else {
                source.revalidate(paid, &mut charge).unwrap();
            }
            for (index, amount) in trace.iter().copied().enumerate() {
                let mut seen = Vec::new();
                let error = {
                    let mut charge = deny_at(index, &mut seen);
                    if table {
                        source.table(paid, &mut charge).err().unwrap()
                    } else {
                        source.revalidate(paid, &mut charge).unwrap_err()
                    }
                };
                assert_denied(error, Denied { index, amount });
                assert_eq!(seen, trace[..=index]);
                assert_eq!(source.identity(), identity);
                assert_eq!(source.canonical_bytes(), bytes);
            }
        }
        source
            .revalidate(source_validation_storage(&source), &mut free)
            .unwrap();
    }
}

fn query_sequence(
    table: &DeviceDescriptorTableV3<'_>,
    charge: &mut impl FnMut(usize) -> Result<(), Denied>,
) -> Result<(), DescriptorWireErrorV3<Denied>> {
    let kernel = table.kernel(0, charge)?;
    let mut cursor = kernel.arguments();
    let arg = cursor.next(charge)?.unwrap();
    let _source_type = table.source_type(arg.source_type(), charge)?;
    let _device_layout = table.device_layout(arg.device_layout(), charge)?;
    let _component = arg.component(0, charge)?;
    assert!(cursor.next(charge)?.is_none());
    let _requirement = table.requirement(0, charge)?;
    let _digest = table.table_digest(charge)?;
    Ok(())
}

#[test]
fn returned_view_keeps_reader_query_refusals_typed() {
    for target in TARGETS {
        let source = own(wire(target, &[MIXED[0]]));
        let table = source.table(table_storage(&source), &mut free).unwrap();
        let mut trace = Vec::new();
        query_sequence(&table, &mut |amount| {
            trace.push(amount);
            Ok(())
        })
        .unwrap();
        for (index, amount) in trace.iter().copied().enumerate() {
            let mut seen = Vec::new();
            let error = query_sequence(&table, &mut deny_at(index, &mut seen)).unwrap_err();
            assert!(
                matches!(error, DescriptorWireErrorV3::Work(actual) if actual == (Denied { index, amount }))
            );
            assert_eq!(seen, trace[..=index]);
        }
        source
            .revalidate(source_validation_storage(&source), &mut free)
            .unwrap();
    }
}

#[test]
fn work_trace_includes_reader_zero_digest_domain_hash_and_identity_comparison() {
    for target in TARGETS {
        let bytes = wire(target, &MIXED);
        let mut wire_trace = Vec::new();
        {
            let _view = decode_device_descriptor_table_v3(&bytes, &mut |amount| {
                wire_trace.push(amount);
                Ok::<(), Denied>(())
            })
            .unwrap();
        }
        let mut table_trace = vec![1];
        table_trace.extend_from_slice(&wire_trace);
        table_trace.push(32);
        let mut validation_trace = table_trace.clone();
        validation_trace
            .push(b"FE2O3/COMPILER-DESCRIPTOR-SOURCE/V3\0".len() + 8 + bytes.len() + 128);
        assert_eq!(construction_trace(bytes.clone()), validation_trace);
        let source = own(bytes);
        let mut seen = Vec::new();
        let _view = source
            .table(table_storage(&source), &mut |amount| {
                seen.push(amount);
                Ok::<(), Denied>(())
            })
            .unwrap();
        assert_eq!(seen, table_trace);
        seen.clear();
        source
            .revalidate(source_validation_storage(&source), &mut |amount| {
                seen.push(amount);
                Ok::<(), Denied>(())
            })
            .unwrap();
        validation_trace.push(size_of::<CompilerDescriptorSourceIdentityV3>() + 1);
        assert_eq!(seen, validation_trace);
    }
}

#[test]
fn exact_work_budget_succeeds_and_one_less_refuses_before_hash_work() {
    for target in TARGETS {
        let bytes = wire(target, &[MIXED[0]]);
        let trace = construction_trace(bytes.clone());
        let full = trace.iter().sum::<usize>();
        for limit in [full - 1, full] {
            let candidate = bytes.clone();
            let paid = validation_storage(&candidate);
            let mut consumed = 0usize;
            let mut calls = 0;
            let result = CompilerDescriptorSourceV3::from_owned_canonical_bytes(
                candidate,
                paid,
                &mut |amount| {
                    let index = calls;
                    calls += 1;
                    if consumed.checked_add(amount).is_none_or(|sum| sum > limit) {
                        return Err(Denied { index, amount });
                    }
                    consumed += amount;
                    Ok(())
                },
            );
            assert_eq!(calls, trace.len());
            if limit == full {
                assert!(result.is_ok());
                assert_eq!(consumed, full);
            } else {
                assert_denied(
                    result.unwrap_err(),
                    Denied {
                        index: trace.len() - 1,
                        amount: *trace.last().unwrap(),
                    },
                );
                assert_eq!(consumed, full - trace.last().unwrap());
            }
        }
    }
}

#[test]
fn typed_error_sources_keep_work_and_wire_causes() {
    let marker = Denied {
        index: 9,
        amount: 77,
    };
    let direct = CompilerDescriptorSourceErrorV3::Work(marker);
    assert_eq!(
        direct.source().unwrap().downcast_ref::<Denied>(),
        Some(&marker)
    );
    let nested = CompilerDescriptorSourceErrorV3::Wire(DescriptorWireErrorV3::Work(marker));
    let wire = nested
        .source()
        .unwrap()
        .downcast_ref::<DescriptorWireErrorV3<Denied>>()
        .unwrap();
    assert_eq!(
        wire.source().unwrap().downcast_ref::<Denied>(),
        Some(&marker)
    );
    let malformed: CompilerDescriptorSourceErrorV3<Denied> = CompilerDescriptorSourceErrorV3::Wire(
        DescriptorWireErrorV3::Decode(DecodeError::InvalidMagic),
    );
    assert_eq!(
        malformed
            .source()
            .unwrap()
            .source()
            .unwrap()
            .downcast_ref::<DecodeError>(),
        Some(&DecodeError::InvalidMagic)
    );
    for error in [
        CompilerDescriptorSourceErrorV3::<Denied>::Arithmetic,
        CompilerDescriptorSourceErrorV3::FinalizedDigest,
        CompilerDescriptorSourceErrorV3::IdentityMismatch,
        CompilerDescriptorSourceErrorV3::Storage {
            required: 9,
            prepaid: 8,
        },
    ] {
        assert!(error.source().is_none());
        assert!(!error.to_string().is_empty());
    }
}

#[test]
fn revalidate_does_not_trust_cached_identity_or_private_corruption() {
    let mut source = own(wire(TARGETS[0], &MIXED));
    source.identity.sha256[0] ^= 1;
    assert!(matches!(
        source.revalidate(source_validation_storage(&source), &mut free),
        Err(CompilerDescriptorSourceErrorV3::IdentityMismatch)
    ));
    let mut source = own(wire(TARGETS[0], &MIXED));
    source.canonical_bytes[CANONICAL_CODE_OBJECT_DIGEST_OFFSET_V3] = 1;
    assert!(matches!(
        source.table(table_storage(&source), &mut free),
        Err(CompilerDescriptorSourceErrorV3::FinalizedDigest)
    ));
    assert!(matches!(
        source.revalidate(source_validation_storage(&source), &mut free),
        Err(CompilerDescriptorSourceErrorV3::FinalizedDigest)
    ));
}
