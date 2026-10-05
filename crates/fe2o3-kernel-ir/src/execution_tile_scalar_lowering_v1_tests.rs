use super::*;
use crate::{
    BasicBlock, BlockId, Function, Kernel, LaunchDomain, LaunchExtent, Module, Signature,
    Terminator, WorkgroupSize,
};
use std::collections::BTreeMap;

fn recipe(
    layout: ExecutionTileLayoutV1,
    lanes: u16,
    elements: u16,
) -> ExecutionTileScalarLoweringV1 {
    ExecutionTileScalarLoweringV1::new(
        ExecutionTileScheduleV1::new(layout, lanes, elements).unwrap(),
        ValueId(0),
        ValueId(1),
        ValueId(10),
    )
    .unwrap()
}

fn emit(lowering: ExecutionTileScalarLoweringV1) -> Vec<Operation> {
    (0..lowering.operation_count())
        .map(|i| lowering.emit_operation(i).unwrap())
        .collect()
}

fn module(lowering: ExecutionTileScalarLoweringV1) -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = emit(lowering);
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("explicit-tile-scalars");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(
            vec![
                Type::slice(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    AccessMode::ReadOnly,
                ),
                Type::INDEX,
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    ));
    let mut kernel = Kernel::new(
        "tile",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(
        u32::from(lowering.schedule.lanes()),
        1,
        1,
    ));
    module.kernels.push(kernel);
    module
}

/// Executes the emitted scalar operations, not the scheduling recipe. Memory
/// reads are separately logged to detect speculative reads and lost components.
fn interpret(
    operations: &[Operation],
    lane: u64,
    base: u64,
    length: u64,
) -> (BTreeMap<ValueId, u64>, Vec<u64>) {
    let mut values = BTreeMap::from([(ValueId(0), 0), (ValueId(1), base)]);
    let mut reads = Vec::new();
    for operation in operations {
        let get = |id: &ValueId| values[id];
        let mut overflow = None;
        let value = match &operation.kind {
            OperationKind::Intrinsic(_) => lane,
            OperationKind::Constant(Constant::Index(value)) => *value,
            OperationKind::Constant(Constant::U32(value)) => u64::from(*value),
            OperationKind::SliceLength { .. } => length,
            OperationKind::SliceData { .. } => 0,
            OperationKind::Binary { op, lhs, rhs } => match op {
                BinaryOp::Multiply => get(lhs).checked_mul(get(rhs)).unwrap(),
                BinaryOp::Add => get(lhs).checked_add(get(rhs)).unwrap(),
                BinaryOp::BitAnd => get(lhs) & get(rhs),
                BinaryOp::Checked(CheckedBinaryOperator::Add) => {
                    let (value, flag) = get(lhs).overflowing_add(get(rhs));
                    overflow = Some(u64::from(flag));
                    value
                }
                _ => panic!("unexpected binary operation"),
            },
            OperationKind::Unary {
                op: UnaryOp::Not,
                operand,
            } => u64::from(get(operand) == 0),
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs,
                rhs,
            } => u64::from(get(lhs) < get(rhs)),
            OperationKind::Select {
                condition,
                true_value,
                false_value,
            } => {
                if get(condition) != 0 {
                    get(true_value)
                } else {
                    get(false_value)
                }
            }
            OperationKind::GetElementPointer { base, offset } => {
                get(base).checked_add(get(offset)).unwrap()
            }
            OperationKind::GuardedLoad {
                pointer,
                predicate,
                fallback,
                ..
            } => {
                if get(predicate) != 0 {
                    let index = get(pointer);
                    assert!(index < length);
                    reads.push(index);
                    u64::from((index as u32).wrapping_mul(17).wrapping_add(29))
                } else {
                    get(fallback)
                }
            }
            _ => panic!("unexpected emitted operation"),
        };
        assert!(values.insert(operation.results[0].id, value).is_none());
        if let Some(flag) = overflow {
            assert!(values.insert(operation.results[1].id, flag).is_none());
        }
    }
    (values, reads)
}

#[test]
fn explicit_tile_scalar_lowering_verifies_as_ordinary_kir_for_both_layouts() {
    for layout in [
        ExecutionTileLayoutV1::Blocked,
        ExecutionTileLayoutV1::Striped,
    ] {
        for (lanes, elements) in [(1, 1), (3, 5), (64, 3), (256, 125)] {
            let lowering = recipe(layout, lanes, elements);
            let operations = emit(lowering);
            lowering.check_replacement(&operations).unwrap();
            crate::verify_module_ref(&module(lowering)).unwrap();
            assert_eq!(
                operations.iter().map(|op| op.results.len()).sum::<usize>(),
                5 + 12 * usize::from(elements)
            );
            assert_eq!(
                operations
                    .iter()
                    .filter(|op| matches!(op.kind, OperationKind::GuardedLoad { .. }))
                    .count(),
                usize::from(elements)
            );
            assert_eq!(lowering.emit_operation(lowering.operation_count()), None);
            assert_eq!(lowering.component(elements), None);
        }
    }
}

