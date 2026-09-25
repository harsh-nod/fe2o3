use super::*;
use fe2o3_compiler_ffi::COMPILER_DESCRIPTOR_SECTION_NAME_V4;
use fe2o3_hsaco_finalize::{
    NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V3, NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V4 as SCRATCH,
    NominalFinalizationErrorV3, NominalFinalizationErrorV4, derive_unfinalized_nominal_hsaco_v4,
    finalize_unfinalized_nominal_hsaco_v4, inspect_finalized_nominal_hsaco_v4,
    inspect_unfinalized_nominal_hsaco_v3, inspect_unfinalized_nominal_hsaco_v4,
};
use fe2o3_kernel_descriptor::*;

#[allow(dead_code)]
#[path = "../../../fe2o3-kernel-descriptor/tests/support/conditional_v4.rs"]
mod descriptor_fixture;

fn free(_: usize) -> Result<(), &'static str> {
    Ok(())
}

fn wires(
    entries: usize,
    customize: impl FnMut(&mut descriptor_fixture::Fixture),
) -> (Vec<u8>, Vec<u8>) {
    descriptor_fixture::with_custom_contracts(GENERAL_V3_TARGET, entries, 2, customize, |input| {
        let kernels = input
            .nominal
            .kernels
            .iter()
            .map(|kernel| KernelDescriptorInputV3 {
                kernel_id: kernel.kernel_id,
                logical_name: kernel.logical_name,
                entry_name: kernel.entry_name,
                descriptor_symbol: kernel.descriptor_symbol,
                source_evidence: kernel.source_evidence,
                executable_ir_evidence: kernel.executable_ir_evidence,
                capabilities: kernel.capabilities,
                abi_layout: KernelAbiLayoutV1::new(48, 304, 8).unwrap(),
                launch: kernel.launch,
                arguments: kernel.arguments,
            })
            .collect::<Vec<_>>();
        let input = DeviceDescriptorTableInputV4 {
            nominal: DeviceDescriptorTableInputV3 {
                kernels: &kernels,
                ..input.nominal
            },
            contracts: input.contracts,
        };
        let mut v4 = vec![0; encoded_device_descriptor_table_v4_len(&input, &mut free).unwrap()];
        encode_device_descriptor_table_v4(&input, &mut v4, &mut free).unwrap();
        let mut v3 =
            vec![0; encoded_device_descriptor_table_v3_len(&input.nominal, &mut free).unwrap()];
        encode_device_descriptor_table_v3(&input.nominal, &mut v3, &mut free).unwrap();
        (v4, v3)
    })
}

fn fixture(
    wire: &[u8],
    entries: usize,
    hidden: bool,
    size: u32,
    mut mutate: impl FnMut(&mut Value),
    count: usize,
    extra: &[&str],
) -> Fixture {
    let mut root = rmpv::decode::read_value(
        &mut general_v3_metadata(GeneralV3Kernel::Alpha, size, 8, GENERAL_V3_TARGET).as_slice(),
    )
    .unwrap();
    let Value::Array(kernels) = field_mut(&mut root, "amdhsa.kernels") else {
        panic!("kernels");
    };
    let template = kernels[0].clone();
    kernels.clear();
    let names = (0..entries)
        .map(|i| (format!("kernel{i}"), format!("kernel{i}.kd")))
        .collect::<Vec<_>>();
    for (name, symbol) in &names {
        let mut kernel = template.clone();
        set_field(&mut kernel, ".name", name.as_str().into());
        set_field(&mut kernel, ".symbol", symbol.as_str().into());
        remove_field(&mut kernel, ".reqd_workgroup_size");
        let mut args = Vec::new();
        for i in 0..3 {
            let (pointer_name, length_name) = [
                ("left_ptr", "left_len"),
                ("right_ptr", "right_len"),
                ("output_ptr", "output_len"),
            ][i as usize];
            let mut pointer = argument(
                Some(pointer_name),
                i * 16,
                8,
                "global_buffer",
                Some("global"),
            );
            map_mut(&mut pointer).extend([
                (".value_type".into(), "f32".into()),
                (
                    ".access".into(),
                    if i == 2 { "write_only" } else { "read_only" }.into(),
                ),
                (".is_restrict".into(), (i == 2).into()),
                (".pointee_align".into(), 4.into()),
            ]);
            args.push(pointer);
            let mut length = argument(Some(length_name), i * 16 + 8, 8, "by_value", None);
            map_mut(&mut length).push((".value_type".into(), "u64".into()));
            args.push(length);
        }
        if hidden {
            args.extend(v5_hidden_arguments(48));
        }
        set_field(&mut kernel, ".args", Value::Array(args));
        mutate(&mut kernel);
        kernels.push(kernel);
    }
    let mut metadata = Vec::new();
    write_value(&mut metadata, &root).unwrap();
    let names = names
        .iter()
        .map(|(name, symbol)| (name.as_str(), symbol.as_str()))
        .collect::<Vec<_>>();
    let mut fixture = build_fixture_for_kernels(wire, &metadata, count, extra, &names, 0);
    write_u32(&mut fixture.bytes, 48, 0x64c);
    for offset in fixture.kernel_descriptor_offsets.iter().copied() {
        write_u32(&mut fixture.bytes, offset + 8, size);
        write_u32(&mut fixture.bytes, offset + 44, 1);
        write_u32(&mut fixture.bytes, offset + 48, 0x00af_0081);
        write_u16(&mut fixture.bytes, offset + 56, 0x001e);
    }
    if count != 0 {
        rename(&mut fixture, COMPILER_DESCRIPTOR_SECTION_NAME_V4);
    }
    fixture
}

