use super::*;

fn frontend_contract_with_assembly(
    launch: Option<fe2o3_rustc_front::FrontendLaunchBoundsV1>,
    resource: Option<fe2o3_rustc_front::KernelResourceContractV1>,
) -> super::super::AuthenticatedKernelFrontendContractV1 {
    use fe2o3_rustc_front::{
        ASSEMBLY_OPERAND_SGPR_V1, ASSEMBLY_OPTION_NOMEM_V1, FrontendUnsafeAssemblyDeclarationV1,
        FrontendUnsafeAssemblyTargetV1, KernelFrontendContractV1,
    };
    let contract = KernelFrontendContractV1::new(
        launch,
        Some(
            FrontendUnsafeAssemblyDeclarationV1::new(
                FrontendUnsafeAssemblyTargetV1::AmdGpuGfx942,
                ASSEMBLY_OPERAND_SGPR_V1,
                ASSEMBLY_OPTION_NOMEM_V1,
                0,
            )
            .unwrap(),
        ),
    )
    .unwrap();
    let mut authenticated = match resource {
        Some(resource) => {
            super::super::AuthenticatedKernelFrontendContractV1::for_test_with_resource(
                contract, resource,
            )
        }
        None => super::super::AuthenticatedKernelFrontendContractV1::for_test(contract),
    };
    authenticated.reachable_assembly = super::super::ReachableAssemblySummaryV1 {
        blocks: 1,
        operand_bits: ASSEMBLY_OPERAND_SGPR_V1,
        option_bits: ASSEMBLY_OPTION_NOMEM_V1,
    };
    authenticated
}

#[test]
fn semantic_source_launch_resolves_only_authenticated_typed_defaults() {
    let typed = Some(reserved_fe2o3_symbols::GeneratedHostContractIdV3::from_bytes([7; 32]));
    let launchless_frontend = frontend_contract_with_assembly(None, None);
    let descriptor = super::super::general_typed_launch_v3(None, "test").unwrap();
    let fe2o3_artifacts::BlockSize::Exact(block) = descriptor.block_size() else {
        panic!("typed default must be exact");
    };
    let expected = [block.x(), block.y(), block.z()];
    assert_eq!(expected, [256, 1, 1]);
    for frontend in [None, Some(&launchless_frontend)] {
        let resolved = semantic_kernel_source_contract_v1(frontend, typed).unwrap();
        let launch = resolved.launch().unwrap();
        assert_eq!(launch.required().unwrap().as_array(), expected);
        assert_eq!(launch.maximum().unwrap().as_array(), expected);
        assert!(launch.min_workgroups_per_compute_unit().is_none());
        assert_eq!(resolved.unsafe_assembly().is_some(), frontend.is_some());
        assert_eq!(resolved.reachable_assembly().is_some(), frontend.is_some());
        assert!(
            semantic_kernel_source_contract_v1(frontend, None)
                .unwrap()
                .launch()
                .is_none()
        );
    }
}

#[test]
fn semantic_source_launch_preserves_explicit_bounds_without_defaulting() {
    use fe2o3_rustc_front::{
        FrontendLaunchBoundsV1, FrontendWorkgroupDimensionsV1, KernelFrontendContractV1,
    };
    let typed = Some(reserved_fe2o3_symbols::GeneratedHostContractIdV3::from_bytes([9; 32]));
    for (required, maximum, minimum_groups) in [
        (Some([3, 1, 1]), None, None),
        (Some([64, 1, 1]), Some([64, 1, 1]), None),
        (Some([255, 1, 1]), Some([256, 1, 1]), None),
        (None, Some([128, 1, 1]), None),
        (Some([64, 1, 1]), Some([64, 1, 1]), Some(2)),
    ] {
        let frontend = super::super::AuthenticatedKernelFrontendContractV1::for_test(
            KernelFrontendContractV1::new(
                Some(
                    FrontendLaunchBoundsV1::new(
                        required.map(|value| FrontendWorkgroupDimensionsV1::new(value).unwrap()),
                        maximum.map(|value| FrontendWorkgroupDimensionsV1::new(value).unwrap()),
                        minimum_groups,
                    )
                    .unwrap(),
                ),
                None,
            )
            .unwrap(),
        );
        for identity in [None, typed] {
            let resolved = semantic_kernel_source_contract_v1(Some(&frontend), identity).unwrap();
            let launch = resolved.launch().unwrap();
            assert_eq!(launch.required().map(|value| value.as_array()), required);
            assert_eq!(launch.maximum().map(|value| value.as_array()), maximum);
            assert_eq!(launch.min_workgroups_per_compute_unit(), minimum_groups);
        }
    }
}

