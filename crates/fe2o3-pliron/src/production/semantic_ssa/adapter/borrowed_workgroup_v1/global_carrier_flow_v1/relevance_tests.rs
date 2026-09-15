use super::*;

fn work_error(error: ProductionSemanticSsaErrorV1) {
    let error = flow_work_profile_v1::original_error_for_test(error);
    assert!(
        matches!(
            error,
            ProductionSemanticSsaErrorV1::AggregateResourceLimit {
                resource: SsaPlannerResourceV1::WorkUnits,
                ..
            }
        ),
        "{error:?}"
    );
}

fn query_boundaries(
    query: impl Fn(&mut Budget) -> Result<bool, ProductionSemanticSsaErrorV1>,
    expected: bool,
) {
    let mut work = budget(MAX_FLOW_WORK);
    assert_eq!(query(&mut work).unwrap(), expected);
    let spent = MAX_FLOW_WORK - work.remaining;
    assert!(spent > 0);
    for limit in 0..spent {
        work_error(query(&mut budget(limit)).unwrap_err());
    }
    let mut exact = budget(spent);
    assert_eq!(query(&mut exact).unwrap(), expected);
    assert_eq!(exact.remaining, 0);
}

fn invalidator_node() -> SemanticBorrowCandidateV1 {
    SemanticBorrowCandidateV1 {
        site: SemanticTransparentBorrowSiteV1 {
            block: u32::MAX,
            statement: u32::MAX,
        },
        source_local: 4,
        source_type: ty(7),
        source_reference: None,
        value_alias: true, source_kind: SemanticBorrowCandidateSourceV1::Direct,
        valid: true,
        consumers: 0,
        intrinsic_consumer: false,
    }
}

fn statement_hit(statement: &SemanticStatementKindV1) -> bool {
    let mut nodes = [invalidator_node()];
    invalidate_reference_uses_in_statement_v1(
        statement,
        SemanticTransparentBorrowSiteV1 {
            block: 0,
            statement: 0,
        },
        &BTreeMap::from([(4, 0)]),
        &mut nodes,
    );
    !nodes[0].valid
}

fn terminator_hit(terminator: &SemanticTerminatorKindV1) -> bool {
    let mut nodes = [invalidator_node()];
    validate_reference_uses_in_terminator_v1(
        terminator,
        &[],
        &BTreeMap::from([(4, 0)]),
        &mut nodes,
    );
    !nodes[0].valid
}

fn edge(role: SemanticEdgeRoleV1) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(0))
}

// This corpus tests exhaustive syntactic invalidation, not type admission.
// Each position is independently the tracked local, a dynamic Index of it,
// or an unrelated local. Real two-Global transport is tested separately below.
fn slots(hot: Option<usize>, indexed: bool) -> [SemanticPlaceV1; 4] {
    std::array::from_fn(|index| {
        if hot != Some(index) {
            return place(3, ty(2));
        }
        if !indexed {
            return place(4, ty(7));
        }
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(3),
            vec![projection(
                SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(4)),
                ty(2),
            )],
            ty(2),
        )
        .unwrap()
    })
}

