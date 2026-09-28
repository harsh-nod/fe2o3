// B2 ORIGINAL content components only. No genuine CFG or source-prefix claim.
mod original_fixed_query_content_controls {
    use super::*;
    use crate::production_ranked_projection_v1::assertion_analyzer_resources_v1::original_fixed_constructor_retention::{
        ConstructorCuts, RetiredOriginalFixedOracleV1, Snapshot,
        Debit, expected_debits,
        QueryCuts, QueryDatum, QuerySource, QueryState, QuerySummary, QueryWitness,
        QUERY_CAP, ContentWitness, ContentRefusal, original_content_queries,
        content_added_frame, content_added_work, retained_content_queries,
        retained_content, exact_content_matches,
    };
    use crate::production_ranked_projection_v1::bf16_nominal_source_preparation_v1::{
        RichNominalSourceTablesV1 as Rich, with_rich_tables_for_test_v1,
    };
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrVerificationResourceErrorV1 as Resource,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    use fe2o3_lower_mir_kernel::Bf16NominalCallQueryErrorV1 as OuterError;
    use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

    const LIMIT: usize = 256 * 1024 * 1024;
    const FLOOR: usize = 47;
    const MAX_CUTS: usize = 4096;
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum Mode {
        Original,
        Retained,
        Retry,
        Occupied,
    }
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum Foreign {
        None,
        Types,
        Function,
        Graph,
        Rich,
    }
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum Failure {
        Incomplete(&'static str),
        Unsupported(&'static str),
        Resource(Resource),
    }
    fn failure(error: &ProductionRankedProjectionErrorV1) -> Failure {
        match error {
            ProductionRankedProjectionErrorV1::Incomplete(s) => Failure::Incomplete(s),
            ProductionRankedProjectionErrorV1::Unsupported(s) => Failure::Unsupported(s),
            ProductionRankedProjectionErrorV1::CanonicalAssertions(
                CanonicalAssertionErrorV1::Resource(e),
            ) => Failure::Resource(*e),
            _ => panic!("unclassified original query error; no equality/coverage claim"),
        }
    }
    #[derive(Debug)]
    struct Report {
        error: Option<Failure>,
        witness: QueryWitness,
        content: ContentWitness,
        result: Option<QuerySummary>,
        work: usize,
        owned: usize,
        prefix_work: usize,
        prefix_storage: usize,
        failed_work: Option<usize>,
        failed_storage: Option<usize>,
        retained: bool,
        state: QueryState,
        retry_inert: bool,
        same_panic: bool,
        outer_panic: bool,
    }
    fn good() -> SemanticFunctionDeclV1 {
        fixed_guard_function(FixedGuardOptions {
            extent: 4,
            ..Default::default()
        })
    }
    fn rebuild(
        f: &SemanticFunctionDeclV1,
        locals: Vec<SemanticLocalDeclV1>,
        entry: SemanticBlockIdV1,
        blocks: Vec<SemanticBasicBlockV1>,
    ) -> SemanticFunctionDeclV1 {
        SemanticFunctionDeclV1::new(
            f.identity(),
            f.role(),
            f.item_definition_identity(),
            f.monomorphization_identity(),
            f.generic_type_arguments_identity(),
            f.const_generic_arguments_identity(),
            f.source(),
            f.abi().clone(),
            locals,
            entry,
            blocks,
        )
        .unwrap()
    }
    fn scalar(n: u128, bytes: u8) -> SemanticOperandV1 {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            U64_TYPE,
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(n, bytes).unwrap()),
        ))
    }
    fn single_definition(separate: bool) -> (SemanticFunctionDeclV1, usize) {
        let f = good();
        let definition = typed_assignment(
            2,
            U64_TYPE,
            SemanticRvalueKindV1::Use(typed_operand(4, U64_TYPE)),
        );
        let mut locals = f.locals().to_vec();
        locals[2] = local(195, U64_TYPE, SemanticLocalRoleV1::Temporary);
        if separate {
            let mut guard = f.blocks()[0].terminator().kind().clone();
            let SemanticTerminatorKindV1::Assert { target, .. } = &mut guard else {
                unreachable!()
            };
            *target = cfg_edge(SemanticEdgeRoleV1::AssertSuccess, 2);
            (
                rebuild(
                    &f,
                    locals,
                    f.entry(),
                    vec![
                        block(
                            188,
                            vec![definition],
                            SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
                        ),
                        block(190, f.blocks()[0].statements().to_vec(), guard),
                        block(191, vec![], SemanticTerminatorKindV1::Return),
                    ],
                ),
                1,
            )
        } else {
            let mut blocks = f.blocks().to_vec();
            let mut statements = blocks[0].statements().to_vec();
            statements.insert(1, definition);
            blocks[0] = block(190, statements, blocks[0].terminator().kind().clone());
            (rebuild(&f, locals, f.entry(), blocks), 0)
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn invoke(
        types: &[SemanticTypeDeclV1],
        f: &SemanticFunctionDeclV1,
        graph: &ProjectedLoopCfgV1,
        rich: &Rich<'_>,
        query_rich: &Rich<'_>,
        other_types: &[SemanticTypeDeclV1],
        other_f: &SemanticFunctionDeclV1,
        other_graph: &ProjectedLoopCfgV1,
        budget: &mut Budget<'_>,
        queries: &[usize],
        mode: Mode,
        foreign: Foreign,
        owned: &mut usize,
        retired: &mut Option<RetiredOriginalFixedOracleV1>,
        state: &mut QueryState,
        cuts: &mut QueryCuts,
        witness: &mut QueryWitness,
        content: &mut ContentWitness,
        saved: &mut Option<ProductionRankedProjectionErrorV1>,
        result: &mut Option<QuerySummary>,
        prefix_work: &mut usize,
        prefix_storage: &mut usize,
        core_work: &mut usize,
        same_panic: &mut bool,
        retry_inert: &mut bool,
    ) -> Result<(), OuterError> {
        *prefix_work = budget.work();
        *prefix_storage = budget.storage();
        let source = QuerySource {
            types,
            function: f,
            graph,
            rich,
        };
        let query_source = QuerySource {
            types: if foreign == Foreign::Types {
                other_types
            } else {
                types
            },
            function: if foreign == Foreign::Function {
                other_f
            } else {
                f
            },
            graph: if foreign == Foreign::Graph {
                other_graph
            } else {
                graph
            },
            rich: query_rich,
        };
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let mut resources = PreparationResourcesV1::new(budget, owned);
            if mode == Mode::Original {
                original_content_queries(
                    source,
                    query_source,
                    &mut resources,
                    queries,
                    witness,
                    content,
                )
            } else {
                retained_content_queries(
                    source,
                    query_source,
                    &mut resources,
                    queries,
                    state,
                    retired,
                    &mut ConstructorCuts::new(None),
                    cuts,
                    witness,
                    content,
                )
            }
        }));
        *core_work = budget.work() - *prefix_work;
        assert_eq!(budget.storage(), *prefix_storage + *owned);
        if matches!(mode, Mode::Retry | Mode::Occupied) && outcome.is_ok() {
            let before_work = budget.work();
            let before_storage = budget.storage();
            let before_snapshot = retired
                .as_ref()
                .map(|p| p.snapshot(witness.before.unwrap().phase));
            let before_witness = *witness;
            let before_content = *content;
            let mut fresh = QueryState::Fresh;
            let again = {
                let mut resources = PreparationResourcesV1::new(budget, owned);
                retained_content_queries(
                    source,
                    query_source,
                    &mut resources,
                    queries,
                    if mode == Mode::Occupied {
                        &mut fresh
                    } else {
                        state
                    },
                    retired,
                    &mut ConstructorCuts::new(None),
                    cuts,
                    witness,
                    content,
                )
            };
            assert_eq!(
                failure(&again.unwrap_err()),
                Failure::Resource(Resource::Accounting)
            );
            assert_eq!(
                (budget.work(), budget.storage()),
                (before_work, before_storage)
            );
            assert_eq!(
                retired
                    .as_ref()
                    .map(|p| p.snapshot(before_witness.before.unwrap().phase)),
                before_snapshot
            );
            assert_eq!(witness.summary, before_witness.summary);
            assert_eq!(*content, before_content);
            *retry_inert = true;
        }
        match outcome {
            Ok(Ok(value)) => {
                *result = Some(value);
                Ok(())
            }
            Ok(Err(error)) => {
                // Keep the ACTUAL original backend Error across outer postflight.
                *saved = Some(error);
                Err(OuterError::Unavailable("original query component refusal"))
            }
            Err(payload) => {
                *same_panic = Some(
                    payload.as_ref() as *const (dyn std::any::Any + Send) as *const () as usize
                ) == cuts.original_address
                    && payload.downcast_ref::<[u64; 2]>()
                        == Some(&[0x6f72696771756572, 0x72657461696e6564]);
                assert!(*same_panic && witness.installed);
                assert_eq!(witness.before, witness.after);
                assert!(exact_content_matches(
                    &content.before.unwrap(),
                    &content.after.unwrap()
                ));
                resume_unwind(payload)
            }
        }
    }
    fn run(
        types: &[SemanticTypeDeclV1],
        f: &SemanticFunctionDeclV1,
        queries: &[usize],
        mode: Mode,
        foreign: Foreign,
        work_limit: usize,
        storage_limit: usize,
        panic_at: Option<usize>,
    ) -> Report {
        let other_types = types.to_vec();
        let other_f = rebuild(f, f.locals().to_vec(), f.entry(), f.blocks().to_vec());
        let graph = projected_loop_cfg_graph_v1(f).unwrap();
        let other_graph = projected_loop_cfg_graph_v1(f).unwrap();
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(FLOOR).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let address = &budget as *const Budget<'_> as usize;
        let mut owned = 0;
        let mut retired = None;
        let mut state = QueryState::Fresh;
        let mut cuts = QueryCuts::new(panic_at);
        let mut witness = QueryWitness::default();
        let mut content = ContentWitness::default();
        let mut saved = None;
        let mut result = None;
        let mut prefix_work = 0;
        let mut prefix_storage = 0;
        let mut core_work = 0;
        let mut same_panic = false;
        let mut retry_inert = false;
        let mut entered = false;
        let outer = with_rich_tables_for_test_v1(&[], types, f, &mut budget, |rich, budget| {
            entered = true;
            if foreign == Foreign::Rich {
                with_rich_tables_for_test_v1(&[], types, f, budget, |other_rich, budget| {
                    invoke(
                        types,
                        f,
                        &graph,
                        rich,
                        other_rich,
                        &other_types,
                        &other_f,
                        &other_graph,
                        budget,
                        queries,
                        mode,
                        foreign,
                        &mut owned,
                        &mut retired,
                        &mut state,
                        &mut cuts,
                        &mut witness,
                        &mut content,
                        &mut saved,
                        &mut result,
                        &mut prefix_work,
                        &mut prefix_storage,
                        &mut core_work,
                        &mut same_panic,
                        &mut retry_inert,
                    )
                })
            } else {
                invoke(
                    types,
                    f,
                    &graph,
                    rich,
                    rich,
                    &other_types,
                    &other_f,
                    &other_graph,
                    budget,
                    queries,
                    mode,
                    foreign,
                    &mut owned,
                    &mut retired,
                    &mut state,
                    &mut cuts,
                    &mut witness,
                    &mut content,
                    &mut saved,
                    &mut result,
                    &mut prefix_work,
                    &mut prefix_storage,
                    &mut core_work,
                    &mut same_panic,
                    &mut retry_inert,
                )
            }
        });
        assert!(
            entered,
            "component limits must admit authentic rich fixture setup"
        );
        assert_eq!(address, &budget as *const Budget<'_> as usize);
        assert!(ledger == budget.work_ledger_identity_v1());
        assert_eq!(budget.storage(), FLOOR + owned);
        let retained = retired.is_some();
        if let Some(payload) = &retired {
            assert_eq!(
                Some(payload.snapshot(witness.before.unwrap().phase)),
                witness.before
            );
            assert_eq!(witness.before, witness.after);
            let final_content = retained_content(payload, witness.before.unwrap().phase);
            assert_eq!(Some(final_content), content.after);
            assert!(exact_content_matches(
                &content.before.unwrap(),
                &final_content
            ));
        }
        let error = saved.as_ref().map(failure);
        let failed_work = budget.failed_work();
        let failed_storage = budget.failed_storage();
        drop(saved);
        drop(retired);
        drop(cuts);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.failed_work(), failed_work);
        assert_eq!(budget.failed_storage(), failed_storage);
        Report {
            error,
            witness,
            content,
            result,
            work: core_work,
            owned,
            prefix_work,
            prefix_storage,
            failed_work,
            failed_storage,
            retained,
            state,
            retry_inert,
            same_panic,
            outer_panic: matches!(outer, Err(OuterError::CallbackPanicked)),
        }
    }
    fn pair(
        types: &[SemanticTypeDeclV1],
        f: &SemanticFunctionDeclV1,
        queries: &[usize],
        foreign: Foreign,
        w: usize,
        s: usize,
    ) -> (Report, Report) {
        let original = run(types, f, queries, Mode::Original, foreign, w, s, None);
        let retained = run(types, f, queries, Mode::Retained, foreign, w, s, None);
        assert_eq!(retained.error, original.error);
        assert_eq!(retained.result, original.result);
        assert_eq!(retained.witness.summary, original.witness.summary);
        assert_eq!(retained.witness.stopped_at, original.witness.stopped_at);
        assert_eq!(retained.witness.logical, original.witness.logical);
        assert_eq!(retained.witness.failed, original.witness.failed);
        match original.content.before {
            Some(Ok(expected)) => assert!(exact_content_matches(
                &Ok(expected),
                &retained.content.before.unwrap()
            )),
            Some(Err(ContentRefusal::OriginalConstructionUnavailable)) => {
                assert!(!original.witness.summary.initialized);
            }
            Some(Err(other)) => panic!("original content coverage refused: {other:?}"),
            None => assert!(!original.witness.admitted),
        }
        assert_eq!(
            (retained.work, retained.owned),
            (original.work, original.owned)
        );
        assert_eq!(
            (retained.failed_work, retained.failed_storage),
            (original.failed_work, original.failed_storage)
        );
        assert_eq!(retained.state, QueryState::Terminal);
        if retained.witness.admitted {
            assert!(retained.retained && retained.witness.installed);
            assert_eq!(retained.witness.before, retained.witness.after);
        }
        (original, retained)
    }

    #[test]
    fn original_query_retained_entry_single_definition_and_repeated_data_match() {
        let types = assertion_proof_types();
        let mut fixtures = vec![
            (good(), 0),
            single_definition(false),
            single_definition(true),
        ];
        for (f, guard) in fixtures.drain(..) {
            let (_, value) = pair(
                &types,
                &f,
                &[guard, guard, guard],
                Foreign::None,
                LIMIT,
                LIMIT,
            );
            assert!(value.error.is_none());
            assert_eq!(value.witness.summary.completed, 3);
            assert!(value.witness.before.unwrap().dominance.unwrap().2 > 0);
            for row in value.witness.summary.rows[..3].iter() {
                assert_eq!(
                    *row,
                    Some(QueryDatum {
                        guard,
                        index: SemanticLocalIdV1::from_index(2),
                        extent: 4
                    })
                );
            }
            assert_eq!(value.witness.before.unwrap().zero.unwrap().2, 0);
            let once = run(
                &types,
                &f,
                &[guard],
                Mode::Retained,
                Foreign::None,
                LIMIT,
                LIMIT,
                None,
            );
            assert_eq!(
                value.owned, once.owned,
                "repeated original cache hits allocate no second proof/cache payload"
            );
            assert_eq!(
                value.witness.before.unwrap().dominance.unwrap().2,
                once.witness.before.unwrap().dominance.unwrap().2
            );
        }
    }
    #[test]
    fn original_query_retained_success_then_refusal_keeps_nonempty_payload() {
        let types = assertion_proof_types();
        let f = good();
        for coordinate in [1, 99] {
            let (_, value) = pair(&types, &f, &[0, coordinate, 0], Foreign::None, LIMIT, LIMIT);
            assert_eq!(value.witness.summary.completed, 1);
            assert_eq!(value.witness.stopped_at, Some(1));
            assert!(matches!(value.error, Some(Failure::Unsupported(_))));
            assert!(value.witness.failed);
            assert!(value.witness.before.unwrap().dominance.unwrap().2 > 0);
        }
    }
    #[test]
    fn original_query_retained_original_source_identity_is_not_structural_equality() {
        let types = assertion_proof_types();
        let f = good();
        for foreign in [
            Foreign::Types,
            Foreign::Function,
            Foreign::Graph,
            Foreign::Rich,
        ] {
            let (_, value) = pair(&types, &f, &[0], foreign, LIMIT, LIMIT);
            assert_eq!(
                value.error,
                Some(Failure::Unsupported(
                    "prepared fixed guard query differs from its original source loan",
                ))
            );
            assert_eq!(value.witness.summary.completed, 0);
            assert!(value.witness.failed);
        }
    }
    #[test]
    fn original_query_retained_shape_comparison_extent_and_order_errors_match() {
        let types = assertion_proof_types();
        let base = FixedGuardOptions {
            extent: 4,
            ..Default::default()
        };
        for options in [
            FixedGuardOptions {
                wrong_comparison: true,
                ..base
            },
            FixedGuardOptions {
                wrong_message: true,
                ..base
            },
            FixedGuardOptions {
                expected_false: true,
                ..base
            },
            FixedGuardOptions { extent: 0, ..base },
        ] {
            let f = fixed_guard_function(options);
            let (_, value) = pair(&types, &f, &[0, 0], Foreign::None, LIMIT, LIMIT);
            assert!(matches!(value.error, Some(Failure::Incomplete(_))));
            assert_eq!(value.witness.summary.completed, 0);
            assert_eq!(value.witness.stopped_at, Some(0));
        }
        // An argument overwritten from a different argument conflicts during
        // rich provenance preparation (covered separately below). A temporary
        // keeps that early boundary out of this query-order negative control.
        let late = fixed_guard_function(FixedGuardOptions {
            late_index: true,
            ..base
        });
        let mut locals = late.locals().to_vec();
        locals[2] = local(195, U64_TYPE, SemanticLocalRoleV1::Temporary);
        let late = rebuild(&late, locals, late.entry(), late.blocks().to_vec());
        let (_, value) = pair(&types, &late, &[0, 0], Foreign::None, LIMIT, LIMIT);
        assert_eq!(
            value.error,
            Some(Failure::Incomplete(
                "a fixed-array bounds check lacks stable exact unsigned index < literal extent evidence",
            )),
        );
        assert_eq!(value.witness.summary.completed, 0);
        assert_eq!(value.witness.stopped_at, Some(0));
        assert!(value.witness.failed);

        let f = good();
        let mut blocks = f.blocks().to_vec();
        let mut guard = blocks[0].terminator().kind().clone();
        let SemanticTerminatorKindV1::Assert {
            message: SemanticAssertMessageV1::BoundsCheck { length, .. },
            ..
        } = &mut guard
        else {
            unreachable!()
        };
        *length = scalar(4, 4);
        blocks[0] = block(190, blocks[0].statements().to_vec(), guard);
        let wrong_width = rebuild(&f, f.locals().to_vec(), f.entry(), blocks);
        assert!(
            pair(&types, &wrong_width, &[0], Foreign::None, LIMIT, LIMIT)
                .1
                .error
                .is_some()
        );
    }
    #[test]
    fn original_query_argument_overwrite_is_refused_before_rich_callback() {
        // Preserve the original hostile fixture: local 2 is still Argument(0),
        // and its late definition copies local 4, Argument(1). The original
        // provenance merge rejects the conflicting origins before a rich loan.
        let types = assertion_proof_types();
        let function = fixed_guard_function(FixedGuardOptions {
            extent: 4,
            late_index: true,
            ..Default::default()
        });
        assert_eq!(
            function.locals()[2].role(),
            SemanticLocalRoleV1::Argument(0)
        );
        assert_eq!(
            function.locals()[4].role(),
            SemanticLocalRoleV1::Argument(1)
        );
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let address = &budget as *const Budget<'_> as usize;
        let ledger = budget.work_ledger_identity_v1();
        let before_work = budget.work();
        let before_peak = budget.peak_storage();
        let mut entered = false;
        let result = with_rich_tables_for_test_v1(&[], &types, &function, &mut budget, |_, _| {
            entered = true;
            Ok(())
        });
        assert!(
            !entered,
            "conflicting origins must not produce a rich query loan"
        );
        assert!(matches!(
            result,
            Err(OuterError::Unavailable("actual source preparation refused")),
        ));
        assert_eq!(address, &budget as *const Budget<'_> as usize);
        assert!(ledger == budget.work_ledger_identity_v1());
        assert_eq!(budget.storage(), FLOOR);
        assert!(budget.work() > before_work);
        assert!(budget.peak_storage() >= before_peak);
        assert_eq!(budget.failed_work(), None);
        assert_eq!(budget.failed_storage(), None);
    }

    #[test]
    fn original_query_retained_exact_unsigned_width_and_extent_data_are_preserved() {
        let ordinary = assertion_proof_types();
        for extent in [1, u64::MAX] {
            let f = fixed_guard_function(FixedGuardOptions {
                extent,
                ..Default::default()
            });
            let (_, value) = pair(&ordinary, &f, &[0], Foreign::None, LIMIT, LIMIT);
            assert_eq!(value.result.unwrap().rows[0].unwrap().extent, extent);
        }
        let f = good();
        let mut types = ordinary;
        types[U64_TYPE.index() as usize] = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(230)),
            SemanticLayoutIdentityV1::from_sha256(bytes(230)),
            SemanticTypeLayoutV1::new(Some(4), 4).unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                bits: 32,
                signed: false,
            }),
        );
        let mut blocks = f.blocks().to_vec();
        let mut statements = blocks[0].statements().to_vec();
        statements[1] = typed_assignment(
            3,
            BOOL_TYPE,
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::LessThan,
                left: typed_operand(2, U64_TYPE),
                right: scalar(4, 4),
            },
        );
        let mut guard = blocks[0].terminator().kind().clone();
        let SemanticTerminatorKindV1::Assert {
            message: SemanticAssertMessageV1::BoundsCheck { length, .. },
            ..
        } = &mut guard
        else {
            unreachable!()
        };
        *length = scalar(4, 4);
        blocks[0] = block(190, statements, guard);
        let f = rebuild(&f, f.locals().to_vec(), f.entry(), blocks);
        let (_, value) = pair(&types, &f, &[0], Foreign::None, LIMIT, LIMIT);
        assert_eq!(
            value.result.unwrap().rows[0],
            Some(QueryDatum {
                guard: 0,
                index: SemanticLocalIdV1::from_index(2),
                extent: 4,
            })
        );
    }
    #[test]
    fn original_query_retained_false_dominance_and_self_success_keep_original_boundary() {
        let types = assertion_proof_types();
        let f = good();
        let mut blocks = f.blocks().to_vec();
        blocks.push(block(
            192,
            vec![],
            SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
        ));
        let bypass = rebuild(
            &f,
            f.locals().to_vec(),
            SemanticBlockIdV1::from_index(2),
            blocks,
        );
        let (_, refused) = pair(&types, &bypass, &[0], Foreign::None, LIMIT, LIMIT);
        assert!(refused.error.is_some());
        assert!(refused.witness.before.unwrap().dominance.unwrap().2 > 0);
        let mut blocks = f.blocks().to_vec();
        let mut guard = blocks[0].terminator().kind().clone();
        let SemanticTerminatorKindV1::Assert { target, .. } = &mut guard else {
            unreachable!()
        };
        *target = cfg_edge(SemanticEdgeRoleV1::AssertSuccess, 0);
        blocks[0] = block(190, blocks[0].statements().to_vec(), guard);
        let self_target = rebuild(&f, f.locals().to_vec(), f.entry(), blocks);
        assert!(
            pair(&types, &self_target, &[0], Foreign::None, LIMIT, LIMIT)
                .1
                .error
                .is_none(),
            "unique-success/entry rejection remains AFTER this original authentication"
        );
    }
    #[test]
    fn original_query_retained_post_query_panics_install_before_same_box_resume() {
        let types = assertion_proof_types();
        let f = good();
        for ordinal in 1..=3 {
            let value = run(
                &types,
                &f,
                &[0, 0, 0],
                Mode::Retained,
                Foreign::None,
                LIMIT,
                LIMIT,
                Some(ordinal),
            );
            assert!(value.same_panic && value.outer_panic && value.retained);
            assert_eq!(value.witness.summary.completed, ordinal);
            assert!(value.witness.before.unwrap().dominance.unwrap().2 > 0);
            assert_eq!(value.state, QueryState::Terminal);
            let original = run(
                &types,
                &f,
                &vec![0; ordinal],
                Mode::Original,
                Foreign::None,
                LIMIT,
                LIMIT,
                None,
            );
            assert!(exact_content_matches(
                &original.content.before.unwrap(),
                &value.content.before.unwrap()
            ));
        }
    }
    #[test]
    fn original_query_retained_empty_cap_retry_and_occupied_slot_are_closed() {
        let types = assertion_proof_types();
        let f = good();
        let (_, empty) = pair(&types, &f, &[], Foreign::None, LIMIT, LIMIT);
        assert!(!empty.witness.summary.initialized);
        assert_eq!(
            (empty.work, empty.owned),
            (
                content_added_work().unwrap(),
                content_added_frame().unwrap()
            )
        );
        let snapshot = empty.witness.before.unwrap();
        assert_eq!((snapshot.checked.1, snapshot.checked.2), (0, 0));
        assert!(snapshot.dominance.is_none() && snapshot.zero.is_none());
        let (_, too_many) = pair(&types, &f, &[0; QUERY_CAP + 1], Foreign::None, LIMIT, LIMIT);
        assert!(too_many.error.is_some() && !too_many.retained && !too_many.witness.admitted);
        assert_eq!((too_many.work, too_many.owned), (0, 0));
        for mode in [Mode::Retry, Mode::Occupied] {
            for queries in [&[0usize][..], &[0usize, 1][..]] {
                assert!(
                    run(&types, &f, queries, mode, Foreign::None, LIMIT, LIMIT, None).retry_inert
                );
            }
        }
    }
    #[test]
    fn original_query_retained_added_prefix_one_short_refuses_before_owner() {
        let types = assertion_proof_types();
        let f = good();
        let baseline = run(
            &types,
            &f,
            &[0],
            Mode::Original,
            Foreign::None,
            LIMIT,
            LIMIT,
            None,
        );
        let prefix = content_added_frame().unwrap();
        let (_, exact) = pair(
            &types,
            &f,
            &[],
            Foreign::None,
            baseline.prefix_work + content_added_work().unwrap(),
            baseline.prefix_storage + prefix,
        );
        assert!(exact.error.is_none() && exact.witness.admitted && exact.retained);
        assert_eq!(
            (exact.work, exact.owned),
            (content_added_work().unwrap(), prefix)
        );
        for (w, s) in [
            (
                baseline.prefix_work + content_added_work().unwrap() - 1,
                LIMIT,
            ),
            (LIMIT, baseline.prefix_storage + prefix - 1),
        ] {
            let (_, value) = pair(&types, &f, &[0], Foreign::None, w, s);
            assert!(!value.witness.admitted && !value.retained);
            assert_eq!(value.witness.summary.completed, 0);
            assert!(matches!(value.error, Some(Failure::Resource(_))));
        }
    }
    #[test]
    fn original_query_retained_all_reached_positive_suffix_debit_cuts_are_original_discovered() {
        // Bounded adaptive ORIGINAL oracle, not candidate residual arithmetic.
        // At each reached denial, the ORIGINAL Budget supplies attempted total.
        // Advancing to it admits that debit and exposes the next positive debit.
        let types = assertion_proof_types();
        let (f, guard) = single_definition(true);
        let queries = [guard, guard, guard];
        let original = run(
            &types,
            &f,
            &queries,
            Mode::Original,
            Foreign::None,
            LIMIT,
            LIMIT,
            None,
        );
        assert!(original.error.is_none());
        let (constructor_work, constructor_storage) =
            expected_debits(&f)
                .unwrap()
                .iter()
                .fold((0usize, 0usize), |(w, s), d| match d {
                    Debit::Work(n) => (w.checked_add(*n).unwrap(), s),
                    Debit::Storage(n) => (w, s.checked_add(*n).unwrap()),
                });
        let prefix = content_added_frame().unwrap();
        let mut counts = [0usize; 2];
        let mut retained_after_success = false;
        let mut partial_cache_before_query_success = false;
        for storage in [false, true] {
            let mut limit = if storage {
                original
                    .prefix_storage
                    .checked_add(prefix)
                    .unwrap()
                    .checked_add(constructor_storage)
                    .unwrap()
            } else {
                original
                    .prefix_work
                    .checked_add(content_added_work().unwrap())
                    .unwrap()
                    .checked_add(constructor_work)
                    .unwrap()
            };
            assert!(limit < LIMIT);
            let mut finished = false;
            for _ in 0..MAX_CUTS {
                let (w, s) = if storage {
                    (LIMIT, limit)
                } else {
                    (limit, LIMIT)
                };
                let denied = run(
                    &types,
                    &f,
                    &queries,
                    Mode::Original,
                    Foreign::None,
                    w,
                    s,
                    None,
                );
                let attempted = if storage {
                    denied.failed_storage
                } else {
                    denied.failed_work
                };
                let Some(attempted) = attempted else {
                    assert!(
                        denied.error.is_none(),
                        "terminal unknown/error ends qualification"
                    );
                    assert_eq!(denied.result, original.result);
                    finished = true;
                    break;
                };
                assert!(
                    attempted > limit && attempted <= LIMIT,
                    "strict finite progress within named budget cap"
                );
                let exact_short = attempted.checked_sub(1).unwrap();
                let (w, s) = if storage {
                    (LIMIT, exact_short)
                } else {
                    (exact_short, LIMIT)
                };
                let (expected, observed) = pair(&types, &f, &queries, Foreign::None, w, s);
                assert_eq!(
                    if storage {
                        expected.failed_storage
                    } else {
                        expected.failed_work
                    },
                    Some(attempted)
                );
                assert!(matches!(expected.error, Some(Failure::Resource(_))));
                assert!(observed.retained);
                if observed.witness.summary.completed == 0
                    && observed
                        .witness
                        .before
                        .unwrap()
                        .dominance
                        .is_some_and(|row| row.2 > 0)
                {
                    partial_cache_before_query_success = true;
                    assert!(exact_content_matches(
                        &expected.content.before.unwrap(),
                        &observed.content.before.unwrap()
                    ));
                }
                if observed.witness.summary.completed > 0 {
                    retained_after_success = true;
                    assert!(observed.witness.before.unwrap().dominance.unwrap().2 > 0);
                }
                counts[usize::from(storage)] += 1;
                limit = attempted;
            }
            assert!(
                finished,
                "MAX_CUTS is a terminal observation bound, never an unlimited retry"
            );
        }
        assert!(counts[0] > 10 && counts[1] > 2);
        assert!(
            partial_cache_before_query_success,
            "original debit cuts must reach a partially populated query cache"
        );
        assert!(
            retained_after_success,
            "later denial must preserve earlier successful query payload"
        );
    }
    #[test]
    fn original_content_checked_entries_match_original_after_query_refusal() {
        let types = assertion_proof_types();
        let function = checked_arithmetic_chain_function(false);
        let (_, retained) = pair(&types, &function, &[0], Foreign::None, LIMIT, LIMIT);
        assert!(retained.error.is_some());
        let content = retained.content.before.unwrap().unwrap();
        assert!(content.checked.used > 0);
        assert!(content.checked.rows > 0);
        assert_eq!(Some(Ok(content)), retained.content.after);
        assert_eq!(content.zero.unwrap().len, 0);
    }
    #[test]
    fn original_content_equal_query_counts_reject_index_extent_coordinate_and_order_changes() {
        // Comparator-only inert DATA: not an authenticated source or query.
        let mut expected = QuerySummary::default();
        expected.initialized = true;
        expected.completed = 2;
        expected.rows[0] = Some(QueryDatum {
            guard: 3,
            index: SemanticLocalIdV1::from_index(2),
            extent: 4,
        });
        expected.rows[1] = Some(QueryDatum {
            guard: 5,
            index: SemanticLocalIdV1::from_index(7),
            extent: 8,
        });
        let mut changed = expected;
        changed.rows[0].as_mut().unwrap().index = SemanticLocalIdV1::from_index(9);
        assert_eq!(changed.completed, expected.completed);
        assert_ne!(changed, expected);
        changed = expected;
        changed.rows[0].as_mut().unwrap().extent = 5;
        assert_ne!(changed, expected);
        changed = expected;
        changed.rows[0].as_mut().unwrap().guard = 6;
        assert_ne!(changed, expected);
        changed = expected;
        changed.rows.swap(0, 1);
        assert_ne!(changed, expected);
    }
}
