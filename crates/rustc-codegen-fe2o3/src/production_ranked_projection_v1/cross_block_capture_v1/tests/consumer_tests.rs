use super::*;

#[test]
fn split_guard_capture_consumer_proves_product_and_increment_without_rewriting() {
    let fixture = Fixture::split_product_guard();
    let types = types();
    let function = fixture.function();
    let mut proof = SemanticAssertProofsV1::new(&types, &function).unwrap();
    assert_eq!(
        proof.range_at_operand(&copy(1, WORD), 4, 0).unwrap(),
        Some(UnsignedRangeProofV1 {
            minimum: 0,
            maximum: 31
        })
    );
    assert_eq!(
        proof.range_at_operand(&copy(1, WORD), 6, 0).unwrap(),
        Some(UnsignedRangeProofV1 {
            minimum: 0,
            maximum: u128::from(u64::MAX)
        })
    );
    let assertions = SemanticAssertProofsV1::analyze(&types, &function).unwrap();
    assert!(
        assertions[1],
        "the checked constant product requires its own proof"
    );
    assert!(
        assertions[4],
        "the checked increment uses the dominating true-edge bound"
    );
    assert_eq!(function, fixture.function());
}

#[test]
fn split_guard_capture_consumer_rejects_changed_values_and_guard_bypass() {
    for mutation in 0..6 {
        let mut fixture = Fixture::split_product_guard();
        match mutation {
            0 => fixture.insert_before_comparison(assign(
                1,
                WORD,
                SemanticRvalueKindV1::Use(scalar(WORD, u128::from(u64::MAX), 8)),
            )),
            1 => fixture.statements(
                3,
                vec![assign(
                    1,
                    WORD,
                    SemanticRvalueKindV1::Use(scalar(WORD, u128::from(u64::MAX), 8)),
                )],
            ),
            2 => fixture.terminator(0, switch(9, 1, 4)),
            3 => fixture.terminator(5, goto(2)),
            4 => fixture.terminator(2, switch(5, 3, 6)),
            5 => {
                fixture.statements(
                    4,
                    vec![checked(
                        6,
                        SemanticBinaryOpV1::Add,
                        copy(1, WORD),
                        scalar(WORD, u128::from(u64::MAX), 8),
                    )],
                );
                fixture.terminator(
                    4,
                    overflow(
                        6,
                        SemanticBinaryOpV1::Add,
                        copy(1, WORD),
                        scalar(WORD, u128::from(u64::MAX), 8),
                        5,
                    ),
                );
            }
            _ => unreachable!(),
        }
        let types = types();
        let function = fixture.function();
        let assertions = SemanticAssertProofsV1::analyze(&types, &function).unwrap();
        assert!(
            assertions[1],
            "unrelated total constant product, mutation {mutation}"
        );
        assert!(
            !assertions[4],
            "mutation {mutation} must retain the failing assertion"
        );
    }
}

#[test]
fn split_guard_capture_consumer_does_not_discharge_overflowing_bound_product() {
    let mut fixture = Fixture::split_product_guard();
    let mut statements = fixture.blocks[1].statements().to_vec();
    statements[1] = checked(
        3,
        SemanticBinaryOpV1::Multiply,
        scalar(WORD, u128::from(u64::MAX), 8),
        scalar(WORD, 2, 8),
    );
    fixture.statements(1, statements);
    fixture.terminator(
        1,
        overflow(
            3,
            SemanticBinaryOpV1::Multiply,
            scalar(WORD, u128::from(u64::MAX), 8),
            scalar(WORD, 2, 8),
            2,
        ),
    );
    let types = types();
    let function = fixture.function();
    let assertions = SemanticAssertProofsV1::analyze(&types, &function).unwrap();
    assert!(
        !assertions[1],
        "a retained assertion is not proof of its own success"
    );
    // The independently valid unsigned comparison still bounds the increment,
    // but cannot discharge the preceding, definitely overflowing product.
    let mut proof = SemanticAssertProofsV1::new(&types, &function).unwrap();
    assert_eq!(
        proof.range_at_operand(&copy(4, WORD), 2, 1).unwrap(),
        Some(UnsignedRangeProofV1 {
            minimum: 0,
            maximum: u128::from(u64::MAX),
        })
    );
    assert_eq!(
        proof.range_at_operand(&copy(1, WORD), 4, 0).unwrap(),
        Some(UnsignedRangeProofV1 {
            minimum: 0,
            maximum: u128::from(u64::MAX - 1),
        })
    );
    assert!(
        assertions[4],
        "the later unsigned strict guard independently proves adding one"
    );
    assert_eq!(
        assertions,
        vec![false, false, false, false, true, false, false]
    );
}

#[test]
fn split_guard_capture_consumer_shares_existing_work_owner() {
    let fixture = Fixture::split_product_guard();
    let types = types();
    let function = fixture.function();
    let mut proof = SemanticAssertProofsV1::new(&types, &function).unwrap();
    proof.work = MAX_PROJECTED_LOOP_GRAPH_WORK_V1 - 1;
    assert!(matches!(
        proof.range_at_operand(&copy(1, WORD), 4, 0),
        Err(ProductionRankedProjectionErrorV1::Unsupported(
            "uniform induction CFG analysis exceeds its work limit"
        ))
    ));
    assert!(proof.work > MAX_PROJECTED_LOOP_GRAPH_WORK_V1);
}
