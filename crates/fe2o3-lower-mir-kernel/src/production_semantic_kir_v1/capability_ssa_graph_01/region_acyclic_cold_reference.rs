// A path region contains every cycle through any of its vertices: leaving the
// region and returning still gives a path from the borrow to the consumer.
impl CapabilitySsaGraphV1<'_> {
    fn region_is_acyclic_cold_reference(
        &mut self,
        region: &[bool],
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        let count = self.body.blocks().len();
        if region.len() != count {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        self.charge(2 * std::mem::size_of::<Vec<usize>>() / std::mem::size_of::<usize>())?;
        // Two payloads plus degree initialization; charge actual excess before
        // using either buffer. Neither scratch buffer is cached or published.
        for _ in 0..3 {
            self.charge(count)?;
        }
        let mut degree = Vec::with_capacity(count);
        let mut ready = Vec::with_capacity(count);
        self.charge(degree.capacity() - count)?;
        self.charge(ready.capacity() - count)?;
        degree.resize(count, 0usize);
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