fn rename(fixture: &mut Fixture, name: &str) {
    let offset = section_name_file_offset(&fixture.bytes, DESCRIPTOR_SECTION_INDEX);
    fixture.bytes[offset..offset + name.len()].copy_from_slice(name.as_bytes());
}

fn contract_start(wire: &[u8]) -> usize {
    wire.windows(8)
        .position(|w| w == CONDITIONAL_INVOCATION_MAGIC_V1)
        .unwrap()
}

fn repair_contract_identity(wire: &mut [u8]) {
    let start = contract_start(wire);
    let length = u32::from_le_bytes(wire[start - 36..start - 32].try_into().unwrap()) as usize;
    let mut hash = Sha256::new();
    hash.update(CONDITIONAL_INVOCATION_DOMAIN_V1);
    hash.update((length as u64).to_le_bytes());
    hash.update(&wire[start..start + length]);
    wire[start - 32..start].copy_from_slice(&hash.finalize());
}

#[test]
fn roundtrip_preserves_every_contract_and_both_cov6_physical_forms() {
    for entries in [1, 2] {
        let (source, _) = wires(entries, |_| {});
        for (hidden, size) in [(false, 48), (true, 304)] {
            let raw = fixture(&source, entries, hidden, size, |_| {}, 1, &[]);
            let before = raw.bytes.clone();
            let result =
                finalize_unfinalized_nominal_hsaco_v4(&raw.bytes, &source, SCRATCH, &mut free)
                    .unwrap();
            let mut hash = Sha256::new();
            hash.update(b"FE2O3/AMDHSA-CODE-OBJECT/V1\0");
            hash.update((raw.bytes.len() as u64).to_le_bytes());
            hash.update(&raw.bytes);
            let expected: [u8; 32] = hash.finalize().into();
            let mut expected_bytes = before.clone();
            let offset = result.location().digest_offset();
            expected_bytes[offset..offset + 32].copy_from_slice(&expected);
            assert_eq!(result.as_bytes(), expected_bytes);
            assert_eq!(result.digest().as_bytes(), &expected);
            assert!(result.artifact_byte_capacity() >= expected_bytes.len());
            assert!(!result.grants_launch_authority());
            let view =
                inspect_finalized_nominal_hsaco_v4(result.as_bytes(), SCRATCH, &mut free).unwrap();
            assert!(!view.grants_launch_authority());
            assert_eq!(view.location(), result.location());
            assert_eq!(view.digest(), result.digest());
            assert_eq!(view.kernel_bindings().bindings().len(), entries);
            assert_eq!(result.kernel_bindings().bindings().len(), entries);
            let original = decode_device_descriptor_table_v4(&source, &mut free).unwrap();
            let table = view.descriptor_table();
            assert_eq!(table.canonical_bytes(), result.descriptor_bytes());
            assert_eq!(table.kernel_count(), entries);
            for index in 0..entries {
                let kernel = table.kernel(index, &mut free).unwrap();
                let contract = kernel.conditional_contract(&mut free).unwrap();
                let original_kernel = original.kernel(index, &mut free).unwrap();
                let original_contract = original_kernel.conditional_contract(&mut free).unwrap();
                assert_eq!(
                    contract.canonical_bytes(),
                    original_contract.canonical_bytes()
                );
                assert_eq!(contract.identity(), original_contract.identity());
                assert_eq!(
                    contract.subjects().kernel_id,
                    *kernel.kernel_id().as_bytes()
                );
                assert_eq!(contract.read_count(), 2);
                assert_eq!(
                    kernel.abi_layout(),
                    KernelAbiLayoutV1::new(48, 304, 8).unwrap()
                );
                let mut arguments = kernel.arguments();
                for kind in [
                    SourceTypeDescriptorV3::SharedSlice(ScalarTypeV1::F32),
                    SourceTypeDescriptorV3::SharedSlice(ScalarTypeV1::F32),
                    SourceTypeDescriptorV3::DisjointSlice(ScalarTypeV1::F32),
                ] {
                    let argument = arguments.next(&mut free).unwrap().unwrap();
                    assert_eq!(
                        table
                            .source_type(argument.source_type(), &mut free)
                            .unwrap()
                            .descriptor(),
                        kind
                    );
                }
                assert!(arguments.next(&mut free).unwrap().is_none());
            }
            assert_eq!(view.into_kernel_bindings().bindings().len(), entries);
            assert_eq!(
                derive_unfinalized_nominal_hsaco_v4(result.as_bytes(), SCRATCH, &mut free).unwrap(),
                before
            );
            assert_eq!(raw.bytes, before);
            assert_eq!(result.into_bytes(), expected_bytes);
        }
    }
}

