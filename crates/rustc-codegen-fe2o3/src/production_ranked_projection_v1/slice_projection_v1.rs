//! Exact per-source-site slice projection over the existing canonical session.

use super::*;

#[cfg(test)]
#[path = "slice_projection_v1_tests.rs"]
mod tests;

pub(super) struct ProjectedSliceInputV1 {
    pub(super) source_argument: u32,
    pub(super) direct_local: Option<SemanticLocalIdV1>,
    pub(super) element_width: u32,
}

struct QueriedSliceV1 {
    site: ProjectedSemanticAccessSiteV1,
    ordinal: u32,
    view: ProductionRankedValueIdV1,
}

pub(super) struct ProjectedViewsV1<'a> {
    locals: Vec<Option<ProjectedViewV1>>,
    scalar_private_singletons: &'a [u8],
    scalar_private_borrows: Option<(
        &'a scalar_borrow_projection_v1::ScalarPrivateBorrowsV1<'a>,
        fe2o3_mir_model::semantic_mir_v1::SemanticTargetDataLayoutV1,
    )>,
    facts: Option<&'a mut dyn ProjectedAssertionFactsV1>,
    site: Option<ProjectedSemanticAccessSiteV1>,
    source_start: usize,
    guarded_start: usize,
    next_access: u32,
    queried: Vec<QueriedSliceV1>,
}

impl<'a> ProjectedViewsV1<'a> {
    pub(super) fn require_unit_local_call(
        &mut self,
        block: usize,
        call: &SemanticDirectCallV1,
        source: SemanticSourceProvenanceV1,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        match self.facts.as_deref_mut() {
            Some(facts) => facts.require_unit_local_call(block, call, source),
            None => Err(
                ProductionRankedProjectionErrorV1::UnresolvedCallableEffect {
                    block,
                    source: Box::new(source),
                    callee: call.callee().index(),
                    tail: false,
                },
            ),
        }
    }

    pub(super) fn new(locals: usize, facts: Option<&'a mut dyn ProjectedAssertionFactsV1>) -> Self {
        Self {
            locals: vec![None; locals],
            scalar_private_singletons: &[],
            scalar_private_borrows: None,
            facts,
            site: None,
            source_start: 0,
            guarded_start: 0,
            next_access: 0,
            queried: Vec::new(),
        }
    }

    pub(super) fn with_scalar_private_singletons(mut self, census: &'a [u8]) -> Self {
        self.scalar_private_singletons = census;
        self
    }

    pub(super) fn scalar_private_singleton(
        &mut self,
        local: SemanticLocalIdV1,
    ) -> Result<bool, ProductionRankedProjectionErrorV1> {
        if self.scalar_private_singletons.is_empty() {
            return Ok(false);
        }
        self.charge_private_array_work(2)?;
        Ok(scalar_singleton_projection_v1::eligible(
            self.scalar_private_singletons,
            local,
        ))
    }

