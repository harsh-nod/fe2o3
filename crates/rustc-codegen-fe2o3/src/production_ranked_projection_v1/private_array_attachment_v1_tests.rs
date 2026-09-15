mod private_array_attachment_tests {
    use super::*;
    use fe2o3_lower_mir_kernel::{
        ProductionMirPlironTranslationErrorV1 as Translation,
        ProductionSemanticKirErrorV1 as LoweringError, ProductionSemanticKirOwnerV1,
        SemanticKirPrivateArrayQueryErrorV1 as QueryError,
    };
    use fe2o3_mir_model::SsaBlockIdV1;
    use fe2o3_pliron::{
        ProductionSemanticSsaOccurrenceSiteV1 as Site, ProductionSemanticSsaOperandRoleV1 as Role,
    };

    mod occurrence_lit_tests {
        include!("private_array_occurrence_lit_v1_tests.rs");
    }

    fn source(index: u32, second: Option<u32>) -> SemanticFunctionDeclV1 {
        let mut statements = assertion_ranked_write_statements(1, 2);
        statements[0] = typed_assignment(
            2,
            A_U32,
            SemanticRvalueKindV1::Use(typed_constant(A_U32, index.into(), 4)),
        );
        if let Some(index) = second {
            statements.push(typed_assignment(
                2,
                A_U32,
                SemanticRvalueKindV1::Use(typed_constant(A_U32, index.into(), 4)),
            ));
            statements.push(assertion_ranked_write_statements(1, 2).remove(1));
        }
        assertion_root_with_access(
            vec![
                (A_UNIT, SemanticLocalRoleV1::Return),
                (A_ARRAY, SemanticLocalRoleV1::Temporary),
                (A_U32, SemanticLocalRoleV1::Temporary),
            ],
            vec![],
            vec![
                block(
                    201,
                    statements,
                    assertion_terminator(typed_constant(A_BOOL, 1, 1), true, 1),
                ),
                block(202, vec![], SemanticTerminatorKindV1::Return),
            ],
            false,
        )
    }

    fn split(
        program: ProductionRankedSemanticProgramV1,
    ) -> (
        fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1,
        ProductionRankedRootProgramV1,
    ) {
        assert!(program.all_kernel_checks_are_clean());
        assert!(!program.grants_artifact_or_launch_authority());
        let ProductionRankedSemanticProgramV1 {
            materialized,
            roots,
        } = program;
        assert_eq!(roots.len(), 1);
        let root = roots.into_vec().into_iter().next().unwrap();
        assert!(root.lowering.all_mandatory_reports_are_clean());
        (materialized, root)
    }

    fn attach(
        materialized: fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1,
        root: ProductionRankedRootProgramV1,
    ) -> Result<ProductionSemanticKirOwnerV1, LoweringError> {
        // This normal candidate check must succeed, so a negative below reaches
        // actual attachment rather than an earlier malformed-roster rejection.
        let receipt = materialized_ranked_fixture_receipt_v1(materialized, root);
        ProductionSemanticKirOwnerV1::try_attach_materialized_ranked_checks(receipt)
    }

    fn access_row(
        site: ProductionRankedAccessSourceV1,
        location: ProductionRankedAccessSourceV1,
    ) -> ProductionRankedAccessSourceV1 {
        ProductionRankedAccessSourceV1::new(
            site.semantic_block(),
            site.semantic_statement(),
            site.semantic_access_ordinal(),
            location.ranked_block(),
            location.ranked_operation(),
        )
    }

    #[test]
    fn actual_private_write_query_project_and_attach_keep_the_same_graph() {
        for index in [0, 7] {
            let materialized = assertion_materialized(source(index, None));
            let graph = materialized.executable().module().functions.as_ptr();
            let canonical = materialized
                .executable()
                .canonical()
                .canonical_bytes()
                .as_ptr();
            with_canonical_assertions_v1(&materialized, |session| {
                let site = Site::Statement {
                    block: SsaBlockIdV1::new(0),
                    statement: 1,
                };
                {
                    let mut facts = session.for_source(ROOT, ROOT);
                    assert!(facts.private_array_access(site, Role::Destination)?);
                }
                let mut wrong_body = session.for_source(ROOT, SemanticFunctionIdV1::from_index(1));
                assert!(matches!(
                    wrong_body.private_array_access(site, Role::Destination),
                    Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                        CanonicalAssertionErrorV1::PrivateArray(QueryError::InvalidSource(_))
                    ))
                ));
                Ok(())
            })
            .unwrap();
            let (materialized, root) = split(assertion_project(materialized).unwrap());
            assert_eq!(root.access_sources.len(), 1);
            let row = root.access_sources[0];
            assert_eq!(
                (
                    row.semantic_block(),
                    row.semantic_statement(),
                    row.semantic_access_ordinal()
                ),
                (0, Some(1), 0)
            );
            let attached = attach(materialized, root).unwrap();
            let retained = attached.pre_ranked_executable().unwrap();
            assert_eq!(retained.module().functions.as_ptr(), graph);
            assert_eq!(retained.canonical().canonical_bytes().as_ptr(), canonical);
            assert!(std::ptr::eq(attached.module(), retained.module()));
            assert!(!attached.grants_artifact_or_launch_authority());
        }
    }

    #[test]
    fn independently_admitted_legal_ranked_index_cannot_replace_the_source_index() {
        let actual = assertion_materialized(source(0, None));
        let (other_source, other_ranked) =
            split(assertion_project(assertion_materialized(source(1, None))).unwrap());
        drop(other_source);
        let Err(error) = attach(actual, other_ranked) else {
            panic!("different legal index must fail exact attachment");
        };
        assert!(
            matches!(error, LoweringError::MirPlironTranslation(Translation::AllocationOriginMismatch { location })
            if location.block == fe2o3_kernel_ir::BlockId(0) && location.operation_index == 6)
        );
    }

    #[test]
    fn unused_retained_private_source_rows_are_rejected_at_same_or_other_statement() {
        for same_statement in [true, false] {
            let actual = assertion_materialized(source(0, None));
            let (other_source, mut root) =
                split(assertion_project(assertion_materialized(source(0, Some(0)))).unwrap());
            drop(other_source);
            assert_eq!(root.access_sources.len(), 2);
            let extra = root.access_sources[1];
            root.access_sources[1] = ProductionRankedAccessSourceV1::new(
                0,
                Some(if same_statement { 1 } else { 0 }),
                u32::from(same_statement),
                extra.ranked_block(),
                extra.ranked_operation(),
            );
            let Err(error) = attach(actual, root) else {
                panic!("unused retained private source row must fail");
            };
            assert!(
                matches!(error, LoweringError::MirPlironTranslation(Translation::ExtraRankedEffect { ranked_block, ranked_operation })
                if ranked_block == extra.ranked_block() && ranked_operation == extra.ranked_operation())
            );
        }
    }

    #[test]
    fn swapped_source_locations_do_not_use_dense_ordinals_as_operand_roles() {
        for second in [0, 1] {
            let (materialized, mut root) =
                split(assertion_project(assertion_materialized(source(0, Some(second)))).unwrap());
            assert_eq!(root.access_sources.len(), 2);
            let first = root.access_sources[0];
            let last = root.access_sources[1];
            assert_eq!(
                (
                    first.semantic_access_ordinal(),
                    last.semantic_access_ordinal()
                ),
                (0, 0)
            );
            root.access_sources[0] = access_row(first, last);
            root.access_sources[1] = access_row(last, first);
            let Err(error) = attach(materialized, root) else {
                panic!("swapped distinct source effects must fail");
            };
            if second == 0 {
                assert!(matches!(
                    error,
                    LoweringError::MirPlironTranslation(Translation::ControlFlowMismatch { .. })
                ));
            } else {
                assert!(matches!(
                    error,
                    LoweringError::MirPlironTranslation(
                        Translation::AllocationOriginMismatch { .. }
                    )
                ));
            }
        }
    }

    #[test]
    fn reassigned_private_indices_use_exact_source_occurrences_and_attach() {
        for (first, second) in [(0, 0), (0, 1), (7, 0)] {
            let materialized = assertion_materialized(source(first, Some(second)));
            let function = &materialized.semantic_ssa().source_semantic().functions()[0];
            // Both writes use local2, but its two real definitions deliberately
            // remain unresolved by the unchanged function-wide constant map.
            assert_eq!(constant_locals(function).unwrap()[2], None);
            let canonical = materialized
                .executable()
                .canonical()
                .canonical_bytes()
                .as_ptr();
            with_canonical_assertions_v1(&materialized, |session| {
                let mut facts = session.for_source(ROOT, ROOT);
                for (statement, index) in [(1, first), (3, second)] {
                    assert_eq!(
                        facts.private_array_constant_index(
                            Site::Statement {
                                block: SsaBlockIdV1::new(0),
                                statement
                            },
                            Role::Destination,
                        )?,
                        Some(u64::from(index))
                    );
                }
                Ok(())
            })
            .unwrap();
            let (materialized, root) = split(assertion_project(materialized).unwrap());
            assert_eq!(root.access_sources.len(), 2);
            for (row, statement) in root.access_sources.iter().zip([1, 3]) {
                assert_eq!(
                    (
                        row.semantic_block(),
                        row.semantic_statement(),
                        row.semantic_access_ordinal()
                    ),
                    (0, Some(statement), 0)
                );
            }
            let attached = attach(materialized, root).unwrap();
            assert_eq!(
                attached
                    .pre_ranked_executable()
                    .unwrap()
                    .canonical()
                    .canonical_bytes()
                    .as_ptr(),
                canonical
            );
            assert!(!attached.grants_artifact_or_launch_authority());
        }
    }

    #[test]
    fn occurrence_index_projection_prepays_each_fixed_query_prefix() {
        let materialized = assertion_materialized(source(0, Some(1)));
        let semantic = materialized.semantic_ssa().source_semantic();
        let function = &semantic.functions()[0];
        let constants = constant_locals(function).unwrap();
        assert_eq!(constants[2], None);
        let SemanticStatementKindV1::Assign(assignment) =
            function.blocks()[0].statements()[1].kind()
        else {
            panic!("the genuine source write is an assignment");
        };
        let floor = materialized.executable_storage().retained_storage()
            + materialized.assert_origin_storage().payload_storage();
        with_canonical_assertions_v1(&materialized, |session| {
            // Shape2, global lookup2, type3, site3, then the index facade1.
            // These prefixes precede the unchanged owner/relation query.
            for (limit, accepted, attempted) in
                [(1, 0, 2), (3, 2, 4), (6, 4, 7), (9, 7, 10), (10, 10, 11)]
            {
                let mut work = Work::new(limit);
                let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
                budget.reserve_storage(floor).unwrap();
                let result = {
                    let mut facts =
                        session.for_source_with_query_budget_v1(&mut budget, ROOT, ROOT);
                    checked_private_array_statement_index_v1(
                        semantic.types(),
                        function,
                        0,
                        1,
                        assignment.destination(),
                        Role::Destination,
                        &constants,
                        &mut facts,
                    )
                };
                assert!(result.is_err());
                assert_eq!(budget.work(), accepted);
                assert_eq!((budget.storage(), budget.peak_storage()), (floor, floor));
                assert_eq!(work.failed_work(), Some(attempted));
            }
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn wrapped_private_write_uses_constructor_selected_body_not_root_ordinal() {
        let materialized = wrapped_literal_assertion();
        let mut work = Work::new(1_000_000);
        let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
        let floor = materialized.executable_storage().retained_storage()
            + materialized.assert_origin_storage().payload_storage();
        budget.reserve_storage(floor).unwrap();
        let site = Site::Statement {
            block: SsaBlockIdV1::new(0),
            statement: 1,
        };
        assert_eq!(
            materialized.has_materialized_private_array_access(
                ROOT,
                SemanticFunctionIdV1::from_index(1),
                site,
                Role::Destination,
                &mut budget
            ),
            Ok(true)
        );
        assert!(matches!(
            materialized.has_materialized_private_array_access(
                ROOT,
                ROOT,
                site,
                Role::Destination,
                &mut budget
            ),
            Err(QueryError::InvalidSource(_))
        ));
        assert_eq!(budget.storage(), floor);
        let (materialized, root) = split(assertion_project(materialized).unwrap());
        let attached = attach(materialized, root).unwrap();
        assert!(attached.pre_ranked_executable().is_some());
        assert!(!attached.grants_artifact_or_launch_authority());
    }
}
