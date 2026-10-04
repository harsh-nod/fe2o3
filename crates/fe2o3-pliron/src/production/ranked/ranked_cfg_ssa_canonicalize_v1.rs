//! Checked normalization of equivalent ranked CFG and block-argument spellings.
//!
//! Rewrites are deliberately position preserving. They cannot renumber blocks,
//! move operations, or change edge operands because those coordinates are part
//! of the retained MIR-to-ranked correspondence.

use super::{
    ProductionRankedKernelV1, ProductionRankedTerminatorV1,
    ranked_index_constant_fold_v1::ProductionRankedTranslationErrorV1,
};

fn run_canonicalizer_v1(kernel: &mut ProductionRankedKernelV1) {
    for block in &mut kernel.blocks {
        let replacement = match &block.terminator {
            ProductionRankedTerminatorV1::IndexLessThanArgs {
                lhs,
                rhs,
                true_arguments,
                false_arguments,
                true_block,
                false_block,
            } if true_arguments.is_empty() && false_arguments.is_empty() => {
                Some(ProductionRankedTerminatorV1::IndexLessThan {
                    lhs: *lhs,
                    rhs: *rhs,
                    true_block: *true_block,
                    false_block: *false_block,
                })
            }
            ProductionRankedTerminatorV1::IndexEqualArgs {
                lhs,
                rhs,
                true_arguments,
                false_arguments,
                true_block,
                false_block,
            } if true_arguments.is_empty() && false_arguments.is_empty() => {
                Some(ProductionRankedTerminatorV1::IndexEqual {
                    lhs: *lhs,
                    rhs: *rhs,
                    true_block: *true_block,
                    false_block: *false_block,
                })
            }
            ProductionRankedTerminatorV1::AnalysisSplitArgs {
                control_dependencies,
                first_arguments,
                second_arguments,
                first_block,
                second_block,
            } if first_arguments.is_empty() && second_arguments.is_empty() => {
                Some(ProductionRankedTerminatorV1::AnalysisSplit {
                    control_dependencies: control_dependencies.clone(),
                    first_block: *first_block,
                    second_block: *second_block,
                })
            }
            ProductionRankedTerminatorV1::BranchArgs { arguments, target }
                if arguments.is_empty() =>
            {
                Some(ProductionRankedTerminatorV1::Branch { target: *target })
            }
            ProductionRankedTerminatorV1::BranchArgsAddAt {
                arguments,
                add_argument: 0,
                step,
                target,
            } if arguments.len() == 1 => Some(ProductionRankedTerminatorV1::BranchArgsAdd {
                value: arguments[0],
                step: *step,
                target: *target,
            }),
            _ => None,
        };
        if let Some(replacement) = replacement {
            block.terminator = replacement;
        }
    }
}

fn is_candidate_v1(terminator: &ProductionRankedTerminatorV1) -> bool {
    match terminator {
        ProductionRankedTerminatorV1::IndexLessThanArgs {
            true_arguments,
            false_arguments,
            ..
        }
        | ProductionRankedTerminatorV1::IndexEqualArgs {
            true_arguments,
            false_arguments,
            ..
        } => true_arguments.is_empty() && false_arguments.is_empty(),
        ProductionRankedTerminatorV1::AnalysisSplitArgs {
            first_arguments,
            second_arguments,
            ..
        } => first_arguments.is_empty() && second_arguments.is_empty(),
        ProductionRankedTerminatorV1::BranchArgs { arguments, .. } => arguments.is_empty(),
        ProductionRankedTerminatorV1::BranchArgsAddAt {
            arguments,
            add_argument,
            ..
        } => arguments.len() == 1 && *add_argument == 0,
        _ => false,
    }
}