#[test]
fn canonical_contract_substitution_still_requires_exact_source() {
    let (source, _) = wires(1, |_| {});
    let (changed, _) = wires(1, |contract| contract.arguments[0].adjusted_argument = 8);
    assert_ne!(source, changed);
    let raw = fixture(&changed, 1, false, 48, |_| {}, 1, &[]);
    assert!(
        !inspect_unfinalized_nominal_hsaco_v4(&raw.bytes, SCRATCH, &mut free)
            .unwrap()
            .grants_launch_authority()
    );
    for expected in [&source[..], &changed[..changed.len() - 1]] {
        assert!(matches!(
            finalize_unfinalized_nominal_hsaco_v4(&raw.bytes, expected, SCRATCH, &mut free),
            Err(NominalFinalizationErrorV4::DescriptorSourceMismatch)
        ));
    }
    // Public structural consistency is permitted, but supplies no proof authority.
    let structural =
        finalize_unfinalized_nominal_hsaco_v4(&raw.bytes, &changed, SCRATCH, &mut free).unwrap();
    assert!(!structural.grants_launch_authority());
    assert!(matches!(
        finalize_unfinalized_nominal_hsaco_v4(
            &raw.bytes,
            structural.descriptor_bytes(),
            SCRATCH,
            &mut free
        ),
        Err(NominalFinalizationErrorV4::DescriptorSourceMismatch)
    ));
}

#[test]
fn damaged_removed_and_rebound_contracts_fail_even_with_repaired_identity() {
    let (source, _) = wires(1, |_| {});
    let start = contract_start(&source);
    for mode in 0..5 {
        let mut bad = source.clone();
        match mode {
            0 => bad[start - 32] ^= 1,
            1 => {
                bad.truncate(start - 40);
                let n = bad.len() as u32;
                bad[12..16].copy_from_slice(&n.to_le_bytes());
            }
            2 => bad[start - 40..start - 38].fill(0),
            3 => {
                bad[start + 28] ^= 1;
                repair_contract_identity(&mut bad);
            }
            _ => {
                bad[start + 624 + 32 + 20] ^= 0x40;
                repair_contract_identity(&mut bad);
            }
        }
        let raw = fixture(&bad, 1, false, 48, |_| {}, 1, &[]);
        assert!(
            matches!(
                inspect_unfinalized_nominal_hsaco_v4(&raw.bytes, SCRATCH, &mut free),
                Err(NominalFinalizationErrorV4::Wire(_))
            ),
            "mode {mode}"
        );
        assert!(
            matches!(
                finalize_unfinalized_nominal_hsaco_v4(&raw.bytes, &bad, SCRATCH, &mut free),
                Err(NominalFinalizationErrorV4::Wire(_))
            ),
            "mode {mode}"
        );
    }
}

