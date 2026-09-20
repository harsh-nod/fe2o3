//! Constructed observation-helper controls, not ordinary-source qualification.
use super::*;
use fe2o3_kernel_ir::{
    AccessMode, BasicBlock, BlockId, CanonicalKirOperationTransitionV1 as Row, MemoryAccess,
    Signature, Terminator, ValueDef,
};

type Fixture = (
    Module,
    Module,
    Vec<Row>,
    BTreeMap<Coordinate, (ValueId, ValueId)>,
);

fn fixture(integer: Integer) -> Fixture {
    let ty = Type::Scalar(integer.scalar());
    let binary = |id, op, lhs, rhs, result_type| {
        Operation::new(
            vec![ValueDef::new(ValueId(id), result_type)],
            Kind::Binary {
                op,
                lhs: ValueId(lhs),
                rhs: ValueId(rhs),
            },
        )
    };
    let store = |value| {
        Operation::new(
            vec![],
            Kind::Store {
                pointer: ValueId(0),
                value: ValueId(value),
                access: MemoryAccess::new(AddressSpace::Global, integer.width() / 8),
            },
        )
    };
    let mut first = BasicBlock::new(BlockId(10));
    first.operations = vec![binary(10, BinaryOp::BitAnd, 1, 2, ty.clone()), store(10)];
    first.terminator = Some(Terminator::Branch {
        target: BlockId(20),
        arguments: vec![],
    });
    let mut second = BasicBlock::new(BlockId(20));
    second.operations = vec![
        binary(20, BinaryOp::Divide, 3, 3, Type::Scalar(ScalarType::U32)),
        binary(11, BinaryOp::BitAnd, 2, 1, ty.clone()),
        store(11),
    ];
    second.terminator = Some(Terminator::Branch {
        target: BlockId(30),
        arguments: vec![],
    });
    let mut trap = BasicBlock::new(BlockId(30));
    trap.operations.push(Diagnostic::Trap.operation(None));
    trap.terminator = Some(Terminator::Unreachable);
    let mut input = Module::new("observation-helper-only");
    input.functions.push(Function::internal_helper(
        "arbitrary-name",
        Signature::new(
            vec![
                Type::pointer(ty.clone(), AddressSpace::Global, AccessMode::ReadWrite),
                ty.clone(),
                ty,
                Type::Scalar(ScalarType::U32),
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
        vec![first, second, trap],
    ));
    input.functions.push(Diagnostic::Trap.declaration());
    let mut output = input.clone();
    let body = output.functions[0].body.as_mut().unwrap();
    body.blocks[1].operations.remove(1);
    body.blocks[1].operations[1] = store(10);
    let mut rows = Vec::new();
    for (bi, block) in body.blocks.iter().enumerate() {
        for oi in 0..block.operations.len() {
            let source = if bi == 1 && oi == 1 { 2 } else { oi };
            rows.push(Row {
                output: coordinate(0, bi, oi),
                origin: Origin::Retained(coordinate(0, bi, source)),
            });
        }
    }
    let replacements = [(coordinate(0, 1, 1), (ValueId(11), ValueId(10)))]
        .into_iter()
        .collect();
    (input, output, rows, replacements)
}
fn candidate(rows: &[Row]) -> Candidate<'_> {
    Candidate {
        functions: &[],
        blocks: &[],
        segments: &[],
        operations: rows,
        definitions: &[],
        definition_outputs: &[],
        uses: &[],
        edges: &[],
        edge_arguments: &[],
    }
}

#[test]
fn commutative_source_observer_requires_swapped_dynamic_dominating_pair_each_width() {
    for integer in Integer::ALL {
        let case = Case {
            integer,
            target: Target::Gfx942,
        };
        let (input, _, _, _) = fixture(integer);
        let function = &input.functions[0];
        let pair = binaries(function, 0, case, BinaryOp::BitAnd).unwrap();
        reversed_pair(function, &pair).unwrap();
        divisions(function, 0).unwrap();
        let mut ordered = function.clone();
        let operation = &mut ordered.body.as_mut().unwrap().blocks[1].operations[1];
        operation.kind = Kind::Binary {
            op: BinaryOp::BitAnd,
            lhs: ValueId(1),
            rhs: ValueId(2),
        };
        assert!(
            reversed_pair(
                &ordered,
                &binaries(&ordered, 0, case, BinaryOp::BitAnd).unwrap()
            )
            .is_err()
        );
        let mut foreign = function.clone();
        foreign.body.as_mut().unwrap().blocks[1].operations[1].kind = Kind::Binary {
            op: BinaryOp::BitAnd,
            lhs: ValueId(2),
            rhs: ValueId(2),
        };
        assert!(binaries(&foreign, 0, case, BinaryOp::BitAnd).is_err());
        let mut same = function.clone();
        let moved = same.body.as_mut().unwrap().blocks[1].operations.remove(1);
        same.body.as_mut().unwrap().blocks[0].operations.push(moved);
        assert!(
            reversed_pair(&same, &binaries(&same, 0, case, BinaryOp::BitAnd).unwrap()).is_err()
        );
        let mut unreachable = function.clone();
        unreachable.body.as_mut().unwrap().blocks[0].terminator =
            Some(Terminator::Return { values: vec![] });
        assert!(
            reversed_pair(
                &unreachable,
                &binaries(&unreachable, 0, case, BinaryOp::BitAnd).unwrap()
            )
            .is_err()
        );
        let mut no_divide = function.clone();
        no_divide.body.as_mut().unwrap().blocks[1]
            .operations
            .remove(0);
        assert!(divisions(&no_divide, 0).is_err());
        let mut other_divide = function.clone();
        other_divide.body.as_mut().unwrap().blocks[1].operations[0].kind = Kind::Binary {
            op: BinaryOp::Divide,
            lhs: ValueId(3),
            rhs: ValueId(1),
        };
        assert!(divisions(&other_divide, 0).is_err());
    }
}

#[test]
fn commutative_source_observer_refuses_lost_divide_trap_store_cfg_and_origin() {
    let (input, output, rows, replacements) = fixture(Integer::U32);
    let traps = preserve(&input, &output, candidate(&rows), &replacements).unwrap();
    assert_eq!(traps.get(&0), Some(&1));
    for mutation in 0..7 {
        let mut changed = output.clone();
        let blocks = &mut changed.functions[0].body.as_mut().unwrap().blocks;
        match mutation {
            0 => {
                blocks[1].operations.remove(0);
            }
            1 => {
                blocks[2].operations.clear();
            }
            2 => {
                blocks[1].operations[1].kind = Kind::Store {
                    pointer: ValueId(0),
                    value: ValueId(20),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                };
            }
            3 => {
                blocks[1].terminator = Some(Terminator::Return { values: vec![] });
            }
            4 => {
                blocks[1].operations[0].results[0].id = ValueId(99);
            }
            5 => {
                blocks[1].operations[0].kind = Kind::Binary {
                    op: BinaryOp::Divide,
                    lhs: ValueId(3),
                    rhs: ValueId(1),
                };
            }
            6 => {
                blocks[1].operations[1].kind = Kind::Store {
                    pointer: ValueId(0),
                    value: ValueId(10),
                    access: MemoryAccess::new(AddressSpace::Global, 8),
                };
            }
            _ => unreachable!(),
        }
        assert!(
            preserve(&input, &changed, candidate(&rows), &replacements).is_err(),
            "mutation {mutation}"
        );
    }
    let mut duplicate = rows.clone();
    duplicate.push(rows[0]);
    assert!(preserve(&input, &output, candidate(&duplicate), &replacements).is_err());
    let mut missing = rows.clone();
    missing.pop();
    assert!(preserve(&input, &output, candidate(&missing), &replacements).is_err());
    let mut moved = rows.clone();
    moved[2].output.block.block = 0;
    assert!(preserve(&input, &output, candidate(&moved), &replacements).is_err());
}