#[test]
fn explicit_tile_scalar_lowering_executes_wide_reference_and_exact_read_trace() {
    for layout in [
        ExecutionTileLayoutV1::Blocked,
        ExecutionTileLayoutV1::Striped,
    ] {
        for (lanes, elements) in [(1, 1), (3, 5), (64, 3), (256, 125)] {
            let lowering = recipe(layout, lanes, elements);
            let operations = emit(lowering);
            for lane in [0, lanes / 2, lanes - 1] {
                for base in [0, 7, u64::MAX - 2, u64::MAX] {
                    for length in [0, 1, 13, 32_001, u64::MAX] {
                        let (values, reads) = interpret(&operations, u64::from(lane), base, length);
                        let mut expected_reads = Vec::new();
                        for element in 0..elements {
                            let offset = match layout {
                                ExecutionTileLayoutV1::Blocked => {
                                    u128::from(lane) * u128::from(elements) + u128::from(element)
                                }
                                ExecutionTileLayoutV1::Striped => {
                                    u128::from(element) * u128::from(lanes) + u128::from(lane)
                                }
                            };
                            let address = u128::from(base) + offset;
                            let active =
                                address < u128::from(length) && address <= u128::from(u64::MAX);
                            let (value, mask) = lowering.component(element).unwrap();
                            assert_eq!(values[&mask], u64::from(active));
                            assert_eq!(
                                values[&value],
                                if active {
                                    u64::from((address as u32).wrapping_mul(17).wrapping_add(29))
                                } else {
                                    0
                                }
                            );
                            if active {
                                expected_reads.push(address as u64);
                            }
                        }
                        assert_eq!(reads, expected_reads);
                    }
                }
            }
        }
    }
}

#[test]
fn explicit_tile_scalar_lowering_checker_refuses_each_changed_definition_and_operation() {
    for layout in [
        ExecutionTileLayoutV1::Blocked,
        ExecutionTileLayoutV1::Striped,
    ] {
        let lowering = recipe(layout, 64, 3);
        let operations = emit(lowering);
        for position in 0..operations.len() {
            let mut changed = operations[position].clone();
            changed.results[0].id = ValueId(9999);
            assert!(lowering.check_operation(position, &changed).is_err());
            changed = operations[position].clone();
            changed.results[0].ty = Type::Unit;
            assert!(lowering.check_operation(position, &changed).is_err());
            changed = operations[position].clone();
            changed.kind = OperationKind::Constant(Constant::Bool(false));
            assert!(lowering.check_operation(position, &changed).is_err());
            changed = operations[position].clone();
            changed
                .results
                .push(ValueDef::new(ValueId(9999), Type::BOOL));
            assert!(lowering.check_operation(position, &changed).is_err());
        }
        assert!(
            lowering
                .check_replacement(&operations[..operations.len() - 1])
                .is_err()
        );
        let mut extra = operations.clone();
        extra.push(operations[0].clone());
        assert!(lowering.check_replacement(&extra).is_err());
        let mut reordered = operations.clone();
        reordered.swap(5, 16);
        assert!(lowering.check_replacement(&reordered).is_err());
        let other = match layout {
            ExecutionTileLayoutV1::Blocked => ExecutionTileLayoutV1::Striped,
            ExecutionTileLayoutV1::Striped => ExecutionTileLayoutV1::Blocked,
        };
        assert!(recipe(other, 64, 3).check_replacement(&operations).is_err());
    }
}

#[test]
fn explicit_tile_scalar_lowering_refuses_id_overflow_aliases_and_guard_weakening() {
    let schedule = ExecutionTileScheduleV1::new(ExecutionTileLayoutV1::Blocked, 64, 3).unwrap();
    for (input, base, first) in [
        (0, 1, u32::MAX),
        (0, 1, 0),
        (20, 1, 10),
        (0, 20, 10),
        (1, 1, 10),
    ] {
        assert!(
            ExecutionTileScalarLoweringV1::new(
                schedule,
                ValueId(input),
                ValueId(base),
                ValueId(first)
            )
            .is_err()
        );
    }
    let lowering = recipe(ExecutionTileLayoutV1::Blocked, 64, 3);
    let mut operations = emit(lowering);
    let last = operations.len() - 1;
    let OperationKind::GuardedLoad { predicate, .. } = &mut operations[last].kind else {
        panic!()
    };
    *predicate = ValueId(17);
    assert!(lowering.check_replacement(&operations).is_err());
    operations = emit(lowering);
    operations[9].results[1].ty = Type::INDEX;
    assert!(lowering.check_replacement(&operations).is_err());
}
