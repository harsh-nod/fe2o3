//! Lossless normal-edge CSR and allocation-free leaf queries; not source admission.

use super::*;

type Error = ProductionRankedProjectionErrorV1;

/// Borrows an existing counter; a caller's lower ceiling is never enlarged.
pub(super) struct CsrWorkV1<'a> {
    used: &'a mut usize,
    ceiling: usize,
}

impl<'a> CsrWorkV1<'a> {
    pub(super) fn new(used: &'a mut usize, ceiling: usize) -> Self {
        Self {
            used,
            ceiling: ceiling.min(MAX_PROJECTED_LOOP_GRAPH_WORK_V1),
        }
    }

    fn charge(&mut self, amount: usize) -> Result<(), Error> {
        project_loop_graph_charge_v1(self.used, amount)?;
        if *self.used > self.ceiling {
            return Err(Error::Unsupported(
                "CSR analysis exceeds its inherited work limit",
            ));
        }
        Ok(())
    }
}

fn table(entries: usize) -> Result<Vec<usize>, Error> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(entries)
        .map_err(|_| Error::Unsupported("CSR table cannot be reserved"))?;
    result.resize(entries, 0);
    Ok(result)
}

fn storage_items(nodes: usize, edges: usize) -> Result<usize, Error> {
    let entries = nodes
        .checked_mul(6)
        .and_then(|n| edges.checked_mul(2).and_then(|e| n.checked_add(e)))
        .and_then(|n| n.checked_add(2))
        .ok_or(Error::Unsupported("CSR storage count overflow"))?;
    if entries > MAX_PROJECTED_CAPABILITY_STATE_ENTRIES_V1 {
        return Err(Error::Unsupported(
            "CSR storage exceeds the existing state entry limit",
        ));
    }
    Ok(entries)
}

struct WorkspaceV1 {
    seen: Vec<usize>,
    region: Vec<usize>,
    pending: Vec<usize>,
    generation: usize,
}

impl WorkspaceV1 {
    fn begin(&mut self, work: &mut CsrWorkV1<'_>) -> Result<usize, Error> {
        work.charge(1)?;
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or(Error::Unsupported("CSR query generation overflow"))?;
        self.pending.clear();
        Ok(self.generation)
    }

    fn collect(
        &mut self,
        source: &SemanticFunctionDeclV1,
        block: usize,
        work: &mut CsrWorkV1<'_>,
    ) -> Result<(), Error> {
        let generation = self.begin(work)?;
        let mut visit = |target: SemanticBlockIdV1, retain: bool| {
            work.charge(1)?;
            let target = target.index() as usize;
            if target >= source.blocks().len() {
                return Err(Error::Unsupported(
                    "a semantic CFG edge outside the function during loop analysis",
                ));
            }
            if retain && self.seen[target] != generation {
                self.seen[target] = generation;
                self.pending.push(target);
            }
            Ok(())
        };
        match source.blocks()[block].terminator().kind() {
            SemanticTerminatorKindV1::Goto(edge) => visit(edge.target(), true)?,
            SemanticTerminatorKindV1::SwitchInt { targets, .. } => {
                for target in targets.values() {
                    visit(target.edge().target(), true)?;
                }
                let otherwise = targets.otherwise().target();
                let omit = targets.values().len() == 2
                    && targets.values().iter().any(|target| target.value() == 0)
                    && targets.values().iter().any(|target| target.value() == 1)
                    && switch_fallback_is_empty_unreachable_v1(source, otherwise.index() as usize);
                visit(otherwise, !omit)?;
            }
            SemanticTerminatorKindV1::Call(call) => {
                if let Some(destination) = call.destination() {
                    visit(destination.edge().target(), true)?;
                }
            }
            SemanticTerminatorKindV1::Assert { target, .. }
            | SemanticTerminatorKindV1::Drop { target, .. } => visit(target.target(), true)?,
            SemanticTerminatorKindV1::FalseEdge { .. } => {
                return Err(Error::Incomplete(
                    "a false edge before uniform induction CFG normalization",
                ));
            }
            SemanticTerminatorKindV1::Return
            | SemanticTerminatorKindV1::TailCall(_)
            | SemanticTerminatorKindV1::UnwindResume
            | SemanticTerminatorKindV1::UnwindTerminate
            | SemanticTerminatorKindV1::Abort
            | SemanticTerminatorKindV1::Unreachable => {}
        }
        Ok(())
    }
}

