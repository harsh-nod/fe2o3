// Ordinary scalar values share the reference path's source expression resolver.
// Reservations are created inside the final write block before source locations
// are recorded, and must all be replaced before ranked construction can escape.
fn reserve_projected_global_write_value_v1(
    operations: &mut Vec<ProductionRankedOperationV1>,
    operation: &mut ProductionRankedOperationV1,
    memory_space: MemorySpaceAttr,
    semantic_site: Option<ProjectedSemanticAccessSiteV1>,
    next_value: &mut u32,
    facts: &mut impl ProjectedAssertionFactsV1,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    if !facts.checked_control_enabled_v1()
        || memory_space != MemorySpaceAttr::Global
        || semantic_site.is_none_or(|site| site.statement.is_none())
    {
        return Ok(());
    }
    let ProductionRankedOperationV1::Access {
        kind: AccessKindAttr::Write,
        view,
        indices,
    } = operation
    else {
        return Ok(());
    };
    facts.charge_private_array_work(3)?;
    if operations.len() >= MAX_PROJECTED_OPERATIONS_V1 {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "ordinary write value reservation exceeds the ranked operation limit",
        ));
    }
    let value = next_value_id(next_value)?;
    operations.push(ProductionRankedOperationV1::SemanticConstant {
        result: value,
        value: 0,
    });
    *operation = ProductionRankedOperationV1::ValueAccess {
        kind: AccessKindAttr::Write,
        view: *view,
        indices: std::mem::take(indices),
        value: ProductionRankedValueV1::Local(value),
    };
    Ok(())
}

struct ProjectedWriteLoadScratchV1 {
    visited: [bool; MAX_RANKED_BOUNDS_BLOCKS],
    pending: [usize; MAX_RANKED_BOUNDS_BLOCKS],
    excluded_rows: [usize; MAX_RANKED_BOUNDS_BLOCKS],
    reachability: Vec<u64>,
    words: usize,
    expression_nodes: usize,
}

fn projected_write_source_at_v1<'a>(
    coordinate: (usize, usize),
    sources: &'a [ProjectedAccessSourceV1],
    facts: &mut impl ProjectedAssertionFactsV1,
) -> Result<Option<&'a ProjectedAccessSourceV1>, ProductionRankedProjectionErrorV1> {
    let mut lower = 0;
    let mut upper = sources.len();
    while lower < upper {
        facts.charge_private_array_work(4)?;
        let middle = lower + (upper - lower) / 2;
        match (sources[middle].block, sources[middle].operation).cmp(&coordinate) {
            std::cmp::Ordering::Less => lower = middle + 1,
            std::cmp::Ordering::Greater => upper = middle,
            std::cmp::Ordering::Equal => return Ok(Some(&sources[middle])),
        }
    }
    Ok(None)
}

impl ProjectedWriteLoadScratchV1 {
    fn new(
        facts: &mut impl ProjectedAssertionFactsV1,
    ) -> Result<Self, ProductionRankedProjectionErrorV1> {
        facts.charge_private_array_work(MAX_RANKED_BOUNDS_BLOCKS * 3)?;
        facts.reserve_checked_control_storage_v1(std::mem::size_of::<Self>())?;
        Ok(Self {
            visited: [false; MAX_RANKED_BOUNDS_BLOCKS],
            pending: [0; MAX_RANKED_BOUNDS_BLOCKS],
            excluded_rows: [usize::MAX; MAX_RANKED_BOUNDS_BLOCKS],
            reachability: Vec::new(),
            words: 0,
            expression_nodes: 0,
        })
    }

