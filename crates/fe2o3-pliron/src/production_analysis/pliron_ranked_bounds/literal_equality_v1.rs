// Rank-one fallback only: an actual x<C edge can prove x<n when every path
// into that edge's source already establishes n==C. No equation is exported.
impl BoundsEdgeTransportV1<'_> {
    fn proves_equal_literal(
        &self,
        block: usize,
        fact: LessThanFact,
        budget: &mut RankedBoundsBudget,
    ) -> Result<bool, RankedBoundsFindingV1> {
        budget.work(1)?;
        if block >= self.blocks.len()
            || self.predecessors.len() != self.blocks.len()
            || !matches!(fact.rhs, IndexExpr::Constant(_))
        {
            return Err(RankedBoundsFindingV1::StructuralVerificationFailed);
        }
        budget.storage(3)?;
        let mut visited = Vec::new();
        bounds_transport_enqueue_v1(
            &mut visited,
            BoundsTransportObligationV1 { block, fact },
            budget,
        )?;
        let mut cursor = 0;
        while cursor < visited.len() {
            budget.work(BOUNDS_TRANSPORT_ROW_ITEMS_V1 + 2)?;
            let obligation = visited[cursor];
            cursor += 1;
            let LessThanFact { lhs, rhs } = obligation.fact;
            if lhs == rhs {
                continue;
            }
            if self.defined_in_block(lhs, obligation.block, budget)?
                || self.defined_in_block(rhs, obligation.block, budget)?
                || obligation.block == 0
                || self.predecessors[obligation.block].is_empty()
            {
                return Ok(false);
            }
            for edge in &self.predecessors[obligation.block] {
                budget.work(1)?;
                let (fact, guard) = self.pull_back_relation(obligation, edge, true, budget)?;
                budget.work(BOUNDS_TRANSPORT_ROW_ITEMS_V1)?;
                if fact.lhs == fact.rhs
                    || guard.is_some_and(|guard| {
                        guard == fact || (guard.lhs == fact.rhs && guard.rhs == fact.lhs)
                    })
                {
                    continue;
                }
                bounds_transport_enqueue_v1(
                    &mut visited,
                    BoundsTransportObligationV1 {
                        block: edge.block,
                        fact,
                    },
                    budget,
                )?;
            }
        }
        // All incoming edges must discharge or preserve the same obligation.
        // A cycle cannot bypass its entry or a fresh definition boundary.
        Ok(true)
    }
}

fn bounds_literal_equality_resource_bound_v1(
    census: ProductionAnalysisInputCensusV1,
) -> (usize, usize) {
    if census.ranked_accesses == 0 || census.successors == 0 {
        return (0, 0);
    }
    // Nested equality queries share the existing transactional runtime meter.
    // Reserve its accepted prefix and largest denied charge rather than an
    // overflowing product of theoretical worklist cardinalities. This also
    // covers ordinary CFGs without any block arguments or forwarding edges.
    (
        MAX_RANKED_BOUNDS_WORK_UNITS + BOUNDS_TRANSPORT_DENIED_WORK_V1,
        MAX_RANKED_BOUNDS_STORAGE_ITEMS,
    )
}