    pub(super) fn with_scalar_private_borrows(
        mut self,
        census: Option<&'a scalar_borrow_projection_v1::ScalarPrivateBorrowsV1<'a>>,
        target: fe2o3_mir_model::semantic_mir_v1::SemanticTargetDataLayoutV1,
    ) -> Self {
        self.scalar_private_borrows = census.map(|census| (census, target));
        self
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn scalar_private_borrow(
        &mut self,
        types: &[SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
        block: usize,
        place: &SemanticPlaceV1,
        access: AccessKindAttr,
        atomic: Option<SemanticAtomicAccessV1>,
        contracts: &ProjectionLocalContractsV1,
    ) -> Result<Option<SemanticLocalIdV1>, ProductionRankedProjectionErrorV1> {
        let Some((census, target)) = self.scalar_private_borrows else {
            return Ok(None);
        };
        let site = self
            .site
            .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                "scalar-borrow source occurrence",
            ))?;
        if site.block != block {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "scalar-borrow source block",
            ));
        }
        let provenance = contracts
            .allocation_provenance
            .get(place.local().index() as usize)
            .copied()
            .flatten();
        let facts =
            self.facts
                .as_deref_mut()
                .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                    "scalar-borrow canonical facts",
                ))?;
        census.resolve(
            function, types, target, site, place, access, atomic, provenance, facts,
        )
    }

    pub(super) fn begin_site(
        &mut self,
        site: ProjectedSemanticAccessSiteV1,
        source_start: usize,
        guarded_start: usize,
    ) {
        self.site = Some(site);
        self.source_start = source_start;
        self.guarded_start = guarded_start;
        self.next_access = 0;
    }

    pub(super) fn get_mut(&mut self, local: usize) -> Option<&mut Option<ProjectedViewV1>> {
        self.locals.get_mut(local)
    }

    pub(super) fn charge_private_array_work(
        &mut self,
        amount: usize,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        self.facts
            .as_deref_mut()
            .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                "private array projection requires canonical facts",
            ))?
            .charge_private_array_work(amount)
    }

    pub(super) fn private_array_initializer_count(
        &mut self,
        block: usize,
        statement: usize,
    ) -> Result<Option<u64>, ProductionRankedProjectionErrorV1> {
        self.facts
            .as_deref_mut()
            .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                "private array projection requires canonical facts",
            ))?
            .private_array_initializer_count(block, statement)
    }

    pub(super) fn slice_input(
        &mut self,
        check: ProjectedBoundsCheckV1,
        sources: &[ProjectedAccessSourceV1],
        guarded: &[GuardedAccessSiteV1],
        operations: &[ProductionRankedOperationV1],
    ) -> Result<(ProjectedSliceInputV1, u32), ProductionRankedProjectionErrorV1> {
        let site = self
            .site
            .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                "slice access has no original semantic site",
            ))?;
        let assertion =
            check
                .assertion_block
                .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                    "projected slice requires an emitted Rust bounds assertion",
                ))?;
        // Count completed effects at this source site in materializer order;
        // ordinary private accesses are absent from the executable census.
        let mut direct = 0_usize;
        for source in sources.get(self.source_start..).ok_or(
            ProductionRankedProjectionErrorV1::Unsupported("invalid slice source cursor"),
        )? {
            let operation = operations.get(source.operation).ok_or(
                ProductionRankedProjectionErrorV1::Unsupported(
                    "slice source operation is out of range",
                ),
            )?;
            if retained_ranked_access_source_v1(source.memory_space, operation) {
                direct += 1;
            }
        }
        let delayed = guarded
            .get(self.guarded_start..)
            .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                "invalid slice guarded cursor",
            ))?
            .iter()
            .filter(|site| site.access.memory_space != MemorySpaceAttr::Private)
            .count();
        let added = direct
            .checked_add(delayed)
            .and_then(|count| u32::try_from(count).ok())
            .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                "slice access ordinal overflow",
            ))?;
        let ordinal = self.next_access.checked_add(added).ok_or(
            ProductionRankedProjectionErrorV1::Unsupported("slice access ordinal overflow"),
        )?;
        self.next_access = ordinal;
        self.source_start = sources.len();
        self.guarded_start = guarded.len();
        let facts =
            self.facts
                .as_deref_mut()
                .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                    "slice access requires canonical facts",
                ))?;
        Ok((facts.slice_access(site, ordinal, assertion)?, ordinal))
    }

    pub(super) fn retain_query(
        &mut self,
        ordinal: u32,
        view: ProductionRankedValueIdV1,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        if self.queried.len() >= MAX_RANKED_BOUNDS_OPERATIONS {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "slice query count exceeds ranked operation limit",
            ));
        }
        self.queried.try_reserve(1).map_err(|_| {
            ProductionRankedProjectionErrorV1::Unsupported(
                "slice query correspondence storage cannot be reserved",
            )
        })?;
        self.queried.push(QueriedSliceV1 {
            site: self
                .site
                .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                    "missing slice query site",
                ))?,
            ordinal,
            view,
        });
        Ok(())
    }

    pub(super) fn finish(self) -> ProjectedSliceQueriesV1 {
        ProjectedSliceQueriesV1(self.queried)
    }
}

pub(super) struct ProjectedSliceQueriesV1(Vec<QueriedSliceV1>);

impl ProjectedSliceQueriesV1 {
    pub(super) fn validate(
        mut self,
        blocks: &[ProductionRankedBlockV1],
        sources: &[ProductionRankedAccessSourceV1],
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        let key = |query: &QueriedSliceV1| (query.site.block, query.site.statement, query.ordinal);
        self.0.sort_unstable_by_key(key);
        for pair in self.0.windows(2) {
            if key(&pair[0]) == key(&pair[1]) {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "duplicate queried slice occurrence",
                ));
            }
        }
        let mut seen = Vec::new();
        seen.try_reserve_exact(self.0.len()).map_err(|_| {
            ProductionRankedProjectionErrorV1::Unsupported(
                "slice census scratch storage cannot be reserved",
            )
        })?;
        seen.resize(self.0.len(), false);
        for source in sources {
            let key = (
                source.semantic_block() as usize,
                source.semantic_statement().map(|value| value as usize),
                source.semantic_access_ordinal(),
            );
            let Ok(index) = self.0.binary_search_by_key(&key, |query| {
                (query.site.block, query.site.statement, query.ordinal)
            }) else {
                continue;
            };
            if std::mem::replace(&mut seen[index], true) {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "duplicate final slice occurrence",
                ));
            }
            let view = self.0[index].view;
            let operation = blocks
                .get(source.ranked_block() as usize)
                .and_then(|block| block.operations().get(source.ranked_operation() as usize));
            if !matches!(operation,
                Some(ProductionRankedOperationV1::Access {
                    kind: AccessKindAttr::Read,
                    view: ProductionRankedValueV1::Local(actual), ..
                }) if *actual == view)
            {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "queried slice differs from the final source access census",
                ));
            }
        }
        if seen.iter().any(|seen| !seen) {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "queried slice disappeared from the final source access census",
            ));
        }
        Ok(())
    }
}