#[test]
fn contracts_cannot_swap_between_elf_entries() {
    let (mut source, _) = wires(2, |_| {});
    let start = contract_start(&source);
    let n = u32::from_le_bytes(source[start - 36..start - 32].try_into().unwrap()) as usize;
    let first = source[start - 36..start + n].to_vec();
    let second = source[start + n..start + n + 36 + n].to_vec();
    source[start - 36..start + n].copy_from_slice(&second);
    source[start + n..start + n + 36 + n].copy_from_slice(&first);
    let raw = fixture(&source, 2, false, 48, |_| {}, 1, &[]);
    assert!(matches!(
        inspect_unfinalized_nominal_hsaco_v4(&raw.bytes, SCRATCH, &mut free),
        Err(NominalFinalizationErrorV4::Wire(_))
    ));
}

#[test]
fn missing_duplicate_and_old_sections_fail_closed() {
    let (source, v3) = wires(1, |_| {});
    for count in [0, 2] {
        let raw = fixture(&source, 1, false, 48, |_| {}, count, &[]);
        let error =
            inspect_unfinalized_nominal_hsaco_v4(&raw.bytes, SCRATCH, &mut free).unwrap_err();
        assert!(if count == 0 {
            matches!(
                error,
                NominalFinalizationErrorV4::Artifact(FinalizationError::MissingDescriptorSection)
            )
        } else {
            matches!(
                error,
                NominalFinalizationErrorV4::Artifact(FinalizationError::DuplicateDescriptorSection)
            )
        });
        assert!(error.to_string().contains(".fe2o3.kd.v4"));
    }
    let raw = fixture(&source, 1, false, 48, |_| {}, 1, &[]);
    assert!(matches!(
        inspect_unfinalized(&raw.bytes),
        Err(FinalizationError::DescriptorSectionVersionMismatch)
    ));
    assert!(matches!(
        inspect_unfinalized_nominal_hsaco_v3(
            &raw.bytes,
            NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V3,
            &mut free
        ),
        Err(NominalFinalizationErrorV3::Artifact(
            FinalizationError::DescriptorSectionVersionMismatch
        ))
    ));
    let mut old = fixture(&v3, 1, false, 48, |_| {}, 1, &[]);
    rename(&mut old, ".fe2o3.kd.v3");
    inspect_unfinalized_nominal_hsaco_v3(
        &old.bytes,
        NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V3,
        &mut free,
    )
    .unwrap();
    for bytes in [&old.bytes, &valid_fixture().bytes] {
        assert!(matches!(
            inspect_unfinalized_nominal_hsaco_v4(bytes, SCRATCH, &mut free),
            Err(NominalFinalizationErrorV4::Artifact(
                FinalizationError::DescriptorSectionVersionMismatch
            ))
        ));
    }
}

#[test]
fn relabeling_section_or_wire_cannot_upgrade_or_downgrade() {
    let (v4, v3) = wires(1, |_| {});
    for wire in [&v4, &v3] {
        for version in [3u16, 4] {
            let mut changed = wire.clone();
            changed[8..10].copy_from_slice(&version.to_le_bytes());
            let mut raw = fixture(&changed, 1, false, 48, |_| {}, 1, &[]);
            if wire == &v4 {
                rename(&mut raw, ".fe2o3.kd.v3");
                assert!(matches!(
                    inspect_unfinalized_nominal_hsaco_v3(
                        &raw.bytes,
                        NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V3,
                        &mut free
                    ),
                    Err(NominalFinalizationErrorV3::Wire(_))
                ));
            } else {
                assert!(matches!(
                    inspect_unfinalized_nominal_hsaco_v4(&raw.bytes, SCRATCH, &mut free),
                    Err(NominalFinalizationErrorV4::Wire(_))
                ));
            }
        }
    }
}

