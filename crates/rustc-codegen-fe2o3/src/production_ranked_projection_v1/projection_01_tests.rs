    use super::*;
    use fe2o3_mir_model::SemanticOptionProducerV1;
    use fe2o3_mir_model::semantic_mir_v1::*;

    const SCALAR_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
    const ARRAY_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
    const POINTER_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
    const ENUM_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
    const BOOL_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
    const U64_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);
    const U8_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(6);
    const I32_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(7);
    const U64_POINTER_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(8);
    const CHECKED_U64_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(9);
    const CHECKED_U64_POINTER_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(10);
    const F64_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(11);
    const U128_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(12);
    const U16_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(13);
    const CHAR_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(14);
    const F32_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(15);
    const VALIDITY_U32_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(16);

    #[test]
    fn blocked_launch_bound_checks_the_last_thread_and_component_without_wrapping() {
        assert!(blocked_mapping_fits_launch_v1(Some(64), 16, 4));
        assert!(!blocked_mapping_fits_launch_v1(None, 16, 4));
        assert!(!blocked_mapping_fits_launch_v1(Some(64), 0, 4));
        assert!(!blocked_mapping_fits_launch_v1(Some(64), 16, 0));
        assert!(!blocked_mapping_fits_launch_v1(Some(64), u64::MAX, 2));
        assert!(blocked_mapping_fits_launch_v1(Some(1_u64 << 62), 16, 4));
        assert!(!blocked_mapping_fits_launch_v1(
            Some((1_u64 << 62) + 1),
            16,
            4,
        ));
    }

    #[test]
    fn authenticated_ranked_projection_exposes_v5_and_inert_induction_evidence() {
        let middle_end_accessor: for<'a> fn(
            &'a AuthenticatedRankedVerificationV5,
        )
            -> &'a fe2o3_pliron::ProductionMiddleEndEvidenceV5 =
            AuthenticatedRankedVerificationV5::middle_end_evidence;
        let induction_accessor: for<'a> fn(
            &'a AuthenticatedRankedVerificationV5,
        ) -> &'a fe2o3_mir_model::SemanticU32InductionNoOverflowReportV1 =
            AuthenticatedRankedVerificationV5::semantic_u32_induction;
        let _ = (middle_end_accessor, induction_accessor);
    }

    #[test]
    fn ranked_roster_receipt_source_preserves_linear_stage_boundaries() {
        let source = include_str!("../production_ranked_projection_v1.rs");
        let receipt = source
            .split("pub struct ProductionRankedSemanticProjectionRosterReceiptV1 {")
            .nth(1)
            .expect("ranked roster receipt declaration")
            .split('}')
            .next()
            .expect("ranked roster receipt fields");
        for retained in [
            "semantic_ssa_owner: ProductionSemanticSsaOwnerV1",
            "source_order_roots: Box<[ProductionRankedVerifiedRootCandidateV1]>",
            "canonical_kernel_order: Box<[usize]>",
            "canonical_roster_identity: ProductionRankedKernelRosterIdentityV1",
        ] {
            assert!(
                receipt.contains(retained),
                "missing custody field: {retained}"
            );
        }
        assert!(!receipt.contains("pub ") && !receipt.contains("pub(crate)"));

        let verified_root = source
            .split("pub(crate) struct ProductionRankedVerifiedRootCandidateV1 {")
            .nth(1)
            .expect("verified ranked root declaration")
            .split('}')
            .next()
            .expect("verified ranked root fields");
        assert!(verified_root.contains("verification: AuthenticatedRankedVerificationV5"));
        assert!(!verified_root.contains("semantic_u32_induction:"));
        let verifier = source
            .split("fn authenticate_ranked_root_v5(")
            .nth(1)
            .expect("ranked root verifier")
            .split("fn ranked_roster_identity_records_v1")
            .next()
            .expect("bounded ranked root verifier");
        assert!(verifier.contains("semantic_u32_induction,"));

        let module = source
            .split("pub(crate) fn into_module_verified_receipt(")
            .nth(1)
            .expect("complete module transition")
            .split("impl ProductionRankedSemanticProgramV1")
            .next()
            .expect("bounded complete module transition");
        assert!(module.contains("for root in source_order_roots.into_vec()"));
        assert!(module.contains("from_unvalidated_ssa_projection_roster_candidate"));
        assert!(module.contains("AuthenticatedRankedVerificationRosterV1"));
        assert!(!module.contains("into_singleton_verified_receipt"));
        assert!(!module.contains("try_lower_after_ranked_checks"));
        for forbidden in ["artifact", "publication", "load", "launch"] {
            assert!(
                !receipt.contains(forbidden),
                "ranked roster receipt gained downstream {forbidden} custody",
            );
        }
    }

    #[test]
    fn value_carrying_accesses_retain_exact_semantic_correspondence() {
        let view = ProductionRankedValueV1::Argument(0);
        let value = ProductionRankedValueV1::Argument(1);
        let blocks = [ProductionRankedBlockV1::new(
            vec![
                ProductionRankedOperationV1::ValueAccess {
                    kind: AccessKindAttr::Write,
                    view,
                    indices: vec![],
                    value,
                },
                ProductionRankedOperationV1::AtomicValueAccess {
                    kind: AccessKindAttr::AtomicReadModifyWrite,
                    ordering: AtomicOrderingAttr::AcquireRelease,
                    scope: AtomicScopeAttr::Device,
                    view,
                    indices: vec![],
                    value,
                },
            ],
            ProductionRankedTerminatorV1::Return,
        )];
        let sites = [
            ProjectedAccessSourceV1 {
                block: 0,
                operation: 0,
                access: AccessKindAttr::Write,
                memory_space: MemorySpaceAttr::Global,
                source: SemanticSourceProvenanceV1::unavailable(),
                semantic_site: Some(ProjectedSemanticAccessSiteV1 {
                    block: 7,
                    statement: None,
                }),
            },
            ProjectedAccessSourceV1 {
                block: 0,
                operation: 1,
                access: AccessKindAttr::AtomicReadModifyWrite,
                memory_space: MemorySpaceAttr::Global,
                source: SemanticSourceProvenanceV1::unavailable(),
                semantic_site: Some(ProjectedSemanticAccessSiteV1 {
                    block: 7,
                    statement: None,
                }),
            },
        ];

        let retained = production_access_sources(&blocks, &sites).unwrap();

        assert_eq!(retained.len(), 2);
        assert_eq!(
            (
                retained[0].semantic_block(),
                retained[0].semantic_statement(),
                retained[0].semantic_access_ordinal(),
                retained[0].ranked_block(),
                retained[0].ranked_operation(),
            ),
            (7, None, 0, 0, 0),
        );
        assert_eq!(
            (
                retained[1].semantic_block(),
                retained[1].semantic_access_ordinal(),
                retained[1].ranked_operation(),
            ),
            (7, 1, 1),
        );
    }

    #[test]
    fn conservative_allocation_effect_retains_exact_semantic_correspondence() {
        let blocks = [ProductionRankedBlockV1::new(
            vec![ProductionRankedOperationV1::AllocationEffect {
                kind: AccessKindAttr::Read,
                memory_space: MemorySpaceAttr::Global,
                allocation_origin: 1,
                noalias_class: 1,
            }],
            ProductionRankedTerminatorV1::Return,
        )];
        let sites = [ProjectedAccessSourceV1 {
            block: 0,
            operation: 0,
            access: AccessKindAttr::Read,
            memory_space: MemorySpaceAttr::Global,
            source: SemanticSourceProvenanceV1::unavailable(),
            semantic_site: Some(ProjectedSemanticAccessSiteV1 {
                block: 9,
                statement: None,
            }),
        }];

        let retained = production_access_sources(&blocks, &sites).unwrap();

        assert_eq!(retained.len(), 1);
        assert_eq!(
            (
                retained[0].semantic_block(),
                retained[0].semantic_statement(),
                retained[0].semantic_access_ordinal(),
                retained[0].ranked_block(),
                retained[0].ranked_operation(),
            ),
            (9, None, 0, 0, 0),
        );
    }

    fn bytes(tag: u8) -> [u8; 32] {
        [tag; 32]
    }

    fn ranked_root_input(name: &str, binding: u8, rank: u8) -> ProductionRankedRootInputV1 {
        let workgroup = match rank {
            1 => [64, 1, 1],
            2 => [8, 8, 1],
            3 => [4, 4, 4],
            _ => unreachable!(),
        };
        let launch = LaunchContract::new(
            rank,
            BlockSize::Exact(
                fe2o3_artifacts::Dimensions::new(workgroup[0], workgroup[1], workgroup[2]).unwrap(),
            ),
            fe2o3_artifacts::Dimensions::new(1, 1, 1).unwrap(),
            0,
            0,
        )
        .unwrap();
        ProductionRankedRootInputV1::new(name, bytes(binding), &launch)
    }

    fn ranked_root_input_1d(name: &str, binding: u8, size: u32) -> ProductionRankedRootInputV1 {
        let launch = LaunchContract::new(
            1,
            BlockSize::Exact(fe2o3_artifacts::Dimensions::new(size, 1, 1).unwrap()),
            fe2o3_artifacts::Dimensions::new(1, 1, 1).unwrap(),
            0,
            0,
        )
        .unwrap();
        ProductionRankedRootInputV1::new(name, bytes(binding), &launch)
    }

    const NEUTRAL_UNIT_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
    const NEUTRAL_LDS_SCOPE_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
    const NEUTRAL_LDS_SCOPE_REFERENCE_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
    const NEUTRAL_ELEMENT_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
    const NEUTRAL_ELEMENT_POINTER_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
    const NEUTRAL_U64_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);
    const NEUTRAL_DYNAMIC_LDS_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(6);
    const NEUTRAL_CONTEXT_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(7);
    const NEUTRAL_CONTEXT_REFERENCE_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(8);

    fn neutral_scalar_backend_v1(
        primitive: SemanticBackendPrimitiveV1,
        maximum: u128,
    ) -> SemanticBackendReprV1 {
        SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
            primitive,
            SemanticScalarValidityRangeV1::new(0, maximum),
        ))
    }

    fn neutral_pointer_type_v1(
        tag: u8,
        pointee: SemanticTypeIdV1,
        kind: SemanticPointerKindV1,
        mutability: SemanticMutabilityV1,
        validity_start: u128,
    ) -> SemanticTypeDeclV1 {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(tag)),
            SemanticLayoutIdentityV1::from_sha256(bytes(tag)),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(8),
                8,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                    SemanticScalarValidityRangeV1::new(validity_start, u64::MAX.into()),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    pointee,
                    kind,
                    mutability,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        )
    }

    fn neutral_reference_type_v1(
        tag: u8,
        pointee: SemanticTypeIdV1,
        mutability: SemanticMutabilityV1,
        pointee_kind: SemanticAbiPointeeKindV1,
    ) -> SemanticTypeDeclV1 {
        neutral_pointer_type_v1(
            tag,
            pointee,
            SemanticPointerKindV1::Reference,
            mutability,
            1,
        )
        .with_rustc_abi_properties(
            SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                Some(SemanticAbiPointeeInfoV1::new(pointee_kind, 0, 1).unwrap()),
                None,
            ),
        )
    }

    fn neutral_semantic_types_v1() -> Vec<SemanticTypeDeclV1> {
        let unit = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(220)),
            SemanticLayoutIdentityV1::from_sha256(bytes(220)),
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                0,
                1,
                SemanticFieldsShapeV1::arbitrary(Vec::new(), Vec::new()).unwrap(),
                SemanticRustcVariantsV1::Single { index: 0 },
                SemanticBackendReprV1::memory(true),
                None,
                false,
                None,
                1,
                0,
                SemanticTypeLayoutDetailsV1::None,
            )
            .unwrap(),
            SemanticTypeShapeV1::Unit,
        );
        let scope = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(221)),
            SemanticLayoutIdentityV1::from_sha256(bytes(221)),
            SemanticTypeLayoutV1::aggregate(
                Some(0),
                1,
                SemanticAggregateLayoutV1::new(vec![0], Vec::new()).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(
                SemanticAggregateTypeV1::new(vec![NEUTRAL_UNIT_TYPE]).unwrap(),
            ),
        );
        let scope_reference = neutral_reference_type_v1(
            222,
            NEUTRAL_LDS_SCOPE_TYPE,
            SemanticMutabilityV1::Mutable,
            SemanticAbiPointeeKindV1::MutableReference { unpin: true },
        );
        let element = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(223)),
            SemanticLayoutIdentityV1::from_sha256(bytes(223)),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(4),
                4,
                neutral_scalar_backend_v1(
                    SemanticBackendPrimitiveV1::integer(false, 32, 4),
                    u32::MAX.into(),
                ),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            }),
        );
        let element_pointer = neutral_pointer_type_v1(
            224,
            NEUTRAL_ELEMENT_TYPE,
            SemanticPointerKindV1::Raw,
            SemanticMutabilityV1::Mutable,
            0,
        );
        let u64_type = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(225)),
            SemanticLayoutIdentityV1::from_sha256(bytes(225)),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(8),
                8,
                neutral_scalar_backend_v1(
                    SemanticBackendPrimitiveV1::integer(false, 64, 8),
                    u64::MAX.into(),
                ),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 64,
            }),
        );
        let dynamic_lds = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(226)),
            SemanticLayoutIdentityV1::from_sha256(bytes(226)),
            SemanticTypeLayoutV1::aggregate(
                Some(24),
                8,
                SemanticAggregateLayoutV1::new(vec![0, 8, 16, 24, 24, 24], Vec::new()).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(
                SemanticAggregateTypeV1::new(vec![
                    NEUTRAL_ELEMENT_POINTER_TYPE,
                    NEUTRAL_U64_TYPE,
                    NEUTRAL_U64_TYPE,
                    NEUTRAL_UNIT_TYPE,
                    NEUTRAL_UNIT_TYPE,
                    NEUTRAL_UNIT_TYPE,
                ])
                .unwrap(),
            ),
        );
        let context = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(227)),
            SemanticLayoutIdentityV1::from_sha256(bytes(227)),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(0),
                1,
                SemanticBackendReprV1::memory(true),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Opaque,
        );
        let context_reference = neutral_reference_type_v1(
            228,
            NEUTRAL_CONTEXT_TYPE,
            SemanticMutabilityV1::Immutable,
            SemanticAbiPointeeKindV1::SharedReference { frozen: true },
        );
        vec![
            unit,
            scope,
            scope_reference,
            element,
            element_pointer,
            u64_type,
            dynamic_lds,
            context,
            context_reference,
        ]
    }

    fn zero_sized_summary_helper_v1(
        tag: u8,
        blocks: Vec<SemanticBasicBlockV1>,
    ) -> SemanticFunctionDeclV1 {
        let abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256(bytes(tag)),
            SemanticLayoutIdentityV1::from_sha256(bytes(tag)),
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::Rust,
            false,
            false,
            0,
            Vec::new(),
            SemanticAbiValueV1::new(NEUTRAL_LDS_SCOPE_TYPE, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap();
        SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256(bytes(tag.wrapping_add(1))),
            SemanticFunctionRoleV1::InternalHelper,
            SemanticItemDefinitionIdentityV1::from_sha256(bytes(tag.wrapping_add(2))),
            SemanticMonomorphizationIdentityV1::from_sha256(bytes(tag.wrapping_add(3))),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(tag.wrapping_add(4))),
            SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(tag.wrapping_add(5))),
            SemanticSourceProvenanceV1::unavailable(),
            abi,
            vec![
                local(
                    tag.wrapping_add(6),
                    NEUTRAL_LDS_SCOPE_TYPE,
                    SemanticLocalRoleV1::Return,
                ),
                local(
                    tag.wrapping_add(7),
                    NEUTRAL_UNIT_TYPE,
                    SemanticLocalRoleV1::Temporary,
                ),
            ],
            SemanticBlockIdV1::from_index(0),
            blocks,
        )
        .unwrap()
    }

    fn exact_zero_sized_summary_helper_v1(tag: u8) -> SemanticFunctionDeclV1 {
        zero_sized_summary_helper_v1(
            tag,
            vec![block(
                tag,
                vec![statement(SemanticStatementKindV1::Assign(
                    SemanticAssignmentV1::new(
                        neutral_test_place_v1(0, NEUTRAL_LDS_SCOPE_TYPE),
                        SemanticRvalueV1::new(
                            NEUTRAL_LDS_SCOPE_TYPE,
                            SemanticRvalueKindV1::Aggregate(
                                SemanticAggregateRvalueV1::new(
                                    SemanticAggregateKindV1::Aggregate,
                                    vec![SemanticOperandV1::Constant(SemanticConstantV1::new(
                                        NEUTRAL_UNIT_TYPE,
                                        SemanticConstantValueV1::ZeroSized,
                                    ))],
                                )
                                .unwrap(),
                            ),
                        ),
                    ),
                ))],
                SemanticTerminatorKindV1::Return,
            )],
        )
    }

    fn neutral_plain_direct_abi_value_v1(ty: SemanticTypeIdV1) -> SemanticAbiValueV1 {
        SemanticAbiValueV1::new(
            ty,
            SemanticAbiPassModeV1::Direct(
                SemanticAbiValueAttributesV1::new(
                    SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                    SemanticAbiExtensionV1::None,
                    0,
                    None,
                )
                .unwrap(),
            ),
        )
    }

    fn neutral_reference_abi_value_v1(ty: SemanticTypeIdV1, shared: bool) -> SemanticAbiValueV1 {
        SemanticAbiValueV1::new(
            ty,
            SemanticAbiPassModeV1::Direct(
                SemanticAbiValueAttributesV1::new(
                    SemanticAbiRegularAttributesV1::new(
                        true,
                        shared.then_some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
                        true,
                        shared,
                        false,
                        true,
                    ),
                    SemanticAbiExtensionV1::None,
                    0,
                    None,
                )
                .unwrap(),
            ),
        )
    }

    fn neutral_dynamic_lds_abi_value_v1() -> SemanticAbiValueV1 {
        SemanticAbiValueV1::new(
            NEUTRAL_DYNAMIC_LDS_TYPE,
            SemanticAbiPassModeV1::Indirect {
                attributes: SemanticAbiValueAttributesV1::new(
                    SemanticAbiRegularAttributesV1::new(
                        true,
                        Some(SemanticAbiPointerCaptureV1::CapturesNone),
                        true,
                        false,
                        false,
                        true,
                    ),
                    SemanticAbiExtensionV1::None,
                    24,
                    Some(8),
                )
                .unwrap(),
                metadata_attributes: None,
                on_stack: false,
            },
        )
    }

    fn neutral_compiler_intrinsic_callable_v1(
        tag: u8,
        inputs: Vec<SemanticAbiValueV1>,
        output: SemanticAbiValueV1,
        operation: SemanticCompilerIntrinsicOperationV1,
    ) -> SemanticCallableDeclV1 {
        let arguments = inputs
            .into_iter()
            .map(SemanticAbiArgumentV1::source)
            .collect::<Vec<_>>();
        let abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256(bytes(tag)),
            SemanticLayoutIdentityV1::from_sha256(bytes(250)),
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::Rust,
            false,
            false,
            u32::try_from(arguments.len()).unwrap(),
            arguments,
            output,
        )
        .unwrap();
        SemanticCallableDeclV1::CompilerIntrinsic {
            binding: SemanticNonBodyCallableBindingV1::new(
                SemanticFunctionIdentityV1::from_sha256(bytes(tag)),
                SemanticItemDefinitionIdentityV1::from_sha256(bytes(tag)),
                SemanticMonomorphizationIdentityV1::from_sha256(bytes(tag)),
                SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(tag)),
                SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(tag)),
                SemanticSourceProvenanceV1::unavailable(),
                abi,
            ),
            operation,
            operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256(bytes(tag)),
        }
    }

    fn neutral_test_place_v1(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), Vec::new(), ty).unwrap()
    }

    fn neutral_test_call_v1(
        callee: u32,
        arguments: Vec<SemanticOperandV1>,
        destination_local: u32,
        destination_ty: SemanticTypeIdV1,
        target: u32,
    ) -> SemanticTerminatorKindV1 {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(callee),
                arguments,
                Some(SemanticCallDestinationV1::new(
                    neutral_test_place_v1(destination_local, destination_ty),
                    cfg_edge(SemanticEdgeRoleV1::CallReturn, target),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    }

    fn neutral_ranked_program_for_operation_v1(
        operation: SemanticCompilerIntrinsicOperationV1,
        elements: u32,
    ) -> ProductionRankedSemanticProgramV1 {
        let scope_borrow = statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            neutral_test_place_v1(2, NEUTRAL_LDS_SCOPE_REFERENCE_TYPE),
            SemanticRvalueV1::new(
                NEUTRAL_LDS_SCOPE_REFERENCE_TYPE,
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Mutable,
                    place: neutral_test_place_v1(1, NEUTRAL_LDS_SCOPE_TYPE),
                },
            ),
        )));
        let context_borrow = statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            neutral_test_place_v1(5, NEUTRAL_CONTEXT_REFERENCE_TYPE),
            SemanticRvalueV1::new(
                NEUTRAL_CONTEXT_REFERENCE_TYPE,
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: neutral_test_place_v1(4, NEUTRAL_CONTEXT_TYPE),
                },
            ),
        )));
        let blocks = vec![
            block(
                230,
                vec![scope_borrow],
                neutral_test_call_v1(
                    1,
                    vec![SemanticOperandV1::Copy(neutral_test_place_v1(
                        2,
                        NEUTRAL_LDS_SCOPE_REFERENCE_TYPE,
                    ))],
                    3,
                    NEUTRAL_DYNAMIC_LDS_TYPE,
                    1,
                ),
            ),
            block(
                231,
                Vec::new(),
                neutral_test_call_v1(2, Vec::new(), 4, NEUTRAL_CONTEXT_TYPE, 2),
            ),
            block(
                232,
                vec![context_borrow],
                neutral_test_call_v1(
                    3,
                    vec![
                        SemanticOperandV1::Copy(neutral_test_place_v1(
                            5,
                            NEUTRAL_CONTEXT_REFERENCE_TYPE,
                        )),
                        // Match the post-borrow-check encoding produced by
                        // ordinary Rust for this non-Copy, no-drop value.
                        SemanticOperandV1::Copy(neutral_test_place_v1(3, NEUTRAL_DYNAMIC_LDS_TYPE)),
                        SemanticOperandV1::Constant(SemanticConstantV1::new(
                            NEUTRAL_ELEMENT_TYPE,
                            SemanticConstantValueV1::Scalar(
                                SemanticScalarValueV1::new(7, 4).unwrap(),
                            ),
                        )),
                    ],
                    6,
                    NEUTRAL_ELEMENT_TYPE,
                    3,
                ),
            ),
            block(233, Vec::new(), SemanticTerminatorKindV1::Return),
        ];
        let root_abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256(bytes(234)),
            SemanticLayoutIdentityV1::from_sha256(bytes(250)),
            SemanticCanonAbiV1::GpuKernel,
            SemanticExternAbiV1::GpuKernel,
            false,
            false,
            0,
            Vec::new(),
            SemanticAbiValueV1::new(NEUTRAL_UNIT_TYPE, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap();
        let locals = [
            (NEUTRAL_UNIT_TYPE, SemanticLocalRoleV1::Return),
            (NEUTRAL_LDS_SCOPE_TYPE, SemanticLocalRoleV1::Temporary),
            (
                NEUTRAL_LDS_SCOPE_REFERENCE_TYPE,
                SemanticLocalRoleV1::Temporary,
            ),
            (NEUTRAL_DYNAMIC_LDS_TYPE, SemanticLocalRoleV1::Temporary),
            (NEUTRAL_CONTEXT_TYPE, SemanticLocalRoleV1::Temporary),
            (
                NEUTRAL_CONTEXT_REFERENCE_TYPE,
                SemanticLocalRoleV1::Temporary,
            ),
            (NEUTRAL_ELEMENT_TYPE, SemanticLocalRoleV1::Temporary),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (ty, role))| local(235 + index as u8, ty, role))
        .collect::<Vec<_>>();
        let dimensions = SemanticWorkgroupDimensionsV1::new([elements, 1, 1]).unwrap();
        let source_contract = SemanticKernelSourceContractV1::new(
            Some(
                SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None)
                    .unwrap(),
            ),
            None,
            None,
        )
        .unwrap();
        let function = SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256(bytes(242)),
            SemanticFunctionRoleV1::KernelRoot,
            SemanticItemDefinitionIdentityV1::from_sha256(bytes(243)),
            SemanticMonomorphizationIdentityV1::from_sha256(bytes(244)),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(245)),
            SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(246)),
            SemanticSourceProvenanceV1::unavailable(),
            root_abi,
            locals,
            SemanticBlockIdV1::from_index(0),
            blocks,
        )
        .unwrap()
        .with_kernel_entry(SemanticKernelEntryV1::new(
            SemanticLinkSymbolV1::new(b"neutral_generated_hostile".to_vec()).unwrap(),
            SemanticKernelBindingIdentityV1::from_sha256(bytes(247)),
            source_contract,
        ));
        let callables = vec![
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
            neutral_compiler_intrinsic_callable_v1(
                248,
                vec![neutral_reference_abi_value_v1(
                    NEUTRAL_LDS_SCOPE_REFERENCE_TYPE,
                    false,
                )],
                neutral_dynamic_lds_abi_value_v1(),
                SemanticCompilerIntrinsicOperationV1::DynamicLdsExactCurrent {
                    scope: NEUTRAL_LDS_SCOPE_TYPE,
                    dynamic_lds: NEUTRAL_DYNAMIC_LDS_TYPE,
                    element_storage: NEUTRAL_ELEMENT_TYPE,
                    elements: u64::from(elements),
                },
            ),
            neutral_compiler_intrinsic_callable_v1(
                249,
                Vec::new(),
                SemanticAbiValueV1::new(NEUTRAL_CONTEXT_TYPE, SemanticAbiPassModeV1::Ignore),
                SemanticCompilerIntrinsicOperationV1::CollectiveContextCurrent {
                    context: NEUTRAL_CONTEXT_TYPE,
                },
            ),
            neutral_compiler_intrinsic_callable_v1(
                250,
                vec![
                    neutral_reference_abi_value_v1(NEUTRAL_CONTEXT_REFERENCE_TYPE, true),
                    neutral_dynamic_lds_abi_value_v1(),
                    neutral_plain_direct_abi_value_v1(NEUTRAL_ELEMENT_TYPE),
                ],
                neutral_plain_direct_abi_value_v1(NEUTRAL_ELEMENT_TYPE),
                operation,
            ),
        ];
        let admitted = InertSemanticMirRequestV1::new_with_callables(
            SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(250))),
            neutral_semantic_types_v1(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![function],
            callables,
            vec![SemanticFunctionIdV1::from_index(0)],
        )
        .unwrap()
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
        let owner = ProductionSemanticMirOwnerV1::try_new(
            admitted,
            fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap();
        let owner = ProductionSemanticSsaOwnerV1::try_new(
            owner,
            fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap();
        project_and_verify_ranked_semantic_mir_v1(
            owner,
            &[ranked_root_input_1d(
                "neutral_generated_hostile",
                247,
                elements,
            )],
            &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
        )
        .unwrap()
    }

    fn neutral_ranked_program_v1() -> ProductionRankedSemanticProgramV1 {
        neutral_ranked_program_for_operation_v1(
            SemanticCompilerIntrinsicOperationV1::NeutralWorkgroupReduceSum {
                context: NEUTRAL_CONTEXT_TYPE,
                dynamic_lds: NEUTRAL_DYNAMIC_LDS_TYPE,
                element_storage: NEUTRAL_ELEMENT_TYPE,
                element: NEUTRAL_ELEMENT_TYPE,
            },
            64,
        )
    }

    fn neutral_scan_ranked_program_v1(
        kind: SemanticWorkgroupScanKindV1,
        elements: u32,
    ) -> ProductionRankedSemanticProgramV1 {
        neutral_ranked_program_for_operation_v1(
            SemanticCompilerIntrinsicOperationV1::NeutralWorkgroupScanSum {
                context: NEUTRAL_CONTEXT_TYPE,
                dynamic_lds: NEUTRAL_DYNAMIC_LDS_TYPE,
                element_storage: NEUTRAL_ELEMENT_TYPE,
                element: NEUTRAL_ELEMENT_TYPE,
                kind,
            },
            elements,
        )
    }

    fn scan_rounds_v1(elements: u32) -> u32 {
        if elements <= 1 {
            0
        } else {
            u32::BITS - (elements - 1).leading_zeros()
        }
    }
