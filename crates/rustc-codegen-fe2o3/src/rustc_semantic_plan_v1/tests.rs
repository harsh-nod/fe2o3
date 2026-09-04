    use super::*;
    use sha2::{Digest, Sha256};
    use std::cell::Cell;

    #[derive(Clone, Copy)]
    struct BodyCommitmentFixtureV5 {
        source_span: [u8; 8],
        local: [u8; 8],
        local_mapping: [u8; 8],
        block: [u8; 8],
        statement: [u8; 8],
        terminator: [u8; 8],
    }

    fn body_commitment_fixture_v5(
        ordinal: u64,
        function: u32,
        fixture: BodyCommitmentFixtureV5,
    ) -> PreflightChildCommitmentV5 {
        let function = SemanticFunctionIdV1::from_index(function);
        let mut child = PreflightChildCommitmentBuilderV5::body(ordinal, function);
        child.field(&function.index().to_le_bytes()).unwrap();
        for field in [
            &fixture.source_span,
            &fixture.local,
            &fixture.local_mapping,
            &fixture.block,
            &fixture.statement,
            &fixture.terminator,
        ] {
            child.field(field).unwrap();
        }
        child.finish()
    }

    fn committed_body_roster_v5(
        bodies: &[(u64, u32, PreflightChildCommitmentV5)],
    ) -> ([u8; 32], Box<[u8]>) {
        let mut parent = BoundedPreflightTranscriptV5::new();
        parent.field(PREFLIGHT_PLAN_DOMAIN_V4).unwrap();
        parent.field(&[PREFLIGHT_SECTION_BODIES_V5]).unwrap();
        parent
            .field(&usize_to_u64_v5(bodies.len()).to_le_bytes())
            .unwrap();
        for (ordinal, function, child) in bodies {
            parent.field(&[PREFLIGHT_SECTION_BODIES_V5]).unwrap();
            parent.field(&ordinal.to_le_bytes()).unwrap();
            parent.field(&function.to_le_bytes()).unwrap();
            for count in [0_u64; 6] {
                parent.field(&count.to_le_bytes()).unwrap();
            }
            parent.field(&child.field_count.to_le_bytes()).unwrap();
            parent
                .field(&child.payload_framed_bytes.to_le_bytes())
                .unwrap();
            parent.field(&child.sha256).unwrap();
        }
        parent.finish().unwrap()
    }

    fn transcript_fields_v5(mut bytes: &[u8]) -> Vec<&[u8]> {
        let mut fields = Vec::new();
        while !bytes.is_empty() {
            let length = u64::from_le_bytes(bytes[..8].try_into().unwrap()) as usize;
            let end = 8 + length;
            fields.push(&bytes[8..end]);
            bytes = &bytes[end..];
        }
        fields
    }

    #[test]
    fn v5_hierarchical_commitments_are_domain_order_and_content_sensitive() {
        let baseline = BodyCommitmentFixtureV5 {
            source_span: [1; 8],
            local: [2; 8],
            local_mapping: [3; 8],
            block: [4; 8],
            statement: [5; 8],
            terminator: [6; 8],
        };
        let first = body_commitment_fixture_v5(0, 7, baseline);
        assert_eq!(first.field_count, 7);
        assert_eq!(first.payload_framed_bytes, 7 * 16 - 4);

        for mutated in [
            BodyCommitmentFixtureV5 {
                source_span: [9; 8],
                ..baseline
            },
            BodyCommitmentFixtureV5 {
                local: [9; 8],
                ..baseline
            },
            BodyCommitmentFixtureV5 {
                local_mapping: [9; 8],
                ..baseline
            },
            BodyCommitmentFixtureV5 {
                block: [9; 8],
                ..baseline
            },
            BodyCommitmentFixtureV5 {
                statement: [9; 8],
                ..baseline
            },
            BodyCommitmentFixtureV5 {
                terminator: [9; 8],
                ..baseline
            },
        ] {
            assert_ne!(body_commitment_fixture_v5(0, 7, mutated), first);
        }
        assert_ne!(body_commitment_fixture_v5(1, 7, baseline), first);
        assert_ne!(body_commitment_fixture_v5(0, 8, baseline), first);

        let second = body_commitment_fixture_v5(1, 8, baseline);
        let ordered = committed_body_roster_v5(&[(0, 7, first), (1, 8, second)]);
        let reordered = committed_body_roster_v5(&[
            (0, 8, body_commitment_fixture_v5(0, 8, baseline)),
            (1, 7, body_commitment_fixture_v5(1, 7, baseline)),
        ]);
        assert_ne!(ordered.0, reordered.0);
        assert_eq!(ordered.0, <[u8; 32]>::from(Sha256::digest(&ordered.1)));

        let fields = transcript_fields_v5(&ordered.1);
        assert_eq!(fields[0], PREFLIGHT_PLAN_DOMAIN_V5);
        assert_eq!(fields[1], PREFLIGHT_PLAN_DOMAIN_V4);
        assert_ne!(PREFLIGHT_PLAN_DOMAIN_V4, PREFLIGHT_PLAN_DOMAIN_V5);
    }

    #[test]
    fn v5_child_commitments_cannot_cross_domain_section_or_ordinal() {
        let fields = [b"same-field-one".as_slice(), b"same-field-two".as_slice()];
        let mut types = PreflightChildCommitmentBuilderV5::section(
            PREFLIGHT_TYPES_DOMAIN_V5,
            PREFLIGHT_SECTION_TYPES_V5,
            "test types",
        );
        let mut sources = PreflightChildCommitmentBuilderV5::section(
            PREFLIGHT_SOURCE_FILES_DOMAIN_V5,
            PREFLIGHT_SECTION_SOURCE_FILES_V5,
            "test sources",
        );
        for field in fields {
            types.field(field).unwrap();
            sources.field(field).unwrap();
        }
        let types = types.finish();
        let sources = sources.finish();
        assert_ne!(types.sha256, sources.sha256);

        let fixture = BodyCommitmentFixtureV5 {
            source_span: [1; 8],
            local: [2; 8],
            local_mapping: [3; 8],
            block: [4; 8],
            statement: [5; 8],
            terminator: [6; 8],
        };
        assert_ne!(
            body_commitment_fixture_v5(0, 7, fixture).sha256,
            body_commitment_fixture_v5(1, 7, fixture).sha256,
        );

        let mut types_parent = BoundedPreflightTranscriptV5::new();
        types_parent
            .section(PREFLIGHT_SECTION_TYPES_V5, 2, types)
            .unwrap();
        let mut transplanted_parent = BoundedPreflightTranscriptV5::new();
        transplanted_parent
            .section(PREFLIGHT_SECTION_SOURCE_FILES_V5, 2, types)
            .unwrap();
        assert_ne!(
            types_parent.finish().unwrap().0,
            transplanted_parent.finish().unwrap().0,
        );
    }

    #[test]
    fn v5_parent_is_bounded_below_the_unchanged_v3_receipt_maximum() {
        const SECTION_COMMITMENT_BYTES: u64 = 9 + 16 + 16 + 16 + 40;
        const BODY_COMMITMENT_BYTES: u64 = 9 + 16 + 12 + 6 * 16 + 16 + 16 + 40;
        const FIXED_PARENT_BYTES: u64 = 50 + 50 + 40 + 40 + 23 * 16 + 9 + 16;
        const NON_BODY_SECTIONS: u64 = 10;
        let predicted_maximum = FIXED_PARENT_BYTES
            + NON_BODY_SECTIONS * SECTION_COMMITMENT_BYTES
            + fe2o3_mir_model::semantic_mir_v1::HARD_MAX_FUNCTIONS_V1 * BODY_COMMITMENT_BYTES;
        assert!(
            predicted_maximum
                < fe2o3_compiler_lineage::MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3 as u64
        );

        let mut exact = BoundedPreflightTranscriptV5::new();
        let maximum = fe2o3_compiler_lineage::MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3;
        let remaining = maximum - exact.framed_bytes as usize - 8;
        exact.field(&vec![0; remaining]).unwrap();
        assert_eq!(exact.framed_bytes as usize, maximum);
        assert!(matches!(
            exact.field(&[]),
            Err(ProductionSemanticPreflightErrorV1::CommitmentBoundExceeded {
                scope: "V5 parent transcript",
                actual,
                maximum: observed_maximum,
            }) if actual == maximum as u64 + 8 && observed_maximum == maximum as u64
        ));
    }

    #[test]
    fn disabled_debug_capture_never_inspects_v2_metadata() {
        let called = Cell::new(false);
        let result: Result<Option<()>, ()> =
            with_debug_source_capture_v2(DebugSourceCaptureRequestV2::Disabled, || {
                called.set(true);
                Err(())
            });
        assert_eq!(result, Ok(None));
        assert!(!called.get());

        let result =
            with_debug_source_capture_v2(DebugSourceCaptureRequestV2::SourceVariables, || {
                called.set(true);
                Ok::<_, ()>(())
            });
        assert_eq!(result, Ok(Some(())));
        assert!(called.get());
    }

    #[test]
    fn v2_debug_names_are_exact_or_rejected() {
        let maximum = fe2o3_kernel_ir::MAX_DEBUG_SOURCE_VARIABLE_NAME_BYTES_V2;
        let exact = "x".repeat(maximum);
        assert_eq!(exact_debug_variable_name_v2(&exact), Some(exact.clone()));
        assert_eq!(exact_debug_variable_name_v2(""), None);
        assert_eq!(exact_debug_variable_name_v2("line\nbreak"), None);
        assert_eq!(exact_debug_variable_name_v2(&"x".repeat(maximum + 1)), None);
    }

    #[test]
    fn optional_debug_count_allocation_and_name_failures_are_typed_gaps() {
        let mut budget = OptionalDebugCaptureBudgetV2::default();
        budget
            .charge_variables(fe2o3_kernel_ir::MAX_DEBUG_SOURCE_VARIABLES_V2)
            .unwrap();
        let count_error = PendingRejectionV1::Fatal(budget.charge_variables(1).unwrap_err());
        assert_eq!(
            classify_optional_debug_rejection_v2(&count_error),
            Some(fe2o3_kernel_ir::ProductionSemanticDebugProducerGapV1::ResourceLimit)
        );
        let mut scope_budget = OptionalDebugCaptureBudgetV2::default();
        scope_budget
            .charge_scopes(fe2o3_kernel_ir::MAX_DEBUG_SOURCE_SCOPES_V2)
            .unwrap();
        assert!(scope_budget.charge_scopes(1).is_err());

        let allocation_error =
            PendingRejectionV1::Fatal(ProductionSemanticPreflightErrorV1::IdentityTableMismatch);
        assert_eq!(
            classify_optional_debug_rejection_v2(&allocation_error),
            Some(fe2o3_kernel_ir::ProductionSemanticDebugProducerGapV1::ResourceLimit)
        );

        let site = RejectionSiteV1 {
            function: SemanticFunctionIdV1::from_index(0),
            block: None,
            statement: None,
            local: None,
            span: rustc_span::DUMMY_SP,
        };
        let name_error = exact_debug_variable_name_or_rejection_v2(
            &"x".repeat(fe2o3_kernel_ir::MAX_DEBUG_SOURCE_VARIABLE_NAME_BYTES_V2 + 1),
            site,
        )
        .unwrap_err();
        assert_eq!(
            classify_optional_debug_rejection_v2(&name_error),
            Some(
                fe2o3_kernel_ir::ProductionSemanticDebugProducerGapV1::SourceObservationUnrepresentable
            )
        );
    }

    #[test]
    fn every_debug_only_failure_leaves_the_ordinary_artifact_path_unchanged() {
        let ordinary_artifact = [0x5a; 32];
        let mut budget = OptionalDebugCaptureBudgetV2::default();
        budget
            .charge_variables(fe2o3_kernel_ir::MAX_DEBUG_SOURCE_VARIABLES_V2)
            .unwrap();
        let over_limit = PendingRejectionV1::Fatal(budget.charge_variables(1).unwrap_err());
        let allocation =
            PendingRejectionV1::Fatal(ProductionSemanticPreflightErrorV1::IdentityTableMismatch);
        let site = RejectionSiteV1 {
            function: SemanticFunctionIdV1::from_index(0),
            block: None,
            statement: None,
            local: None,
            span: rustc_span::DUMMY_SP,
        };
        let overlong_name = exact_debug_variable_name_or_rejection_v2(
            &"x".repeat(fe2o3_kernel_ir::MAX_DEBUG_SOURCE_VARIABLE_NAME_BYTES_V2 + 1),
            site,
        )
        .unwrap_err();

        for (error, expected) in [
            (
                over_limit,
                fe2o3_kernel_ir::ProductionSemanticDebugProducerGapV1::ResourceLimit,
            ),
            (
                allocation,
                fe2o3_kernel_ir::ProductionSemanticDebugProducerGapV1::ResourceLimit,
            ),
            (
                overlong_name,
                fe2o3_kernel_ir::ProductionSemanticDebugProducerGapV1::SourceObservationUnrepresentable,
            ),
        ] {
            let observed = match observe_optional_debug_capture_v2::<()>(|| Err(error)) {
                Ok(observed) => observed,
                Err(_) => panic!("debug-only rejection escaped the observational boundary"),
            };
            assert_eq!(observed.unwrap_err(), expected);
            assert_eq!(ordinary_artifact, [0x5a; 32]);
        }

        let structural =
            PendingRejectionV1::Fatal(ProductionSemanticPreflightErrorV1::TypeIdentityCollision);
        assert!(observe_optional_debug_capture_v2::<()>(|| Err(structural)).is_err());
        assert_eq!(ordinary_artifact, [0x5a; 32]);
    }

    #[test]
    fn optional_debug_fallback_never_hides_structural_preflight_failures() {
        let structural =
            PendingRejectionV1::Fatal(ProductionSemanticPreflightErrorV1::TypeIdentityCollision);
        assert_eq!(classify_optional_debug_rejection_v2(&structural), None);
    }

    #[test]
    fn raw_count_budget_rejects_overflow_before_record_construction() {
        let limits = SemanticMirLimitsV1::default()
            .with_limit(SemanticMirResourceV1::Statements, 2)
            .unwrap();
        let mut counts = RawMirPreflightCountsV1::default();
        counts
            .charge(SemanticMirResourceV1::Statements, 2, limits)
            .unwrap();
        assert!(matches!(
            counts.charge(SemanticMirResourceV1::Statements, 1, limits),
            Err(ProductionSemanticPreflightErrorV1::LimitExceeded {
                resource: SemanticMirResourceV1::Statements,
                actual: 3,
                maximum: 2,
            })
        ));
    }

    #[test]
    fn deterministic_call_path_uses_sorted_roots_and_edges() {
        let id = SemanticFunctionIdV1::from_index;
        let edges = BTreeSet::from([
            CallEdgeV1 {
                caller: id(2),
                callee: id(3),
            },
            CallEdgeV1 {
                caller: id(0),
                callee: id(2),
            },
            CallEdgeV1 {
                caller: id(1),
                callee: id(3),
            },
        ]);
        assert_eq!(call_path_v1(&[id(0), id(1)], &edges, id(3)), [id(1), id(3)]);
        assert_eq!(call_path_v1(&[id(0)], &edges, id(3)), [id(0), id(2), id(3)]);
        assert_eq!(call_path_v1(&[id(0)], &edges, id(9)), [id(9)]);
        assert_eq!(
            first_unreachable_function_v1(&[id(0)], &edges, 4),
            Some(id(1))
        );
        assert_eq!(
            first_unreachable_function_v1(&[id(0), id(1)], &edges, 4),
            None,
        );
    }

    #[test]
    fn terminal_recipe_tags_are_closed_and_distinct() {
        let tags = [
            terminal_expansion_tag_v1(ProductionTerminalExpansionV1::ThreadIndex1d),
            terminal_expansion_tag_v1(ProductionTerminalExpansionV1::ThreadIndexGet),
            terminal_expansion_tag_v1(ProductionTerminalExpansionV1::DisjointSliceGetMut),
            terminal_expansion_tag_v1(ProductionTerminalExpansionV1::Trap),
        ];
        assert_eq!(tags, [0, 1, 2, 86]);
        assert_eq!(
            [
                ProductionTerminalExpansionV1::WorkgroupCollectiveContextCurrent,
                ProductionTerminalExpansionV1::NeutralWorkgroupReduceSum,
                ProductionTerminalExpansionV1::NeutralWorkgroupInclusiveScanSum,
                ProductionTerminalExpansionV1::NeutralWorkgroupExclusiveScanSum,
            ]
            .map(|expansion| terminal_expansion_tag_for_schema_v1(
                expansion,
                TerminalIdentitySchemaV1::CombinedV3,
            )),
            [111, 112, 113, 114],
        );
        assert_eq!(
            [
                ProductionTerminalExpansionV1::RustcFabsF32,
                ProductionTerminalExpansionV1::MathF32(fe2o3_kernel_ir::F32MathFunction::Abs,),
                ProductionTerminalExpansionV1::MemoryVolatileLoad,
                ProductionTerminalExpansionV1::NeutralWorkgroupInclusiveScanSum,
                ProductionTerminalExpansionV1::NeutralWorkgroupExclusiveScanSum,
                ProductionTerminalExpansionV1::WorkgroupLdsScopeCurrent,
            ]
            .map(|expansion| terminal_expansion_tag_for_schema_v1(
                expansion,
                TerminalIdentitySchemaV1::CombinedV4,
            )),
            [113, 114, 115, 116, 117, 118],
        );

        let gfx950 = [
            terminal_expansion_tag_v1(ProductionTerminalExpansionV1::Gfx950MatrixContextCurrent),
            terminal_expansion_tag_v1(ProductionTerminalExpansionV1::Gfx950Fp8MatrixARowMajor),
            terminal_expansion_tag_v1(ProductionTerminalExpansionV1::Gfx950Fp8MatrixBRowMajor),
            terminal_expansion_tag_v1(ProductionTerminalExpansionV1::Gfx950Fp8MatrixALoadM16K128),
            terminal_expansion_tag_v1(ProductionTerminalExpansionV1::Gfx950Fp8MatrixBLoadK128N16),
            terminal_expansion_tag_v1(ProductionTerminalExpansionV1::Gfx950Fp8AccumulatorZero),
            terminal_expansion_tag_v1(
                ProductionTerminalExpansionV1::Gfx950Fp8AccumulatorIntoValues,
            ),
            terminal_expansion_tag_v1(ProductionTerminalExpansionV1::Gfx950Fp8MultiplyAccumulate),
        ];
        assert_eq!(gfx950, [61, 62, 63, 64, 65, 66, 67, 68]);

        let gfx950_fp4 = [
            terminal_expansion_tag_v1(ProductionTerminalExpansionV1::Gfx950Fp4MatrixARowMajor),
            terminal_expansion_tag_v1(ProductionTerminalExpansionV1::Gfx950Fp4MatrixBRowMajor),
            terminal_expansion_tag_v1(ProductionTerminalExpansionV1::Gfx950Fp4MatrixALoadM16K128),
            terminal_expansion_tag_v1(ProductionTerminalExpansionV1::Gfx950Fp4MatrixBLoadK128N16),
            terminal_expansion_tag_v1(ProductionTerminalExpansionV1::Gfx950Fp4AccumulatorZero),
            terminal_expansion_tag_v1(
                ProductionTerminalExpansionV1::Gfx950Fp4AccumulatorIntoValues,
            ),
            terminal_expansion_tag_v1(ProductionTerminalExpansionV1::Gfx950Fp4MultiplyAccumulate),
        ];
        assert_eq!(gfx950_fp4, [69, 70, 71, 72, 73, 74, 75]);
        assert_eq!(
            terminal_expansion_tag_v1(
                ProductionTerminalExpansionV1::Gfx950Fp4Fp8MultiplyAccumulate,
            ),
            87,
        );

        let gfx950_collectives_and_lds_transpose = [
            terminal_expansion_tag_v1(ProductionTerminalExpansionV1::Gfx950SubgroupCurrent),
            terminal_expansion_tag_v1(ProductionTerminalExpansionV1::Gfx950SubgroupReduceMaxF32),
            terminal_expansion_tag_v1(ProductionTerminalExpansionV1::Gfx950SubgroupReduceSumF32),
            terminal_expansion_tag_v1(ProductionTerminalExpansionV1::Gfx950SubgroupBroadcastF32),
            terminal_expansion_tag_v1(ProductionTerminalExpansionV1::Gfx950LdsTransposeTileCurrent),
            terminal_expansion_tag_v1(ProductionTerminalExpansionV1::Gfx950LdsTransposeStageB4),
            terminal_expansion_tag_v1(ProductionTerminalExpansionV1::Gfx950LdsTransposeStageB8),
            terminal_expansion_tag_v1(ProductionTerminalExpansionV1::Gfx950LdsTransposePublish),
            terminal_expansion_tag_v1(ProductionTerminalExpansionV1::Gfx950LdsTransposeReadB4),
            terminal_expansion_tag_v1(ProductionTerminalExpansionV1::Gfx950LdsTransposeReadB8),
        ];
        assert_eq!(
            gfx950_collectives_and_lds_transpose,
            [76, 77, 78, 79, 80, 81, 82, 83, 84, 85]
        );
        assert_eq!(
            [
                terminal_expansion_tag_v1(ProductionTerminalExpansionV1::Bf16Conversion(
                    crate::production_semantic_terminal_v1::ProductionBf16ConversionV1::FromBits,
                )),
                terminal_expansion_tag_v1(ProductionTerminalExpansionV1::Bf16Conversion(
                    crate::production_semantic_terminal_v1::ProductionBf16ConversionV1::ToBits,
                )),
                terminal_expansion_tag_v1(ProductionTerminalExpansionV1::Bf16Conversion(
                    crate::production_semantic_terminal_v1::ProductionBf16ConversionV1::FromF32RoundTiesEven,
                )),
                terminal_expansion_tag_v1(ProductionTerminalExpansionV1::Bf16Conversion(
                    crate::production_semantic_terminal_v1::ProductionBf16ConversionV1::ToF32,
                )),
            ],
            [91, 92, 93, 94]
        );
        let pipeline = [
            ProductionTerminalExpansionV1::WorkgroupPipelineCurrent,
            ProductionTerminalExpansionV1::WorkgroupPipelineStage,
            ProductionTerminalExpansionV1::WorkgroupPipelineWrite,
            ProductionTerminalExpansionV1::WorkgroupPipelineCommit,
            ProductionTerminalExpansionV1::WorkgroupPipelineWait,
            ProductionTerminalExpansionV1::WorkgroupPipelineConsume,
            ProductionTerminalExpansionV1::WorkgroupPipelineRead,
            ProductionTerminalExpansionV1::WorkgroupPipelineDiscard,
            ProductionTerminalExpansionV1::WorkgroupPipelineRelease,
        ];
        assert_eq!(
            pipeline.map(terminal_expansion_tag_v1),
            [91, 92, 93, 94, 95, 96, 97, 98, 99]
        );

        let combined_schema = TerminalIdentitySchemaV1::CombinedV3;
        assert_eq!(combined_schema, TerminalIdentitySchemaV1::CombinedV3);
        assert_ne!(PREFLIGHT_PLAN_DOMAIN_V1, PREFLIGHT_PLAN_DOMAIN_V2);
        assert_ne!(PREFLIGHT_PLAN_DOMAIN_V2, PREFLIGHT_PLAN_DOMAIN_V3);
        assert_ne!(PREFLIGHT_PLAN_DOMAIN_V3, PREFLIGHT_PLAN_DOMAIN_V4);
        assert_eq!(
            pipeline.map(|expansion| {
                terminal_expansion_tag_for_schema_v1(expansion, combined_schema)
            }),
            [91, 92, 93, 94, 95, 96, 97, 98, 99]
        );
        assert_eq!(
            [
                ProductionTerminalExpansionV1::RustcFabsF32,
                ProductionTerminalExpansionV1::MathF32(fe2o3_kernel_ir::F32MathFunction::Abs),
                ProductionTerminalExpansionV1::MemoryVolatileLoad,
            ]
            .map(|expansion| terminal_expansion_tag_for_schema_v1(
                expansion,
                TerminalIdentitySchemaV1::CombinedV4,
            )),
            [113, 114, 115],
        );
        assert_eq!(
            [
                crate::production_semantic_terminal_v1::ProductionBf16ConversionV1::FromBits,
                crate::production_semantic_terminal_v1::ProductionBf16ConversionV1::ToBits,
                crate::production_semantic_terminal_v1::ProductionBf16ConversionV1::FromF32RoundTiesEven,
                crate::production_semantic_terminal_v1::ProductionBf16ConversionV1::ToF32,
            ]
            .map(|conversion| terminal_expansion_tag_for_schema_v1(
                ProductionTerminalExpansionV1::Bf16Conversion(conversion),
                combined_schema,
            )),
            [100, 101, 102, 103]
        );
        assert_eq!(
            [
                ProductionTerminalExpansionV1::Bf16Conversion(
                    crate::production_semantic_terminal_v1::ProductionBf16ConversionV1::FromBits,
                ),
                ProductionTerminalExpansionV1::Bf16Conversion(
                    crate::production_semantic_terminal_v1::ProductionBf16ConversionV1::ToBits,
                ),
                ProductionTerminalExpansionV1::Bf16Conversion(
                    crate::production_semantic_terminal_v1::ProductionBf16ConversionV1::FromF32RoundTiesEven,
                ),
                ProductionTerminalExpansionV1::Bf16Conversion(
                    crate::production_semantic_terminal_v1::ProductionBf16ConversionV1::ToF32,
                ),
                ProductionTerminalExpansionV1::WriteOnlyDisjointSliceLen,
                ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWrite,
                ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWriteDisjoint,
                ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWriteExclusive,
                ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWriteBlock,
                ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWriteTiled2d,
                ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWriteRowStriped2d,
                ProductionTerminalExpansionV1::WorkgroupCollectiveContextCurrent,
                ProductionTerminalExpansionV1::NeutralWorkgroupReduceSum,
                ProductionTerminalExpansionV1::NeutralWorkgroupInclusiveScanSum,
                ProductionTerminalExpansionV1::NeutralWorkgroupExclusiveScanSum,
            ]
            .map(|expansion| terminal_expansion_tag_for_schema_v1(expansion, combined_schema)),
            [
                100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114,
            ]
        );
        assert_eq!(
            [
                ProductionTerminalExpansionV1::WorkgroupCollectiveContextCurrent,
                ProductionTerminalExpansionV1::NeutralWorkgroupReduceSum,
            ]
            .map(|expansion| terminal_expansion_tag_for_schema_v1(
                expansion,
                TerminalIdentitySchemaV1::CombinedV2,
            )),
            [104, 105],
        );
    }

    #[test]
    fn diagnostics_are_bounded_by_unicode_scalar_count() {
        let bounded = bounded_diagnostic_component_v1(&"x".repeat(1_024));
        assert_eq!(bounded.len(), MAX_DIAGNOSTIC_COMPONENT_CHARS_V1);
    }
