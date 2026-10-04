#[derive(Clone, Copy, Debug)]
enum BinaryReadOperandV1 {
    Constant,
    Copy,
    Move,
}

fn private_binary_read_fixture_v1(
    read_rhs: bool,
    scalar: BinaryReadOperandV1,
) -> SemanticFunctionDeclV1 {
    let old = private_initializer_fixture_v1([9; 8], 1);
    let mut statements = old.blocks()[0].statements().to_vec();
    let SemanticStatementKindV1::Assign(write) = statements[2].kind() else {
        panic!("indexed destination");
    };
    let destination = write.destination().clone();
    let indexed = SemanticOperandV1::Copy(destination.clone());
    let scalar = match scalar {
        BinaryReadOperandV1::Constant => typed_constant(A_U32, 7, 4),
        BinaryReadOperandV1::Copy => SemanticOperandV1::Copy(typed_place(2, A_U32)),
        BinaryReadOperandV1::Move => SemanticOperandV1::Move(typed_place(2, A_U32)),
    };
    let (left, right) = if read_rhs {
        (scalar, indexed)
    } else {
        (indexed, scalar)
    };
    statements[2] = typed_assignment(
        2,
        A_U32,
        SemanticRvalueKindV1::Use(typed_constant(A_U32, 7, 4)),
    );
    statements.push(statement(SemanticStatementKindV1::Assign(
        SemanticAssignmentV1::new(
            destination,
            SemanticRvalueV1::new(
                A_U32,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::Subtract,
                    left,
                    right,
                },
            ),
        ),
    )));
    private_write_statements_v1(&old, statements)
}

#[test]
fn private_array_binary_read_binds_operand_role_not_access_ordinal() {
    use fe2o3_lower_mir_kernel::{
        ProductionMirPlironTranslationErrorV1 as Translation,
        ProductionSemanticKirErrorV1 as Lowering,
    };
    use fe2o3_pliron::ProductionSemanticSsaOperandRoleV1 as Role;
    for legacy in [false, true] {
        for read_rhs in [false, true] {
            for scalar in [
                BinaryReadOperandV1::Constant,
                BinaryReadOperandV1::Copy,
                BinaryReadOperandV1::Move,
            ] {
                let result = attach_private_write_fixture_v1(
                    private_binary_read_fixture_v1(read_rhs, scalar),
                    legacy,
                    |owner, root| {
                        assert_eq!(root.access_sources.len(), 9);
                        let row = root.access_sources.last().unwrap();
                        assert_eq!(
                            (
                                row.semantic_block(),
                                row.semantic_statement(),
                                row.semantic_access_ordinal()
                            ),
                            (0, Some(3), 0)
                        );
                        assert!(matches!(
                            root.verification
                                .ordinary()
                                .expect("ordinary test root")
                                .kernel()
                                .blocks()[row.ranked_block() as usize]
                                .operations()[row.ranked_operation() as usize],
                            ProductionRankedOperationV1::Access {
                                kind: AccessKindAttr::Read,
                                ..
                            }
                        ));
                        with_canonical_assertions_v1(owner, |session| {
                            let mut facts = session.for_source(ROOT, ROOT);
                            assert_eq!(
                                facts.private_array_access_index_v1(
                                    private_copy_source_v1(3).semantic_site.unwrap(),
                                    Role::RvalueOperand(u32::from(read_rhs))
                                )?,
                                Some(0)
                            );
                            assert!(
                                facts
                                    .private_array_access_index_v1(
                                        private_copy_source_v1(3).semantic_site.unwrap(),
                                        Role::RvalueOperand(u32::from(!read_rhs))
                                    )
                                    .is_err()
                            );
                            Ok(())
                        })
                        .unwrap();
                    },
                );
                // A real read relation does not prove the destination value.
                assert!(
                    matches!(
                        result,
                        Err(Lowering::MirPlironTranslation(
                            Translation::MissingRankedEffect {
                                semantic_block: 0,
                                semantic_statement: Some(3),
                                semantic_access_ordinal: 1
                            }
                        ))
                    ),
                    "legacy={legacy} rhs={read_rhs} scalar={scalar:?}: {result:?}"
                );
            }
        }
    }
}