fn replay_canonicalization_v1(
    before: &ProductionRankedKernelV1,
    after: &ProductionRankedKernelV1,
) -> Result<usize, ProductionRankedTranslationErrorV1> {
    if before.function_name != after.function_name {
        return Err(ProductionRankedTranslationErrorV1::FunctionIdentityChanged);
    }
    if before.argument_count != after.argument_count {
        return Err(ProductionRankedTranslationErrorV1::FunctionSignatureChanged);
    }
    if before.blocks.len() != after.blocks.len() {
        return Err(ProductionRankedTranslationErrorV1::BlockStructureChanged {
            block: before.blocks.len().min(after.blocks.len()),
            component: "block count",
        });
    }
    if before.tree_work != after.tree_work {
        return Err(ProductionRankedTranslationErrorV1::TreeWorkChanged);
    }

    let mut rewrites = 0;
    for (block_index, (before_block, after_block)) in
        before.blocks.iter().zip(&after.blocks).enumerate()
    {
        if before_block.index_argument_count != after_block.index_argument_count
            || before_block.operations != after_block.operations
        {
            return Err(
                ProductionRankedTranslationErrorV1::IllegalControlFlowCanonicalization {
                    block: block_index,
                },
            );
        }
        if before_block.terminator == after_block.terminator {
            if is_candidate_v1(&after_block.terminator) {
                return Err(
                    ProductionRankedTranslationErrorV1::MissedControlFlowCanonicalization {
                        block: block_index,
                    },
                );
            }
            continue;
        }

        let legal = match (&before_block.terminator, &after_block.terminator) {
            (
                ProductionRankedTerminatorV1::IndexLessThanArgs {
                    lhs,
                    rhs,
                    true_arguments,
                    false_arguments,
                    true_block,
                    false_block,
                },
                ProductionRankedTerminatorV1::IndexLessThan {
                    lhs: after_lhs,
                    rhs: after_rhs,
                    true_block: after_true,
                    false_block: after_false,
                },
            ) => {
                true_arguments.is_empty()
                    && false_arguments.is_empty()
                    && lhs == after_lhs
                    && rhs == after_rhs
                    && true_block == after_true
                    && false_block == after_false
            }
            (
                ProductionRankedTerminatorV1::IndexEqualArgs {
                    lhs,
                    rhs,
                    true_arguments,
                    false_arguments,
                    true_block,
                    false_block,
                },
                ProductionRankedTerminatorV1::IndexEqual {
                    lhs: after_lhs,
                    rhs: after_rhs,
                    true_block: after_true,
                    false_block: after_false,
                },
            ) => {
                true_arguments.is_empty()
                    && false_arguments.is_empty()
                    && lhs == after_lhs
                    && rhs == after_rhs
                    && true_block == after_true
                    && false_block == after_false
            }
            (
                ProductionRankedTerminatorV1::AnalysisSplitArgs {
                    control_dependencies,
                    first_arguments,
                    second_arguments,
                    first_block,
                    second_block,
                },
                ProductionRankedTerminatorV1::AnalysisSplit {
                    control_dependencies: after_dependencies,
                    first_block: after_first,
                    second_block: after_second,
                },
            ) => {
                first_arguments.is_empty()
                    && second_arguments.is_empty()
                    && control_dependencies == after_dependencies
                    && first_block == after_first
                    && second_block == after_second
            }
            (
                ProductionRankedTerminatorV1::BranchArgs { arguments, target },
                ProductionRankedTerminatorV1::Branch {
                    target: after_target,
                },
            ) => arguments.is_empty() && target == after_target,
            (
                ProductionRankedTerminatorV1::BranchArgsAddAt {
                    arguments,
                    add_argument,
                    step,
                    target,
                },
                ProductionRankedTerminatorV1::BranchArgsAdd {
                    value,
                    step: after_step,
                    target: after_target,
                },
            ) => {
                arguments.as_slice() == [*value]
                    && *add_argument == 0
                    && step == after_step
                    && target == after_target
            }
            _ => false,
        };
        if !legal {
            return Err(
                ProductionRankedTranslationErrorV1::IllegalControlFlowCanonicalization {
                    block: block_index,
                },
            );
        }
        rewrites += 1;
    }
    Ok(rewrites)
}

pub(super) fn canonicalize_and_validate_cfg_ssa_v1(
    before: ProductionRankedKernelV1,
) -> Result<ProductionRankedKernelV1, ProductionRankedTranslationErrorV1> {
    let mut after = before.clone();
    run_canonicalizer_v1(&mut after);
    replay_canonicalization_v1(&before, &after)?;
    Ok(after)
}

