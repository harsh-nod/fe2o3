// One completed forward cone, bound to the exact immutable inputs. Region
// masks and authority results are never retained under the start-only key.
struct CapabilityPathRegionScratchV1<'a> {
    body: &'a SemanticFunctionDeclV1,
    ssa: &'a SsaConstructionPlanV1,
    completed_from: Option<u32>,
    seen: Vec<usize>,
    predecessors: Vec<Vec<u32>>,
    pending: Vec<u32>,
    epoch: usize,
}

impl<'a> CapabilitySsaGraphV1<'a> {
    fn new_path_region_scratch(
        &mut self,
        count: usize,
    ) -> Result<Box<CapabilityPathRegionScratchV1<'a>>, ProductionSemanticKirErrorV1> {
        let word = std::mem::size_of::<usize>();
        let row_words = std::mem::size_of::<Vec<u32>>().div_ceil(word);
        self.charge(std::mem::size_of::<CapabilityPathRegionScratchV1<'a>>().div_ceil(word))?;
        // Retained payloads, plus both initialization loops. Pending and each
        // predecessor ID are conservatively charged as full logical words.
        self.charge(count.saturating_mul(row_words.saturating_add(4)))?;
        let mut seen = Vec::with_capacity(count);
        let mut predecessors = Vec::with_capacity(count);
        let pending = Vec::with_capacity(count);
        self.charge(seen.capacity() - count)?;
        self.charge((predecessors.capacity() - count).saturating_mul(row_words))?;
        self.charge(pending.capacity() - count)?;
        seen.resize(count, 0usize);
        predecessors.resize_with(count, Vec::new);
        // Initialize both exact bindings and the unpublished cone key.
        self.charge(3)?;
        Ok(Box::new(CapabilityPathRegionScratchV1 {
            body: self.body,
            ssa: self.ssa,
            completed_from: None,
            seen,
            predecessors,
            pending,
            epoch: 0,
        }))
    }

    /// Exact intersection: reuse only the last completed forward walk, then
    /// run the original backward walk into a freshly initialized result.
    pub(super) fn path_region(
        &mut self,
        from: u32,
        to: u32,
    ) -> Result<Vec<bool>, ProductionSemanticKirErrorV1> {
        let count = self.body.blocks().len();
        if from as usize >= count || to as usize >= count {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        // The returned region is still fresh and fully initialized per query.
        self.charge(count)?;
        let mut region = vec![false; count];
        if !self.ssa.is_reachable(SsaBlockIdV1::new(from)) {
            return Ok(region);
        }
        // Prepay checkout and successful return of the exclusively owned Box.
        self.charge(2)?;
        let mut scratch = match self.reuse.path_region_scratch.take() {
            Some(scratch) => scratch,
            None => self.new_path_region_scratch(count)?,
        };
        // Any error drops the checked-out scratch, including a failed charge
        // for allocator excess. Partially paid capacity is never retained.
        self.path_region_with_scratch(from, to, &mut region, &mut scratch)?;
        self.reuse.path_region_scratch = Some(scratch);
        Ok(region)
    }

    fn path_region_with_scratch(
        &mut self,
        from: u32,
        to: u32,
        region: &mut [bool],
        scratch: &mut CapabilityPathRegionScratchV1<'a>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        // Prepay body identity, SSA identity, and completed-start dispatch.
        self.charge(3)?;
        if !std::ptr::eq(self.body, scratch.body) || !std::ptr::eq(self.ssa, scratch.ssa) {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        if scratch.completed_from != Some(from) {
            self.charge(1)?;
            scratch.completed_from = None;
            self.path_region_forward_cone(from, region.len(), scratch)?;
            self.charge(1)?;
            scratch.completed_from = Some(from);
        }
        // Completion guarantees an empty pending stack and exact incoming
        // rows (including duplicate edges) for every vertex in this epoch.
        if scratch.seen[to as usize] != scratch.epoch {
            return Ok(());
        }
        region[to as usize] = true;
        scratch.pending.push(to);
        while let Some(block) = scratch.pending.pop() {
            self.charge(1)?;
            for &predecessor in &scratch.predecessors[block as usize] {
                self.charge(1)?;
                if !region[predecessor as usize] {
                    region[predecessor as usize] = true;
                    scratch.pending.push(predecessor);
                }
            }
        }
        Ok(())
    }

    fn path_region_forward_cone(
        &mut self,
        from: u32,
        count: usize,
        scratch: &mut CapabilityPathRegionScratchV1<'a>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.charge(1)?;
        if scratch.epoch == usize::MAX {
            self.charge(scratch.seen.len())?;
            scratch.seen.fill(0);
            scratch.epoch = 0;
        }
        scratch.epoch += 1;
        let epoch = scratch.epoch;
        scratch.pending.clear();
        scratch.seen[from as usize] = epoch;
        scratch.predecessors[from as usize].clear();
        scratch.pending.push(from);
        while let Some(block) = scratch.pending.pop() {
            self.charge(1)?;
            self.body.blocks()[block as usize]
                .terminator()
                .kind()
                .try_for_each_edge::<ProductionSemanticKirErrorV1>(|edge| {
                    self.charge(1)?;
                    let target = edge.target().index();
                    if target as usize >= count {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    }
                    if self.ssa.is_reachable(SsaBlockIdV1::new(target)) {
                        if scratch.seen[target as usize] != epoch {
                            scratch.seen[target as usize] = epoch;
                            // Every row read by the backward walk is cleared
                            // on its first discovery in this forward walk.
                            scratch.predecessors[target as usize].clear();
                            scratch.pending.push(target);
                        }
                        let row = &mut scratch.predecessors[target as usize];
                        if row.len() == row.capacity() {
                            let capacity = row.capacity().saturating_mul(2).max(1);
                            // Cover the new payload and worst-case movement
                            // of live IDs before growing the retained row.
                            self.charge(capacity.saturating_add(row.len()))?;
                            row.reserve_exact(capacity - row.len());
                            self.charge(row.capacity() - capacity)?;
                        }
                        row.push(block);
                    }
                    Ok(())
                })?;
        }
        Ok(())
    }
}