#[test]
fn mixed_valid_sections_are_rejected_in_both_header_orders() {
    let (source, v3) = wires(1, |_| {});
    let legacy = valid_fixture();
    let legacy = inspect_unfinalized(&legacy.bytes).unwrap();
    let v1 = encode_device_descriptor_table_v1(legacy.descriptor_table()).unwrap();
    for (name, wire) in [(".fe2o3.kd.v1", v1), (".fe2o3.kd.v3", v3)] {
        let mut mixed = fixture(&source, 1, false, 48, |_| {}, 1, &[name]);
        align(&mut mixed.bytes, 8);
        let offset = mixed.bytes.len();
        mixed.bytes.extend_from_slice(&wire);
        let extra = mixed.extra_headers[0];
        write_u64(&mut mixed.bytes, extra + 24, offset as u64);
        write_u64(&mut mixed.bytes, extra + 32, wire.len() as u64);
        for swap in [false, true] {
            if swap {
                let first = mixed.descriptor_headers[0];
                for i in 0..64 {
                    mixed.bytes.swap(first + i, extra + i);
                }
            }
            assert!(matches!(
                inspect_unfinalized_nominal_hsaco_v4(&mixed.bytes, SCRATCH, &mut free),
                Err(NominalFinalizationErrorV4::Artifact(
                    FinalizationError::DescriptorSectionVersionMismatch
                ))
            ));
            assert!(matches!(
                inspect_unfinalized_nominal_hsaco_v3(
                    &mixed.bytes,
                    NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V3,
                    &mut free
                ),
                Err(NominalFinalizationErrorV3::Artifact(
                    FinalizationError::DescriptorSectionVersionMismatch
                ))
            ));
            assert!(matches!(
                inspect_unfinalized(&mixed.bytes),
                Err(FinalizationError::DescriptorSectionVersionMismatch)
            ));
        }
    }
}

#[test]
fn future_descriptor_namespace_is_rejected_by_every_locator_path() {
    let (v4, v3) = wires(1, |_| {});
    for name in [
        ".fe2o3.kd.v2",
        ".fe2o3.kd.v5",
        ".fe2o3.kd.v999",
        ".fe2o3.kd.future",
        ".fe2o3.kd.v4.extra",
        ".fe2o3.kd.",
    ] {
        for version in [1, 3, 4] {
            let mut mixed = match version {
                1 => {
                    let legacy = valid_fixture();
                    let legacy = inspect_unfinalized(&legacy.bytes).unwrap();
                    let wire =
                        encode_device_descriptor_table_v1(legacy.descriptor_table()).unwrap();
                    build_fixture(
                        &wire,
                        &metadata(&[("vecadd", "vecadd.kd", 272)]),
                        1,
                        &[name],
                    )
                }
                3 => {
                    let mut raw = fixture(&v3, 1, false, 48, |_| {}, 1, &[name]);
                    rename(&mut raw, ".fe2o3.kd.v3");
                    raw
                }
                _ => fixture(&v4, 1, false, 48, |_| {}, 1, &[name]),
            };
            let first = mixed.descriptor_headers[0];
            let extra = mixed.extra_headers[0];
            for swap in [false, true] {
                if swap {
                    for i in 0..64 {
                        mixed.bytes.swap(first + i, extra + i);
                    }
                }
                match version {
                    1 => assert!(matches!(
                        inspect_unfinalized(&mixed.bytes),
                        Err(FinalizationError::DescriptorSectionVersionMismatch)
                    )),
                    3 => assert!(matches!(
                        inspect_unfinalized_nominal_hsaco_v3(
                            &mixed.bytes,
                            NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V3,
                            &mut free
                        ),
                        Err(NominalFinalizationErrorV3::Artifact(
                            FinalizationError::DescriptorSectionVersionMismatch
                        ))
                    )),
                    _ => assert!(matches!(
                        inspect_unfinalized_nominal_hsaco_v4(&mixed.bytes, SCRATCH, &mut free),
                        Err(NominalFinalizationErrorV4::Artifact(
                            FinalizationError::DescriptorSectionVersionMismatch
                        ))
                    )),
                }
            }
            // An unsupported family alone also fails before decoding any wire.
            write_u32(&mut mixed.bytes, extra, 0);
            assert!(matches!(
                inspect_unfinalized_nominal_hsaco_v4(&mixed.bytes, SCRATCH, &mut free),
                Err(NominalFinalizationErrorV4::Artifact(
                    FinalizationError::DescriptorSectionVersionMismatch
                ))
            ));
        }
    }
    // Similar unrelated names do not accidentally reserve another namespace.
    let raw = fixture(
        &v4,
        1,
        false,
        48,
        |_| {},
        1,
        &[".fe2o3.kdx.v5", ".debug.fe2o3.kd.v5"],
    );
    inspect_unfinalized_nominal_hsaco_v4(&raw.bytes, SCRATCH, &mut free).unwrap();
}