pub(super) fn require_canonical_cfg_ssa_v1(
    kernel: &ProductionRankedKernelV1,
) -> Result<(), ProductionRankedTranslationErrorV1> {
    if let Some(block) = kernel
        .blocks
        .iter()
        .position(|block| is_candidate_v1(&block.terminator))
    {
        Err(ProductionRankedTranslationErrorV1::MissedControlFlowCanonicalization { block })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::production::{
        ProductionRankedBlockV1, ProductionRankedOperationV1, ProductionRankedValueIdV1,
        ProductionRankedValueV1,
    };

    fn raw_kernel(blocks: Vec<ProductionRankedBlockV1>) -> ProductionRankedKernelV1 {
        let mut kernel = ProductionRankedKernelV1 {
            function_name: "cfg_ssa".to_owned(),
            argument_count: 0,
            blocks,
            tree_work: 0,
        };
        kernel.tree_work = kernel.validate().expect("valid raw recipe");
        kernel
    }

    #[test]
    fn canonicalizes_empty_edge_arguments_without_moving_operations() {
        let before = raw_kernel(vec![
            ProductionRankedBlockV1::new(
                Vec::new(),
                ProductionRankedTerminatorV1::BranchArgs {
                    arguments: Vec::new(),
                    target: 1,
                },
            ),
            ProductionRankedBlockV1::new(Vec::new(), ProductionRankedTerminatorV1::Return),
        ]);
        let after = canonicalize_and_validate_cfg_ssa_v1(before.clone()).unwrap();
        assert!(matches!(
            after.blocks[0].terminator,
            ProductionRankedTerminatorV1::Branch { target: 1 }
        ));
        assert_eq!(after.tree_work, before.tree_work);
        require_canonical_cfg_ssa_v1(&after).unwrap();
    }

    #[test]
    fn canonicalizes_empty_conditional_and_analysis_edges() {
        let value = ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0));
        let operations = || {
            vec![ProductionRankedOperationV1::IndexConstant {
                result: ProductionRankedValueIdV1::new(0),
                value: 1,
            }]
        };
        let terminal_blocks = || {
            [
                ProductionRankedBlockV1::new(Vec::new(), ProductionRankedTerminatorV1::Return),
                ProductionRankedBlockV1::new(Vec::new(), ProductionRankedTerminatorV1::Return),
            ]
        };
        let cases = [
            ProductionRankedTerminatorV1::IndexLessThanArgs {
                lhs: value,
                rhs: value,
                true_arguments: Vec::new(),
                false_arguments: Vec::new(),
                true_block: 1,
                false_block: 2,
            },
            ProductionRankedTerminatorV1::IndexEqualArgs {
                lhs: value,
                rhs: value,
                true_arguments: Vec::new(),
                false_arguments: Vec::new(),
                true_block: 1,
                false_block: 2,
            },
            ProductionRankedTerminatorV1::AnalysisSplitArgs {
                control_dependencies: vec![value],
                first_arguments: Vec::new(),
                second_arguments: Vec::new(),
                first_block: 1,
                second_block: 2,
            },
        ];
        for terminator in cases {
            let [first, second] = terminal_blocks();
            let before = raw_kernel(vec![
                ProductionRankedBlockV1::new(operations(), terminator),
                first,
                second,
            ]);
            let after = canonicalize_and_validate_cfg_ssa_v1(before).unwrap();
            assert!(matches!(
                after.blocks[0].terminator,
                ProductionRankedTerminatorV1::IndexLessThan { .. }
                    | ProductionRankedTerminatorV1::IndexEqual { .. }
                    | ProductionRankedTerminatorV1::AnalysisSplit { .. }
            ));
            require_canonical_cfg_ssa_v1(&after).unwrap();
        }
    }

    #[test]
    fn canonicalizes_single_induction_update_spelling() {
        let value = ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0));
        let before = raw_kernel(vec![
            ProductionRankedBlockV1::new(
                vec![ProductionRankedOperationV1::IndexConstant {
                    result: ProductionRankedValueIdV1::new(0),
                    value: 1,
                }],
                ProductionRankedTerminatorV1::BranchArgsAddAt {
                    arguments: vec![value],
                    add_argument: 0,
                    step: value,
                    target: 1,
                },
            ),
            ProductionRankedBlockV1::with_index_arguments(
                1,
                Vec::new(),
                ProductionRankedTerminatorV1::Return,
            ),
        ]);
        let after = canonicalize_and_validate_cfg_ssa_v1(before).unwrap();
        assert!(matches!(
            after.blocks[0].terminator,
            ProductionRankedTerminatorV1::BranchArgsAdd { value: actual, step, target: 1 }
                if actual == value && step == value
        ));
    }

    #[test]
    fn replay_rejects_a_changed_successor() {
        let before = raw_kernel(vec![
            ProductionRankedBlockV1::new(
                Vec::new(),
                ProductionRankedTerminatorV1::BranchArgs {
                    arguments: Vec::new(),
                    target: 1,
                },
            ),
            ProductionRankedBlockV1::new(Vec::new(), ProductionRankedTerminatorV1::Return),
            ProductionRankedBlockV1::new(Vec::new(), ProductionRankedTerminatorV1::Return),
        ]);
        let mut after = before.clone();
        run_canonicalizer_v1(&mut after);
        after.blocks[0].terminator = ProductionRankedTerminatorV1::Branch { target: 2 };
        assert_eq!(
            replay_canonicalization_v1(&before, &after),
            Err(
                ProductionRankedTranslationErrorV1::IllegalControlFlowCanonicalization { block: 0 }
            )
        );
    }
}
