// Two explicit constructor sites form a source cut, not a choice of one
// arbitrary historical assignment. All queries borrow the existing workspace.
impl CsrQueryV1<'_, '_> {
    pub(super) fn enum_definitions_cover_use(
        &mut self,
        definitions: [ScalarAssignmentSiteV1; 2],
        use_site: ScalarAssignmentSiteV1,
    ) -> Result<bool, Error> {
        let [left, right] = definitions;
        self.check(&[left.block, right.block, use_site.block])?;
        if left.block == right.block
            || [left.block, right.block].contains(&use_site.block)
            || self.reachable[use_site.block] == 0
            || [left, right].iter().any(|site| {
                self.reachable[site.block] == 0
                    || site.statement >= self.source.blocks()[site.block].statements().len()
            })
            || use_site.statement > self.source.blocks()[use_site.block].statements().len()
        {
            return Ok(false);
        }
        Ok(!self.entry_reaches(use_site.block, None, |from, _| {
            from == left.block || from == right.block
        })?)
    }

    fn enum_path_reaches(
        &mut self,
        source: usize,
        target: usize,
        omitted: [Option<usize>; 2],
        successors_only: bool,
    ) -> Result<bool, Error> {
        self.check(&[source, target])?;
        for omitted in omitted.into_iter().flatten() {
            self.check(&[omitted])?;
        }
        let generation = self.workspace.begin(&mut self.work)?;
        if successors_only {
            let row = &self.successors
                [self.successor_offsets[source]..self.successor_offsets[source + 1]];
            self.work.charge(row.len())?;
            for &next in row {
                if !omitted.contains(&Some(next)) && self.workspace.seen[next] != generation {
                    self.workspace.seen[next] = generation;
                    self.workspace.pending.push(next);
                }
            }
        } else if !omitted.contains(&Some(source)) {
            self.workspace.seen[source] = generation;
            self.workspace.pending.push(source);
        }
        while let Some(block) = self.workspace.pending.pop() {
            self.work.charge(1)?;
            if block == target {
                return Ok(true);
            }
            let row =
                &self.successors[self.successor_offsets[block]..self.successor_offsets[block + 1]];
            self.work.charge(row.len())?;
            for &next in row {
                if !omitted.contains(&Some(next)) && self.workspace.seen[next] != generation {
                    self.workspace.seen[next] = generation;
                    self.workspace.pending.push(next);
                }
            }
        }
        Ok(false)
    }

    pub(super) fn enum_capture_is_fresh(
        &mut self,
        constructor: usize,
        capture: usize,
        projected_use: usize,
    ) -> Result<bool, Error> {
        self.check(&[constructor, capture, projected_use])?;
        // Entry dominance alone permits a loop to skip a later re-capture.
        // Check order from this constructor and every subsequent use as well.
        Ok(
            !self.enum_path_reaches(constructor, projected_use, [Some(capture), None], false)?
                && !self.enum_path_reaches(
                    projected_use,
                    projected_use,
                    [Some(capture), None],
                    true,
                )?,
        )
    }

    pub(super) fn enum_constructor_is_fresh(
        &mut self,
        constructor: usize,
        other_constructor: usize,
        projected_use: usize,
    ) -> Result<bool, Error> {
        if !self.enum_path_reaches(constructor, projected_use, [None; 2], false)?
            || self.enum_path_reaches(
                projected_use,
                projected_use,
                [Some(constructor), Some(other_constructor)],
                true,
            )?
        {
            return Ok(false);
        }
        // A cycle before the first use may carry an earlier scalar version.
        // The conservative slice accepts re-execution only after that use and
        // only when a constructor and every whole-value capture run again.
        // The other constructor fixes a different variant and cannot supply
        // this payload after the checked variant branch.
        self.enum_acyclic_before_use(constructor, projected_use)
    }

    fn enum_acyclic_before_use(&mut self, source: usize, target: usize) -> Result<bool, Error> {
        self.check(&[source, target])?;
        // region temporarily holds DFS cursors, not generation tags. Its full
        // cleanup is charged before mutation, including the error path.
        self.work.charge(self.workspace.region.len())?;
        let result = (|| {
            let generation = self.workspace.begin(&mut self.work)?;
            self.workspace.seen[source] = generation;
            self.workspace.region[source] = self.successor_offsets[source] + 1;
            self.workspace.pending.push(source);
            while let Some(&block) = self.workspace.pending.last() {
                self.work.charge(1)?;
                let cursor = self.workspace.region[block] - 1;
                if block == target || cursor == self.successor_offsets[block + 1] {
                    self.workspace.region[block] = 0;
                    self.workspace.pending.pop();
                    continue;
                }
                self.work.charge(1)?;
                let next = self.successors[cursor];
                self.workspace.region[block] += 1;
                if self.workspace.seen[next] == generation {
                    if self.workspace.region[next] != 0 {
                        return Ok(false);
                    }
                } else {
                    self.workspace.seen[next] = generation;
                    self.workspace.region[next] = self.successor_offsets[next] + 1;
                    self.workspace.pending.push(next);
                }
            }
            Ok(true)
        })();
        self.workspace.region.fill(0);
        self.workspace.pending.clear();
        result
    }
}
