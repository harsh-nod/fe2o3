use super::*;

// Malformed or escaping uses must reject even before full memory/type admission.
// These are transparency tests, not source or atomic-operation authority.
fn escaping_statements() -> Vec<SemanticStatementKindV1> {
    let access = SemanticAtomicAccessV1::new(
        SemanticAtomicOrderingV1::Relaxed,
        SemanticAtomicScopeV1::Device,
    );
    let tracked = place(2, 5);
    let other = place(4, 7);
    let copy = |place: &SemanticPlaceV1| SemanticOperandV1::Copy(place.clone());
    let mut cases = vec![
        SemanticStatementKindV1::Assume(copy(&tracked)),
        SemanticStatementKindV1::Deinitialize(tracked.clone()),
        SemanticStatementKindV1::SetDiscriminant {
            place: tracked.clone(),
            variant_index: 0,
        },
    ];
    for target in 0..2 {
        cases.push(SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
            if target == 0 { &tracked } else { &other }.clone(),
            copy(if target == 1 { &tracked } else { &other }),
            SemanticVolatilityV1::NonVolatile,
            None,
        )));
    }
    for target in 0..3 {
        cases.push(SemanticStatementKindV1::AtomicRmw(
            SemanticAtomicRmwV1::new(
                if target == 0 { &tracked } else { &other }.clone(),
                if target == 1 { &tracked } else { &other }.clone(),
                copy(if target == 2 { &tracked } else { &other }),
                SemanticAtomicRmwOpV1::Add,
                access,
            ),
        ));
    }
    for target in 0..4 {
        cases.push(SemanticStatementKindV1::AtomicCompareExchange(
            SemanticAtomicCompareExchangeV1::new(
                if target == 0 { &tracked } else { &other }.clone(),
                if target == 1 { &tracked } else { &other }.clone(),
                copy(if target == 2 { &tracked } else { &other }),
                copy(if target == 3 { &tracked } else { &other }),
                access,
                SemanticAtomicOrderingV1::Relaxed,
                false,
            ),
        ));
    }
    cases
}

#[test]
fn non_assignment_dispatch_keeps_every_escaping_operand() {
    let callable = borrowed_callable(5, true);
    assert_eq!(
        direct_sites(
            &direct_function(direct_statements(), false),
            &[callable.clone()]
        )
        .len(),
        1
    );
    for (index, kind) in escaping_statements().into_iter().enumerate() {
        let mut statements = direct_statements();
        statements.push(statement(kind));
        assert!(
            direct_sites(&direct_function(statements, false), &[callable.clone()]).is_empty(),
            "non-assignment operand case {index} escaped its complete audit"
        );
    }
}

#[test]
fn non_assignment_dispatch_keeps_neutral_frames_and_exact_work_boundary() {
    let callable = borrowed_callable(5, true);
    let mut statements = direct_statements();
    for _ in 0..32 {
        statements.extend([
            statement(SemanticStatementKindV1::StorageLive(
                SemanticLocalIdV1::from_index(4),
            )),
            statement(SemanticStatementKindV1::StorageDead(
                SemanticLocalIdV1::from_index(4),
            )),
            statement(SemanticStatementKindV1::Nop),
        ]);
    }
    let body = direct_function(statements, false);
    let run = |limit| sites(&body, std::slice::from_ref(&callable), &[], limit, None);
    let expected = run(MAX_FLOW_WORK).unwrap();
    assert_eq!(expected.len(), 1);
    let (mut low, mut high) = (0, MAX_FLOW_WORK);
    while low < high {
        let mid = low + (high - low) / 2;
        if run(mid).is_ok() {
            high = mid;
        } else {
            low = mid + 1;
        }
    }
    assert_eq!(run(low).unwrap(), expected);
    let ProductionSemanticSsaErrorV1::BorrowFlowWork { error, .. } = run(low - 1).unwrap_err()
    else {
        panic!("the shared work owner must reject before returning partial sites");
    };
    assert!(
        matches!(*error, ProductionSemanticSsaErrorV1::AggregateResourceLimit {
        resource: SsaPlannerResourceV1::WorkUnits, required, limit,
    } if required == low && limit == low - 1)
    );
}
