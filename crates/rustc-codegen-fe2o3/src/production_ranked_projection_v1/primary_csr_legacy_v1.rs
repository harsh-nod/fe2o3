// Frozen pre-conversion methods; test oracle, never production admission.
use super::*;

pub(super) struct LegacyQueriesV1<'a> {
    function: &'a SemanticFunctionDeclV1,
    graph: ProjectedLoopCfgV1,
    block_definitions: Vec<Vec<usize>>,
    dominance: HashMap<(usize, usize), bool>,
    work: usize,
}
impl<'a> LegacyQueriesV1<'a> {
    pub(super) fn new(
        function: &'a SemanticFunctionDeclV1,
    ) -> Result<Self, ProductionRankedProjectionErrorV1> {
        Ok(Self {
            function,
            graph: projected_loop_cfg_graph_v1(function)?,
            block_definitions: assertion_definition_inventory(function)?.blocks,
            dominance: HashMap::new(),
            work: 0,
        })
    }
    fn charge(&mut self, amount: usize) -> Result<(), ProductionRankedProjectionErrorV1> {
        project_loop_graph_charge_v1(&mut self.work, amount)
    }

    pub(super) fn blocks_reaching(
        &mut self,
        use_block: usize,
    ) -> Result<Vec<bool>, ProductionRankedProjectionErrorV1> {
        let block_count = self.function.blocks().len();
        if use_block >= block_count {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "an assertion proof reachability query is outside the semantic CFG",
            ));
        }
        let mut can_reach_use = Vec::new();
        can_reach_use.try_reserve_exact(block_count).map_err(|_| {
            ProductionRankedProjectionErrorV1::Unsupported(
                "assertion proof reverse-reachability storage cannot be reserved",
            )
        })?;
        can_reach_use.resize(block_count, false);
        let mut reverse = VecDeque::new();
        reverse.try_reserve(block_count).map_err(|_| {
            ProductionRankedProjectionErrorV1::Unsupported(
                "assertion proof reverse-reachability worklist cannot be reserved",
            )
        })?;
        reverse.push_back(use_block);
        can_reach_use[use_block] = true;
        while let Some(block) = reverse.pop_front() {
            self.charge(1)?;
            self.charge(self.graph.predecessors[block].len())?;
            for predecessor in &self.graph.predecessors[block] {
                if !can_reach_use[*predecessor] {
                    can_reach_use[*predecessor] = true;
                    reverse.push_back(*predecessor);
                }
            }
        }
        Ok(can_reach_use)
    }

    pub(super) fn local_is_stable_from_revalidating_edge_to_use(
        &mut self,
        local: usize,
        guard_source: usize,
        guard_target: usize,
        use_block: usize,
        can_reach_use: &[bool],
        visited: &mut [usize],
        pending: &mut VecDeque<usize>,
        generation: usize,
    ) -> Result<bool, ProductionRankedProjectionErrorV1> {
        let block_count = self.function.blocks().len();
        if local >= self.function.locals().len()
            || guard_source >= block_count
            || guard_target >= block_count
            || use_block >= block_count
            || guard_source == guard_target
            || can_reach_use.len() != block_count
            || visited.len() != block_count
        {
            return Ok(false);
        }

        let reverse_generation =
            generation
                .checked_mul(2)
                .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                    "assertion proof revalidating-edge generation overflowed",
                ))?;
        let forward_generation = reverse_generation.checked_add(1).ok_or(
            ProductionRankedProjectionErrorV1::Unsupported(
                "assertion proof revalidating-edge generation overflowed",
            ),
        )?;

        // Mark exactly the blocks that can reach the use without first
        // returning to the guard. This excludes latch-side definitions whose
        // only route back to the use crosses a fresh successful guard edge.
        pending.clear();
        pending.push_back(use_block);
        visited[use_block] = reverse_generation;
        while let Some(block) = pending.pop_front() {
            self.charge(1)?;
            self.charge(self.graph.predecessors[block].len())?;
            for predecessor in &self.graph.predecessors[block] {
                if *predecessor == guard_source
                    || !can_reach_use[*predecessor]
                    || visited[*predecessor] == reverse_generation
                    || visited[*predecessor] == forward_generation
                {
                    continue;
                }
                visited[*predecessor] = reverse_generation;
                pending.push_back(*predecessor);
            }
        }
        if visited[guard_target] != reverse_generation {
            return Ok(true);
        }

        pending.clear();
        pending.push_back(guard_target);
        visited[guard_target] = forward_generation;
        while let Some(block) = pending.pop_front() {
            self.charge(1)?;
            if self.block_defines_local(block, local) {
                return Ok(false);
            }
            if block == use_block {
                continue;
            }
            self.charge(self.graph.successors[block].len())?;
            for successor in &self.graph.successors[block] {
                if visited[*successor] == reverse_generation {
                    visited[*successor] = forward_generation;
                    pending.push_back(*successor);
                }
            }
        }
        Ok(true)
    }

    pub(super) fn block_dominates(
        &mut self,
        dominator: usize,
        block: usize,
    ) -> Result<bool, ProductionRankedProjectionErrorV1> {
        if dominator >= self.graph.successors.len() || block >= self.graph.successors.len() {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "an assertion proof dominance query is outside the semantic CFG",
            ));
        }
        if let Some(result) = self.dominance.get(&(dominator, block)).copied() {
            return Ok(result);
        }
        if dominator == block {
            let result = self.graph.reachable[block];
            insert_assertion_proof_cache(&mut self.dominance, (dominator, block), result)?;
            return Ok(result);
        }
        if !self.graph.reachable[dominator] || !self.graph.reachable[block] {
            insert_assertion_proof_cache(&mut self.dominance, (dominator, block), false)?;
            return Ok(false);
        }
        let node_count = self.graph.successors.len();
        let mut visited = Vec::new();
        visited.try_reserve_exact(node_count).map_err(|_| {
            ProductionRankedProjectionErrorV1::Unsupported(
                "assertion proof dominance storage cannot be reserved",
            )
        })?;
        visited.resize(node_count, false);
        let mut pending = Vec::new();
        pending.try_reserve(node_count).map_err(|_| {
            ProductionRankedProjectionErrorV1::Unsupported(
                "assertion proof dominance worklist cannot be reserved",
            )
        })?;
        if self.graph.entry != dominator {
            visited[self.graph.entry] = true;
            pending.push(self.graph.entry);
        }
        while let Some(current) = pending.pop() {
            self.charge(1)?;
            if current == block {
                insert_assertion_proof_cache(&mut self.dominance, (dominator, block), false)?;
                return Ok(false);
            }
            self.charge(self.graph.successors[current].len())?;
            for successor in &self.graph.successors[current] {
                if *successor != dominator && !visited[*successor] {
                    visited[*successor] = true;
                    pending.push(*successor);
                }
            }
        }
        insert_assertion_proof_cache(&mut self.dominance, (dominator, block), true)?;
        Ok(true)
    }

    pub(super) fn edge_set_dominates(
        &mut self,
        excluding_edges: &HashSet<(usize, usize)>,
        block: usize,
    ) -> Result<bool, ProductionRankedProjectionErrorV1> {
        if block >= self.graph.successors.len() {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "an assertion proof edge-set dominance query is outside the semantic CFG",
            ));
        }
        if excluding_edges.is_empty() || !self.graph.reachable[block] {
            return Ok(false);
        }
        let node_count = self.graph.successors.len();
        let mut visited = Vec::new();
        visited.try_reserve_exact(node_count).map_err(|_| {
            ProductionRankedProjectionErrorV1::Unsupported(
                "assertion proof edge-dominance storage cannot be reserved",
            )
        })?;
        visited.resize(node_count, false);
        let mut pending = Vec::new();
        pending.try_reserve(node_count).map_err(|_| {
            ProductionRankedProjectionErrorV1::Unsupported(
                "assertion proof edge-dominance worklist cannot be reserved",
            )
        })?;
        visited[self.graph.entry] = true;
        pending.push(self.graph.entry);
        while let Some(current) = pending.pop() {
            self.charge(1)?;
            if current == block {
                return Ok(false);
            }
            self.charge(self.graph.successors[current].len())?;
            for successor in &self.graph.successors[current] {
                if !excluding_edges.contains(&(current, *successor)) && !visited[*successor] {
                    visited[*successor] = true;
                    pending.push(*successor);
                }
            }
        }
        Ok(true)
    }

    pub(super) fn block_defines_local(&self, block: usize, local: usize) -> bool {
        self.block_definitions
            .get(block)
            .is_some_and(|definitions| definitions.binary_search(&local).is_ok())
    }
}
