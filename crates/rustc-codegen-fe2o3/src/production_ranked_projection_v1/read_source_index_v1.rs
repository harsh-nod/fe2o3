// Original source ordinals keep equal rows distinct and preserve read-join order.
struct RankedReadSourceIndexV1 {
    rows: Vec<RankedReadSourceRowV1>,
}

struct RankedReadSourceRowV1 {
    source: usize,
    block: usize,
    statement: usize,
    ordinal: usize,
    count: usize,
}

impl RankedReadSourceIndexV1 {
    fn site(source: &ProjectedAccessSourceV1) -> Option<(usize, usize)> {
        if source.access != AccessKindAttr::Read || source.memory_space != MemorySpaceAttr::Global {
            return None;
        }
        let site = source.semantic_site?;
        Some((site.block, site.statement?))
    }

    fn new(sources: &[ProjectedAccessSourceV1]) -> Result<Self, ProductionRankedProjectionErrorV1> {
        let count = sources
            .iter()
            .filter(|source| Self::site(source).is_some())
            .count();
        let mut rows = Vec::new();
        if count == 0 {
            return Ok(Self { rows });
        }
        rows.try_reserve_exact(count).map_err(|_| {
            ProductionRankedProjectionErrorV1::Unsupported(
                "ranked read source-index storage cannot be reserved",
            )
        })?;
        for (source, access) in sources.iter().enumerate() {
            if let Some((block, statement)) = Self::site(access) {
                rows.push(RankedReadSourceRowV1 {
                    source,
                    block,
                    statement,
                    ordinal: 0,
                    count: 0,
                });
            }
        }
        rows.sort_unstable_by_key(|row| (row.block, row.statement, row.source));
        let mut start = 0;
        while start < rows.len() {
            let site = (rows[start].block, rows[start].statement);
            let mut end = start + 1;
            while end < rows.len() && (rows[end].block, rows[end].statement) == site {
                end += 1;
            }
            for (ordinal, row) in rows[start..end].iter_mut().enumerate() {
                row.ordinal = ordinal;
                row.count = end - start;
            }
            start = end;
        }
        rows.sort_unstable_by_key(|row| row.source);
        Ok(Self { rows })
    }

    fn get(&self, source: usize) -> Option<(usize, usize)> {
        let index = self
            .rows
            .binary_search_by_key(&source, |row| row.source)
            .ok()?;
        let row = &self.rows[index];
        Some((row.ordinal, row.count))
    }
}