#[test]
fn semantic_source_launch_preserves_resources_and_assembly_and_rejects_mismatch() {
    use fe2o3_rustc_front::{FrontendLaunchBoundsV1, FrontendWorkgroupDimensionsV1};
    let dimensions = FrontendWorkgroupDimensionsV1::new([64, 1, 1]).unwrap();
    let explicit = FrontendLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None).unwrap();
    let typed = Some(reserved_fe2o3_symbols::GeneratedHostContractIdV3::from_bytes([11; 32]));
    for launch in [None, Some(explicit)] {
        let mut frontend = frontend_contract_with_assembly(
            launch,
            Some(fe2o3_rustc_front::KernelResourceContractV1::new(256, 1024).unwrap()),
        );
        let untyped = semantic_kernel_source_contract_v1(Some(&frontend), None).unwrap();
        let resolved = semantic_kernel_source_contract_v1(Some(&frontend), typed).unwrap();
        let expected = if launch.is_some() {
            [64, 1, 1]
        } else {
            [256, 1, 1]
        };
        assert_eq!(
            resolved.launch().unwrap().required().unwrap().as_array(),
            expected
        );
        let resources = resolved.resources().unwrap();
        assert_eq!(resources.static_shared_memory_bytes(), 256);
        assert_eq!(resources.max_dynamic_shared_memory_bytes(), 1024);
        assert_eq!(resolved.resources(), untyped.resources());
        assert_eq!(resolved.unsafe_assembly(), untyped.unsafe_assembly());
        assert_eq!(resolved.reachable_assembly(), untyped.reachable_assembly());
        assert!(resolved.unsafe_assembly().is_some());
        assert!(resolved.reachable_assembly().is_some());

        frontend.reachable_assembly.operand_bits = 0;
        for identity in [None, typed] {
            assert!(matches!(
                semantic_kernel_source_contract_v1(Some(&frontend), identity),
                Err(ProductionSemanticImportErrorV1::SemanticSchema(_))
            ));
        }
    }
}

fn assert_export_table_error(
    result: Result<ProductionSemanticFunctionExportV1, ProductionSemanticImportErrorV1>,
    expected: &str,
) {
    match result {
        Err(ProductionSemanticImportErrorV1::BodyConstruction(error)) => {
            assert!(matches!(
                *error,
                ProductionSemanticBodyErrorV1::IdentityTableMismatch { table } if table == expected
            ));
        }
        other => panic!("expected {expected} rejection, got {other:?}"),
    }
}

#[test]
fn semantic_source_launch_marker_keeps_export_guards_and_error_order() {
    let typed = Some(reserved_fe2o3_symbols::GeneratedHostContractIdV3::from_bytes([13; 32]));
    let binding = Some(reserved_fe2o3_symbols::KernelBindingIdV1::from_bytes(
        [14; 32],
    ));
    for role in [
        CollectedFunctionRole::InternalHelper,
        CollectedFunctionRole::DeviceFfiExport,
    ] {
        for symbol in [None, Some("kernel")] {
            assert_export_table_error(
                semantic_function_export_metadata_v1(role, symbol, None, typed, None),
                "function export metadata",
            );
        }
    }
    assert!(matches!(
        semantic_function_export_metadata_v1(
            CollectedFunctionRole::InternalHelper,
            None,
            None,
            None,
            None,
        )
        .unwrap(),
        ProductionSemanticFunctionExportV1::None
    ));
    assert!(matches!(
        semantic_function_export_metadata_v1(
            CollectedFunctionRole::DeviceFfiExport,
            Some("ffi"),
            None,
            None,
            None,
        )
        .unwrap(),
        ProductionSemanticFunctionExportV1::DeviceFfi(_)
    ));
    assert_export_table_error(
        semantic_function_export_metadata_v1(
            CollectedFunctionRole::DeviceFfiExport,
            None,
            None,
            None,
            None,
        ),
        "function export symbol",
    );
    for identity in [None, typed] {
        assert_export_table_error(
            semantic_function_export_metadata_v1(
                CollectedFunctionRole::KernelEntry,
                None,
                None,
                identity,
                None,
            ),
            "kernel binding identity",
        );
        assert_export_table_error(
            semantic_function_export_metadata_v1(
                CollectedFunctionRole::KernelEntry,
                None,
                binding,
                identity,
                None,
            ),
            "function export symbol",
        );
        let ProductionSemanticFunctionExportV1::Kernel(entry) =
            semantic_function_export_metadata_v1(
                CollectedFunctionRole::KernelEntry,
                Some("kernel"),
                binding,
                identity,
                None,
            )
            .unwrap()
        else {
            panic!("kernel metadata must produce a kernel export");
        };
        assert_eq!(
            entry.source_contract().launch().is_some(),
            identity.is_some()
        );
    }
}