    fn prepare_reachability_v1(
        &mut self,
        blocks: &[ProductionRankedBlockV1],
        facts: &mut impl ProjectedAssertionFactsV1,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        if blocks.is_empty() || blocks.len() > self.visited.len() {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "ordinary write load has an invalid ranked control coordinate",
            ));
        }
        let mut rows = 1usize;
        for row in &mut self.excluded_rows[..blocks.len()] {
            facts.charge_private_array_work(2)?;
            if *row != usize::MAX {
                *row = rows;
                rows += 1;
            }
        }
        if rows == 1 {
            return Ok(());
        }
        self.words = blocks.len().div_ceil(64);
        let count =
            rows.checked_mul(self.words)
                .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                    "ordinary write reachability storage overflow",
                ))?;
        let bytes = count.checked_mul(std::mem::size_of::<u64>()).ok_or(
            ProductionRankedProjectionErrorV1::Unsupported(
                "ordinary write reachability storage overflow",
            ),
        )?;
        facts.charge_private_array_work(count + 4)?;
        facts.reserve_checked_control_storage_v1(bytes)?;
        self.reachability.try_reserve_exact(count).map_err(|_| {
            ProductionRankedProjectionErrorV1::Unsupported(
                "ordinary write reachability allocation failed",
            )
        })?;
        let actual = self
            .reachability
            .capacity()
            .checked_mul(std::mem::size_of::<u64>())
            .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                "ordinary write reachability capacity overflow",
            ))?;
        if actual > bytes {
            facts.reserve_checked_control_storage_v1(actual - bytes)?;
        }
        self.reachability.resize(count, 0);
        self.populate_reachability_v1(blocks, None, 0, facts)?;
        for excluded in 0..blocks.len() {
            facts.charge_private_array_work(1)?;
            let row = self.excluded_rows[excluded];
            if row != usize::MAX {
                self.populate_reachability_v1(blocks, Some(excluded), row, facts)?;
            }
        }
        Ok(())
    }

    fn populate_reachability_v1(
        &mut self,
        blocks: &[ProductionRankedBlockV1],
        excluded: Option<usize>,
        row: usize,
        facts: &mut impl ProjectedAssertionFactsV1,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        facts.charge_private_array_work(blocks.len())?;
        self.visited[..blocks.len()].fill(false);
        if excluded == Some(0) {
            return Ok(());
        }
        self.visited[0] = true;
        self.pending[0] = 0;
        let mut count = 1;
        while count != 0 {
            facts.charge_private_array_work(2)?;
            count -= 1;
            let block = self.pending[count];
            self.reachability[row * self.words + block / 64] |= 1 << (block % 64);
            for successor in projected_write_successors_v1(blocks[block].terminator())
                .into_iter()
                .flatten()
            {
                facts.charge_private_array_work(3)?;
                if successor >= blocks.len() {
                    return Err(ProductionRankedProjectionErrorV1::Incomplete(
                        "ordinary write load has an invalid ranked control edge",
                    ));
                }
                if Some(successor) != excluded && !self.visited[successor] {
                    self.visited[successor] = true;
                    self.pending[count] = successor;
                    count += 1;
                }
            }
        }
        Ok(())
    }

    fn check_load_v1(
        &mut self,
        load: &ProductionSemanticLoadV2,
        write: (usize, usize),
        blocks: &[ProductionRankedBlockV1],
        sources: &[ProjectedAccessSourceV1],
        facts: &mut impl ProjectedAssertionFactsV1,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        facts.charge_private_array_work(7)?;
        let coordinate = (load.block as usize, load.operation as usize);
        let source = projected_write_source_at_v1(coordinate, sources, facts)?;
        let exact_source = source.is_some_and(|source| {
            source.access == AccessKindAttr::Read
                && source.memory_space == MemorySpaceAttr::Global
                && source
                    .semantic_site
                    .is_some_and(|site| site.statement.is_some())
        });
        facts.charge_private_array_work(load.indices.len())?;
        let exact_read = matches!(
            blocks.get(coordinate.0).and_then(|block| block.operations().get(coordinate.1)),
            Some(ProductionRankedOperationV1::Access {
                kind: AccessKindAttr::Read,
                view,
                indices,
            }) if *view == load.view && indices.as_slice() == load.indices.as_ref()
        );
        if !exact_source || !exact_read {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "ordinary write expression load is not its exact source-derived ranked read",
            ));
        }
        let dominates = if coordinate.0 == write.0 {
            coordinate.1 < write.1
        } else {
            let row = self.excluded_rows[coordinate.0];
            let bit = 1u64 << (write.0 % 64);
            row != usize::MAX
                && self.words != 0
                && write.0 < blocks.len()
                && self.reachability[write.0 / 64] & bit != 0
                && self.reachability[row * self.words + write.0 / 64] & bit == 0
        };
        if !dominates {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "ordinary write expression load does not dominate its exact write-site value",
            ));
        }
        Ok(())
    }

    fn check_expression_v1(
        &mut self,
        expression: &ProductionSemanticExpressionV2,
        write: (usize, usize),
        blocks: &[ProductionRankedBlockV1],
        sources: &[ProjectedAccessSourceV1],
        (check, depth): (bool, usize),
        facts: &mut impl ProjectedAssertionFactsV1,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        facts.charge_private_array_work(1)?;
        if depth > fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "ordinary write load traversal exceeds the shared expression depth",
            ));
        }
        if !check {
            self.expression_nodes = self.expression_nodes.checked_add(1).ok_or(
                ProductionRankedProjectionErrorV1::Unsupported(
                    "ordinary write expression node count overflow",
                ),
            )?;
            if self.expression_nodes > fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2 {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "ordinary write expressions exceed the shared root-wide source node limit",
                ));
            }
        }
        match expression {
            ProductionSemanticExpressionV2::Load(load) => {
                if !check {
                    // Prepay the retained clone's index payload. Shared typed
                    // validation below limits each load to at most eight indices.
                    facts.charge_private_array_work(load.indices.len())?;
                }
                if check {
                    self.check_load_v1(load, write, blocks, sources, facts)?;
                } else if load.block as usize != write.0 {
                    let marker = self.excluded_rows.get_mut(load.block as usize).ok_or(
                        ProductionRankedProjectionErrorV1::Incomplete(
                            "ordinary write load block is outside ranked limits",
                        ),
                    )?;
                    if load.block as usize >= blocks.len() {
                        return Err(ProductionRankedProjectionErrorV1::Incomplete(
                            "ordinary write load block is absent",
                        ));
                    }
                    *marker = 0;
                }
            }
            ProductionSemanticExpressionV2::Symbol { .. }
            | ProductionSemanticExpressionV2::Constant { .. } => {}
            ProductionSemanticExpressionV2::Unary { operand, .. }
            | ProductionSemanticExpressionV2::Cast { operand, .. } => {
                self.check_expression_v1(
                    operand,
                    write,
                    blocks,
                    sources,
                    (check, depth + 1),
                    facts,
                )?;
            }
            ProductionSemanticExpressionV2::Binary { lhs, rhs, .. }
            | ProductionSemanticExpressionV2::Compare { lhs, rhs, .. } => {
                self.check_expression_v1(lhs, write, blocks, sources, (check, depth + 1), facts)?;
                self.check_expression_v1(rhs, write, blocks, sources, (check, depth + 1), facts)?;
            }
            ProductionSemanticExpressionV2::Select {
                condition,
                when_true,
                when_false,
                ..
            } => {
                self.check_expression_v1(
                    condition,
                    write,
                    blocks,
                    sources,
                    (check, depth + 1),
                    facts,
                )?;
                self.check_expression_v1(
                    when_true,
                    write,
                    blocks,
                    sources,
                    (check, depth + 1),
                    facts,
                )?;
                self.check_expression_v1(
                    when_false,
                    write,
                    blocks,
                    sources,
                    (check, depth + 1),
                    facts,
                )?;
            }
        }
        Ok(())
    }
}