/// Every CSR row is still the original source block index, including unreachable rows.
pub(super) struct LosslessCsrV1<'body> {
    source: &'body SemanticFunctionDeclV1,
    successor_offsets: Vec<usize>,
    successors: Vec<usize>,
    predecessor_offsets: Vec<usize>,
    predecessors: Vec<usize>,
    reachable: Vec<usize>,
    workspace: WorkspaceV1,
}

impl<'body> LosslessCsrV1<'body> {
    pub(super) fn build(
        source: &'body SemanticFunctionDeclV1,
        mut work: CsrWorkV1<'_>,
    ) -> Result<Self, Error> {
        let nodes = source.blocks().len();
        if nodes == 0 {
            return Err(Error::Unsupported(
                "semantic CFG exceeds the ranked block limit before loop analysis",
            ));
        }
        if nodes > MAX_RANKED_BOUNDS_BLOCKS {
            return Err(Error::SourceCfgLimit(
                source_cfg_diagnostic_v1::SourceCfgLimitV1::capture(source),
            ));
        }
        let entry = source.entry().index() as usize;
        if entry >= nodes {
            return Err(Error::Unsupported(
                "semantic entry block outside the function during loop analysis",
            ));
        }
        work.charge(storage_items(nodes, 0)?)?;
        let mut workspace = WorkspaceV1 {
            seen: table(nodes)?,
            region: table(nodes)?,
            pending: table(nodes)?,
            generation: 0,
        };
        let mut successor_offsets = table(nodes + 1)?;
        let mut predecessor_offsets = table(nodes + 1)?;
        let mut reachable = table(nodes)?;
        for block in 0..nodes {
            workspace.collect(source, block, &mut work)?;
            let end = successor_offsets[block]
                .checked_add(workspace.pending.len())
                .ok_or(Error::Unsupported(
                    "semantic CFG edge count overflow during loop analysis",
                ))?;
            if end > MAX_RANKED_BOUNDS_EDGES {
                return Err(Error::Unsupported(
                    "semantic CFG exceeds the ranked edge limit before loop analysis",
                ));
            }
            successor_offsets[block + 1] = end;
            work.charge(workspace.pending.len())?;
            for &target in &workspace.pending {
                predecessor_offsets[target + 1] += 1;
            }
        }
        work.charge(nodes)?;
        for block in 0..nodes {
            predecessor_offsets[block + 1] += predecessor_offsets[block];
        }
        let edges = successor_offsets[nodes];
        storage_items(nodes, edges)?;
        work.charge(
            edges
                .checked_mul(2)
                .ok_or(Error::Unsupported("CSR edge storage overflow"))?,
        )?;
        let mut successors = table(edges)?;
        let mut predecessors = table(edges)?;
        work.charge(nodes)?;
        workspace
            .region
            .copy_from_slice(&predecessor_offsets[..nodes]);
        for block in 0..nodes {
            workspace.collect(source, block, &mut work)?;
            let count = workspace.pending.len();
            // Same logical sort debit as semantic SSA's charge_sort_work.
            let passes = if count < 2 {
                0
            } else {
                usize::BITS as usize - (count - 1).leading_zeros() as usize
            };
            work.charge(
                count
                    .checked_mul(passes)
                    .ok_or(Error::Unsupported("CSR sort work overflow"))?,
            )?;
            workspace.pending.sort_unstable();
            work.charge(
                count
                    .checked_mul(2)
                    .ok_or(Error::Unsupported("CSR emission work overflow"))?,
            )?;
            successors[successor_offsets[block]..successor_offsets[block + 1]]
                .copy_from_slice(&workspace.pending);
            for &target in &workspace.pending {
                predecessors[workspace.region[target]] = block;
                workspace.region[target] += 1;
            }
        }
        // Cursor values are not generation tags. Reset before reusing this table.
        work.charge(nodes)?;
        workspace.region.fill(0);
        workspace.pending.clear();
        workspace.pending.push(entry);
        reachable[entry] = 1;
        while let Some(block) = workspace.pending.pop() {
            let targets = &successors[successor_offsets[block]..successor_offsets[block + 1]];
            work.charge(1 + targets.len())?;
            for &target in targets {
                if reachable[target] == 0 {
                    reachable[target] = 1;
                    workspace.pending.push(target);
                }
            }
        }
        Ok(Self {
            source,
            successor_offsets,
            successors,
            predecessor_offsets,
            predecessors,
            reachable,
            workspace,
        })
    }

