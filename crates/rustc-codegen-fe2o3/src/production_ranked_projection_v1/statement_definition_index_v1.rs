// Sparse positions only. No expression, proof-result, or cross-function cache.
struct StatementDefinitionIndexV1 {
    rows: Vec<(usize, usize, usize)>,
}

impl StatementDefinitionIndexV1 {
    fn new(function: &SemanticFunctionDeclV1) -> Result<Self, ProductionRankedProjectionErrorV1> {
        let mut count = Some(0_usize);
        for statement in function
            .blocks()
            .iter()
            .flat_map(|block| block.statements())
        {
            visit_statement_definition_places(statement.kind(), &mut |place| {
                if local_definition_index(place).is_some() {
                    count = count.and_then(|count| count.checked_add(1));
                }
            });
        }
        let count = count.ok_or(ProductionRankedProjectionErrorV1::Unsupported(
            "GPU semantic definition-index length overflowed",
        ))?;
        let mut rows = Vec::new();
        rows.try_reserve_exact(count).map_err(|_| {
            ProductionRankedProjectionErrorV1::Unsupported(
                "GPU semantic definition-index storage cannot be reserved",
            )
        })?;
        for (block, body) in function.blocks().iter().enumerate() {
            for (statement, value) in body.statements().iter().enumerate() {
                visit_statement_definition_places(value.kind(), &mut |place| {
                    if let Some(local) = local_definition_index(place) {
                        rows.push((local, block, statement));
                    }
                });
            }
        }
        rows.sort_unstable();
        Ok(Self { rows })
    }

    fn before(&self, local: usize, block: usize, statement: usize) -> Option<usize> {
        let end = self
            .rows
            .partition_point(|row| *row < (local, block, statement));
        let &(found_local, found_block, found_statement) = self.rows.get(end.checked_sub(1)?)?;
        (found_local == local && found_block == block).then_some(found_statement)
    }
}

// Match repeated charge(1), including the exact state at its first failure.
// The existing counter models legacy logical visits, not measured CPU instructions.
fn charge_statement_scan_equivalent_v1(
    work: &mut usize,
    visits: usize,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    if visits == 0 {
        return Ok(());
    }
    let first_failure = MAX_PROJECTED_LOOP_GRAPH_WORK_V1
        .saturating_sub(*work)
        .saturating_add(1);
    project_loop_graph_charge_v1(work, visits.min(first_failure))
}
