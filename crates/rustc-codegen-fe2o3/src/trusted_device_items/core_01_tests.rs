    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::{
        CompilerProviderObservationV1, HALF_MATH_DIAGNOSTIC_ITEMS,
        ReviewedProviderSemanticDefinitionV1, ReviewedSafeCoreF32IsFiniteContractV1,
        ReviewedSafeCoreF32IsFiniteRouteBodyContractV1, TrustedAmdGpuDiagnosticOperation,
        TrustedAmdGpuInlineOperation, TrustedDeviceItem, TrustedHalfOperation,
        WORKGROUP_SYNC_PROVIDER_SOURCE_CLOSURE_DOMAIN_V1,
        WORKGROUP_SYNC_PROVIDER_SOURCE_IDENTITY_DOMAIN_V1,
        authenticate_reviewed_safe_core_f32_is_finite_contract_v1,
        authenticate_reviewed_safe_core_f32_is_finite_route_body_contract_v1,
        canonical_compiler_definition_path, exact_provider_compiler_definition_path_v1,
        pinned_core_semantic_terminal_identity_v1,
        reviewed_provider_source_closure_from_definition,
        reviewed_provider_source_closure_identity, reviewed_provider_source_identity_from_path,
        safe_execution_compiler_definition_path, safe_execution_provider_bound_item,
        sort_reviewed_source_files_by_relative_path, structural_local_definition_component_v1,
        validate_compiled_provider_source_hash_v1,
        validate_reviewed_fe2o3_device_provider_definition_v1,
    };
    use dialect_amdgcn::{DeviceMathDiagnosticItem, DeviceValueDiagnosticItem};
    use rustc_span::{SourceFileHash, SourceFileHashAlgorithm};

    #[test]
    fn safe_core_f32_is_finite_contract_is_exact_and_closed() {
        let reviewed = ReviewedSafeCoreF32IsFiniteContractV1 {
            item_instance: true,
            core_lang_crate: true,
            crate_name: "core",
            generic_arguments: 0,
            mir_available: true,
            canonical_path: "core::f32::<impl f32>::is_finite",
            safe_signature: true,
            rust_abi: true,
            variadic: false,
            input_f32: true,
            output_bool: true,
            exact_reviewed_body: true,
        };
        assert!(authenticate_reviewed_safe_core_f32_is_finite_contract_v1(
            reviewed
        ));

        for hostile in [
            ReviewedSafeCoreF32IsFiniteContractV1 {
                canonical_path: "core::f32::<impl f32>::is_finite_near",
                ..reviewed
            },
            ReviewedSafeCoreF32IsFiniteContractV1 {
                core_lang_crate: false,
                ..reviewed
            },
            ReviewedSafeCoreF32IsFiniteContractV1 {
                crate_name: "user_core",
                ..reviewed
            },
            ReviewedSafeCoreF32IsFiniteContractV1 {
                item_instance: false,
                ..reviewed
            },
            ReviewedSafeCoreF32IsFiniteContractV1 {
                generic_arguments: 1,
                ..reviewed
            },
            ReviewedSafeCoreF32IsFiniteContractV1 {
                mir_available: false,
                ..reviewed
            },
            ReviewedSafeCoreF32IsFiniteContractV1 {
                safe_signature: false,
                ..reviewed
            },
            ReviewedSafeCoreF32IsFiniteContractV1 {
                rust_abi: false,
                ..reviewed
            },
            ReviewedSafeCoreF32IsFiniteContractV1 {
                variadic: true,
                ..reviewed
            },
            ReviewedSafeCoreF32IsFiniteContractV1 {
                input_f32: false,
                ..reviewed
            },
            ReviewedSafeCoreF32IsFiniteContractV1 {
                output_bool: false,
                ..reviewed
            },
            ReviewedSafeCoreF32IsFiniteContractV1 {
                exact_reviewed_body: false,
                ..reviewed
            },
        ] {
            assert!(
                !authenticate_reviewed_safe_core_f32_is_finite_contract_v1(hostile),
                "hostile near-name/provider/kind/generic/body/ABI/type mutation was admitted: {hostile:?}",
            );
        }
    }

    #[test]
    fn safe_core_f32_is_finite_route_body_contract_is_exact_and_closed() {
        let reviewed = ReviewedSafeCoreF32IsFiniteRouteBodyContractV1 {
            argument_count: 1,
            local_count: 3,
            block_count: 2,
            source_scope_count: 1,
            exact_local_types: true,
            root_scope_not_inlined: true,
            entry_has_no_statements: true,
            exact_abs_callee: true,
            copies_input: true,
            writes_absolute_temporary: true,
            comparison_target: true,
            unwind_unreachable: true,
            exact_less_than_infinity: true,
            returns_result: true,
        };
        assert!(authenticate_reviewed_safe_core_f32_is_finite_route_body_contract_v1(reviewed));

        for hostile in [
            ReviewedSafeCoreF32IsFiniteRouteBodyContractV1 {
                argument_count: 2,
                ..reviewed
            },
            ReviewedSafeCoreF32IsFiniteRouteBodyContractV1 {
                local_count: 4,
                ..reviewed
            },
            ReviewedSafeCoreF32IsFiniteRouteBodyContractV1 {
                block_count: 3,
                ..reviewed
            },
            ReviewedSafeCoreF32IsFiniteRouteBodyContractV1 {
                source_scope_count: 2,
                ..reviewed
            },
            ReviewedSafeCoreF32IsFiniteRouteBodyContractV1 {
                exact_local_types: false,
                ..reviewed
            },
            ReviewedSafeCoreF32IsFiniteRouteBodyContractV1 {
                root_scope_not_inlined: false,
                ..reviewed
            },
            ReviewedSafeCoreF32IsFiniteRouteBodyContractV1 {
                entry_has_no_statements: false,
                ..reviewed
            },
            ReviewedSafeCoreF32IsFiniteRouteBodyContractV1 {
                exact_abs_callee: false,
                ..reviewed
            },
            ReviewedSafeCoreF32IsFiniteRouteBodyContractV1 {
                copies_input: false,
                ..reviewed
            },
            ReviewedSafeCoreF32IsFiniteRouteBodyContractV1 {
                writes_absolute_temporary: false,
                ..reviewed
            },
            ReviewedSafeCoreF32IsFiniteRouteBodyContractV1 {
                comparison_target: false,
                ..reviewed
            },
            ReviewedSafeCoreF32IsFiniteRouteBodyContractV1 {
                unwind_unreachable: false,
                ..reviewed
            },
            ReviewedSafeCoreF32IsFiniteRouteBodyContractV1 {
                exact_less_than_infinity: false,
                ..reviewed
            },
            ReviewedSafeCoreF32IsFiniteRouteBodyContractV1 {
                returns_result: false,
                ..reviewed
            },
        ] {
            assert!(
                !authenticate_reviewed_safe_core_f32_is_finite_route_body_contract_v1(hostile),
                "hostile route-form call/type/body/CFG mutation was admitted: {hostile:?}",
            );
        }
    }

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

    struct ProviderPackageFixture {
        root: PathBuf,
    }

    impl ProviderPackageFixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "fe2o3-provider-profile-{}-{}",
                std::process::id(),
                NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(root.join("src/nested")).unwrap();
            fs::write(root.join("Cargo.toml"), b"[package]\nname='fixture'\n").unwrap();
            fs::write(root.join("src/lib.rs"), b"pub mod nested;\n").unwrap();
            fs::write(root.join("src/nested/mod.rs"), b"pub fn value() {}\n").unwrap();
            Self { root }
        }

        fn source_root(&self) -> PathBuf {
            self.root.join("src")
        }

        fn definition(&self) -> PathBuf {
            self.source_root().join("lib.rs")
        }
    }

    impl Drop for ProviderPackageFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn digest(hex: &str) -> [u8; 32] {
        assert_eq!(hex.len(), 64);
        let mut output = [0_u8; 32];
        for (byte, pair) in output.iter_mut().zip(hex.as_bytes().chunks_exact(2)) {
            *byte = u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap();
        }
        output
    }

    fn semantic_definition(
        path: &str,
        source_closure_identity: [u8; 32],
        definition_source_identity: [u8; 32],
    ) -> ReviewedProviderSemanticDefinitionV1 {
        ReviewedProviderSemanticDefinitionV1 {
            provider: CompilerProviderObservationV1 {
                crate_name: "fe2o3_device".into(),
                stable_crate_id: 7,
                crate_hash_observation: [3; 16],
            },
            canonical_definition_path: format!("fe2o3_device::{path}"),
            structural_local_definition_component: structural_local_definition_component_v1(path)
                .unwrap(),
            cargo_metadata_build_observation: [4; 32],
            source_closure_identity,
            definition_source_identity,
        }
    }

    #[test]
    fn provider_semantic_identity_excludes_volatile_compilation_disambiguators() {
        fn identity(definition: &ReviewedProviderSemanticDefinitionV1) -> Result<[u8; 32], String> {
            definition.durable_semantic_identity("fe2o3_device::thread::thread_idx_x")
        }

        let definition = ReviewedProviderSemanticDefinitionV1 {
            provider: CompilerProviderObservationV1 {
                crate_name: "fe2o3_device".into(),
                stable_crate_id: 7,
                crate_hash_observation: [3; 16],
            },
            canonical_definition_path: "fe2o3_device::thread::thread_idx_x".into(),
            structural_local_definition_component: structural_local_definition_component_v1(
                "thread::thread_idx_x",
            )
            .unwrap(),
            cargo_metadata_build_observation: [4; 32],
            source_closure_identity: [5; 32],
            definition_source_identity: [6; 32],
        };
        let exact = identity(&definition).expect("complete provider semantic identity");
        assert_eq!(
            exact,
            digest("36349edbdabe77499ba36d983bf758f7c00e982d7fbd930397042192af1e7416")
        );

        let mut mutation = definition.clone();
        mutation.provider.stable_crate_id ^= 1;
        assert_ne!(mutation.provider, definition.provider);
        assert_eq!(identity(&mutation).unwrap(), exact);
        mutation = definition.clone();
        mutation.provider.crate_hash_observation[0] ^= 1;
        assert_ne!(mutation.provider, definition.provider);
        assert_eq!(identity(&mutation).unwrap(), exact);

        mutation = definition.clone();
        mutation.provider.crate_name = "fake_fe2o3_device".into();
        assert!(identity(&mutation).is_err());
        mutation = definition.clone();
        mutation.canonical_definition_path = "fe2o3_device::thread::block_idx_x".into();
        assert!(identity(&mutation).is_err());
        mutation = definition.clone();
        mutation.canonical_definition_path = "fe2o3_device::thread::block_idx_x".into();
        mutation.structural_local_definition_component =
            structural_local_definition_component_v1("thread::block_idx_x").unwrap();
        assert_ne!(identity(&mutation).unwrap(), exact);
        mutation = definition.clone();
        mutation.structural_local_definition_component[0] ^= 1;
        assert!(identity(&mutation).is_err());
        mutation = definition.clone();
        mutation.cargo_metadata_build_observation[0] ^= 1;
        assert_ne!(identity(&mutation).unwrap(), exact);
        mutation = definition.clone();
        mutation.source_closure_identity[0] ^= 1;
        assert_ne!(identity(&mutation).unwrap(), exact);
        mutation = definition.clone();
        mutation.definition_source_identity[0] ^= 1;
        assert_ne!(identity(&mutation).unwrap(), exact);
        mutation = definition.clone();
        mutation.source_closure_identity = [0; 32];
        assert!(identity(&mutation).is_err());
        mutation = definition.clone();
        mutation.definition_source_identity = [0; 32];
        assert!(identity(&mutation).is_err());
        mutation = definition.clone();
        mutation.cargo_metadata_build_observation = [0; 32];
        assert!(identity(&mutation).is_err());
        mutation = definition.clone();
        mutation.provider.stable_crate_id = 0;
        assert!(identity(&mutation).is_err());
        mutation = definition.clone();
        mutation.provider.crate_hash_observation = [0; 16];
        assert!(identity(&mutation).is_err());
        assert_eq!(
            definition
                .durable_semantic_identity("fe2o3_device::thread::thread_idx_x")
                .unwrap(),
            exact
        );
        assert!(definition.durable_semantic_identity("").is_err());
        assert_ne!(
            definition
                .durable_semantic_identity("fe2o3_device::thread::block_idx_x",)
                .unwrap(),
            exact
        );
    }