    pub(super) fn source(&self) -> &'body SemanticFunctionDeclV1 {
        self.source
    }
    pub(super) fn successors(&self, block: usize) -> Option<&[usize]> {
        Some(
            &self.successors
                [*self.successor_offsets.get(block)?..*self.successor_offsets.get(block + 1)?],
        )
    }
    pub(super) fn predecessors(&self, block: usize) -> Option<&[usize]> {
        Some(
            &self.predecessors[*self.predecessor_offsets.get(block)?
                ..*self.predecessor_offsets.get(block + 1)?],
        )
    }
    pub(super) fn is_entry_reachable(&self, block: usize) -> bool {
        self.reachable.get(block).is_some_and(|&v| v != 0)
    }
    pub(super) fn storage_items(&self) -> usize {
        6 * self.reachable.len() + 2 * self.successors.len() + 2
    }

    /// Scratch borrows cannot escape this closure or coexist with a nested query.
    pub(super) fn query<R>(
        &mut self,
        work: CsrWorkV1<'_>,
        query: impl for<'q> FnOnce(CsrQueryV1<'q, 'body>) -> Result<R, Error>,
    ) -> Result<R, Error> {
        query(CsrQueryV1 {
            source: self.source,
            successor_offsets: &self.successor_offsets,
            successors: &self.successors,
            predecessor_offsets: &self.predecessor_offsets,
            predecessors: &self.predecessors,
            reachable: &self.reachable,
            workspace: &mut self.workspace,
            work,
        })
    }

    #[cfg(test)]
    pub(super) fn workspace_identity(&self) -> [(*const usize, usize); 3] {
        [
            &self.workspace.seen,
            &self.workspace.region,
            &self.workspace.pending,
        ]
        .map(|v| (v.as_ptr(), v.capacity()))
    }
}

pub(super) struct CsrQueryV1<'q, 'body> {
    source: &'body SemanticFunctionDeclV1,
    successor_offsets: &'q [usize],
    successors: &'q [usize],
    predecessor_offsets: &'q [usize],
    predecessors: &'q [usize],
    reachable: &'q [usize],
    workspace: &'q mut WorkspaceV1,
    work: CsrWorkV1<'q>,
}

impl CsrQueryV1<'_, '_> {
    fn check(&mut self, blocks: &[usize]) -> Result<(), Error> {
        self.work.charge(blocks.len())?;
        if blocks.iter().any(|&block| block >= self.reachable.len()) {
            return Err(Error::Unsupported(
                "CSR query is outside the retained source CFG",
            ));
        }
        Ok(())
    }

    fn entry_reaches(
        &mut self,
        target: usize,
        omitted: Option<usize>,
        omit_edge: impl Fn(usize, usize) -> bool,
    ) -> Result<bool, Error> {
        let entry = self.source.entry().index() as usize;
        if omitted == Some(entry) {
            return Ok(false);
        }
        let generation = self.workspace.begin(&mut self.work)?;
        self.workspace.seen[entry] = generation;
        self.workspace.pending.push(entry);
        while let Some(block) = self.workspace.pending.pop() {
            self.work.charge(1)?;
            if block == target {
                return Ok(true);
            }
            let successors =
                &self.successors[self.successor_offsets[block]..self.successor_offsets[block + 1]];
            self.work.charge(successors.len())?;
            for &next in successors {
                if omitted != Some(next)
                    && !omit_edge(block, next)
                    && self.workspace.seen[next] != generation
                {
                    self.workspace.seen[next] = generation;
                    self.workspace.pending.push(next);
                }
            }
        }
        Ok(false)
    }