fn statement_corpus(p: &[SemanticPlaceV1; 4]) -> Vec<SemanticStatementKindV1> {
    let operand = |i: usize| SemanticOperandV1::Copy(p[i].clone());
    let access = SemanticAtomicAccessV1::new(
        SemanticAtomicOrderingV1::Relaxed,
        SemanticAtomicScopeV1::Device,
    );
    let rvalues = vec![
        SemanticRvalueKindV1::Use(operand(1)),
        SemanticRvalueKindV1::Use(SemanticOperandV1::Move(p[1].clone())),
        SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
            ty(0),
            SemanticConstantValueV1::ZeroSized,
        ))),
        SemanticRvalueKindV1::Unary {
            operation: SemanticUnaryOpV1::Not,
            operand: operand(1),
        },
        SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::Add,
            left: operand(1),
            right: operand(2),
        },
        SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
            SemanticCheckedBinaryOpV1::Add,
            operand(1),
            operand(2),
        )),
        SemanticRvalueKindV1::UncheckedBinary(SemanticUncheckedBinaryRvalueV1::new(
            SemanticUncheckedBinaryOpV1::Add,
            operand(1),
            operand(2),
        )),
        SemanticRvalueKindV1::Cast {
            kind: SemanticCastKindV1::Transmute,
            operand: operand(1),
        },
        SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Shared,
            place: p[1].clone(),
        },
        SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Mutable,
            place: p[1].clone(),
        },
        SemanticRvalueKindV1::AddressOf {
            mutability: SemanticMutabilityV1::Mutable,
            place: p[1].clone(),
        },
        SemanticRvalueKindV1::Length(p[1].clone()),
        SemanticRvalueKindV1::Discriminant(p[1].clone()),
        SemanticRvalueKindV1::Aggregate(
            SemanticAggregateRvalueV1::new(
                SemanticAggregateKindV1::Tuple,
                vec![operand(1), operand(2), operand(3)],
            )
            .unwrap(),
        ),
        SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
            p[1].clone(),
            SemanticVolatilityV1::Volatile,
            Some(access),
        )),
    ];
    let mut statements: Vec<_> = rvalues
        .into_iter()
        .map(|value| assignment(p[0].clone(), p[0].ty(), value))
        .collect();
    statements.extend([
        SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
            p[0].clone(),
            operand(1),
            SemanticVolatilityV1::Volatile,
            Some(access),
        )),
        SemanticStatementKindV1::AtomicRmw(SemanticAtomicRmwV1::new(
            p[0].clone(),
            p[1].clone(),
            operand(2),
            SemanticAtomicRmwOpV1::Add,
            access,
        )),
        SemanticStatementKindV1::AtomicCompareExchange(SemanticAtomicCompareExchangeV1::new(
            p[0].clone(),
            p[1].clone(),
            operand(2),
            operand(3),
            access,
            SemanticAtomicOrderingV1::Relaxed,
            false,
        )),
        SemanticStatementKindV1::SetDiscriminant {
            place: p[0].clone(),
            variant_index: 0,
        },
        SemanticStatementKindV1::Deinitialize(p[0].clone()),
        SemanticStatementKindV1::Assume(operand(0)),
        SemanticStatementKindV1::StorageLive(p[0].local()),
        SemanticStatementKindV1::StorageDead(p[0].local()),
        SemanticStatementKindV1::Nop,
    ]);
    statements
}

fn terminator_corpus(p: &[SemanticPlaceV1; 4]) -> Vec<SemanticTerminatorKindV1> {
    let operand = |i: usize| SemanticOperandV1::Copy(p[i].clone());
    let mut terms = vec![
        SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto)),
        SemanticTerminatorKindV1::SwitchInt {
            discriminant: operand(0),
            targets: SemanticSwitchTargetsV1::new(
                vec![],
                edge(SemanticEdgeRoleV1::SwitchOtherwise),
            )
            .unwrap(),
        },
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(0),
                vec![operand(1), operand(2), operand(3)],
                Some(SemanticCallDestinationV1::new(
                    p[0].clone(),
                    edge(SemanticEdgeRoleV1::CallReturn),
                )),
                SemanticUnwindActionV1::Continue,
            )
            .unwrap(),
        ),
        SemanticTerminatorKindV1::TailCall(
            SemanticDirectTailCallV1::new_callable(
                SemanticCallableIdV1::from_index(0),
                vec![operand(0), operand(1), operand(2), operand(3)],
                SemanticUnwindActionV1::Continue,
            )
            .unwrap(),
        ),
        SemanticTerminatorKindV1::Drop {
            place: p[0].clone(),
            drop_glue: SemanticFunctionIdV1::from_index(0),
            target: edge(SemanticEdgeRoleV1::DropReturn),
            unwind: SemanticUnwindActionV1::Continue,
        },
        SemanticTerminatorKindV1::FalseEdge {
            real_target: edge(SemanticEdgeRoleV1::FalseEdgeReal),
            imaginary_target: edge(SemanticEdgeRoleV1::FalseEdgeImaginary),
        },
        SemanticTerminatorKindV1::Return,
        SemanticTerminatorKindV1::UnwindResume,
        SemanticTerminatorKindV1::UnwindTerminate,
        SemanticTerminatorKindV1::Abort,
        SemanticTerminatorKindV1::Unreachable,
    ];
    let messages = [
        SemanticAssertMessageV1::BoundsCheck {
            length: operand(1),
            index: operand(2),
        },
        SemanticAssertMessageV1::Overflow {
            operation: SemanticBinaryOpV1::Add,
            left: operand(1),
            right: operand(2),
        },
        SemanticAssertMessageV1::DivisionByZero(operand(1)),
        SemanticAssertMessageV1::RemainderByZero(operand(1)),
        SemanticAssertMessageV1::MisalignedPointerDereference {
            required_alignment: operand(1),
            found_alignment: operand(2),
        },
        SemanticAssertMessageV1::NullPointerDereference,
        SemanticAssertMessageV1::ResumedAfterReturn,
        SemanticAssertMessageV1::ResumedAfterPanic,
    ];
    terms.extend(
        messages
            .into_iter()
            .map(|message| SemanticTerminatorKindV1::Assert {
                condition: operand(0),
                expected: true,
                message,
                target: edge(SemanticEdgeRoleV1::AssertSuccess),
                unwind: SemanticUnwindActionV1::Continue,
            }),
    );
    terms
}

