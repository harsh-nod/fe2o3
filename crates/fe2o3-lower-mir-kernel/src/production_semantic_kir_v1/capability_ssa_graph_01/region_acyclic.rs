// Graph-private working capacity only; no mask or acyclicity result is retained.
struct CapabilityRegionAcyclicScratchV1 {
    degree: Vec<usize>,
    ready: Vec<usize>,
}

impl CapabilitySsaGraphV1<'_> {
    fn new_region_acyclic_scratch(
        &mut self,
        count: usize,
    ) -> Result<Box<CapabilityRegionAcyclicScratchV1>, ProductionSemanticKirErrorV1> {
        let word = std::mem::size_of::<usize>();
        self.charge(std::mem::size_of::<CapabilityRegionAcyclicScratchV1>().div_ceil(word))?;
        self.charge(count)?;
        self.charge(count)?;
        let degree = Vec::with_capacity(count);
        let ready = Vec::with_capacity(count);
        // Charge allocator excess before either payload can be used or retained.
        self.charge(degree.capacity() - count)?;
        self.charge(ready.capacity() - count)?;
        Ok(Box::new(CapabilityRegionAcyclicScratchV1 { degree, ready }))
    }

    fn region_is_acyclic_reusing_scratch(
        &mut self,
        region: &[bool],
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        let count = self.body.blocks().len();
        if region.len() != count {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        // Prepay checkout and successful return of exclusively owned storage.
        self.charge(2)?;
        let mut scratch = match self.reuse.region_acyclic_scratch.take() {
            Some(scratch) => scratch,
            None => self.new_region_acyclic_scratch(count)?,
        };
        // Fully reset after true AND false. Clear usize lengths in constant
        // time, then initialize all V degrees; no previous degree is evidence.
        self.charge(count.saturating_add(2))?;
        scratch.degree.clear();
        scratch.ready.clear();
        scratch.degree.resize(count, 0);
        // Errors drop checked-out scratch. Ok(false) retains only paid capacity.
        let acyclic = self.region_acyclic_with_scratch(region, &mut scratch)?;
        self.reuse.region_acyclic_scratch = Some(scratch);
        Ok(acyclic)
    }

    // Every call performs the original full induced-region Kahn traversal.
    fn region_acyclic_with_scratch(
        &mut self,
        region: &[bool],
        scratch: &mut CapabilityRegionAcyclicScratchV1,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        let degree = &mut scratch.degree;
        let ready = &mut scratch.ready;
        let mut vertices = 0usize;
        for (block, &included) in region.iter().enumerate() {
            self.charge(1)?;
            if !included {
                continue;
            }
            vertices += 1;
            self.body.blocks()[block]
                .terminator()
                .kind()
                .try_for_each_edge::<ProductionSemanticKirErrorV1>(|edge| {
                    self.charge(1)?;
                    let target = edge.target().index() as usize;
                    let included = region
                        .get(target)
                        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                    if *included {
                        degree[target] = degree[target]
                            .checked_add(1)
                            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                    }
                    Ok(())
                })?;
        }
        for (block, &included) in region.iter().enumerate() {
            self.charge(1)?;
            if included && degree[block] == 0 {
                ready.push(block);
            }
        }
        let mut next = 0;
        while next < ready.len() {
            self.charge(1)?;
            let block = ready[next];
            next += 1;
            self.body.blocks()[block]
                .terminator()
                .kind()
                .try_for_each_edge::<ProductionSemanticKirErrorV1>(|edge| {
                    self.charge(1)?;
                    let target = edge.target().index() as usize;
                    if region[target] {
                        degree[target] = degree[target]
                            .checked_sub(1)
                            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                        if degree[target] == 0 {
                            ready.push(target);
                        }
                    }
                    Ok(())
                })?;
        }
        Ok(next == vertices)
    }
}

#[cfg(test)]
include!("region_acyclic_cold_reference.rs");
