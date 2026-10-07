use super::*;
use fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV3;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBackendScalarV1, SemanticKernelLaunchBoundsV1, SemanticKernelResourceContractV1,
    SemanticKernelSourceContractV1, SemanticWorkgroupDimensionsV1,
};

// Historical leaf decoding exercises inert contract shape only, never proof admission.
fn with_inert_contract(test: impl FnOnce(&SemanticKernelEntryV1, &CompilerDescriptorSourceV1)) {
    let handoff = InertSemanticCompilerModuleHandoffV3::decode(include_bytes!(
        "../../../../../fe2o3-verifier/src/conditional_fill_program_v1/fill.handoff"
    ))
    .unwrap();
    let receipts = handoff.capsule().receipts();
    let semantic = AdmittedInertSemanticMirV1::decode_current_production_canonical(
        receipts.semantic_mir().canonical_preimage(),
        fe2o3_mir_model::semantic_mir_v1::SemanticMirLimitsV1::default(),
    )
    .unwrap();
    let source = CompilerDescriptorSourceV1::decode(receipts.abi().canonical_preimage()).unwrap();
    let entry = semantic.functions()[semantic.roots()[0].index() as usize]
        .kernel_entry()
        .unwrap();
    test(entry, &source);
}

#[test]
fn inert_fill_descriptor_and_entry_produce_a_deterministic_contract() {
    with_inert_contract(|entry, source| {
        let identity = derive_contract(&source.table().kernels()[0], entry).unwrap();
        assert_ne!(identity, [0; 32]);
        assert_eq!(
            identity,
            derive_contract(&source.table().kernels()[0], entry).unwrap()
        );
    });
}

fn descriptor_with(
    original: &KernelDescriptorV1,
    arguments: Vec<LogicalArgumentV1>,
    abi: KernelAbiLayoutV1,
    launch: fe2o3_kernel_descriptor::LaunchConstraintsV1,
) -> KernelDescriptorV1 {
    KernelDescriptorV1::new(
        original.kernel_id(),
        original.logical_name().clone(),
        original.entry_name().clone(),
        original.descriptor_symbol().clone(),
        original.source_evidence(),
        original.executable_ir_evidence(),
        original.capabilities().to_vec(),
        abi,
        launch,
        arguments,
    )
    .unwrap()
}

#[test]
fn descriptor_argument_shape_substitutions_reject() {
    with_inert_contract(|entry, source| {
        let original = &source.table().kernels()[0];
        for (name, scalar, access, offset) in [
            ("wrong", ScalarTypeV1::U32, AccessMode::WriteOnly, 0),
            ("arg0", ScalarTypeV1::I32, AccessMode::WriteOnly, 0),
            ("arg0", ScalarTypeV1::U64, AccessMode::WriteOnly, 0),
            ("arg0", ScalarTypeV1::U32, AccessMode::ReadWrite, 0),
            ("arg0", ScalarTypeV1::U32, AccessMode::ReadOnly, 0),
            ("arg0", ScalarTypeV1::U32, AccessMode::WriteOnly, 8),
        ] {
            let argument = LogicalArgumentV1::disjoint_slice(
                0,
                ValidName::new(name).unwrap(),
                &SourceTypeRecordV1::new(SourceTypeDescriptorV1::disjoint_slice(scalar)),
                &DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::disjoint_slice(scalar)),
                access,
                offset,
            )
            .unwrap();
            let changed = descriptor_with(
                original,
                vec![argument],
                KernelAbiLayoutV1::new(16 + offset, 272, 8).unwrap(),
                original.launch().clone(),
            );
            assert!(matches!(
                derive_contract(&changed, entry),
                Err(Error::CompilerInputs("descriptor ABI"))
            ));
        }
        for abi in [
            KernelAbiLayoutV1::new(16, 280, 8).unwrap(),
            KernelAbiLayoutV1::new(16, 272, 16).unwrap(),
        ] {
            let changed = descriptor_with(
                original,
                original.arguments().to_vec(),
                abi,
                original.launch().clone(),
            );
            assert!(matches!(
                derive_contract(&changed, entry),
                Err(Error::CompilerInputs("descriptor ABI"))
            ));
        }
    });
}

