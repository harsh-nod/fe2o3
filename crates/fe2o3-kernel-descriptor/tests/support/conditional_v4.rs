//! Inert multi-entry fixture shared by descriptor and compiler transport tests.
use fe2o3_kernel_descriptor::*;
#[path = "conditional_invocation.rs"]
mod contract_fixture;
pub use contract_fixture::Fixture;
use contract_fixture::free;

pub fn with_input<R>(
    target: &str,
    entries: usize,
    inputs: usize,
    consume: impl FnOnce(DeviceDescriptorTableInputV4<'_>) -> R,
) -> R {
    with_custom_contracts(target, entries, inputs, |_| {}, consume)
}

pub fn with_custom_contracts<R>(
    target: &str,
    entries: usize,
    inputs: usize,
    mut customize: impl FnMut(&mut Fixture),
    consume: impl FnOnce(DeviceDescriptorTableInputV4<'_>) -> R,
) -> R {
    let compiler = CompilerIdentityV1::new(
        Text::new("rustc").unwrap(),
        Text::new("nightly").unwrap(),
        [7; 20],
    );
    let producer = ProducerIdentityV1::new(
        Text::new("fe2o3").unwrap(),
        Text::new("inert-test").unwrap(),
    );
    let input_type = SourceTypeRecordV3::new(
        SourceTypeDescriptorV3::SharedSlice(ScalarTypeV1::F32),
        &mut free,
    )
    .unwrap();
    let output_type = SourceTypeRecordV3::new(
        SourceTypeDescriptorV3::DisjointSlice(ScalarTypeV1::F32),
        &mut free,
    )
    .unwrap();
    let input_layout = device_layout_record_v3(
        DeviceLayoutDescriptorV1::shared_slice(ScalarTypeV1::F32),
        &mut free,
    )
    .unwrap();
    let output_layout = device_layout_record_v3(
        DeviceLayoutDescriptorV1::disjoint_slice(ScalarTypeV1::F32),
        &mut free,
    )
    .unwrap();
    let mut types = vec![output_type];
    let mut layouts = vec![output_layout.clone()];
    if inputs > 0 {
        types.push(input_type);
        layouts.push(input_layout.clone());
    }
    types.sort_by_key(|r| r.identity());
    layouts.sort_by_key(|r| r.identity());
    let components = (0..=inputs)
        .map(|i| {
            [
                PhysicalComponentV3 {
                    kind: PhysicalAbiComponentKind::GlobalPointer,
                    offset: (16 * i) as u32,
                    size: 8,
                    alignment: 8,
                    access: if i == inputs {
                        AccessMode::WriteOnly
                    } else {
                        AccessMode::ReadOnly
                    },
                    alias: if i == inputs {
                        AliasSemantics::Exclusive
                    } else {
                        AliasSemantics::SharedReadOnly
                    },
                },
                PhysicalComponentV3 {
                    kind: PhysicalAbiComponentKind::SliceLengthU64,
                    offset: (16 * i + 8) as u32,
                    size: 8,
                    alignment: 8,
                    access: AccessMode::ByValue,
                    alias: AliasSemantics::Value,
                },
            ]
        })
        .collect::<Vec<_>>();
    let argument_names = (0..=inputs).map(|i| format!("arg{i}")).collect::<Vec<_>>();
    let arguments = (0..=inputs)
        .map(|i| LogicalArgumentInputV3 {
            source_index: i as u16,
            name: &argument_names[i],
            source_type: if i == inputs {
                output_type.identity()
            } else {
                input_type.identity()
            },
            device_layout: if i == inputs {
                output_layout.identity()
            } else {
                input_layout.identity()
            },
            ownership: if i == inputs {
                OwnershipSemantics::UniqueBorrow
            } else {
                OwnershipSemantics::SharedBorrow
            },
            access: components[i][0].access,
            alias: components[i][0].alias,
            components: &components[i],
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
    let names = (0..entries)
        .map(|i| format!("kernel{i}"))
        .collect::<Vec<_>>();
    let symbols = (0..entries)
        .map(|i| format!("kernel{i}.kd"))
        .collect::<Vec<_>>();
    let kernels = (0..entries)
        .map(|i| KernelDescriptorInputV3 {
            kernel_id: KernelId::from_bytes([13 + i as u8; 32]),
            logical_name: &names[i],
            entry_name: &names[i],
            descriptor_symbol: &symbols[i],
            source_evidence: evidence,
            executable_ir_evidence: evidence,
            capabilities: &[CapabilityV1::AmdWave],
            abi_layout: KernelAbiLayoutV1::new(
                ((inputs + 1) * 16) as u32,
                ((inputs + 1) * 16) as u32,
                8,
            )
            .unwrap(),
            launch: &launch,
            arguments: &arguments,
        })
        .collect::<Vec<_>>();
    let requirements = kernels
        .iter()
        .map(|k| {
            KernelTargetRequirementsV2::new(
                k.kernel_id,
                LdsRequirementsV2::new(0, 0).unwrap(),
                RequiredWavefrontWidthV2::Wave64,
                false,
                SynchronizationRequirementsV2::empty(),
                AtomicRequirementsV2::empty(),
            )
        })
        .collect::<Vec<_>>();
    let contract_bytes = kernels
        .iter()
        .map(|k| {
            let mut fixture = Fixture::new(inputs, inputs);
            fixture.subjects.kernel_id = *k.kernel_id.as_bytes();
            for (binding, argument) in fixture.arguments.iter_mut().zip(&arguments) {
                binding.source_type_identity = *argument.source_type.as_bytes();
                binding.device_layout_identity = *argument.device_layout.as_bytes();
            }
            customize(&mut fixture);
            fixture.wire()
        })
        .collect::<Vec<_>>();
    let contracts = contract_bytes
        .iter()
        .map(|bytes| decode_conditional_invocation_contract_v1(bytes, &mut free).unwrap())
        .collect::<Vec<_>>();
    consume(DeviceDescriptorTableInputV4 {
        nominal: DeviceDescriptorTableInputV3 {
            canonical_code_object_digest: CanonicalCodeObjectDigest::from_bytes([0; 32]),
            code_object_version: CodeObjectVersion::V6,
            compiler: &compiler,
            producer: &producer,
            device_target: DeviceTargetV1::parse(target).unwrap(),
            type_records: &types,
            layout_records: &layouts,
            kernels: &kernels,
            requirements: &requirements,
        },
        contracts: &contracts,
    })
}
pub fn wire(target: &str, entries: usize, inputs: usize) -> Vec<u8> {
    wire_custom(target, entries, inputs, |_| {})
}

pub fn wire_custom(
    target: &str,
    entries: usize,
    inputs: usize,
    customize: impl FnMut(&mut Fixture),
) -> Vec<u8> {
    with_custom_contracts(target, entries, inputs, customize, |input| {
        let mut bytes = vec![0; encoded_device_descriptor_table_v4_len(&input, &mut free).unwrap()];
        encode_device_descriptor_table_v4(&input, &mut bytes, &mut free).unwrap();
        bytes
    })
}

pub fn mixed_read_alignments(fixture: &mut Fixture) {
    fixture.reads = Fixture::new(1, 2).reads;
    fixture.reads[1].alignment = 1;
    fixture.refresh_premises();
}