fn framed_identity_fields(fields: &[&[u8]]) -> Vec<u8> {
    let mut framed = Vec::new();
    for field in fields {
        framed.extend_from_slice(&(field.len() as u64).to_le_bytes());
        framed.extend_from_slice(field);
    }
    framed
}

#[test]
fn semantic_source_launch_inventory_uses_exact_v2_domain() {
    use fe2o3_mir_model::semantic_mir_v1::SemanticLayoutIdentityV1;
    use sha2::{Digest, Sha256};

    let target =
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([31; 32]));
    let (identity, transcript) = identity_inventory_identity_and_transcript_v1(target, &[], &[]);
    let expected =
        framed_identity_fields(&[b"fe2o3/semantic-mir/rustc-identity-inventory/v2", &[31; 32]]);
    assert_eq!(transcript.as_ref(), expected);
    assert_eq!(identity, <[u8; 32]>::from(Sha256::digest(&expected)));
    let old =
        framed_identity_fields(&[b"fe2o3/semantic-mir/rustc-identity-inventory/v1", &[31; 32]]);
    assert_ne!(identity, <[u8; 32]>::from(Sha256::digest(old)));
}

#[test]
fn semantic_source_launch_inventory_marker_frames_presence_and_every_identity_bit() {
    use reserved_fe2o3_symbols::GeneratedHostContractIdV3;
    use sha2::{Digest, Sha256};

    let observe = |identity| {
        let mut digest =
            SemanticIdentityDigestV1::new_with_canonical_transcript(IDENTITY_INVENTORY_DOMAIN_V2);
        identity_inventory_generated_host_contract_v1(&mut digest, identity);
        digest.finish_with_canonical_transcript()
    };
    let absent = observe(None);
    let zero = observe(Some(GeneratedHostContractIdV3::from_bytes([0; 32])));
    assert_ne!(absent.0, zero.0);
    for (observed, fields) in [
        (&absent, vec![IDENTITY_INVENTORY_DOMAIN_V2, &[0]]),
        (&zero, vec![IDENTITY_INVENTORY_DOMAIN_V2, &[1], &[0; 32]]),
    ] {
        let expected = framed_identity_fields(&fields);
        assert_eq!(observed.1.as_ref(), expected);
        assert_eq!(observed.0, <[u8; 32]>::from(Sha256::digest(expected)));
    }
    for bit in 0..256 {
        let mut bytes = [0; 32];
        bytes[bit / 8] = 1 << (bit % 8);
        let observed = observe(Some(GeneratedHostContractIdV3::from_bytes(bytes)));
        let expected = framed_identity_fields(&[IDENTITY_INVENTORY_DOMAIN_V2, &[1], &bytes]);
        assert_eq!(observed.1.as_ref(), expected);
        assert_eq!(observed.0, <[u8; 32]>::from(Sha256::digest(expected)));
        assert_ne!(observed.0, zero.0);
        assert_ne!(observed.1, zero.1);
        assert_ne!(observed.0, absent.0);
    }
}