fn projected_write_successors_v1(terminator: &ProductionRankedTerminatorV1) -> [Option<usize>; 2] {
    use ProductionRankedTerminatorV1 as T;
    match terminator {
        T::IndexLessThan {
            true_block,
            false_block,
            ..
        }
        | T::IndexLessThanArgs {
            true_block,
            false_block,
            ..
        }
        | T::IndexEqual {
            true_block,
            false_block,
            ..
        }
        | T::IndexEqualArgs {
            true_block,
            false_block,
            ..
        } => [Some(*true_block as usize), Some(*false_block as usize)],
        T::AnalysisSplit {
            first_block,
            second_block,
            ..
        }
        | T::AnalysisSplitArgs {
            first_block,
            second_block,
            ..
        } => [Some(*first_block as usize), Some(*second_block as usize)],
        T::Branch { target }
        | T::BranchArgs { target, .. }
        | T::BranchArgsAdd { target, .. }
        | T::BranchArgsAddAt { target, .. } => [Some(*target as usize), None],
        T::Return | T::Trap => [None, None],
    }
}

fn attach_projected_global_write_values_v1(
    blocks: &mut [ProductionRankedBlockV1],
    sources: &[ProjectedAccessSourceV1],
    writes: &[crate::production_reference_effect_join_v2::RankedGpuWriteV2],
    facts: &mut impl ProjectedAssertionFactsV1,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    if !facts.checked_control_enabled_v1() {
        return Ok(());
    }
    for adjacent in sources.windows(2) {
        facts.charge_private_array_work(2)?;
        if (adjacent[0].block, adjacent[0].operation) >= (adjacent[1].block, adjacent[1].operation)
        {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "ordinary write attachment requires unique ordered source coordinates",
            ));
        }
    }
    for adjacent in writes.windows(2) {
        facts.charge_private_array_work(2)?;
        if (adjacent[0].block, adjacent[0].operation) >= (adjacent[1].block, adjacent[1].operation)
        {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "ordinary write attachment requires unique ordered write coordinates",
            ));
        }
    }
    let mut reservations = 0usize;
    for (block_index, block) in blocks.iter().enumerate() {
        for (index, operation) in block.operations().iter().enumerate() {
            facts.charge_private_array_work(2)?;
            let ProductionRankedOperationV1::ValueAccess { value, .. } = operation else {
                continue;
            };
            facts.charge_private_array_work(5)?;
            let source = projected_write_source_at_v1((block_index, index), sources, facts)?;
            if !matches!(
                operation,
                ProductionRankedOperationV1::ValueAccess {
                    kind: AccessKindAttr::Write,
                    ..
                }
            ) || !source.is_some_and(|source| {
                source.access == AccessKindAttr::Write
                    && source.memory_space == MemorySpaceAttr::Global
                    && source
                        .semantic_site
                        .is_some_and(|site| site.statement.is_some())
            }) {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "ordinary write reservation lacks its exact source-owned Global write",
                ));
            }
            let previous = index
                .checked_sub(1)
                .and_then(|index| block.operations().get(index));
            if !matches!(previous, Some(ProductionRankedOperationV1::SemanticConstant { result, value: 0 })
                if *value == ProductionRankedValueV1::Local(*result))
            {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "ordinary write reservation is not the exact adjacent closed placeholder",
                ));
            }
            reservations += 1;
        }
    }
    if reservations == 0 {
        return Ok(());
    }
    let mut scratch = ProjectedWriteLoadScratchV1::new(facts)?;
    for check in [false, true] {
        let mut targets = 0;
        for write in writes {
            facts.charge_private_array_work(3)?;
            let Some(ProductionRankedOperationV1::ValueAccess { .. }) = blocks
                .get(write.block)
                .and_then(|block| block.operations().get(write.operation))
            else {
                continue;
            };
            targets += 1;
            let expression = write
                .value
                .as_ref()
                .map_err(|detail| ProductionRankedProjectionErrorV1::Unsupported(detail))?;
            let expression_operation = write.operation.checked_sub(1).ok_or(
                ProductionRankedProjectionErrorV1::Incomplete(
                    "ordinary write reservation is absent",
                ),
            )?;
            let before = scratch.expression_nodes;
            scratch.check_expression_v1(
                expression,
                (write.block, expression_operation),
                blocks,
                sources,
                (check, 0),
                facts,
            )?;
            if !check {
                // The paid structural walk bounds depth and root-wide nodes
                // before validation. Shared validate() walks both validity and
                // statistics; also prepay the retained expression clone and
                // exact numerical-contract traversal once per node.
                let nodes = scratch.expression_nodes - before;
                facts.charge_private_array_work(nodes * 4 + 1)?;
                expression.validate().map_err(|_| {
                    ProductionRankedProjectionErrorV1::Incomplete(
                        "ordinary write expression exceeds the shared typed-expression contract",
                    )
                })?;
            }
        }
        if targets != reservations {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "ordinary write reservation has no complete source-derived value attachment",
            ));
        }
        if !check {
            scratch.prepare_reachability_v1(blocks, facts)?;
        }
    }
    let mut cursor = 0;
    for (block_index, block) in blocks.iter_mut().enumerate() {
        facts.charge_private_array_work(1)?;
        let start = cursor;
        while cursor < writes.len() && writes[cursor].block == block_index {
            cursor += 1;
        }
        if start == cursor {
            continue;
        }
        let operations = block.operations_mut();
        for write in &writes[start..cursor] {
            facts.charge_private_array_work(6)?;
            let Some(ProductionRankedOperationV1::ValueAccess {
                kind: AccessKindAttr::Write,
                view,
                indices,
                value: ProductionRankedValueV1::Local(value),
            }) = operations.get(write.operation)
            else {
                continue;
            };
            facts.charge_private_array_work(indices.len())?;
            if *view != write.view || *indices != write.indices {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "ordinary write attachment changed its exact view or coordinates",
                ));
            }
            let value = *value;
            let previous = write.operation.checked_sub(1).ok_or(
                ProductionRankedProjectionErrorV1::Incomplete(
                    "ordinary write reservation is absent",
                ),
            )?;
            if !matches!(operations.get(previous), Some(ProductionRankedOperationV1::SemanticConstant {
                result, value: 0,
            }) if *result == value)
            {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "ordinary write reservation is not the exact adjacent closed placeholder",
                ));
            }
            let expression = write
                .value
                .as_ref()
                .map_err(|detail| ProductionRankedProjectionErrorV1::Unsupported(detail))?;
            operations[previous] = ProductionRankedOperationV1::SemanticExpression {
                result: value,
                // Retained projector-owned tree: source extraction keeps the
                // independent copy for reference joining. Its shared root cap
                // is 8192 nodes, depth 128 and eight indices per load; this is
                // not owned by the temporary canonical scratch receipt.
                expression: expression.clone(),
                numerical_contract:
                    fe2o3_pliron::ProductionNumericalContractV2::exact_for_expression(expression),
            };
        }
    }
    if cursor != writes.len() {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "ordinary write attachment has a stale block coordinate",
        ));
    }
    Ok(())
}

#[cfg(test)]
include!("ordinary_write_values_v1_tests.rs");