    pub(super) fn block_dominates(
        &mut self,
        dominator: usize,
        target: usize,
    ) -> Result<bool, Error> {
        self.check(&[dominator, target])?;
        Ok(self.reachable[dominator] != 0
            && self.reachable[target] != 0
            && !self.entry_reaches(target, Some(dominator), |_, _| false)?)
    }

    pub(super) fn edge_set_dominates(
        &mut self,
        edges: &HashSet<(usize, usize)>,
        target: usize,
    ) -> Result<bool, Error> {
        self.check(&[target])?;
        Ok(!edges.is_empty()
            && self.reachable[target] != 0
            && !self.entry_reaches(target, None, |a, b| edges.contains(&(a, b)))?)
    }

    fn mark_reaching(&mut self, target: usize, omitted: Option<usize>) -> Result<usize, Error> {
        let generation = self.workspace.begin(&mut self.work)?;
        self.workspace.region[target] = generation;
        self.workspace.pending.push(target);
        while let Some(block) = self.workspace.pending.pop() {
            let predecessors = &self.predecessors
                [self.predecessor_offsets[block]..self.predecessor_offsets[block + 1]];
            self.work.charge(1 + predecessors.len())?;
            for &next in predecessors {
                if omitted != Some(next) && self.workspace.region[next] != generation {
                    self.workspace.region[next] = generation;
                    self.workspace.pending.push(next);
                }
            }
        }
        Ok(generation)
    }

    pub(super) fn reaches(&mut self, source: usize, target: usize) -> Result<bool, Error> {
        self.check(&[source, target])?;
        let generation = self.mark_reaching(target, None)?;
        Ok(self.workspace.region[source] == generation)
    }

    /// CFG freshness only: the caller must still authenticate the comparison/value origin.
    pub(super) fn guarded_local_stable(
        &mut self,
        local: usize,
        guard: (usize, usize),
        target: usize,
    ) -> Result<bool, Error> {
        self.check(&[guard.0, guard.1, target])?;
        if local >= self.source.locals().len() || guard.0 == guard.1 {
            return Ok(false);
        }
        let row =
            &self.successors[self.successor_offsets[guard.0]..self.successor_offsets[guard.0 + 1]];
        self.work.charge(row.len())?;
        if !row.contains(&guard.1)
            || self.reachable[target] == 0
            || self.entry_reaches(target, None, |a, b| (a, b) == guard)?
        {
            return Ok(false);
        }
        let region = self.mark_reaching(target, Some(guard.0))?;
        if self.workspace.region[guard.1] != region {
            return Ok(true);
        }
        let seen = self.workspace.begin(&mut self.work)?;
        self.workspace.seen[guard.1] = seen;
        self.workspace.pending.push(guard.1);
        while let Some(block) = self.workspace.pending.pop() {
            let body = &self.source.blocks()[block];
            self.work.charge(1 + body.statements().len())?;
            let mut defines = false;
            for statement in body.statements() {
                visit_statement_definition_places(statement.kind(), &mut |place| {
                    defines |= local_definition_index(place) == Some(local);
                });
            }
            if let SemanticTerminatorKindV1::Call(call) = body.terminator().kind() {
                defines |= call
                    .destination()
                    .and_then(|d| local_definition_index(d.place()))
                    == Some(local);
            }
            if defines {
                return Ok(false);
            }
            if block == target {
                continue;
            }
            let successors =
                &self.successors[self.successor_offsets[block]..self.successor_offsets[block + 1]];
            self.work.charge(successors.len())?;
            for &next in successors {
                if self.workspace.region[next] == region && self.workspace.seen[next] != seen {
                    self.workspace.seen[next] = seen;
                    self.workspace.pending.push(next);
                }
            }
        }
        Ok(true)
    }
}

include!("lossless_csr_v1/binding_query_v1.rs");

include!("lossless_csr_v1/primary_queries_v1.rs");

include!("lossless_csr_v1/enum_join_v1.rs");

include!("lossless_csr_v1/natural_loop_v1.rs");