#[test]
fn missing_or_incompatible_source_launch_and_resources_reject() {
    with_inert_contract(|original, source| {
        let descriptor = &source.table().kernels()[0];
        let dim = |x| SemanticWorkgroupDimensionsV1::new([x, 1, 1]).unwrap();
        let bounds = |required, maximum, occupancy| {
            SemanticKernelLaunchBoundsV1::new(required, maximum, occupancy).unwrap()
        };
        for launch in [
            None,
            Some(bounds(Some(dim(256)), None, None)),
            Some(bounds(Some(dim(64)), Some(dim(128)), None)),
            Some(bounds(Some(dim(64)), Some(dim(64)), Some(1))),
            Some(bounds(None, Some(dim(64)), None)),
        ] {
            let entry = SemanticKernelEntryV1::new(
                original.export_symbol().clone(),
                original.kernel_binding_identity(),
                SemanticKernelSourceContractV1::new(launch, None, None).unwrap(),
            );
            assert!(derive_contract(descriptor, &entry).is_err());
        }
        for resource in [
            SemanticKernelResourceContractV1::new(4, 0).unwrap(),
            SemanticKernelResourceContractV1::new(0, 4).unwrap(),
        ] {
            let entry = SemanticKernelEntryV1::new(
                original.export_symbol().clone(),
                original.kernel_binding_identity(),
                SemanticKernelSourceContractV1::new_with_resources(
                    original.source_contract().launch(),
                    Some(resource),
                    None,
                    None,
                )
                .unwrap(),
            );
            assert!(matches!(
                derive_contract(descriptor, &entry),
                Err(Error::CompilerInputs("unsupported resources"))
            ));
        }
        let without_maximum = SemanticKernelEntryV1::new(
            original.export_symbol().clone(),
            original.kernel_binding_identity(),
            SemanticKernelSourceContractV1::new(
                Some(bounds(Some(dim(64)), None, None)),
                None,
                None,
            )
            .unwrap(),
        );
        assert_eq!(
            derive_contract(descriptor, original).unwrap(),
            derive_contract(descriptor, &without_maximum).unwrap()
        );
    });
}

#[test]
fn semantic_slice_abi_requires_exact_primitives_and_alignment() {
    let pointer = SemanticBackendPrimitiveV1::pointer(0, 8, 8);
    let length = SemanticBackendPrimitiveV1::integer(false, 64, 8);
    let layout = |size, alignment, first, second| {
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(size),
            alignment,
            SemanticBackendReprV1::scalar_pair(
                SemanticBackendScalarV1::union(first),
                SemanticBackendScalarV1::union(second),
            ),
            false,
        )
        .unwrap()
    };
    assert!(check_slice_layout(&layout(16, 8, pointer, length)).is_ok());
    for changed in [
        layout(
            32,
            16,
            SemanticBackendPrimitiveV1::pointer(0, 16, 16),
            length,
        ),
        layout(16, 8, SemanticBackendPrimitiveV1::pointer(1, 8, 8), length),
        layout(16, 8, SemanticBackendPrimitiveV1::pointer(0, 4, 4), length),
        layout(16, 8, SemanticBackendPrimitiveV1::pointer(0, 8, 4), length),
        layout(
            16,
            8,
            pointer,
            SemanticBackendPrimitiveV1::integer(true, 64, 8),
        ),
        layout(
            16,
            8,
            pointer,
            SemanticBackendPrimitiveV1::integer(false, 32, 4),
        ),
        layout(
            16,
            8,
            pointer,
            SemanticBackendPrimitiveV1::integer(false, 64, 4),
        ),
        layout(16, 8, length, pointer),
        SemanticTypeLayoutV1::new(Some(16), 8).unwrap(),
    ] {
        assert!(matches!(
            check_slice_layout(&changed),
            Err(Error::CompilerInputs("semantic slice layout"))
        ));
    }
}

#[test]
fn semantic_element_layout_requires_standard_u32_alignment() {
    let layout = |size, alignment, primitive| {
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(size),
            alignment,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::union(primitive)),
            false,
        )
        .unwrap()
    };
    let u32_primitive = SemanticBackendPrimitiveV1::integer(false, 32, 4);
    assert!(check_element_layout(&layout(4, 4, u32_primitive)).is_ok());
    for changed in [
        layout(8, 8, SemanticBackendPrimitiveV1::integer(false, 64, 8)),
        layout(4, 2, SemanticBackendPrimitiveV1::integer(false, 32, 2)),
        layout(4, 4, SemanticBackendPrimitiveV1::integer(true, 32, 4)),
        layout(4, 4, SemanticBackendPrimitiveV1::float(32, 4)),
        SemanticTypeLayoutV1::new(Some(4), 4).unwrap(),
    ] {
        assert!(matches!(
            check_element_layout(&changed),
            Err(Error::CompilerInputs("semantic element layout"))
        ));
    }
}

