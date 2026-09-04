use super::{
    ProductionRankedBlockV1, ProductionRankedOperationV1, ProductionRankedProjectionErrorV1,
    ProductionRankedTerminatorV1, forward_live_inductions, live_induction_block_arguments,
    projected_target, push_block_at, push_block_at_with_index_arguments, ranked_block_id,
};

pub(super) fn append_analysis_multi_split_blocks(
    blocks: &mut Vec<ProductionRankedBlockV1>,
    first_block: usize,
    first_operations: Vec<ProductionRankedOperationV1>,
    targets: &[usize],
    base_blocks: &[Option<usize>],
) -> Result<(), ProductionRankedProjectionErrorV1> {
    if targets.len() < 3 {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "an analysis multi-split has fewer than three successors",
        ));
    }
    let mut first_operations = Some(first_operations);
    for index in 0..targets.len() - 1 {
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
        push_block_at(
            blocks,
            block,
            first_operations.take().unwrap_or_default(),
            ProductionRankedTerminatorV1::AnalysisSplit {
                control_dependencies: Vec::new(),
                first_block: ranked_block_id(projected_target(base_blocks, targets[index])?)?,
                second_block: ranked_block_id(second_block)?,
            },
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
    let mut first_operations = Some(first_operations);
    for index in 0..targets.len() - 1 {
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
                forward_live_inductions(block_id, live, second_live)?,
                projected_target(base_blocks, second)?,
            )
        } else {
            (
                live_induction_block_arguments(block_id, live)?,
                block
                    .checked_add(1)
                    .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                        "analysis switch CFG block count overflow",
                    ))?,
            )
        };
        push_block_at_with_index_arguments(
            blocks,
            block,
            argument_count,
            first_operations.take().unwrap_or_default(),
            ProductionRankedTerminatorV1::AnalysisSplitArgs {
                control_dependencies: Vec::new(),
                first_arguments: forward_live_inductions(block_id, live, first_live)?,
                second_arguments,
                first_block: ranked_block_id(projected_target(base_blocks, targets[index])?)?,
                second_block: ranked_block_id(second_block)?,
            },
        )?;
    }
    Ok(())
}