#[test]
fn private_array_binary_read_prepays_branch_and_preserves_query_failures() {
    for read_rhs in [false, true] {
        let function = private_binary_read_fixture_v1(read_rhs, BinaryReadOperandV1::Move);
        let types = assertion_types();
        for limit in [47, 95, 96] {
            let mut work = Work::new(limit);
            let mut budget = Budget::new(&mut work, 23);
            budget.reserve_storage(23).unwrap();
            let mut facts = PrivateSourceMeterV1 {
                budget: &mut budget,
                calls: 0,
            };
            let result = private_array_read_source_v1::retained_read(
                &types,
                &function,
                &private_copy_source_v1(3),
                &mut facts,
            );
            if limit == 96 {
                assert!(matches!(
                    result,
                    Err(ProductionRankedProjectionErrorV1::Incomplete(
                        "private array read requires live canonical correspondence"
                    ))
                ));
                assert_eq!(facts.budget.work(), 96);
            } else {
                assert!(
                    matches!(result, Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(CanonicalAssertionErrorV1::Resource(Resource::Work(error)))) if error.actual() == limit + 1)
                );
                assert_eq!(facts.budget.work(), if limit == 47 { 0 } else { 48 });
            }
            assert_eq!(facts.calls, if limit == 47 { 1 } else { 2 });
            assert_eq!(
                (facts.budget.storage(), facts.budget.peak_storage()),
                (23, 23)
            );
        }
        let materialized = assertion_materialized(function);
        let floor = materialized.retained_analysis_storage_v1();
        let mut work = Work::new(1_000_000);
        let mut budget = Budget::new(&mut work, floor);
        budget.reserve_storage(floor).unwrap();
        assert_eq!(
            materialized.materialized_private_array_constant_index(
                ROOT,
                ROOT,
                fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1::Statement {
                    block: fe2o3_mir_model::SsaBlockIdV1::new(0),
                    statement: 3
                },
                fe2o3_pliron::ProductionSemanticSsaOperandRoleV1::RvalueOperand(u32::from(
                    read_rhs
                )),
                &mut budget
            ),
            Ok(Some(0))
        );
        let exact = budget.work() + 100; // helper96 + canonical adapter4
        with_canonical_assertions_v1(&materialized, |session| {
            for limit in [exact - 1, exact] {
                let mut work = Work::new(limit);
                let mut budget = Budget::new(&mut work, floor);
                budget.reserve_storage(floor).unwrap();
                let source = materialized.semantic_ssa().source_semantic();
                let result = private_array_read_source_v1::retained_read(source.types(), &source.functions()[0], &private_copy_source_v1(3), &mut session.for_source_with_query_budget_v1(&mut budget, ROOT, ROOT));
                if limit == exact {
                    assert!(result.unwrap());
                    assert_eq!(budget.work(), exact);
                } else {
                    assert!(matches!(result, Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(CanonicalAssertionErrorV1::PrivateArray(fe2o3_lower_mir_kernel::SemanticKirPrivateArrayQueryErrorV1::Resource(Resource::Work(error))))) if error.actual() == exact && error.limit() == limit));
                }
                assert_eq!((budget.storage(), budget.peak_storage()), (floor, floor));
            }
            Ok(())
        }).unwrap();
    }
}

#[test]
fn private_array_binary_read_rejects_unmodeled_operand_and_destination_shapes() {
    let old = private_binary_read_fixture_v1(false, BinaryReadOperandV1::Constant);
    let SemanticStatementKindV1::Assign(update) = old.blocks()[0].statements()[3].kind() else {
        panic!("update");
    };
    for case in 0..5 {
        let indexed = update.destination().clone();
        let (destination, left, right, result) = match case {
            0 => (
                indexed.clone(),
                SemanticOperandV1::Copy(indexed.clone()),
                SemanticOperandV1::Copy(indexed),
                A_U32,
            ),
            1 => (
                indexed.clone(),
                SemanticOperandV1::Move(indexed),
                typed_constant(A_U32, 1, 4),
                A_U32,
            ),
            2 => (
                typed_place(2, A_U32),
                SemanticOperandV1::Copy(indexed),
                typed_constant(A_U32, 1, 4),
                A_U32,
            ),
            3 => (
                indexed.clone(),
                SemanticOperandV1::Copy(indexed),
                typed_constant(A_BOOL, 1, 1),
                A_U32,
            ),
            _ => (
                indexed.clone(),
                SemanticOperandV1::Copy(indexed),
                typed_constant(A_U32, 1, 4),
                A_BOOL,
            ),
        };
        let mut statements = old.blocks()[0].statements().to_vec();
        statements[3] = statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            destination,
            SemanticRvalueV1::new(
                result,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::Add,
                    left,
                    right,
                },
            ),
        )));
        // Malformed-type cases are isolated predicate inputs, not admitted MIR.
        let function = private_write_statements_v1(&old, statements);
        let mut work = Work::new(96);
        let mut budget = Budget::new(&mut work, 0);
        let mut facts = PrivateSourceMeterV1 {
            budget: &mut budget,
            calls: 0,
        };
        assert!(
            !private_array_read_source_v1::retained_read(
                &assertion_types(),
                &function,
                &private_copy_source_v1(3),
                &mut facts
            )
            .unwrap(),
            "case={case}"
        );
        assert_eq!(facts.budget.work(), 96);
    }
}