#[test]
fn descriptor_launch_changes_reject_or_commit_to_changed_grid() {
    use fe2o3_kernel_descriptor::LaunchConstraintsV1;
    with_inert_contract(|entry, source| {
        let original = &source.table().kernels()[0];
        let block = DimensionsV1::new(64, 1, 1).unwrap();
        for (rank, block, max_flat, shared, dynamic) in [
            (2, BlockSizeV1::Exact(block), 64, 0, 0),
            (1, BlockSizeV1::AtMost(block), 64, 0, 0),
            (1, BlockSizeV1::Any, 64, 0, 0),
            (
                1,
                BlockSizeV1::Exact(DimensionsV1::new(32, 1, 1).unwrap()),
                64,
                0,
                0,
            ),
            (1, BlockSizeV1::Exact(block), 128, 0, 0),
            (1, BlockSizeV1::Exact(block), 64, 4, 0),
            (1, BlockSizeV1::Exact(block), 64, 0, 4),
        ] {
            let launch = LaunchConstraintsV1::new(
                rank,
                block,
                original.launch().max_grid(),
                max_flat,
                shared,
                dynamic,
            )
            .unwrap();
            let changed = descriptor_with(
                original,
                original.arguments().to_vec(),
                original.abi_layout(),
                launch,
            );
            assert!(derive_contract(&changed, entry).is_err());
        }
        let changed = descriptor_with(
            original,
            original.arguments().to_vec(),
            original.abi_layout(),
            LaunchConstraintsV1::new(
                1,
                BlockSizeV1::Exact(block),
                DimensionsV1::new(4096, 1, 1).unwrap(),
                64,
                0,
                0,
            )
            .unwrap(),
        );
        assert_ne!(
            derive_contract(&changed, entry).unwrap(),
            derive_contract(original, entry).unwrap()
        );

        // Even inert equality checking cannot substitute a different descriptor source.
        let expected = fe2o3_compiler_lineage::InertAbiReceiptV3::from_canonical_preimage(
            source.canonical_bytes(),
        )
        .unwrap()
        .identity();
        check_source_identity(source, *expected.sha256(), expected.byte_len()).unwrap();
        let table = source.table();
        let replacement = CompilerDescriptorSourceV1::new(
            fe2o3_kernel_descriptor::DeviceDescriptorTableV1::new(
                table.canonical_code_object_digest(),
                table.code_object_version(),
                table.compiler().clone(),
                table.producer().clone(),
                table.device_target(),
                table.type_records().to_vec(),
                table.layout_records().to_vec(),
                vec![changed],
            )
            .unwrap(),
        )
        .unwrap();
        assert!(matches!(
            check_source_identity(&replacement, *expected.sha256(), expected.byte_len()),
            Err(Error::CompilerInputs("original descriptor source"))
        ));
    });
}

#[test]
fn mismatched_semantic_entry_and_additional_capabilities_reject() {
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticKernelBindingIdentityV1, SemanticLinkSymbolV1, SemanticReachableAssemblyV1,
        SemanticUnsafeAssemblyDeclarationV1, SemanticUnsafeAssemblyTargetV1,
    };
    with_inert_contract(|entry, source| {
        let descriptor = &source.table().kernels()[0];
        for changed in [
            SemanticKernelEntryV1::new(
                SemanticLinkSymbolV1::new(b"other".to_vec()).unwrap(),
                entry.kernel_binding_identity(),
                entry.source_contract(),
            ),
            SemanticKernelEntryV1::new(
                entry.export_symbol().clone(),
                SemanticKernelBindingIdentityV1::from_sha256([17; 32]),
                entry.source_contract(),
            ),
        ] {
            assert!(matches!(
                derive_contract(descriptor, &changed),
                Err(Error::CompilerInputs("semantic entry binding"))
            ));
        }
        for capabilities in [
            vec![],
            vec![CapabilityV1::Subgroup, CapabilityV1::AmdWave],
            vec![CapabilityV1::Atomics, CapabilityV1::AmdWave],
        ] {
            let changed = KernelDescriptorV1::new(
                descriptor.kernel_id(),
                descriptor.logical_name().clone(),
                descriptor.entry_name().clone(),
                descriptor.descriptor_symbol().clone(),
                descriptor.source_evidence(),
                descriptor.executable_ir_evidence(),
                capabilities,
                descriptor.abi_layout(),
                descriptor.launch().clone(),
                descriptor.arguments().to_vec(),
            )
            .unwrap();
            assert!(matches!(
                derive_contract(&changed, entry),
                Err(Error::CompilerInputs("unsupported resources"))
            ));
        }
        let changed = SemanticKernelEntryV1::new(
            entry.export_symbol().clone(),
            entry.kernel_binding_identity(),
            SemanticKernelSourceContractV1::new(
                entry.source_contract().launch(),
                Some(
                    SemanticUnsafeAssemblyDeclarationV1::new(
                        SemanticUnsafeAssemblyTargetV1::AmdGpuGfx942,
                        1,
                        1,
                        0,
                    )
                    .unwrap(),
                ),
                Some(SemanticReachableAssemblyV1::new(1, 1, 1, 0).unwrap()),
            )
            .unwrap(),
        );
        assert!(matches!(
            derive_contract(descriptor, &changed),
            Err(Error::CompilerInputs("unsupported resources"))
        ));
    });
}
