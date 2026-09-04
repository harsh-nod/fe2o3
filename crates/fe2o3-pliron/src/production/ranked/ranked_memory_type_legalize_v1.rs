//! Checked legalization of implicit ranked-memory typing.
//!
//! The legacy `View` recipe already materializes as a global-space ranked view.
//! This pass makes that default explicit before identities and proofs are derived.

use super::{
    ProductionRankedKernelV1, ProductionRankedOperationV1,
    ranked_index_constant_fold_v1::ProductionRankedTranslationErrorV1,
};
use dialect_kernel::MemorySpaceAttr;

fn run_legalizer_v1(kernel: &mut ProductionRankedKernelV1) {
    for block in &mut kernel.blocks {
        for operation in &mut block.operations {
            let replacement = match operation {
                ProductionRankedOperationV1::View {
                    result,
                    element_width,
                    writable,
                    shape,
                    dynamic_extents,
                    allocation_origin,
                    noalias_class,
                } => Some(ProductionRankedOperationV1::ViewInSpace {
                    result: *result,
                    element_width: *element_width,
                    writable: *writable,
                    shape: shape.clone(),
                    dynamic_extents: dynamic_extents.clone(),
                    memory_space: MemorySpaceAttr::Global,
                    allocation_origin: *allocation_origin,
                    noalias_class: *noalias_class,
                }),
                _ => None,
            };
            if let Some(replacement) = replacement {
                *operation = replacement;
            }
        }
    }
}

fn replay_legalization_v1(
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
            || before_block.terminator != after_block.terminator
            || before_block.operations.len() != after_block.operations.len()
        {
            return Err(
                ProductionRankedTranslationErrorV1::IllegalMemoryTypeLegalization {
                    block: block_index,
                    operation: 0,
                },
            );
        }
        for (operation_index, (before_operation, after_operation)) in before_block
            .operations
            .iter()
            .zip(&after_block.operations)
            .enumerate()
        {
            if before_operation == after_operation {
                if matches!(before_operation, ProductionRankedOperationV1::View { .. }) {
                    return Err(
                        ProductionRankedTranslationErrorV1::MissedMemoryTypeLegalization {
                            block: block_index,
                            operation: operation_index,
                        },
                    );
                }
                continue;
            }
            let legal = matches!(
                (before_operation, after_operation),
                (
                    ProductionRankedOperationV1::View {
                        result,
                        element_width,
                        writable,
                        shape,
                        dynamic_extents,
                        allocation_origin,
                        noalias_class,
                    },
                    ProductionRankedOperationV1::ViewInSpace {
                        result: after_result,
                        element_width: after_element_width,
                        writable: after_writable,
                        shape: after_shape,
                        dynamic_extents: after_dynamic_extents,
                        memory_space: MemorySpaceAttr::Global,
                        allocation_origin: after_origin,
                        noalias_class: after_noalias,
                    },
                ) if result == after_result
                    && element_width == after_element_width
                    && writable == after_writable
                    && shape == after_shape
                    && dynamic_extents == after_dynamic_extents
                    && allocation_origin == after_origin
                    && noalias_class == after_noalias
            );
            if !legal {
                return Err(
                    ProductionRankedTranslationErrorV1::IllegalMemoryTypeLegalization {
                        block: block_index,
                        operation: operation_index,
                    },
                );
            }
            rewrites += 1;
        }
    }
    Ok(rewrites)
}

pub(super) fn legalize_and_validate_memory_types_v1(
    before: ProductionRankedKernelV1,
) -> Result<ProductionRankedKernelV1, ProductionRankedTranslationErrorV1> {
    let mut after = before.clone();
    run_legalizer_v1(&mut after);
    replay_legalization_v1(&before, &after)?;
    Ok(after)
}

pub(super) fn require_explicit_memory_types_v1(
    kernel: &ProductionRankedKernelV1,
) -> Result<(), ProductionRankedTranslationErrorV1> {
    for (block, operation, item) in kernel.blocks.iter().enumerate().flat_map(|(block, item)| {
        item.operations
            .iter()
            .enumerate()
            .map(move |(operation, item)| (block, operation, item))
    }) {
        if matches!(item, ProductionRankedOperationV1::View { .. }) {
            return Err(
                ProductionRankedTranslationErrorV1::MissedMemoryTypeLegalization {
                    block,
                    operation,
                },
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::production::{ProductionRankedBlockV1, ProductionRankedTerminatorV1};

    fn implicit_view() -> ProductionRankedOperationV1 {
        ProductionRankedOperationV1::View {
            result: super::super::ProductionRankedValueIdV1::new(0),
            element_width: 32,
            writable: true,
            shape: vec![16],
            dynamic_extents: Vec::new(),
            allocation_origin: 7,
            noalias_class: 3,
        }
    }

    fn raw_kernel(operation: ProductionRankedOperationV1) -> ProductionRankedKernelV1 {
        let mut kernel = ProductionRankedKernelV1 {
            function_name: "memory_types".to_owned(),
            argument_count: 0,
            blocks: vec![ProductionRankedBlockV1::new(
                vec![operation],
                ProductionRankedTerminatorV1::Return,
            )],
            tree_work: 0,
        };
        kernel.tree_work = kernel.validate().expect("valid raw recipe");
        kernel
    }

    #[test]
    fn makes_the_global_memory_space_explicit_at_the_same_site() {
        let before = raw_kernel(implicit_view());
        let after = legalize_and_validate_memory_types_v1(before.clone()).unwrap();
        assert!(matches!(
            &after.blocks[0].operations[0],
            ProductionRankedOperationV1::ViewInSpace {
                result,
                element_width: 32,
                writable: true,
                shape,
                dynamic_extents,
                memory_space: MemorySpaceAttr::Global,
                allocation_origin: 7,
                noalias_class: 3,
            } if result.get() == 0 && shape == &[16] && dynamic_extents.is_empty()
        ));
        assert_eq!(after.tree_work, before.tree_work);
        require_explicit_memory_types_v1(&after).unwrap();
    }

    #[test]
    fn preserves_an_explicit_non_global_view() {
        let explicit = ProductionRankedOperationV1::ViewInSpace {
            result: super::super::ProductionRankedValueIdV1::new(0),
            element_width: 32,
            writable: true,
            shape: vec![16],
            dynamic_extents: Vec::new(),
            memory_space: MemorySpaceAttr::Workgroup,
            allocation_origin: 7,
            noalias_class: 3,
        };
        let before = raw_kernel(explicit);
        assert_eq!(
            legalize_and_validate_memory_types_v1(before.clone()).unwrap(),
            before
        );
    }

    #[test]
    fn replay_rejects_a_changed_memory_space() {
        let before = raw_kernel(implicit_view());
        let mut after = before.clone();
        run_legalizer_v1(&mut after);
        let ProductionRankedOperationV1::ViewInSpace { memory_space, .. } =
            &mut after.blocks[0].operations[0]
        else {
            panic!("legalized view");
        };
        *memory_space = MemorySpaceAttr::Private;
        assert_eq!(
            replay_legalization_v1(&before, &after),
            Err(
                ProductionRankedTranslationErrorV1::IllegalMemoryTypeLegalization {
                    block: 0,
                    operation: 0,
                }
            )
        );
    }
}