#[test]
fn malformed_physical_abi_and_hidden_tails_are_rejected() {
    let (source, _) = wires(1, |_| {});
    for (hidden, size) in [(true, 296), (true, 312), (false, 304)] {
        let raw = fixture(&source, 1, hidden, size, |_| {}, 1, &[]);
        assert!(inspect_unfinalized_nominal_hsaco_v4(&raw.bytes, SCRATCH, &mut free).is_err());
    }
    for mode in 0..9 {
        let mut raw = fixture(
            &source,
            1,
            false,
            48,
            |kernel| match mode {
                0 => set_field(&mut arguments_mut(kernel)[0], ".value_type", "u64".into()),
                1 => set_field(
                    &mut arguments_mut(kernel)[0],
                    ".access",
                    "read_write".into(),
                ),
                2 => set_field(
                    &mut arguments_mut(kernel)[0],
                    ".address_space",
                    "constant".into(),
                ),
                3 => set_field(&mut arguments_mut(kernel)[1], ".offset", 9.into()),
                4 => set_field(kernel, ".kernarg_segment_align", 16.into()),
                5 => set_field(kernel, ".max_flat_workgroup_size", 128.into()),
                6 => set_field(kernel, ".group_segment_fixed_size", 128.into()),
                7 => set_field(&mut arguments_mut(kernel)[0], ".is_restrict", true.into()),
                _ => {}
            },
            1,
            &[],
        );
        if mode == 8 {
            let kd = raw.kernel_descriptor_offsets[0];
            write_u32(&mut raw.bytes, kd + 8, 304);
        }
        assert!(
            inspect_unfinalized_nominal_hsaco_v4(&raw.bytes, SCRATCH, &mut free).is_err(),
            "mode {mode}"
        );
    }
}

#[test]
fn malformed_detached_section_placement_is_rejected() {
    let (source, _) = wires(1, |_| {});
    for (field, value) in [(4, 8), (8, 2), (48, 1)] {
        let mut raw = fixture(&source, 1, false, 48, |_| {}, 1, &[]);
        let header = raw.descriptor_headers[0];
        if field == 4 {
            write_u32(&mut raw.bytes, header + field, value as u32);
        } else {
            write_u64(&mut raw.bytes, header + field, value);
        }
        assert!(inspect_unfinalized_nominal_hsaco_v4(&raw.bytes, SCRATCH, &mut free).is_err());
    }
}

#[test]
fn finalized_digest_covers_valid_contract_substitution_and_all_artifact_bytes() {
    let (source, _) = wires(1, |_| {});
    let raw = fixture(&source, 1, false, 48, |_| {}, 1, &[]);
    assert!(matches!(
        inspect_finalized_nominal_hsaco_v4(&raw.bytes, SCRATCH, &mut free),
        Err(NominalFinalizationErrorV4::Artifact(
            FinalizationError::ExpectedFinalizedDigest
        ))
    ));
    let result =
        finalize_unfinalized_nominal_hsaco_v4(&raw.bytes, &source, SCRATCH, &mut free).unwrap();
    assert!(matches!(
        finalize_unfinalized_nominal_hsaco_v4(result.as_bytes(), &source, SCRATCH, &mut free),
        Err(NominalFinalizationErrorV4::Artifact(
            FinalizationError::ExpectedZeroDigest
        ))
    ));
    let (changed, _) = wires(1, |contract| contract.arguments[0].adjusted_argument = 8);
    let start = contract_start(&source);
    let mut rebound = result.as_bytes().to_vec();
    let offset = raw.descriptor_offsets[0];
    rebound[offset + start - 32..offset + source.len()].copy_from_slice(&changed[start - 32..]);
    for bad in [
        rebound,
        {
            let mut bad = result.as_bytes().to_vec();
            bad[result.location().digest_offset()] ^= 1;
            bad
        },
        {
            let mut bad = result.as_bytes().to_vec();
            bad.push(0xa5);
            bad
        },
    ] {
        let before = bad.clone();
        assert!(matches!(
            inspect_finalized_nominal_hsaco_v4(&bad, SCRATCH, &mut free),
            Err(NominalFinalizationErrorV4::Artifact(
                FinalizationError::CanonicalDigestMismatch { .. }
            ))
        ));
        assert!(derive_unfinalized_nominal_hsaco_v4(&bad, SCRATCH, &mut free).is_err());
        assert_eq!(bad, before);
    }
}

