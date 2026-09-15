use super::*;

// Isolated receiver-audit fixtures, not canonical-admission positives. The
// unrelated operands must still be charged even though none mention the Grid.
fn with_operands(kind: u8, count: usize) -> SemanticFunctionDeclV1 {
    let original = caller(0);
    let mut blocks = original.blocks().to_vec();
    let operands = vec![constant(types().rank, 0, 8); count];
    blocks[3] = match kind {
        0 => block(
            4,
            vec![assign(
                7,
                types().grid,
                SemanticRvalueKindV1::Aggregate(
                    SemanticAggregateRvalueV1::new(SemanticAggregateKindV1::Array, operands)
                        .unwrap(),
                ),
            )],
            SemanticTerminatorKindV1::Return,
        ),
        1 => {
            blocks.push(block(5, vec![], SemanticTerminatorKindV1::Return));
            block(4, vec![], call(4, operands, 7, types().grid, 4))
        }
        2 => block(
            4,
            vec![],
            SemanticTerminatorKindV1::TailCall(
                SemanticDirectTailCallV1::new_callable(
                    SemanticCallableIdV1::from_index(4),
                    operands,
                    SemanticUnwindActionV1::Unreachable,
                )
                .unwrap(),
            ),
        ),
        _ => unreachable!(),
    };
    function(
        3,
        original.abi().clone(),
        &original
            .locals()
            .iter()
            .map(|local| (local.ty(), local.role()))
            .collect::<Vec<_>>(),
        0,
        blocks,
    )
}

fn spent(function: &SemanticFunctionDeclV1) -> u64 {
    let mut remaining = 1_000_000;
    observe_receiver(function, &mut remaining).unwrap();
    1_000_000 - remaining
}

#[test]
fn guarded_grid_variable_operands_charge_exact_existing_budget() {
    for kind in 0..3 {
        let empty = with_operands(kind, 0);
        let populated = with_operands(kind, 32);
        let mut small = spent(&empty);
        let cost = spent(&populated);
        assert!(cost >= small + 64, "kind {kind}: {small} -> {cost}");
        let mut exact = cost;
        observe_receiver(&populated, &mut exact).unwrap();
        assert_eq!(exact, 0);
        assert_eq!(
            observe_receiver(&populated, &mut (cost - 1)),
            Err(SemanticMirErrorV1::InvalidFunctionAbi)
        );
        assert_eq!(
            observe_receiver(&populated, &mut small),
            Err(SemanticMirErrorV1::InvalidFunctionAbi)
        );
    }
}

#[test]
fn guarded_grid_repeated_receiver_audits_share_remaining_work() {
    let function = with_operands(0, 64);
    let cost = spent(&function);
    let mut remaining = cost * 2 - 1;
    observe_receiver(&function, &mut remaining).unwrap();
    assert_eq!(remaining, cost - 1);
    assert_eq!(
        observe_receiver(&function, &mut remaining),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    );
    assert!(
        remaining < cost - 1,
        "failed audit cannot refund its prior work"
    );
}
