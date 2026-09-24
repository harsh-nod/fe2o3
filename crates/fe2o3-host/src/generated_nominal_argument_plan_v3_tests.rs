use super::*;
use fe2o3_kernel_descriptor::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::mem::size_of_val;

#[path = "generated_nominal_argument_plan_v3_lifetime_tests.rs"]
mod lifetime_tests;

#[derive(Clone, Copy)]
enum Shape {
    Usize,
    Isize,
    Scalar(ScalarTypeV1),
    Read(ScalarTypeV1),
    Write(ScalarTypeV1),
    ReadWrite(ScalarTypeV1),
    Raw,
}
impl Shape {
    fn source(self) -> SourceTypeDescriptorV3 {
        match self {
            Self::Usize => SourceTypeDescriptorV3::Usize,
            Self::Isize => SourceTypeDescriptorV3::Isize,
            Self::Scalar(s) => SourceTypeDescriptorV3::Scalar(s),
            Self::Read(s) => SourceTypeDescriptorV3::SharedSlice(s),
            Self::Write(s) | Self::ReadWrite(s) => SourceTypeDescriptorV3::DisjointSlice(s),
            Self::Raw => SourceTypeDescriptorV3::GlobalMutPointer(ScalarTypeV1::U32),
        }
    }
}
fn free(_: usize) -> std::result::Result<(), Resource> {
    Ok(())
}
fn fixture(shapes: &[Shape], target: &str, padding: bool) -> Vec<u8> {
    fixture_with_segment(shapes, target, padding, None)
}
fn fixture_with_segment(
    shapes: &[Shape],
    target: &str,
    padding: bool,
    segment: Option<u32>,
) -> Vec<u8> {
    let compiler = CompilerIdentityV1::new(
        Text::new("rustc").unwrap(),
        Text::new("nominal-host-test").unwrap(),
        [7; 20],
    );
    let producer = ProducerIdentityV1::new(
        Text::new("fe2o3").unwrap(),
        Text::new("nominal-host-test").unwrap(),
    );
    let mut types = shapes
        .iter()
        .map(|s| SourceTypeRecordV3::new(s.source(), &mut free).unwrap())
        .collect::<Vec<_>>();
    types.sort_by_key(|r| r.identity());
    types.dedup();
    let layouts = shapes
        .iter()
        .map(|s| {
            let d = match *s {
                Shape::Usize => DeviceLayoutDescriptorV1::scalar(ScalarTypeV1::U64),
                Shape::Isize => DeviceLayoutDescriptorV1::scalar(ScalarTypeV1::I64),
                Shape::Scalar(s) => DeviceLayoutDescriptorV1::scalar(s),
                Shape::Read(s) => DeviceLayoutDescriptorV1::shared_slice(s),
                Shape::Write(s) | Shape::ReadWrite(s) => {
                    DeviceLayoutDescriptorV1::disjoint_slice(s)
                }
                Shape::Raw => DeviceLayoutDescriptorV1::global_mut_pointer(ScalarTypeV1::U32),
            };
            device_layout_record_v3(d, &mut free).unwrap()
        })
        .collect::<Vec<_>>();
    let mut unique_layouts = layouts.clone();
    unique_layouts.sort_by_key(|r| r.identity());
    unique_layouts.dedup();
    let names = (0..shapes.len())
        .map(|n| format!("arg_{n}"))
        .collect::<Vec<_>>();
    let mut end = 0u32;
    let mut max_align = 1u32;
    let components = shapes
        .iter()
        .map(|s| {
            let (kind, size, align, access, alias) = match *s {
                Shape::Scalar(s) => (
                    PhysicalAbiComponentKind::ScalarByValue(s),
                    s.size_bytes(),
                    s.alignment_bytes(),
                    AccessMode::ByValue,
                    AliasSemantics::Value,
                ),
                Shape::Usize => (
                    PhysicalAbiComponentKind::ScalarByValue(ScalarTypeV1::U64),
                    8,
                    8,
                    AccessMode::ByValue,
                    AliasSemantics::Value,
                ),
                Shape::Isize => (
                    PhysicalAbiComponentKind::ScalarByValue(ScalarTypeV1::I64),
                    8,
                    8,
                    AccessMode::ByValue,
                    AliasSemantics::Value,
                ),
                Shape::Read(_) => (
                    PhysicalAbiComponentKind::GlobalPointer,
                    8,
                    8,
                    AccessMode::ReadOnly,
                    AliasSemantics::SharedReadOnly,
                ),
                Shape::Write(_) => (
                    PhysicalAbiComponentKind::GlobalPointer,
                    8,
                    8,
                    AccessMode::WriteOnly,
                    AliasSemantics::Exclusive,
                ),
                Shape::ReadWrite(_) | Shape::Raw => (
                    PhysicalAbiComponentKind::GlobalPointer,
                    8,
                    8,
                    AccessMode::ReadWrite,
                    AliasSemantics::Exclusive,
                ),
            };
            max_align = max_align.max(u32::from(align));
            if padding {
                end += u32::from(align);
            }
            end = (end + u32::from(align) - 1) & !(u32::from(align) - 1);
            let first = PhysicalComponentV3 {
                kind,
                offset: end,
                size,
                alignment: align,
                access,
                alias,
            };
            end += u32::from(size);
            let mut values = vec![first];
            if matches!(s, Shape::Read(_) | Shape::Write(_) | Shape::ReadWrite(_)) {
                values.push(PhysicalComponentV3 {
                    kind: PhysicalAbiComponentKind::SliceLengthU64,
                    offset: end,
                    size: 8,
                    alignment: 8,
                    access: AccessMode::ByValue,
                    alias: AliasSemantics::Value,
                });
                end += 8;
            }
            values
        })
        .collect::<Vec<_>>();
    let explicit = (end + max_align - 1) & !(max_align - 1);
    let arguments = shapes
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let (ownership, access, alias) = match *s {
                Shape::Read(_) => (
                    OwnershipSemantics::SharedBorrow,
                    AccessMode::ReadOnly,
                    AliasSemantics::SharedReadOnly,
                ),
                Shape::Write(_) => (
                    OwnershipSemantics::UniqueBorrow,
                    AccessMode::WriteOnly,
                    AliasSemantics::Exclusive,
                ),
                Shape::ReadWrite(_) | Shape::Raw => (
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
                source_type: SourceTypeRecordV3::new(s.source(), &mut free)
                    .unwrap()
                    .identity(),
                device_layout: layouts[i].identity(),
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
    let id = KernelId::from_bytes([13; 32]);
    let evidence = BuildEvidenceV1::new(
        EvidenceIdentity::from_opaque_bytes([11; 32]),
        EvidenceDigest::from_sha256_bytes([12; 32]),
    );
    let kernels = [KernelDescriptorInputV3 {
        kernel_id: id,
        logical_name: "kernel",
        entry_name: "kernel",
        descriptor_symbol: "kernel.kd",
        source_evidence: evidence,
        executable_ir_evidence: evidence,
        capabilities: &[CapabilityV1::AmdWave],
        abi_layout: KernelAbiLayoutV1::new(
            explicit,
            segment.unwrap_or(explicit + if padding { max_align * 2 } else { 0 }),
            max_align,
        )
        .unwrap(),
        launch: &launch,
        arguments: &arguments,
    }];
    let requirements = [KernelTargetRequirementsV2::new(
        id,
        LdsRequirementsV2::new(0, 0).unwrap(),
        RequiredWavefrontWidthV2::Wave64,
        false,
        SynchronizationRequirementsV2::from_bits(0).unwrap(),
        AtomicRequirementsV2::from_bits(0).unwrap(),
    )];
    let input = DeviceDescriptorTableInputV3 {
        canonical_code_object_digest: CanonicalCodeObjectDigest::from_bytes([0; 32]),
        code_object_version: CodeObjectVersion::V6,
        compiler: &compiler,
        producer: &producer,
        device_target: DeviceTargetV1::parse(target).unwrap(),
        type_records: &types,
        layout_records: &unique_layouts,
        kernels: &kernels,
        requirements: &requirements,
    };
    let n = encoded_device_descriptor_table_v3_len(&input, &mut free).unwrap();
    let mut bytes = vec![0; n];
    encode_device_descriptor_table_v3(&input, &mut bytes, &mut free).unwrap();
    bytes
}
fn with_plan(
    shapes: &[Shape],
    target: &str,
    padding: bool,
    call: impl FnOnce(&GeneratedNominalArgumentPlanV3<'_, '_>, &mut Budget<'_>),
) {
    let wire = fixture(shapes, target, padding);
    let sibling = [0x53u8; 19];
    let floor = size_of_val(&wire) + wire.capacity() + size_of_val(&sibling);
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget
        .reserve_storage(
            floor + DESCRIPTOR_TABLE_VIEW_STORAGE_V3 + DESCRIPTOR_READER_SCRATCH_STORAGE_V3,
        )
        .unwrap();
    let ledger = budget.work_ledger_identity_v1();
    {
        let view =
            decode_device_descriptor_table_v3(&wire, &mut |n| budget.charge_work(n)).unwrap();
        budget
            .release_storage(DESCRIPTOR_READER_SCRATCH_STORAGE_V3)
            .unwrap();
        {
            let (plan, receipt) = GeneratedNominalArgumentPlanV3::new(
                &view,
                KernelId::from_bytes([13; 32]),
                &mut budget,
            )
            .unwrap();
            assert_eq!(receipt.retained_storage(), NOMINAL_ARGUMENT_PLAN_STORAGE_V3);
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            assert_eq!(budget.storage(), plan.retained_storage_floor());
            assert!(!plan.grants_launch_authority());
            let live = budget.storage();
            call(&plan, &mut budget);
            assert_eq!(budget.storage(), live);
        }
        budget
            .release_storage(NOMINAL_ARGUMENT_PLAN_STORAGE_V3)
            .unwrap();
    }
    budget
        .release_storage(DESCRIPTOR_TABLE_VIEW_STORAGE_V3)
        .unwrap();
    assert_eq!(budget.storage(), floor);
    assert_eq!(sibling, [0x53; 19]);
    assert!(budget.work_ledger_identity_v1() == ledger);
}
fn reserve(budget: &mut Budget<'_>, receipt: GeneratedNominalStorageV3) {
    budget.reserve_storage(receipt.retained_storage()).unwrap();
}

#[test]
fn nominal_host_v3_unsigned_signed_boundaries_and_padding_are_exact() {
    for target in ["gfx942:xnack-", "gfx950:xnack-"] {
        for unsigned in [0usize, 1, usize::MAX] {
            for signed in [isize::MIN, -1, 0, isize::MAX] {
                with_plan(
                    &[Shape::Usize, Shape::Isize],
                    target,
                    true,
                    |plan, budget| {
                        let floor = budget.storage();
                        {
                            let (u, r) = plan.bind_usize(0, unsigned, budget).unwrap();
                            reserve(budget, r);
                            let (i, r) = plan.bind_isize(1, signed, budget).unwrap();
                            reserve(budget, r);
                            assert_ne!(u.nominal_type_identity(), i.nominal_type_identity());
                            assert_eq!(
                                u.nominal_type_identity(),
                                Some(
                                    RustNominalScalarEvidenceV3::new(
                                        RustNominalScalarKindV3::Usize,
                                        PointerWidth::Bits64
                                    )
                                    .unwrap()
                                    .type_identity()
                                )
                            );
                            budget
                                .reserve_storage(NOMINAL_ARGUMENT_REF_STORAGE_V3 * 2)
                                .unwrap();
                            let inputs = [i.as_input(), u.as_input()];
                            let mut bytes = vec![0xa5; plan.output_len()];
                            budget
                                .reserve_storage(size_of_val(&bytes) + bytes.capacity())
                                .unwrap();
                            let expected_floor = budget.storage();
                            let receipt;
                            {
                                let (packed, r) = plan.pack(&inputs, &mut bytes, budget).unwrap();
                                reserve(budget, r);
                                receipt = r;
                                assert!(!packed.grants_launch_authority());
                                assert_eq!(packed.kernel_id(), plan.kernel_id());
                                assert_eq!(
                                    packed.retained_storage_floor(),
                                    expected_floor + r.retained_storage()
                                );
                                let mut expected = vec![0; plan.output_len()];
                                let a = plan.row(0).unwrap().components[0].offset as usize;
                                let b = plan.row(1).unwrap().components[0].offset as usize;
                                expected[a..a + 8]
                                    .copy_from_slice(&(unsigned as u64).to_le_bytes());
                                expected[b..b + 8].copy_from_slice(&(signed as i64).to_le_bytes());
                                assert_eq!(packed.as_bytes(), expected);
                                assert_eq!(packed.required_alignment(), 8);
                            }
                            budget.release_storage(receipt.retained_storage()).unwrap();
                        }
                        budget.release_storage(budget.storage() - floor).unwrap();
                    },
                );
            }
        }
    }
}

#[test]
fn nominal_host_v3_mixed_actual_slice_borrows_use_zero_pointers_and_real_lengths() {
    for target in ["gfx942:xnack-", "gfx950:xnack-"] {
        for n in [0, 3] {
            with_plan(
                &[
                    Shape::Usize,
                    Shape::Isize,
                    Shape::Scalar(ScalarTypeV1::U32),
                    Shape::Read(ScalarTypeV1::I16),
                    Shape::Write(ScalarTypeV1::U8),
                    Shape::ReadWrite(ScalarTypeV1::F64),
                ],
                target,
                true,
                |plan, budget| {
                    let floor = budget.storage();
                    {
                        let read = [-1i16; 3];
                        let mut write = [7u8; 3];
                        let mut rw = [1.5f64; 3];
                        budget
                            .reserve_storage(
                                size_of_val(&read) + size_of_val(&write) + size_of_val(&rw),
                            )
                            .unwrap();
                        let (u, r) = plan.bind_usize(0, usize::MAX, budget).unwrap();
                        reserve(budget, r);
                        let (i, r) = plan.bind_isize(1, -1, budget).unwrap();
                        reserve(budget, r);
                        let (s, r) = plan.bind_scalar(2, 0x12345678u32, budget).unwrap();
                        reserve(budget, r);
                        let (a, r) = plan
                            .bind_read_slice(3, GeneratedKfdReadSlice::new(&read[..n]), budget)
                            .unwrap();
                        reserve(budget, r);
                        let (b, r) = plan
                            .bind_write_slice(
                                4,
                                GeneratedKfdWriteSlice::new(&mut write[..n]),
                                budget,
                            )
                            .unwrap();
                        reserve(budget, r);
                        let (c, r) = plan
                            .bind_read_write_slice(
                                5,
                                GeneratedKfdReadWriteSlice::new(&mut rw[..n]),
                                budget,
                            )
                            .unwrap();
                        reserve(budget, r);
                        budget
                            .reserve_storage(NOMINAL_ARGUMENT_REF_STORAGE_V3 * 6)
                            .unwrap();
                        let inputs = [
                            c.as_input(),
                            a.as_input(),
                            i.as_input(),
                            b.as_input(),
                            s.as_input(),
                            u.as_input(),
                        ];
                        let mut bytes = vec![0xa5; plan.output_len()];
                        budget
                            .reserve_storage(size_of_val(&bytes) + bytes.capacity())
                            .unwrap();
                        let receipt;
                        {
                            let (packed, r) = plan.pack(&inputs, &mut bytes, budget).unwrap();
                            reserve(budget, r);
                            receipt = r;
                            let mut expected = vec![0; plan.output_len()];
                            for (index, value) in [(0, usize::MAX as u64), (1, u64::MAX)] {
                                let start = plan.row(index).unwrap().components[0].offset as usize;
                                expected[start..start + 8].copy_from_slice(&value.to_le_bytes());
                            }
                            let start = plan.row(2).unwrap().components[0].offset as usize;
                            expected[start..start + 4]
                                .copy_from_slice(&0x12345678u32.to_le_bytes());
                            for index in 3..6 {
                                let start = plan.row(index).unwrap().components[1].offset as usize;
                                expected[start..start + 8]
                                    .copy_from_slice(&(n as u64).to_le_bytes());
                            }
                            assert_eq!(packed.as_bytes(), expected);
                        }
                        budget.release_storage(receipt.retained_storage()).unwrap();
                    }
                    budget.release_storage(budget.storage() - floor).unwrap();
                },
            );
        }
    }
}

#[test]
fn nominal_host_v3_all_existing_fixed_scalars_keep_exact_tags_and_bytes() {
    macro_rules! check {
        ($kind:ident,$value:expr,$wrong:expr) => {
            with_plan(
                &[Shape::Scalar(ScalarTypeV1::$kind)],
                "gfx942:xnack-",
                true,
                |plan, budget| {
                    let floor = budget.storage();
                    assert!(matches!(
                        plan.bind_scalar(0, $wrong, budget).err().unwrap(),
                        GeneratedNominalPackErrorV3::Argument {
                            reason: "exact source kind/ownership/access",
                            ..
                        }
                    ));
                    {
                        let value = $value;
                        let (binding, r) = plan.bind_scalar(0, value, budget).unwrap();
                        reserve(budget, r);
                        assert_eq!(binding.nominal_type_identity(), None);
                        budget
                            .reserve_storage(NOMINAL_ARGUMENT_REF_STORAGE_V3)
                            .unwrap();
                        let inputs = [binding.as_input()];
                        let mut bytes = vec![0xa5; plan.output_len()];
                        budget
                            .reserve_storage(size_of_val(&bytes) + bytes.capacity())
                            .unwrap();
                        let receipt;
                        {
                            let (packed, r) = plan.pack(&inputs, &mut bytes, budget).unwrap();
                            reserve(budget, r);
                            receipt = r;
                            let mut expected = vec![0; plan.output_len()];
                            let start = plan.row(0).unwrap().components[0].offset as usize;
                            let encoded = value.to_le_bytes();
                            expected[start..start + encoded.len()].copy_from_slice(&encoded);
                            assert_eq!(packed.as_bytes(), expected);
                        }
                        budget.release_storage(receipt.retained_storage()).unwrap();
                    }
                    budget.release_storage(budget.storage() - floor).unwrap();
                },
            );
        };
    }
    check!(I8, -7i8, 7u8);
    check!(U8, 251u8, -1i8);
    check!(I16, -123i16, 123u16);
    check!(U16, 65530u16, -1i16);
    check!(I32, -123456i32, 1u32);
    check!(U32, 0xf1234567u32, -1i32);
    check!(I64, i64::MIN, u64::MAX);
    check!(U64, u64::MAX, -1i64);
    check!(F32, -1.25f32, 0u32);
    check!(F64, -2.5f64, 0u64);
}

#[test]
fn nominal_host_v3_nominal_and_fixed_width_substitutions_are_refused() {
    with_plan(
        &[
            Shape::Usize,
            Shape::Isize,
            Shape::Scalar(ScalarTypeV1::U64),
            Shape::Scalar(ScalarTypeV1::I64),
        ],
        "gfx942:xnack-",
        false,
        |plan, budget| {
            let floor = budget.storage();
            assert!(plan.bind_scalar(0, 0u64, budget).is_err());
            assert!(plan.bind_scalar(1, 0i64, budget).is_err());
            assert!(plan.bind_usize(1, 0, budget).is_err());
            assert!(plan.bind_isize(0, 0, budget).is_err());
            assert!(plan.bind_usize(2, 0, budget).is_err());
            assert!(plan.bind_isize(3, 0, budget).is_err());
            assert_eq!(budget.storage(), floor);
        },
    );
}

#[test]
fn nominal_host_v3_slice_element_access_and_ownership_are_exact() {
    with_plan(
        &[
            Shape::Read(ScalarTypeV1::U32),
            Shape::Write(ScalarTypeV1::U32),
            Shape::ReadWrite(ScalarTypeV1::U32),
        ],
        "gfx950:xnack-",
        false,
        |plan, budget| {
            let floor = budget.storage();
            let read = [0u32; 2];
            let wrong = [0i32; 2];
            let mut output = [0u32; 2];
            assert!(
                plan.bind_read_slice(0, GeneratedKfdReadSlice::new(&wrong), budget)
                    .is_err()
            );
            assert!(
                plan.bind_read_slice(1, GeneratedKfdReadSlice::new(&read), budget)
                    .is_err()
            );
            assert!(
                plan.bind_read_slice(2, GeneratedKfdReadSlice::new(&read), budget)
                    .is_err()
            );
            assert!(
                plan.bind_write_slice(0, GeneratedKfdWriteSlice::new(&mut output), budget)
                    .is_err()
            );
            assert!(
                plan.bind_write_slice(2, GeneratedKfdWriteSlice::new(&mut output), budget)
                    .is_err()
            );
            assert!(
                plan.bind_read_write_slice(1, GeneratedKfdReadWriteSlice::new(&mut output), budget)
                    .is_err()
            );
            assert_eq!(budget.storage(), floor);
        },
    );
}

#[test]
fn nominal_host_v3_equal_content_tables_and_distinct_plans_are_foreign() {
    with_plan(&[Shape::Usize], "gfx942:xnack-", false, |first, budget| {
        let floor = budget.storage();
        {
            let (same, r) =
                GeneratedNominalArgumentPlanV3::new(first.table, first.kernel, budget).unwrap();
            reserve(budget, r);
            let (binding, r) = same.bind_usize(0, 1, budget).unwrap();
            reserve(budget, r);
            budget
                .reserve_storage(NOMINAL_ARGUMENT_REF_STORAGE_V3 + 8)
                .unwrap();
            let inputs = [binding.as_input()];
            let mut bytes = [0xa5; 8];
            assert!(matches!(
                first.pack(&inputs, &mut bytes, budget).err().unwrap(),
                GeneratedNominalPackErrorV3::Argument {
                    reason: "foreign actual plan",
                    ..
                }
            ));
            assert_eq!(bytes, [0xa5; 8]);
        }
        budget.release_storage(budget.storage() - floor).unwrap();
        with_plan(&[Shape::Usize], "gfx942:xnack-", false, |second, other| {
            assert_eq!(
                first.table.canonical_bytes(),
                second.table.canonical_bytes()
            );
            let credit;
            {
                let (binding, r) = second.bind_usize(0, 1, other).unwrap();
                reserve(other, r);
                credit = r.retained_storage();
                let original = budget.storage();
                budget
                    .reserve_storage(r.retained_storage() + NOMINAL_ARGUMENT_REF_STORAGE_V3 + 8)
                    .unwrap();
                {
                    let inputs = [binding.as_input()];
                    let mut bytes = [0xa5; 8];
                    assert!(matches!(
                        first.pack(&inputs, &mut bytes, budget).err().unwrap(),
                        GeneratedNominalPackErrorV3::Argument {
                            reason: "foreign actual plan",
                            ..
                        }
                    ));
                    assert_eq!(bytes, [0xa5; 8]);
                }
                budget.release_storage(budget.storage() - original).unwrap();
            }
            // The external reference is gone before the binding's addition is refunded.
            other.release_storage(credit).unwrap();
        });
    });
}

#[test]
fn nominal_host_v3_counts_duplicate_indices_and_last_identity_denial_are_atomic() {
    with_plan(
        &[Shape::Usize, Shape::Isize],
        "gfx942:xnack-",
        false,
        |plan, budget| {
            let floor = budget.storage();
            {
                let (a, r) = plan.bind_usize(0, 1, budget).unwrap();
                reserve(budget, r);
                let (mut b, r) = plan.bind_isize(1, -1, budget).unwrap();
                reserve(budget, r);
                budget
                    .reserve_storage(NOMINAL_ARGUMENT_REF_STORAGE_V3 * 2 + 16)
                    .unwrap();
                let mut bytes = [0xa5; 16];
                assert!(matches!(
                    plan.pack(&[], &mut bytes, budget).err().unwrap(),
                    GeneratedNominalPackErrorV3::Count {
                        expected: 2,
                        actual: 0
                    }
                ));
                assert_eq!(bytes, [0xa5; 16]);
                {
                    let inputs = [a.as_input(), a.as_input()];
                    assert!(matches!(
                        plan.pack(&inputs, &mut bytes, budget).err().unwrap(),
                        GeneratedNominalPackErrorV3::Argument {
                            reason: "duplicate argument",
                            ..
                        }
                    ));
                    assert_eq!(bytes, [0xa5; 16]);
                }
                let saved = b.core.row;
                for field in 0..3 {
                    b.core.row = saved;
                    match field {
                        0 => b.core.row.source = RustTypeIdentity::from_bytes([0; 32]),
                        1 => b.core.row.layout = DeviceLayoutIdentity::from_bytes([0; 32]),
                        _ => b.core.row.components[0].offset += 8,
                    }
                    let inputs = [a.as_input(), b.as_input()];
                    assert!(matches!(
                        plan.pack(&inputs, &mut bytes, budget).err().unwrap(),
                        GeneratedNominalPackErrorV3::Argument {
                            index: 1,
                            reason: "source/layout/component identity"
                        }
                    ));
                    assert_eq!(bytes, [0xa5; 16]);
                }
                b.core.row = saved;
                b.core.nominal = a.core.nominal;
                let inputs = [a.as_input(), b.as_input()];
                assert!(matches!(
                    plan.pack(&inputs, &mut bytes, budget).err().unwrap(),
                    GeneratedNominalPackErrorV3::Argument {
                        index: 1,
                        reason: "nominal declaration identity"
                    }
                ));
                assert_eq!(bytes, [0xa5; 16]);
            }
            budget.release_storage(budget.storage() - floor).unwrap();
        },
    );
}

#[test]
fn nominal_host_v3_argument_bounds_output_extents_and_empty_kernel_are_closed() {
    with_plan(&[Shape::Usize], "gfx942:xnack-", false, |plan, budget| {
        let floor = budget.storage();
        assert!(matches!(
            plan.bind_usize(1, 0, budget).err().unwrap(),
            GeneratedNominalPackErrorV3::Argument {
                index: 1,
                reason: "argument index"
            }
        ));
        {
            let (a, r) = plan.bind_usize(0, 1, budget).unwrap();
            reserve(budget, r);
            budget
                .reserve_storage(NOMINAL_ARGUMENT_REF_STORAGE_V3 + 9)
                .unwrap();
            let inputs = [a.as_input()];
            for length in [0, 7, 9] {
                let mut bytes = vec![0xa5; length];
                assert!(
                    matches!(plan.pack(&inputs,&mut bytes,budget).err().unwrap(),GeneratedNominalPackErrorV3::OutputLength{expected:8,actual} if actual==length)
                );
                assert_eq!(bytes, vec![0xa5; length]);
            }
        }
        budget.release_storage(budget.storage() - floor).unwrap();
    });
    with_plan(&[], "gfx942:xnack-", false, |plan, budget| {
        let receipt;
        {
            let mut bytes = [];
            let (packed, r) = plan.pack(&[], &mut bytes, budget).unwrap();
            reserve(budget, r);
            receipt = r;
            assert!(packed.as_bytes().is_empty());
        }
        budget.release_storage(receipt.retained_storage()).unwrap();
    });
}

#[test]
fn nominal_host_v3_invalid_wire_width_alignment_and_overlap_fail_descriptor_admission() {
    let bytes = fixture(&[Shape::Usize, Shape::Isize], "gfx942:xnack-", false);
    let mut width = bytes.clone();
    width[49] = 4;
    assert!(decode_device_descriptor_table_v3(&width, &mut free).is_err());
    assert!(matches!(
        host_width(4),
        Err(GeneratedNominalPackErrorV3::HostWidth { actual: 4 })
    ));
    let second = [1, 7, 1, 1, 8, 0, 0, 0, 8, 0, 8, 0, 0, 0, 0, 0];
    let positions = bytes
        .windows(16)
        .enumerate()
        .filter_map(|(i, w)| (w == second).then_some(i))
        .collect::<Vec<_>>();
    assert_eq!(positions.len(), 1);
    for (offset, value) in [(4, 0), (8, 4), (10, 3), (1, 8)] {
        let mut invalid = bytes.clone();
        invalid[positions[0] + offset] = value;
        assert!(decode_device_descriptor_table_v3(&invalid, &mut free).is_err());
    }
}

#[test]
fn nominal_host_v3_valid_raw_pointer_has_no_inert_binding_route() {
    let wire = fixture(&[Shape::Raw], "gfx942:xnack-", false);
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget
        .reserve_storage(
            size_of_val(&wire)
                + wire.capacity()
                + DESCRIPTOR_TABLE_VIEW_STORAGE_V3
                + DESCRIPTOR_READER_SCRATCH_STORAGE_V3,
        )
        .unwrap();
    let view = decode_device_descriptor_table_v3(&wire, &mut |n| budget.charge_work(n)).unwrap();
    budget
        .release_storage(DESCRIPTOR_READER_SCRATCH_STORAGE_V3)
        .unwrap();
    let floor = budget.storage();
    assert!(matches!(
        GeneratedNominalArgumentPlanV3::new(&view, KernelId::from_bytes([13; 32]), &mut budget)
            .err()
            .unwrap(),
        GeneratedNominalPackErrorV3::Argument {
            index: 0,
            reason: "raw pointer not supported"
        }
    ));
    assert_eq!(budget.storage(), floor);
}

#[derive(Clone, Copy)]
enum Phase {
    Plan,
    Bind,
    Pack,
}
struct Observation {
    error: Option<GeneratedNominalPackErrorV3>,
    floor: usize,
    work: usize,
    peak: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
    header: usize,
    scratch: usize,
}
fn measured(phase: Phase, w: usize, p: usize) -> Observation {
    let mut observed = None;
    with_plan(&[Shape::Usize], "gfx942:xnack-", false, |plan, parent| {
        let original = parent.storage();
        {
            let (binding, r) = plan.bind_usize(0, 7, parent).unwrap();
            reserve(parent, r);
            parent
                .reserve_storage(NOMINAL_ARGUMENT_REF_STORAGE_V3 + 8)
                .unwrap();
            let inputs = [binding.as_input()];
            let mut output = [0xa5; 8];
            let floor = parent.storage();
            let mut work = Work::new(w);
            let (error, accepted, peak, failed_storage, header, scratch) = {
                let mut budget = Budget::new(&mut work, p);
                budget.reserve_storage(floor).unwrap();
                budget.charge_work(17).unwrap();
                let ledger = budget.work_ledger_identity_v1();
                let (result, header, scratch) = match phase {
                    Phase::Plan => (
                        GeneratedNominalArgumentPlanV3::new(plan.table, plan.kernel, &mut budget)
                            .map(|(_v, r)| r),
                        NOMINAL_ARGUMENT_PLAN_STORAGE_V3,
                        DESCRIPTOR_QUERY_STORAGE_V3
                            + size_of::<KernelDescriptorRefV3<'_, '_>>()
                            + size_of::<ArgumentCursorV3<'_, '_>>()
                            + size_of::<LogicalArgumentRefV3<'_, '_>>()
                            + size_of::<SourceTypeRecordV3>()
                            + size_of::<DeviceLayoutRecordV1>()
                            + size_of::<DeviceLayoutDescriptorV1>()
                            + size_of::<[PhysicalComponentV3; 2]>()
                            + size_of::<PhysicalComponentV3>()
                            + size_of::<Argument>(),
                    ),
                    Phase::Bind => (
                        plan.bind_usize(0, 7, &mut budget).map(|(_v, r)| r),
                        size_of::<GeneratedNominalBindingV3<'_, '_, '_, usize>>(),
                        NOMINAL_BIND_SCRATCH_STORAGE_V3,
                    ),
                    Phase::Pack => (
                        plan.pack(&inputs, &mut output, &mut budget)
                            .map(|(_v, r)| r),
                        size_of::<GeneratedNominalPackedArgumentsV3<'_, '_, '_, '_, '_>>(),
                        NOMINAL_PACK_SCRATCH_STORAGE_V3,
                    ),
                };
                assert_eq!(budget.storage(), floor);
                assert!(budget.work_ledger_identity_v1() == ledger);
                if result.is_err() {
                    assert_eq!(output, [0xa5; 8]);
                } else if matches!(phase, Phase::Pack) {
                    assert_eq!(output, 7u64.to_le_bytes());
                }
                (
                    result.err(),
                    budget.work(),
                    budget.peak_storage(),
                    budget.failed_storage(),
                    header,
                    scratch,
                )
            };
            observed = Some(Observation {
                error,
                floor,
                work: accepted,
                peak,
                failed_work: work.failed_work(),
                failed_storage,
                header,
                scratch,
            });
        }
        parent.release_storage(parent.storage() - original).unwrap();
    });
    observed.unwrap()
}

#[test]
fn nominal_host_v3_exact_work_storage_and_short_phases_are_independently_typed() {
    for phase in [Phase::Plan, Phase::Bind, Phase::Pack] {
        let full = measured(phase, usize::MAX, usize::MAX);
        assert!(full.error.is_none(), "{:?}", full.error);
        let expected = full.floor + full.header + full.scratch;
        assert_eq!(full.peak, expected);
        let exact = measured(phase, full.work, expected);
        assert!(exact.error.is_none(), "{:?}", exact.error);
        assert_eq!((exact.work, exact.peak), (full.work, full.peak));
        assert_eq!((exact.failed_work, exact.failed_storage), (None, None));
        let short = measured(phase, full.work, expected - 1);
        assert!(
            matches!(short.error,Some(GeneratedNominalPackErrorV3::Resource(Resource::Storage(e))) if (e.actual(),e.limit())==(expected,expected-1))
        );
        assert_eq!(
            (
                short.work,
                short.peak,
                short.failed_work,
                short.failed_storage
            ),
            (18, full.floor + full.header, None, Some(expected))
        );
        let suffix = if matches!(phase, Phase::Pack) {
            8 + 8 + 1
        } else {
            1
        };
        let short = measured(phase, full.work - 1, expected);
        assert!(
            matches!(short.error,Some(GeneratedNominalPackErrorV3::Resource(Resource::Work(e))) if (e.actual(),e.limit())==(full.work,full.work-1))
        );
        assert_eq!(
            (
                short.work,
                short.peak,
                short.failed_work,
                short.failed_storage
            ),
            (full.work - suffix, expected, Some(full.work), None)
        );
    }
}

#[test]
fn nominal_host_v3_first_header_and_every_pack_work_denial_preserve_output() {
    for phase in [Phase::Plan, Phase::Bind, Phase::Pack] {
        let full = measured(phase, usize::MAX, usize::MAX);
        let attempted = full.floor + full.header;
        let short = measured(phase, full.work, attempted - 1);
        assert!(
            matches!(short.error,Some(GeneratedNominalPackErrorV3::Resource(Resource::Storage(e))) if (e.actual(),e.limit())==(attempted,attempted-1))
        );
        assert_eq!(
            (short.work, short.peak, short.failed_storage),
            (18, full.floor, Some(attempted))
        );
    }
    let full = measured(Phase::Pack, usize::MAX, usize::MAX);
    for limit in 17..full.work {
        let short = measured(Phase::Pack, limit, full.peak);
        assert!(
            matches!(short.error,Some(GeneratedNominalPackErrorV3::Resource(Resource::Work(e))) if e.limit()==limit && e.actual()>limit)
        );
        assert!(short.failed_work.is_some());
        assert_eq!(short.failed_storage, None);
        assert!(short.work <= limit);
    }
}

#[test]
fn nominal_host_v3_scoped_unwind_and_foreign_ledger_preserve_original_credit() {
    use std::cell::Cell;
    struct Mark<'a>(&'a Cell<bool>);
    impl Drop for Mark<'_> {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }
    let dropped = Cell::new(false);
    let mut work = Work::new(100);
    let mut other_work = Work::new(100);
    let mut budget = Budget::new(&mut work, 128);
    let mut other = Budget::new(&mut other_work, 128);
    budget.reserve_storage(19).unwrap();
    other.reserve_storage(19).unwrap();
    assert!(budget.reserve_storage(129).is_err());
    let failed = budget.failed_storage();
    let ledger = budget.work_ledger_identity_v1();
    let result: Result<()> = scoped(19, &mut budget, |b| {
        b.reserve_storage(7)?;
        let _mark = Mark(&dropped);
        b.charge_work(3)?;
        panic!("nominal scope unwind");
    });
    assert!(matches!(result, Err(GeneratedNominalPackErrorV3::Panicked)));
    assert!(dropped.get());
    assert_eq!(
        (budget.storage(), budget.work(), budget.failed_storage()),
        (19, 3, failed)
    );
    let result = scoped(19, &mut budget, |b| {
        b.reserve_storage(7)?;
        std::mem::swap(b, &mut other);
        Ok(())
    });
    assert!(matches!(
        result,
        Err(GeneratedNominalPackErrorV3::Resource(Resource::Accounting))
    ));
    std::mem::swap(&mut budget, &mut other);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(budget.storage(), 26);
    budget.release_storage(7).unwrap();
    assert_eq!(budget.failed_storage(), failed);
    assert_eq!(other.storage(), 19);
    struct PanickingPayload;
    impl Drop for PanickingPayload {
        fn drop(&mut self) {
            panic!("nominal payload destructor");
        }
    }
    for foreign in [false, true] {
        let escaped = catch_unwind(AssertUnwindSafe(|| {
            let _: Result<()> = scoped(19, &mut budget, |b| {
                b.reserve_storage(7)?;
                if foreign {
                    std::mem::swap(b, &mut other);
                }
                std::panic::panic_any(PanickingPayload);
            });
        }));
        assert!(escaped.is_err());
        if foreign {
            assert_eq!((budget.storage(), other.storage()), (19, 26));
            std::mem::swap(&mut budget, &mut other);
            budget.release_storage(7).unwrap();
        }
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!((budget.storage(), other.storage()), (19, 19));
        assert_eq!((budget.work(), budget.failed_storage()), (3, failed));
    }
}

#[test]
fn nominal_host_v3_exact_argument_cap_and_maximum_output_extent_are_not_relaxed() {
    with_plan(
        &[Shape::Usize; MAX_ARGUMENTS_PER_KERNEL],
        "gfx942:xnack-",
        false,
        |plan, budget| {
            let floor = budget.storage();
            {
                let mut bindings = Vec::with_capacity(MAX_ARGUMENTS_PER_KERNEL);
                budget
                    .reserve_storage(
                        size_of_val(&bindings)
                            + bindings.capacity()
                                * size_of::<GeneratedNominalBindingV3<'_, '_, '_, usize>>(),
                    )
                    .unwrap();
                for index in 0..MAX_ARGUMENTS_PER_KERNEL {
                    let (binding, receipt) = plan.bind_usize(index, index, budget).unwrap();
                    reserve(budget, receipt);
                    bindings.push(binding);
                }
                budget
                    .reserve_storage(NOMINAL_ARGUMENT_REF_STORAGE_V3 * MAX_ARGUMENTS_PER_KERNEL)
                    .unwrap();
                let inputs: [_; MAX_ARGUMENTS_PER_KERNEL] = std::array::from_fn(|index| {
                    bindings[MAX_ARGUMENTS_PER_KERNEL - index - 1].as_input()
                });
                let mut bytes = vec![0xa5; plan.output_len()];
                budget
                    .reserve_storage(size_of_val(&bytes) + bytes.capacity())
                    .unwrap();
                let receipt;
                {
                    let (packed, r) = plan.pack(&inputs, &mut bytes, budget).unwrap();
                    reserve(budget, r);
                    receipt = r;
                    for (index, bytes) in packed.as_bytes().chunks_exact(8).enumerate() {
                        assert_eq!(bytes, (index as u64).to_le_bytes());
                    }
                }
                budget.release_storage(receipt.retained_storage()).unwrap();
            }
            budget.release_storage(budget.storage() - floor).unwrap();
        },
    );
    let wire = fixture_with_segment(
        &[Shape::Usize],
        "gfx950:xnack-",
        false,
        Some(MAX_KERNARG_SEGMENT_BYTES),
    );
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget
        .reserve_storage(
            size_of_val(&wire)
                + wire.capacity()
                + DESCRIPTOR_TABLE_VIEW_STORAGE_V3
                + DESCRIPTOR_READER_SCRATCH_STORAGE_V3,
        )
        .unwrap();
    let view = decode_device_descriptor_table_v3(&wire, &mut |n| budget.charge_work(n)).unwrap();
    budget
        .release_storage(DESCRIPTOR_READER_SCRATCH_STORAGE_V3)
        .unwrap();
    let base = budget.storage();
    {
        let (plan, r) =
            GeneratedNominalArgumentPlanV3::new(&view, KernelId::from_bytes([13; 32]), &mut budget)
                .unwrap();
        reserve(&mut budget, r);
        assert!(matches!(
            GeneratedNominalArgumentPlanV3::new(&view, KernelId::from_bytes([14; 32]), &mut budget)
                .err()
                .unwrap(),
            GeneratedNominalPackErrorV3::Descriptor(DescriptorWireErrorV3::Decode(
                DecodeError::Validation(ValidationError::DanglingReference { field: "kernel" })
            ))
        ));
        let (binding, r) = plan.bind_usize(0, 7, &mut budget).unwrap();
        reserve(&mut budget, r);
        budget
            .reserve_storage(NOMINAL_ARGUMENT_REF_STORAGE_V3)
            .unwrap();
        let inputs = [binding.as_input()];
        let mut bytes = vec![0xa5; MAX_KERNARG_SEGMENT_BYTES as usize];
        budget
            .reserve_storage(size_of_val(&bytes) + bytes.capacity())
            .unwrap();
        let n = bytes.len();
        assert!(
            matches!(plan.pack(&inputs, &mut bytes[..n-1], &mut budget).err().unwrap(), GeneratedNominalPackErrorV3::OutputLength { expected, actual } if expected == n && actual == n-1)
        );
        assert!(bytes.iter().all(|b| *b == 0xa5));
        let receipt;
        {
            let (packed, r) = plan.pack(&inputs, &mut bytes, &mut budget).unwrap();
            reserve(&mut budget, r);
            receipt = r;
            assert_eq!(&packed.as_bytes()[..8], &7u64.to_le_bytes());
            assert!(packed.as_bytes()[8..].iter().all(|b| *b == 0));
        }
        budget.release_storage(receipt.retained_storage()).unwrap();
    }
    budget.release_storage(budget.storage() - base).unwrap();
}