fn work_refused(error: NominalFinalizationErrorV4<&'static str>) {
    assert!(
        matches!(
            error,
            NominalFinalizationErrorV4::Work("denied")
                | NominalFinalizationErrorV4::Wire(DescriptorWireErrorV4::Nominal(
                    DescriptorWireErrorV3::Work("denied")
                ))
                | NominalFinalizationErrorV4::Wire(DescriptorWireErrorV4::Contract(
                    ConditionalInvocationWireErrorV1::Work("denied")
                ))
        ),
        "{error:?}"
    );
}

#[test]
fn insufficient_scratch_precedes_work_for_every_v4_operation() {
    let (source, _) = wires(1, |_| {});
    let raw = fixture(&source, 1, false, 48, |_| {}, 1, &[]);
    let result =
        finalize_unfinalized_nominal_hsaco_v4(&raw.bytes, &source, SCRATCH, &mut free).unwrap();
    let mut calls = 0;
    let mut charge = |_| {
        calls += 1;
        Ok::<_, &'static str>(())
    };
    let errors = [
        inspect_unfinalized_nominal_hsaco_v4(&raw.bytes, SCRATCH - 1, &mut charge).unwrap_err(),
        inspect_finalized_nominal_hsaco_v4(result.as_bytes(), SCRATCH - 1, &mut charge)
            .unwrap_err(),
        finalize_unfinalized_nominal_hsaco_v4(&raw.bytes, &source, SCRATCH - 1, &mut charge)
            .unwrap_err(),
        derive_unfinalized_nominal_hsaco_v4(result.as_bytes(), SCRATCH - 1, &mut charge)
            .unwrap_err(),
    ];
    for error in errors {
        assert!(
            matches!(error, NominalFinalizationErrorV4::Scratch { required: SCRATCH, prepaid } if prepaid == SCRATCH - 1)
        );
    }
    assert_eq!(calls, 0);
}

#[test]
fn work_denial_at_each_finalization_stage_leaves_input_unchanged() {
    let (source, _) = wires(1, |_| {});
    let raw = fixture(&source, 1, false, 48, |_| {}, 1, &[]);
    let before = raw.bytes.clone();
    let mut charges = Vec::new();
    finalize_unfinalized_nominal_hsaco_v4(&raw.bytes, &source, SCRATCH, &mut |n| {
        charges.push(n);
        free(0)
    })
    .unwrap();
    // Includes initial inspection, mandatory contract decoding, physical joins,
    // exact-source comparison, hash/copy, and independent finalized inspection.
    for deny in 0..charges.len() {
        let mut calls = 0;
        let error = finalize_unfinalized_nominal_hsaco_v4(&raw.bytes, &source, SCRATCH, &mut |n| {
            assert_eq!(n, charges[calls]);
            let current = calls;
            calls += 1;
            if current == deny {
                Err("denied")
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        work_refused(error);
        assert_eq!(calls, deny + 1);
        assert_eq!(raw.bytes, before);
    }
    let total: usize = charges.iter().sum();
    for budget in [total - 1, total] {
        let mut remaining = budget;
        let result =
            finalize_unfinalized_nominal_hsaco_v4(&raw.bytes, &source, SCRATCH, &mut |n| {
                remaining = remaining.checked_sub(n).ok_or("denied")?;
                Ok(())
            });
        if budget == total {
            result.unwrap();
            assert_eq!(remaining, 0);
        } else {
            work_refused(result.unwrap_err());
        }
    }
}

#[test]
fn raw_reconstruction_obeys_exact_and_one_short_work_budgets() {
    let (source, _) = wires(1, |_| {});
    let raw = fixture(&source, 1, false, 48, |_| {}, 1, &[]);
    let result =
        finalize_unfinalized_nominal_hsaco_v4(&raw.bytes, &source, SCRATCH, &mut free).unwrap();
    let before = result.as_bytes().to_vec();
    let mut total = 0;
    derive_unfinalized_nominal_hsaco_v4(result.as_bytes(), SCRATCH, &mut |n| {
        total += n;
        free(0)
    })
    .unwrap();
    for budget in [0, total - 1, total] {
        let mut remaining = budget;
        let reconstructed =
            derive_unfinalized_nominal_hsaco_v4(result.as_bytes(), SCRATCH, &mut |n| {
                remaining = remaining.checked_sub(n).ok_or("denied")?;
                Ok(())
            });
        if budget == total {
            assert_eq!(reconstructed.unwrap(), raw.bytes);
            assert_eq!(remaining, 0);
        } else {
            work_refused(reconstructed.unwrap_err());
        }
        assert_eq!(result.as_bytes(), before);
    }
}
