//! Independent local-rule checker coverage, not production pass-capture evidence.
use super::*;
use fe2o3_kernel_ir::{AccessMode, AddressSpace, BinaryOp, MemoryAccess};

#[test]
fn cross_block_exact_pure_cse_preserves_ordered_store_and_rejects_operand_forgery() {
    for swapped in [false, true] {
        let parameters = vec![
            Type::pointer(U32, AddressSpace::Global, AccessMode::ReadWrite),
            U32,
            U32,
        ];
        let expression = |id, lhs, rhs| {
            value(
                id,
                U32,
                OperationKind::Binary {
                    op: BinaryOp::BitAnd,
                    lhs: ValueId(lhs),
                    rhs: ValueId(rhs),
                },
            )
        };
        let store = |value| {
            Operation::new(
                vec![],
                OperationKind::Store {
                    pointer: ValueId(99),
                    value: ValueId(value),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            )
        };
        let mut entry = BasicBlock::new(BlockId(10));
        entry.operations.push(expression(1, 8, 9));
        entry.terminator = Some(Terminator::Branch {
            target: BlockId(20),
            arguments: vec![],
        });
        let input = module(
            parameters.clone(),
            vec![U32],
            vec![99, 8, 9],
            vec![
                entry.clone(),
                returning(
                    20,
                    vec![
                        if swapped {
                            expression(2, 9, 8)
                        } else {
                            expression(2, 8, 9)
                        },
                        store(2),
                    ],
                    &[2],
                ),
            ],
        );
        let output = module(
            parameters,
            vec![U32],
            vec![99, 8, 9],
            vec![entry, returning(20, vec![store(1)], &[1])],
        );
        inspect(
            input,
            output,
            |a, b| {
                Plan {
                    chains: vec![vec![(0, None)], vec![(1, None)]],
                    operations: vec![Origin::Retained(op(0, 0)), Origin::Retained(op(1, 1))],
                    relations: vec![(99, 99, R), (8, 8, R), (9, 9, R), (1, 1, R), (2, 1, S)],
                    uses: vec![
                        operand(0, 0, 0),
                        operand(0, 0, 1),
                        operand(1, 1, 0),
                        operand(1, 1, 1),
                        term(1, 0),
                    ],
                    edges: vec![edge(0, 0)],
                    ..Plan::default()
                }
                .rows(a, b)
            },
            |a, b, rows, floor| {
                if swapped {
                    assert!(matches!(rejected(a, b, rows, floor), Error::Rule(_)));
                } else {
                    accepted(a, b, rows, floor);
                    rows.uses[0].input = operand(1, 0, 0);
                    assert_eq!(
                        rejected(a, b, rows, floor),
                        Error::Rule("final operation operand origin")
                    );
                }
            },
        );
    }
}