#[test]
fn global_carrier_relevance_all_statement_positions_match_invalidator_and_work_bounds() {
    let filter = Relevance::new(14, [4].into_iter(), &mut budget(MAX_FLOW_WORK)).unwrap();
    for indexed in [false, true] {
        for hot in [None, Some(0), Some(1), Some(2), Some(3)] {
            for statement in statement_corpus(&slots(hot, indexed)) {
                query_boundaries(
                    |work| filter.statement(&statement, work),
                    statement_hit(&statement),
                );
            }
        }
    }
}

#[test]
fn global_carrier_relevance_all_terminator_positions_match_invalidator_and_work_bounds() {
    let filter = Relevance::new(14, [4].into_iter(), &mut budget(MAX_FLOW_WORK)).unwrap();
    for indexed in [false, true] {
        for hot in [None, Some(0), Some(1), Some(2), Some(3)] {
            for term in terminator_corpus(&slots(hot, indexed)) {
                query_boundaries(|work| filter.terminator(&term, work), terminator_hit(&term));
            }
        }
    }
}

#[test]
fn global_carrier_relevance_bitmap_capacity_boundaries_and_invalid_local() {
    for locals in [0, 1, 63, 64, 65, 128, 129] {
        let tracked: Vec<u32> = (0..locals).map(|local| local as u32).collect();
        let mut work = budget(MAX_FLOW_WORK);
        let filter = Relevance::new(locals, tracked.iter().copied(), &mut work).unwrap();
        let spent = MAX_FLOW_WORK - work.remaining;
        assert!(
            spent
                >= 3 + filter.capacity_words()
                    * std::mem::size_of::<u64>().div_ceil(std::mem::size_of::<usize>())
                    + tracked.len() * 2
        );
        for limit in 0..spent {
            let error = Relevance::new(locals, tracked.iter().copied(), &mut budget(limit))
                .err()
                .unwrap();
            work_error(error);
        }
        let mut exact = budget(spent);
        let _ = Relevance::new(locals, tracked.iter().copied(), &mut exact).unwrap();
        assert_eq!(exact.remaining, 0);
        for local in 0..=locals {
            let use_local = SemanticStatementKindV1::Assume(SemanticOperandV1::Copy(place(
                local as u32,
                ty(2),
            )));
            query_boundaries(|work| filter.statement(&use_local, work), local < locals);
        }
    }
    assert!(matches!(
        Relevance::new(64, [64].into_iter(), &mut budget(MAX_FLOW_WORK)),
        Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
    ));
}

#[test]
fn global_carrier_relevance_late_index_and_last_operand_are_not_skipped() {
    let filter = Relevance::new(14, [4].into_iter(), &mut budget(MAX_FLOW_WORK)).unwrap();
    let mut projections = vec![projection(SemanticProjectionKindV1::Field(0), ty(2)); 20];
    projections.push(projection(
        SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(4)),
        ty(2),
    ));
    let indexed =
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(3), projections, ty(2)).unwrap();
    let statement = SemanticStatementKindV1::Assume(SemanticOperandV1::Copy(indexed));
    assert!(statement_hit(&statement));
    query_boundaries(|work| filter.statement(&statement, work), true);
    let mut operands = vec![SemanticOperandV1::Copy(place(3, ty(2))); 64];
    operands.push(SemanticOperandV1::Move(place(4, ty(7))));
    let value = SemanticRvalueKindV1::Aggregate(
        SemanticAggregateRvalueV1::new(SemanticAggregateKindV1::Tuple, operands.clone()).unwrap(),
    );
    let aggregate = assignment(place(3, ty(2)), ty(2), value);
    assert!(statement_hit(&aggregate));
    query_boundaries(|work| filter.statement(&aggregate, work), true);
    let call = SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(0),
            operands,
            None,
            SemanticUnwindActionV1::Continue,
        )
        .unwrap(),
    );
    assert!(terminator_hit(&call));
    query_boundaries(|work| filter.terminator(&call, work), true);
}

