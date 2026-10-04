fn mixed_source_contract_descriptor_v26(
    root: &ProductionKernelArgumentAbiRootV18<'_>,
    fault: u8,
) -> Result<Vec<u8>, fe2o3_kernel_descriptor::DescriptorWireErrorV3<ArgumentResourceV1>> {
    use fe2o3_kernel_descriptor::*;
    let mut free = |_: usize| Ok::<(), ArgumentResourceV1>(());
    let mut sources = Vec::new();
    let mut layouts = Vec::new();
    let mut components = Vec::new();
    let mut arguments = Vec::new();
    for input in root.arguments {
        if let ProductionKernelArgumentAbiKindV18::Descriptor { source, argument } = &input.kind {
            let source = if fault == 2 && arguments.is_empty() {
                match source {
                    SourceTypeDescriptorV3::SharedSlice(_) => {
                        SourceTypeDescriptorV3::SharedSlice(ScalarTypeV1::U64)
                    }
                    SourceTypeDescriptorV3::DisjointSlice(_) => {
                        SourceTypeDescriptorV3::DisjointSlice(ScalarTypeV1::U64)
                    }
                    other => *other,
                }
            } else {
                *source
            };
            let layout = match source {
                SourceTypeDescriptorV3::Scalar(s) => DeviceLayoutDescriptorV1::scalar(s),
                SourceTypeDescriptorV3::SharedSlice(s) => DeviceLayoutDescriptorV1::shared_slice(s),
                SourceTypeDescriptorV3::DisjointSlice(s) => {
                    DeviceLayoutDescriptorV1::disjoint_slice(s)
                }
                SourceTypeDescriptorV3::GlobalMutPointer(s) => {
                    DeviceLayoutDescriptorV1::global_mut_pointer(s)
                }
                SourceTypeDescriptorV3::Usize => {
                    DeviceLayoutDescriptorV1::scalar(ScalarTypeV1::U64)
                }
                SourceTypeDescriptorV3::Isize => {
                    DeviceLayoutDescriptorV1::scalar(ScalarTypeV1::I64)
                }
            };
            sources.push(SourceTypeRecordV3::new(source, &mut free).unwrap());
            let layout = if fault == 7 && arguments.is_empty() {
                DeviceLayoutDescriptorV1::shared_slice(source.physical_scalar())
            } else {
                layout
            };
            layouts.push(device_layout_record_v3(layout, &mut free).unwrap());
            components.push(
                argument
                    .physical_components()
                    .map(|(kind, offset, size, alignment)| PhysicalComponentV3 {
                        kind,
                        offset,
                        size,
                        alignment,
                        access: if kind == PhysicalAbiComponentKind::GlobalPointer {
                            argument.access()
                        } else {
                            AccessMode::ByValue
                        },
                        alias: if kind == PhysicalAbiComponentKind::GlobalPointer {
                            argument.alias()
                        } else {
                            AliasSemantics::Value
                        },
                    })
                    .collect::<Vec<_>>(),
            );
            arguments.push(argument);
        }
    }
    if fault == 5 {
        components[0][0].access = AccessMode::ByValue;
    }
    if fault == 6 {
        components[0][0].alias = AliasSemantics::Value;
    }
    let fields: Vec<_> = arguments
        .iter()
        .enumerate()
        .map(|(index, argument)| LogicalArgumentInputV3 {
            source_index: index.try_into().unwrap(),
            name: argument.name().as_str(),
            source_type: sources[index].identity(),
            device_layout: layouts[index].identity(),
            ownership: argument.ownership(),
            access: argument.access(),
            alias: argument.alias(),
            components: &components[index],
        })
        .collect();
    sources.sort_by_key(|source| source.identity());
    sources.dedup_by_key(|source| source.identity());
    layouts.sort_by_key(|layout| layout.identity());
    layouts.dedup_by_key(|layout| layout.identity());
    let launch = LaunchConstraintsV1::new(
        1,
        BlockSizeV1::Exact(DimensionsV1::new(64, 1, 1).unwrap()),
        DimensionsV1::new(if fault == 3 { 2 } else { 1 }, 1, 1).unwrap(),
        64,
        0,
        0,
    )
    .unwrap();
    let mut id = *root.kernel_binding;
    if fault == 1 {
        id[0] ^= 1;
    }
    let id = KernelId::from_bytes(id);
    let evidence = BuildEvidenceV1::new(
        EvidenceIdentity::from_opaque_bytes([1; 32]),
        EvidenceDigest::from_sha256_bytes([2; 32]),
    );
    let kernels = [KernelDescriptorInputV3 {
        kernel_id: id,
        logical_name: root.export,
        entry_name: root.export,
        descriptor_symbol: "mixed_contract_test.kd",
        source_evidence: evidence,
        executable_ir_evidence: evidence,
        capabilities: &[CapabilityV1::AmdWave],
        abi_layout: KernelAbiLayoutV1::new(
            root.explicit_argument_bytes,
            root.explicit_argument_bytes + 256,
            root.kernarg_alignment_bytes,
        )
        .unwrap(),
        launch: &launch,
        arguments: &fields,
    }];
    let requirements = [KernelTargetRequirementsV2::new(
        id,
        LdsRequirementsV2::new(0, 0).unwrap(),
        RequiredWavefrontWidthV2::Wave64,
        false,
        SynchronizationRequirementsV2::from_bits(0).unwrap(),
        AtomicRequirementsV2::from_bits(0).unwrap(),
    )];
    let compiler = CompilerIdentityV1::new(
        Text::new("rustc").unwrap(),
        Text::new("source-contract-test").unwrap(),
        [7; 20],
    );
    let producer = ProducerIdentityV1::new(
        Text::new("fe2o3").unwrap(),
        Text::new("source-contract-test").unwrap(),
    );
    let table = DeviceDescriptorTableInputV3 {
        canonical_code_object_digest: CanonicalCodeObjectDigest::from_bytes([0; 32]),
        code_object_version: CodeObjectVersion::V6,
        compiler: &compiler,
        producer: &producer,
        device_target: DeviceTargetV1::parse("gfx942:xnack-").unwrap(),
        type_records: &sources,
        layout_records: &layouts,
        kernels: &kernels,
        requirements: &requirements,
    };
    let n = encoded_device_descriptor_table_v3_len(&table, &mut free)?;
    let mut bytes = vec![0; n];
    encode_device_descriptor_table_v3(&table, &mut bytes, &mut free)?;
    Ok(bytes)
}
