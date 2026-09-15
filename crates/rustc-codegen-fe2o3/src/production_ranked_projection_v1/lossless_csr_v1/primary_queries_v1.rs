impl LosslessCsrV1<'_> {
    pub(super) fn entry(&self) -> usize {
        self.source.entry().index() as usize
    }
}

#[cfg(test)]
impl fmt::Debug for LosslessCsrV1<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LosslessCsrV1")
            .field("nodes", &self.reachable.len())
            .field("edges", &self.successors.len())
            .finish_non_exhaustive()
    }
}

impl CsrQueryV1<'_, '_> {
    pub(super) fn reaching_mask(&mut self, target: usize) -> Result<Vec<bool>, Error> {
        self.check(&[target])?;
        let generation = self.mark_reaching(target, None)?;
        self.work.charge(self.reachable.len())?;
        let mut result = Vec::new();
        result
            .try_reserve_exact(self.reachable.len())
            .map_err(|_| Error::Unsupported("CSR retained capture mask cannot be reserved"))?;
        result.extend(self.workspace.region.iter().map(|&mark| mark == generation));
        Ok(result)
    }

    fn definition_in_range(
        &mut self,
        block: usize,
        local: usize,
        start: usize,
        end: usize,
    ) -> Result<bool, Error> {
        let Some(statements) = self.source.blocks()[block].statements().get(start..end) else {
            return Err(Error::Unsupported(
                "CSR definition range is outside retained source",
            ));
        };
        self.work.charge(statements.len())?;
        Ok(statements.iter().any(|statement| {
            let mut defines = false;
            visit_statement_definition_places(statement.kind(), &mut |place| {
                defines |= local_definition_index(place) == Some(local);
            });
            defines
        }))
    }

    fn definition_in_block(&mut self, block: usize, local: usize) -> Result<bool, Error> {
        let body = &self.source.blocks()[block];
        self.work.charge(1)?;
        let terminator_defines = matches!(
            body.terminator().kind(), SemanticTerminatorKindV1::Call(call)
                if call.destination().and_then(|d| local_definition_index(d.place())) == Some(local)
        );
        Ok(terminator_defines
            || self.definition_in_range(block, local, 0, body.statements().len())?)
    }

    fn enqueue_region(&mut self, block: usize, region: usize, seen: usize) {
        if self.workspace.region[block] == region && self.workspace.seen[block] != seen {
            self.workspace.seen[block] = seen;
            self.workspace.pending.push(block);
        }
    }

    fn stable_pending(
        &mut self,
        local: usize,
        target: usize,
        use_statement: Option<usize>,
        region: usize,
        seen: usize,
    ) -> Result<bool, Error> {
        while let Some(block) = self.workspace.pending.pop() {
            self.work.charge(1)?;
            let defines = if block == target
                && let Some(end) = use_statement
            {
                self.definition_in_range(block, local, 0, end)?
            } else {
                self.definition_in_block(block, local)?
            };
            if defines {
                return Ok(false);
            }
            if block == target {
                continue;
            }
            let start = self.successor_offsets[block];
            let end = self.successor_offsets[block + 1];
            self.work.charge(end - start)?;
            for edge in start..end {
                self.enqueue_region(self.successors[edge], region, seen);
            }
        }
        Ok(true)
    }

    fn stable_from_edge(
        &mut self,
        local: usize,
        edge_target: usize,
        target: usize,
        use_statement: Option<usize>,
    ) -> Result<bool, Error> {
        self.check(&[edge_target, target])?;
        if local >= self.source.locals().len()
            || use_statement
                .is_some_and(|end| end > self.source.blocks()[target].statements().len())
        {
            return Ok(false);
        }
        let region = self.mark_reaching(target, None)?;
        if self.workspace.region[edge_target] != region {
            return Ok(false);
        }
        let seen = self.workspace.begin(&mut self.work)?;
        self.enqueue_region(edge_target, region, seen);
        self.stable_pending(local, target, use_statement, region, seen)
    }

    /// Freshness only; callers retain their exact comparison and dominance checks.
    pub(super) fn stable_edge_to_block(
        &mut self,
        local: usize,
        edge_target: usize,
        target: usize,
    ) -> Result<bool, Error> {
        self.stable_from_edge(local, edge_target, target, None)
    }

    /// The target prefix excludes the use statement and its terminator.
    pub(super) fn stable_edge_to_site(
        &mut self,
        local: usize,
        edge_target: usize,
        use_site: ScalarAssignmentSiteV1,
    ) -> Result<bool, Error> {
        self.stable_from_edge(local, edge_target, use_site.block, Some(use_site.statement))
    }

    /// Caller has checked capture dominance and the capture block's suffix/terminator.
    pub(super) fn stable_capture_successors_to_site(
        &mut self,
        local: usize,
        capture: usize,
        use_site: ScalarAssignmentSiteV1,
    ) -> Result<bool, Error> {
        self.check(&[capture, use_site.block])?;
        if local >= self.source.locals().len()
            || use_site.statement > self.source.blocks()[use_site.block].statements().len()
        {
            return Ok(false);
        }
        let region = self.mark_reaching(use_site.block, None)?;
        let seen = self.workspace.begin(&mut self.work)?;
        let start = self.successor_offsets[capture];
        let end = self.successor_offsets[capture + 1];
        self.work.charge(end - start)?;
        for edge in start..end {
            self.enqueue_region(self.successors[edge], region, seen);
        }
        self.stable_pending(
            local,
            use_site.block,
            Some(use_site.statement),
            region,
            seen,
        )
    }

    /// Every subsequent dynamic use must cross this same successful edge again.
    pub(super) fn guard_authenticates_each_use(
        &mut self,
        guard: (usize, usize),
        target: usize,
    ) -> Result<bool, Error> {
        self.check(&[guard.0, guard.1, target])?;
        if self.reachable[target] == 0
            || self.entry_reaches(target, None, |a, b| (a, b) == guard)?
        {
            return Ok(false);
        }
        let seen = self.workspace.begin(&mut self.work)?;
        self.workspace.seen[target] = seen;
        self.workspace.pending.push(target);
        while let Some(block) = self.workspace.pending.pop() {
            let start = self.successor_offsets[block];
            let end = self.successor_offsets[block + 1];
            self.work.charge(1 + end - start)?;
            for edge in start..end {
                let next = self.successors[edge];
                if (block, next) == guard {
                    continue;
                }
                if next == target {
                    return Ok(false);
                }
                if self.workspace.seen[next] != seen {
                    self.workspace.seen[next] = seen;
                    self.workspace.pending.push(next);
                }
            }
        }
        Ok(true)
    }
}