fn audit_mode(
    function: &SemanticFunctionDeclV1,
    types: &[SemanticTypeDeclV1],
    terminator: &SemanticTerminatorKindV1,
    filtered: bool,
    limit: usize,
) -> Result<(BTreeSet<SemanticTransparentBorrowSiteV1>, usize), ProductionSemanticSsaErrorV1> {
    let facts: Vec<_> = GlobalBf16BorrowV1::for_callable(types, &callable(0, 62))
        .into_iter()
        .collect();
    assert_eq!(facts.len(), 1);
    let mut work = budget(limit);
    let index = global_statement_index_v1::GlobalStatementIndex::new(&facts, &mut work)?;
    let explicit = direct_definition_or_lifetime_locals_v1(function);
    let mut audit = Audit::new(function, Some(types), &facts, &index, &explicit, &mut work)?;
    for (block, body) in function.blocks().iter().enumerate() {
        for (statement, source) in body.statements().iter().enumerate() {
            let site = SemanticTransparentBorrowSiteV1 {
                block: block as u32,
                statement: statement as u32,
            };
            if filtered {
                audit.statement(site, source.kind(), &mut work)?;
            } else {
                audit.statement_relevant(site, source.kind(), &mut work)?;
            }
        }
        if filtered {
            audit.terminator(terminator, &mut work)?;
        } else {
            audit.terminator_relevant(terminator, &mut work)?;
        }
    }
    Ok((audit.finish(&mut work)?, limit - work.remaining))
}

#[test]
fn global_carrier_relevance_full_audit_original_roots_and_poison_differential() {
    let types = types();
    for mutation in 0..7 {
        let mut statements = source_statements();
        let mut term = SemanticTerminatorKindV1::Return;
        let mut expected = root_sites();
        match mutation {
            0 => {}
            1 => {
                statements.push(copy(7, 30, place(7, ty(30))));
                expected.clear();
            }
            2 => {
                statements[3] = copy(7, 30, place(7, ty(30)));
                expected.retain(|site| site.statement != 7);
            }
            3 => {
                statements[0] = SemanticStatementKindV1::Nop;
                expected.clear();
            }
            4 => {
                statements.push(SemanticStatementKindV1::Assume(SemanticOperandV1::Copy(
                    place(9, ty(7)),
                )));
                expected.clear();
            }
            5 => {
                term = SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new_callable(
                        SemanticCallableIdV1::from_index(0),
                        vec![SemanticOperandV1::Copy(place(9, ty(7)))],
                        None,
                        SemanticUnwindActionV1::Continue,
                    )
                    .unwrap(),
                );
                expected.clear();
            }
            6 => {
                statements.push(SemanticStatementKindV1::StorageDead(
                    SemanticLocalIdV1::from_index(9),
                ));
                statements.push(SemanticStatementKindV1::StorageLive(
                    SemanticLocalIdV1::from_index(9),
                ));
            }
            _ => unreachable!(),
        }
        let body = body(statements);
        let old = audit_mode(&body, &types, &term, false, MAX_FLOW_WORK).unwrap();
        let new = audit_mode(&body, &types, &term, true, MAX_FLOW_WORK).unwrap();
        assert_eq!(old.0, expected, "legacy mutation {mutation}");
        assert_eq!(new.0, old.0, "filtered mutation {mutation}");
        for filtered in [false, true] {
            let spent = if filtered { new.1 } else { old.1 };
            assert_eq!(
                audit_mode(&body, &types, &term, filtered, spent).unwrap().0,
                expected
            );
            work_error(audit_mode(&body, &types, &term, filtered, spent - 1).unwrap_err());
        }
    }
}

#[test]
fn global_carrier_relevance_scalar_tail_reduces_cost_without_changing_full_checks() {
    let mut statements = source_statements();
    statements.extend((0..1024).map(|_| copy(3, 2, place(3, ty(2)))));
    let body = body(statements);
    let types = types();
    let term = SemanticTerminatorKindV1::Return;
    let old = audit_mode(&body, &types, &term, false, MAX_FLOW_WORK).unwrap();
    let new = audit_mode(&body, &types, &term, true, MAX_FLOW_WORK).unwrap();
    assert_eq!(new.0, root_sites());
    assert_eq!(old.0, new.0);
    assert!(new.1 < old.1, "filtered {} full {}", new.1, old.1);
    assert_eq!(audit_mode(&body, &types, &term, true, new.1).unwrap(), new);
    work_error(audit_mode(&body, &types, &term, true, new.1 - 1).unwrap_err());
    work_error(audit_mode(&body, &types, &term, false, new.1).unwrap_err());
}
