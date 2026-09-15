impl CsrQueryV1<'_, '_> {
    pub(super) fn natural_loop_topology(
        &mut self,
        callables: &[SemanticCallableDeclV1],
        header: usize,
        body_entry: usize,
        exit: usize,
    ) -> Result<Option<ProjectedNaturalLoopTopologyV1>, Error> {
        self.check(&[header, body_entry, exit])?;
        if self.reachable[header] == 0 {
            return Err(Error::Incomplete(
                "a uniform induction header is unreachable",
            ));
        }
        let function = self.source;
        let without_header = self.workspace.begin(&mut self.work)?;
        let entry = function.entry().index() as usize;
        if entry != header {
            self.workspace.seen[entry] = without_header;
            self.workspace.pending.push(entry);
        }
        while let Some(block) = self.workspace.pending.pop() {
            let successors =
                &self.successors[self.successor_offsets[block]..self.successor_offsets[block + 1]];
            self.work.charge(1 + successors.len())?;
            for &next in successors {
                if next != header && self.workspace.seen[next] != without_header {
                    self.workspace.seen[next] = without_header;
                    self.workspace.pending.push(next);
                }
            }
        }
        let predecessors = &self.predecessors
            [self.predecessor_offsets[header]..self.predecessor_offsets[header + 1]];
        self.work.charge(predecessors.len())?;
        let (mut latch, mut preheader) = (0, 0);
        let (mut backedges, mut preheaders) = (0, 0);
        for &predecessor in predecessors {
            if self.reachable[predecessor] == 0 {
                continue;
            }
            if self.workspace.seen[predecessor] == without_header {
                preheaders += 1;
                preheader = predecessor;
            } else {
                backedges += 1;
                latch = predecessor;
            }
        }
        if backedges == 0 {
            return Ok(None);
        }
        if backedges != 1 {
            return Err(Error::Incomplete(
                "a uniform induction without one unique dominated backedge",
            ));
        }
        if preheaders != 1 {
            return Err(Error::Incomplete(
                "a uniform induction without one unique preheader",
            ));
        }
        let latch_is_exact = matches!(
            function.blocks()[latch].terminator().kind(),
            SemanticTerminatorKindV1::Goto(edge)
                if edge.role() == SemanticEdgeRoleV1::Goto
                    && edge.target().index() as usize == header
        );
        let latch_successors =
            &self.successors[self.successor_offsets[latch]..self.successor_offsets[latch + 1]];
        if !latch_is_exact || latch_successors != [header] {
            return Err(Error::Incomplete(
                "a uniform induction preheader or latch has non-canonical control",
            ));
        }
        let preheader_successors = &self.successors
            [self.successor_offsets[preheader]..self.successor_offsets[preheader + 1]];
        let preheader_control = match function.blocks()[preheader].terminator().kind() {
            SemanticTerminatorKindV1::Goto(edge)
                if edge.role() == SemanticEdgeRoleV1::Goto
                    && edge.target().index() as usize == header
                    && preheader_successors == [header] =>
            {
                ProjectedInductionPreheaderControlV1::Direct
            }
            SemanticTerminatorKindV1::SwitchInt {
                discriminant,
                targets,
            } if targets.values().len() == 1 => {
                let explicit = targets.values()[0];
                let explicit_target = explicit.edge().target().index() as usize;
                let otherwise = targets.otherwise().target().index() as usize;
                if explicit.edge().role() != SemanticEdgeRoleV1::SwitchValue
                    || targets.otherwise().role() != SemanticEdgeRoleV1::SwitchOtherwise
                    || explicit_target == otherwise
                    || !((explicit_target == header && otherwise == exit)
                        || (explicit_target == exit && otherwise == header))
                    || preheader_successors != [header.min(exit), header.max(exit)]
                {
                    return Err(Error::Incomplete(
                        "an optional uniform induction preheader is not one exact header-or-exit switch",
                    ));
                }
                ProjectedInductionPreheaderControlV1::Optional {
                    discriminant: discriminant.clone(),
                    explicit_value: explicit.value(),
                    explicit_target,
                    otherwise,
                }
            }
            _ => {
                return Err(Error::Incomplete(
                    "an optional uniform induction preheader is not one exact header-or-exit switch",
                ));
            }
        };

        // Unlike an unrestricted reverse query, natural closure includes the
        // header but stops there, and excludes entry-unreachable predecessors.
        let region = self.workspace.begin(&mut self.work)?;
        self.workspace.region[header] = region;
        self.workspace.region[latch] = region;
        self.workspace.pending.push(latch);
        while let Some(block) = self.workspace.pending.pop() {
            self.work.charge(1)?;
            if block == header {
                continue;
            }
            let predecessors = &self.predecessors
                [self.predecessor_offsets[block]..self.predecessor_offsets[block + 1]];
            self.work.charge(predecessors.len())?;
            for &predecessor in predecessors {
                if self.reachable[predecessor] != 0 && self.workspace.region[predecessor] != region
                {
                    self.workspace.region[predecessor] = region;
                    self.workspace.pending.push(predecessor);
                }
            }
        }
        if self.workspace.region[body_entry] != region || self.workspace.region[exit] == region {
            return Err(Error::Incomplete(
                "a uniform induction body and exit do not form a natural loop",
            ));
        }
        let mut members = 0;
        for block in 0..self.reachable.len() {
            self.work.charge(1)?;
            if self.workspace.region[block] != region {
                continue;
            }
            members += 1;
            if self.workspace.seen[block] == without_header {
                return Err(Error::Incomplete(
                    "an irreducible entry enters a uniform induction body",
                ));
            }
            let predecessors = &self.predecessors
                [self.predecessor_offsets[block]..self.predecessor_offsets[block + 1]];
            self.work.charge(predecessors.len())?;
            for &predecessor in predecessors {
                if self.reachable[predecessor] != 0
                    && self.workspace.region[predecessor] != region
                    && !(block == header && predecessor == preheader)
                {
                    return Err(Error::Incomplete(
                        "a uniform induction region has more than one entry",
                    ));
                }
            }
        }
        // All entry checks are finished; trap queries may reuse seen while
        // preserving the complete natural-loop region in the other mark table.
        let mut header_exit_count = 0;
        for block in 0..self.reachable.len() {
            self.work.charge(1)?;
            if self.workspace.region[block] != region {
                continue;
            }
            let start = self.successor_offsets[block];
            let end = self.successor_offsets[block + 1];
            self.work.charge(end - start)?;
            for index in start..end {
                let target = self.successors[index];
                if self.workspace.region[target] == region {
                    continue;
                }
                if (block, target) == (header, exit) {
                    header_exit_count += 1;
                } else if !self.natural_loop_trap_path(callables, region, target)? {
                    return Err(Error::Incomplete(
                        "a uniform induction region does not have one unique header exit",
                    ));
                }
            }
        }
        if header_exit_count != 1 {
            return Err(Error::Incomplete(
                "a uniform induction region does not have one unique header exit",
            ));
        }
        let output_cells = |capacity: usize| {
            capacity
                .checked_add(3)
                .filter(|&cells| cells <= MAX_PROJECTED_CAPABILITY_STATE_ENTRIES_V1)
                .ok_or(Error::Unsupported(
                    "natural loop roster exceeds the existing state entry limit",
                ))
        };
        self.work.charge(output_cells(members)?)?;
        let mut loop_blocks = Vec::new();
        loop_blocks
            .try_reserve_exact(members)
            .map_err(|_| Error::Unsupported("natural loop roster cannot be reserved"))?;
        output_cells(loop_blocks.capacity())?;
        self.work.charge(loop_blocks.capacity() - members)?;
        for block in 0..self.reachable.len() {
            self.work.charge(1)?;
            if self.workspace.region[block] == region {
                loop_blocks.push(block);
            }
        }
        Ok(Some(ProjectedNaturalLoopTopologyV1 {
            preheader,
            preheader_control,
            latch,
            loop_blocks,
        }))
    }

    fn natural_loop_trap_path(
        &mut self,
        callables: &[SemanticCallableDeclV1],
        region: usize,
        mut current: usize,
    ) -> Result<bool, Error> {
        let visited = self.workspace.begin(&mut self.work)?;
        loop {
            self.work.charge(1)?;
            let Some(block) = self.source.blocks().get(current) else {
                return Err(Error::Unsupported(
                    "a uniform induction terminal side exit is outside the semantic CFG",
                ));
            };
            if self.workspace.region[current] == region || self.workspace.seen[current] == visited {
                return Ok(false);
            }
            self.workspace.seen[current] = visited;
            self.work.charge(block.statements().len())?;
            if block.statements().iter().any(|statement| {
                !matches!(
                    statement.kind(),
                    SemanticStatementKindV1::StorageLive(_)
                        | SemanticStatementKindV1::StorageDead(_)
                        | SemanticStatementKindV1::Nop
                )
            }) {
                return Ok(false);
            }
            match block.terminator().kind() {
                SemanticTerminatorKindV1::Goto(edge) => current = edge.target().index() as usize,
                SemanticTerminatorKindV1::Call(call)
                    if call.destination().is_none()
                        && call.arguments().is_empty()
                        && !matches!(call.unwind(), SemanticUnwindActionV1::Cleanup(_))
                        && matches!(
                            callables.get(call.callee().index() as usize),
                            Some(SemanticCallableDeclV1::CompilerIntrinsic {
                                operation: SemanticCompilerIntrinsicOperationV1::Trap,
                                ..
                            })
                        ) =>
                {
                    return Ok(true);
                }
                _ => return Ok(false),
            }
        }
    }

    #[cfg(test)]
    pub(super) fn reviewed_trap_path_for_test(
        &mut self,
        callables: &[SemanticCallableDeclV1],
        in_loop: &[bool],
        start: usize,
    ) -> Result<bool, Error> {
        assert_eq!(in_loop.len(), self.reachable.len());
        let region = self.workspace.begin(&mut self.work)?;
        self.work.charge(in_loop.len())?;
        for (block, &inside) in in_loop.iter().enumerate() {
            if inside {
                self.workspace.region[block] = region;
            }
        }
        self.natural_loop_trap_path(callables, region, start)
    }
}
