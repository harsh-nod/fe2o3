//! Capacity bound for the existing per-frame value-transfer buffer.
use fe2o3_kernel_ir::{Module, OperationKind, Terminator};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct IncomingValueCapacityV1(pub(super) usize);

/// Count list positions, not distinct SSA IDs. All admitted reachable functions
/// are included because frame reset and mem::take retain and transfer buffers.
/// resolve_values_into clears then try_reserve_exact(list.len()); it never appends
/// another list without clearing. The maximum therefore bounds retained capacity.
pub(crate) fn incoming_value_capacity(
    module: &Module,
    reachable: &[usize],
) -> Option<IncomingValueCapacityV1> {
    let mut maximum = 0;
    for &index in reachable {
        let function = module.functions.get(index)?;
        let Some(body) = &function.body else { continue };
        for block in &body.blocks {
            for operation in &block.operations {
                if let OperationKind::Call { arguments, .. } = &operation.kind {
                    maximum = maximum.max(arguments.len());
                }
            }
            match block.terminator.as_ref() {
                Some(Terminator::Return { values }) => maximum = maximum.max(values.len()),
                Some(Terminator::Branch { arguments, .. }) => {
                    maximum = maximum.max(arguments.len())
                }
                Some(Terminator::ConditionalBranch {
                    then_arguments,
                    else_arguments,
                    ..
                }) => {
                    maximum = maximum.max(then_arguments.len()).max(else_arguments.len());
                }
                Some(Terminator::Switch {
                    cases,
                    default_arguments,
                    ..
                }) => {
                    maximum = maximum.max(default_arguments.len());
                    for case in cases {
                        maximum = maximum.max(case.arguments.len());
                    }
                }
                Some(Terminator::IntegerSwitch {
                    cases,
                    default_arguments,
                    ..
                }) => {
                    maximum = maximum.max(default_arguments.len());
                    for case in cases {
                        maximum = maximum.max(case.arguments.len());
                    }
                }
                Some(Terminator::Unreachable) | None => {}
            }
        }
    }
    Some(IncomingValueCapacityV1(maximum))
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::{
        BasicBlock, BlockId, Constant, Function, IntegerSwitchCase, Operation, Signature,
        SwitchCase, ValueId,
    };
    fn ids(n: usize) -> Vec<ValueId> {
        vec![ValueId(0); n]
    }
    fn graph(operations: Vec<Operation>, terminator: Terminator) -> Module {
        let mut block = BasicBlock::new(BlockId(0));
        block.operations = operations;
        block.terminator = Some(terminator);
        let mut module = Module::new("incoming-count");
        module.functions.push(Function::kernel_entry(
            "entry",
            Signature::new(vec![], vec![]),
            vec![],
            vec![block],
        ));
        module
    }
    fn count(module: &Module, reachable: &[usize]) -> usize {
        incoming_value_capacity(module, reachable).unwrap().0
    }
    #[test]
    fn repeated_call_and_return_positions_are_not_deduplicated() {
        for (call, returned) in [(0, 0), (1, 7), (19, 3), (3, 19)] {
            let module = graph(
                vec![Operation::new(
                    vec![],
                    OperationKind::Call {
                        callee: "callee".into(),
                        arguments: ids(call),
                    },
                )],
                Terminator::Return {
                    values: ids(returned),
                },
            );
            assert_eq!(count(&module, &[0]), call.max(returned));
        }
    }
    #[test]
    fn both_conditional_arms_and_plain_branch_are_counted() {
        for (left, right) in [(1, 13), (17, 2), (0, 0)] {
            let module = graph(
                vec![],
                Terminator::ConditionalBranch {
                    condition: ValueId(0),
                    then_target: BlockId(1),
                    then_arguments: ids(left),
                    else_target: BlockId(2),
                    else_arguments: ids(right),
                },
            );
            assert_eq!(count(&module, &[0]), left.max(right));
        }
        assert_eq!(
            count(
                &graph(
                    vec![],
                    Terminator::Branch {
                        target: BlockId(1),
                        arguments: ids(23)
                    }
                ),
                &[0]
            ),
            23
        );
    }
    #[test]
    fn every_switch_case_and_default_list_contributes() {
        for (first, second, default) in [(31, 2, 3), (1, 37, 3), (1, 2, 41)] {
            let module = graph(
                vec![],
                Terminator::Switch {
                    selector: ValueId(0),
                    cases: vec![
                        SwitchCase {
                            value: 0,
                            target: BlockId(1),
                            arguments: ids(first),
                        },
                        SwitchCase {
                            value: 1,
                            target: BlockId(2),
                            arguments: ids(second),
                        },
                    ],
                    default_target: BlockId(3),
                    default_arguments: ids(default),
                },
            );
            assert_eq!(count(&module, &[0]), first.max(second).max(default));
            let integer = graph(
                vec![],
                Terminator::IntegerSwitch {
                    selector: ValueId(0),
                    cases: vec![
                        IntegerSwitchCase {
                            value: Constant::U32(0),
                            target: BlockId(1),
                            arguments: ids(first),
                        },
                        IntegerSwitchCase {
                            value: Constant::U32(1),
                            target: BlockId(2),
                            arguments: ids(second),
                        },
                    ],
                    default_target: BlockId(3),
                    default_arguments: ids(default),
                },
            );
            assert_eq!(count(&integer, &[0]), first.max(second).max(default));
        }
    }
    #[test]
    fn unreachable_functions_do_not_inflate_but_bad_reachable_indices_refuse() {
        let mut module = graph(vec![], Terminator::Return { values: vec![] });
        let mut unused = graph(vec![], Terminator::Return { values: ids(257) });
        module.functions.push(unused.functions.remove(0));
        assert_eq!(count(&module, &[0]), 0);
        assert_eq!(count(&module, &[0, 1]), 257);
        assert_eq!(count(&module, &[]), 0);
        assert_eq!(incoming_value_capacity(&module, &[2]), None);
        assert_eq!(count(&graph(vec![], Terminator::Unreachable), &[0]), 0);
    }
}
