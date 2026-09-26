use super::{
    ProductionRankedBlockV1, ProductionRankedOperationV1, ProductionRankedProjectionErrorV1,
    ProductionRankedTerminatorV1, forward_live_inductions_with_allocation_v18,
    live_induction_block_arguments_with_allocation_v18, projected_target,
    push_block_at_core_v18, ranked_block_id,
};
use super::source_ranked_consumer_resources_v18::ProjectionAllocationV18;

pub(super) fn append_analysis_multi_split_blocks(
    blocks: &mut Vec<ProductionRankedBlockV1>,
    first_block: usize,
    first_operations: Vec<ProductionRankedOperationV1>,
    targets: &[usize],
    base_blocks: &[Option<usize>],
) -> Result<(), ProductionRankedProjectionErrorV1> {
    append_analysis_multi_split_blocks_core_v18(blocks,first_block,first_operations,targets,base_blocks,
        &mut ProjectionAllocationV18::Legacy)
}

pub(super) fn append_analysis_multi_split_blocks_core_v18(
    blocks:&mut Vec<ProductionRankedBlockV1>,
    first_block:usize,
    first_operations:Vec<ProductionRankedOperationV1>,
    targets:&[usize],
    base_blocks:&[Option<usize>],
    allocation:&mut ProjectionAllocationV18<'_>,
) -> Result<(),ProductionRankedProjectionErrorV1> {
    if targets.len() < 3 {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "an analysis multi-split has fewer than three successors",
        ));
    }
    allocation.header::<Option<Vec<ProductionRankedOperationV1>>>()?;
    let mut first_operations = Some(first_operations);
    for index in 0..targets.len() - 1 {
        allocation.charge(1)?;
        let block = first_block.checked_add(index).ok_or(
            ProductionRankedProjectionErrorV1::Unsupported(
                "analysis switch CFG block count overflow",
            ),
        )?;
        let second_block = if index + 2 == targets.len() {
            projected_target(base_blocks, targets[index + 1])?
        } else {
            block
                .checked_add(1)
                .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                    "analysis switch CFG block count overflow",
                ))?
        };
        let operations=match first_operations.take() {Some(operations)=>operations,None=>allocation.empty()?};
        push_block_at_core_v18(
            blocks,
            block,
            None,
            operations,
            ProductionRankedTerminatorV1::AnalysisSplit {
                control_dependencies: allocation.empty()?,
                first_block: ranked_block_id(projected_target(base_blocks, targets[index])?)?,
                second_block: ranked_block_id(second_block)?,
            },
            allocation,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn append_analysis_multi_split_blocks_with_arguments(
    blocks: &mut Vec<ProductionRankedBlockV1>,
    first_block: usize,
    first_operations: Vec<ProductionRankedOperationV1>,
    targets: &[usize],
    base_blocks: &[Option<usize>],
    live: &[usize],
    live_inductions: &[Vec<usize>],
) -> Result<(), ProductionRankedProjectionErrorV1> {
    append_analysis_multi_split_blocks_with_arguments_core_v18(blocks,first_block,first_operations,targets,base_blocks,
        live,live_inductions,&mut ProjectionAllocationV18::Legacy)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn append_analysis_multi_split_blocks_with_arguments_core_v18(
    blocks:&mut Vec<ProductionRankedBlockV1>,
    first_block:usize,
    first_operations:Vec<ProductionRankedOperationV1>,
    targets:&[usize],
    base_blocks:&[Option<usize>],
    live:&[usize],
    live_inductions:&[Vec<usize>],
    allocation:&mut ProjectionAllocationV18<'_>,
) -> Result<(),ProductionRankedProjectionErrorV1> {
    if targets.len() < 3 {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "an analysis multi-split has fewer than three successors",
        ));
    }
    let argument_count = u32::try_from(live.len()).map_err(|_| {
        ProductionRankedProjectionErrorV1::Unsupported(
            "live induction argument count does not fit u32",
        )
    })?;
    allocation.header::<Option<Vec<ProductionRankedOperationV1>>>()?;
    let mut first_operations = Some(first_operations);
    for index in 0..targets.len() - 1 {
        allocation.charge(1)?;
        let block = first_block.checked_add(index).ok_or(
            ProductionRankedProjectionErrorV1::Unsupported(
                "analysis switch CFG block count overflow",
            ),
        )?;
        let block_id = ranked_block_id(block)?;
        let first_live = live_inductions.get(targets[index]).ok_or(
            ProductionRankedProjectionErrorV1::Unsupported(
                "an analysis switch successor is outside the live induction table",
            ),
        )?;
        let (second_arguments, second_block) = if index + 2 == targets.len() {
            let second = targets[index + 1];
            let second_live = live_inductions.get(second).ok_or(
                ProductionRankedProjectionErrorV1::Unsupported(
                    "an analysis switch successor is outside the live induction table",
                ),
            )?;
            (
                forward_live_inductions_with_allocation_v18(block_id, live, second_live,allocation)?,
                projected_target(base_blocks, second)?,
            )
        } else {
            (
                live_induction_block_arguments_with_allocation_v18(block_id, live,allocation)?,
                block
                    .checked_add(1)
                    .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                        "analysis switch CFG block count overflow",
                    ))?,
            )
        };
        let operations=match first_operations.take() {Some(operations)=>operations,None=>allocation.empty()?};
        push_block_at_core_v18(
            blocks,
            block,
            Some(argument_count),
            operations,
            ProductionRankedTerminatorV1::AnalysisSplitArgs {
                control_dependencies: allocation.empty()?,
                first_arguments: forward_live_inductions_with_allocation_v18(block_id, live, first_live,allocation)?,
                second_arguments,
                first_block: ranked_block_id(projected_target(base_blocks, targets[index])?)?,
                second_block: ranked_block_id(second_block)?,
            },
            allocation,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::source_ranked_consumer_resources_v18::SourceAssertionMeterV18;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    use std::mem::size_of;

    #[test]
    fn source_multi_split_preserves_cfg_and_pays_exact_nested_storage() {
        let targets = [0, 1, 2];
        let base = [Some(2), Some(3), Some(4)];
        let live = [10, 20];
        let target_live = [vec![20], vec![10], vec![20, 10]];
        for with_arguments in [false, true] {
            let mut expected = Vec::with_capacity(2);
            if with_arguments {
                append_analysis_multi_split_blocks_with_arguments(
                    &mut expected, 0, Vec::new(), &targets, &base, &live, &target_live,
                ).unwrap();
            } else {
                append_analysis_multi_split_blocks(
                    &mut expected, 0, Vec::new(), &targets, &base,
                ).unwrap();
            }
            // Both edges in each generated split keep their complete argument
            // order; the caller-owned destination capacity is not charged twice.
            let work = if with_arguments { 18 } else { 4 };
            let storage = size_of::<Option<Vec<ProductionRankedOperationV1>>>()
                + size_of::<Vec<ProductionRankedOperationV1>>()
                + 2 * size_of::<Vec<super::super::ProductionRankedValueV1>>()
                + if with_arguments {
                    4 * size_of::<Vec<super::super::ProductionRankedValueV1>>()
                        + 6 * size_of::<super::super::ProductionRankedValueV1>()
                } else { 0 };
            for (work_limit, storage_limit) in [
                (work, storage), (work - 1, storage), (work, storage - 1),
            ] {
                let mut ledger = Work::new(work_limit);
                let mut budget = Budget::new(&mut ledger, storage_limit);
                let mut actual = Vec::with_capacity(2);
                let mut meter = SourceAssertionMeterV18(&mut budget);
                let mut allocation = ProjectionAllocationV18::Source(&mut meter);
                let result = if with_arguments {
                    append_analysis_multi_split_blocks_with_arguments_core_v18(
                        &mut actual, 0, Vec::new(), &targets, &base, &live,
                        &target_live, &mut allocation,
                    )
                } else {
                    append_analysis_multi_split_blocks_core_v18(
                        &mut actual, 0, Vec::new(), &targets, &base, &mut allocation,
                    )
                };
                if (work_limit, storage_limit) == (work, storage) {
                    result.unwrap();
                    assert_eq!(actual, expected);
                    assert_eq!((budget.work(), budget.storage()), (work, storage));
                } else {
                    assert!(result.is_err());
                    if storage_limit < storage {
                        assert_eq!(budget.failed_storage(), Some(storage));
                    }
                }
            }
        }
    }

    #[test]
    fn invalid_multi_split_retains_first_legacy_refusal_without_source_work() {
        let mut ledger = Work::new(0);
        let mut budget = Budget::new(&mut ledger, 0);
        for with_arguments in [false, true] {
            let mut blocks = Vec::new();
            let mut meter = SourceAssertionMeterV18(&mut budget);
            let mut allocation = ProjectionAllocationV18::Source(&mut meter);
            let result = if with_arguments {
                append_analysis_multi_split_blocks_with_arguments_core_v18(
                    &mut blocks, 0, Vec::new(), &[0, 1], &[], &[], &[], &mut allocation,
                )
            } else {
                append_analysis_multi_split_blocks_core_v18(
                    &mut blocks, 0, Vec::new(), &[0, 1], &[], &mut allocation,
                )
            };
            assert!(matches!(result, Err(ProductionRankedProjectionErrorV1::Unsupported(
                "an analysis multi-split has fewer than three successors"
            ))));
            assert!(blocks.is_empty());
            assert_eq!((budget.work(), budget.storage(), budget.failed_storage()), (0, 0, None));
        }
    }
}
