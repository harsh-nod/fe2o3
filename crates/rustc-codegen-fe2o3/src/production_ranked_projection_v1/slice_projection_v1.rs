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
    pub(super) fn with_assertion_facts_v1<T>(
        &mut self,
        action: impl for<'f> FnOnce(
            &'f mut dyn ProjectedAssertionFactsV1,
        ) -> Result<T, ProductionRankedProjectionErrorV1>,
    ) -> Result<T, ProductionRankedProjectionErrorV1> {
        let facts =
            self.facts
                .as_deref_mut()
                .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                    "multi-entry induction requires live canonical facts",
                ))?;
        action(facts)
    }

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

    pub(super) fn call_projection_disposition_v18<'call>(
        &mut self,
        block: usize,
        call: &'call SemanticDirectCallV1,
        source: SemanticSourceProvenanceV1,
    ) -> Result<CallProjectionDispositionV18<'call>, ProductionRankedProjectionErrorV1> {
        match self.facts.as_deref_mut() {
            Some(facts) => facts.call_projection_disposition_v18(block, call, source),
            None => Err(ProductionRankedProjectionErrorV1::UnresolvedCallableEffect {
                block,
                source: Box::new(source),
                callee: call.callee().index(),
                tail: false,
            }),
        }
    }

    pub(super) fn accept_pending_source_call_v18(
        &mut self,
        pending: PendingSourceCallV18<'_>,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        match self.facts.as_deref_mut() {
            Some(facts) => facts.accept_pending_source_call_v18(pending),
            None => Err(ProductionRankedProjectionErrorV1::Incomplete(
                "pending source storage has no original source facts scope",
            )),
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

    pub(super) fn new_with_facts_v18(
        count: usize,
        facts: &'a mut dyn ProjectedAssertionFactsV1,
    ) -> Result<Self, ProductionRankedProjectionErrorV1> {
        if facts.projection_meter_v18().is_none() {
            return Ok(Self::new(count, Some(facts)));
        }
        let (locals, queried) = {
            let mut allocation = source_ranked_consumer_resources_v18::ProjectionAllocationV18::from_facts(facts)?;
            allocation.header::<Self>()?;
            allocation.header::<Result<Self, ProductionRankedProjectionErrorV1>>()?;
            (allocation.optional(count)?, allocation.empty()?)
        };
        Ok(Self {
            locals, queried, scalar_private_singletons: &[], scalar_private_borrows: None,
            facts: Some(facts), site: None, source_start: 0, guarded_start: 0, next_access: 0,
        })
    }

    pub(super) fn with_allocation_v18<T>(
        &mut self,
        action: impl FnOnce(&mut source_ranked_consumer_resources_v18::ProjectionAllocationV18<'_>)
            -> Result<T, ProductionRankedProjectionErrorV1>,
    ) -> Result<T, ProductionRankedProjectionErrorV1> {
        match self.facts.as_deref_mut() {
            Some(facts) => action(&mut source_ranked_consumer_resources_v18::ProjectionAllocationV18::from_facts(facts)?),
            None => action(&mut source_ranked_consumer_resources_v18::ProjectionAllocationV18::Legacy),
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
        let completed = sources.get(self.source_start..).ok_or(
            ProductionRankedProjectionErrorV1::Unsupported("invalid slice source cursor"),
        )?;
        let mut allocation = match self.facts.as_deref_mut() {
            Some(facts) => source_ranked_consumer_resources_v18::ProjectionAllocationV18::from_facts(facts)?,
            None => source_ranked_consumer_resources_v18::ProjectionAllocationV18::Legacy,
        };
        for source in completed {
            allocation.charge(1)?;
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
            ))?;
        allocation.charge(delayed.len())?;
        let delayed = delayed.iter()
            .filter(|site| site.access.memory_space != MemorySpaceAttr::Private)
            .count();
        drop(allocation);
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
        let mut allocation = match self.facts.as_deref_mut() {
            Some(facts) => source_ranked_consumer_resources_v18::ProjectionAllocationV18::from_facts(facts)?,
            None => source_ranked_consumer_resources_v18::ProjectionAllocationV18::Legacy,
        };
        allocation.reserve(&mut self.queried, 1, false, "slice query correspondence storage cannot be reserved")?;
        allocation.push(&mut self.queried, QueriedSliceV1 {
            site: self
                .site
                .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                    "missing slice query site",
                ))?,
            ordinal,
            view,
        })?;
        Ok(())
    }

    pub(super) fn finish(self) -> ProjectedSliceQueriesV1 {
        ProjectedSliceQueriesV1(self.queried)
    }
}

pub(super) struct ProjectedSliceQueriesV1(Vec<QueriedSliceV1>);

impl ProjectedSliceQueriesV1 {
    pub(super) fn validate(
        self,
        blocks: &[ProductionRankedBlockV1],
        sources: &[ProductionRankedAccessSourceV1],
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        self.validate_core_v18(blocks, sources,
            &mut source_ranked_consumer_resources_v18::ProjectionAllocationV18::Legacy)
    }

    pub(super) fn validate_with_facts_v18(
        self,
        blocks: &[ProductionRankedBlockV1],
        sources: &[ProductionRankedAccessSourceV1],
        facts: &mut dyn ProjectedAssertionFactsV1,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        self.validate_core_v18(blocks, sources,
            &mut source_ranked_consumer_resources_v18::ProjectionAllocationV18::from_facts(facts)?)
    }

    fn validate_core_v18(
        mut self,
        blocks: &[ProductionRankedBlockV1],
        sources: &[ProductionRankedAccessSourceV1],
        allocation: &mut source_ranked_consumer_resources_v18::ProjectionAllocationV18<'_>,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        allocation.header::<Result<(), ProductionRankedProjectionErrorV1>>()?;
        let key = |query: &QueriedSliceV1| (query.site.block, query.site.statement, query.ordinal);
        allocation.sort_fixed_key(&mut self.0, key)?;
        for pair in self.0.windows(2) {
            allocation.charge(1)?;
            if key(&pair[0]) == key(&pair[1]) {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "duplicate queried slice occurrence",
                ));
            }
        }
        let mut seen = allocation.empty()?;
        allocation.reserve(&mut seen, self.0.len(), true, "slice census scratch storage cannot be reserved")?;
        allocation.charge(self.0.len())?;
        seen.resize(self.0.len(), false);
        for source in sources {
            allocation.charge(1)?;
            let wanted = (
                source.semantic_block() as usize,
                source.semantic_statement().map(|value| value as usize),
                source.semantic_access_ordinal(),
            );
            let Some(index) = allocation.find_fixed_key(&self.0, &wanted, key)? else {
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
        for present in seen {
            allocation.charge(1)?;
            if !present { return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "queried slice disappeared from the final source access census",
            )); }
        }
        Ok(())
    }
}

#[cfg(test)]
mod live_query_tests_v18 {
    use super::*;
    use fe2o3_kernel_ir::{CanonicalKernelIrWorkBudgetV1 as Work,
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget};
    use source_ranked_consumer_resources_v18::{ProjectionAllocationV18, SourceAssertionMeterV18};
    use std::mem::size_of;

    fn query() -> ProjectedSliceQueriesV1 {
        ProjectedSliceQueriesV1(vec![QueriedSliceV1 {
            site: ProjectedSemanticAccessSiteV1 { block: 3, statement: Some(7) },
            ordinal: 2, view: ProductionRankedValueIdV1::new(0),
        }])
    }

    fn blocks() -> Vec<ProductionRankedBlockV1> {
        vec![ProductionRankedBlockV1::new(vec![ProductionRankedOperationV1::Access {
            kind: AccessKindAttr::Read, view: ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0)),
            indices: vec![],
        }], ProductionRankedTerminatorV1::Return)]
    }

    #[test]
    fn live_slice_census_has_independent_exact_and_short_resource_oracles() {
        type Key = (usize, Option<usize>, u32);
        type Error = ProductionRankedProjectionErrorV1;
        let storage = 2 * size_of::<Result<(), Error>>()
            + size_of::<(usize, usize, usize, Key, Key)>()
            + size_of::<Vec<bool>>() + 1
            + size_of::<(usize, usize, usize, Key)>()
            + size_of::<Result<Option<usize>, Error>>();
        let sources = [ProductionRankedAccessSourceV1::new(3, Some(7), 2, 0, 0)];
        let blocks = blocks();
        query().validate(&blocks, &sources).unwrap();
        for (work, limit) in [(4, storage), (3, storage), (4, storage - 1)] {
            let mut ledger = Work::new(work);
            let mut budget = Budget::new(&mut ledger, 23 + limit);
            budget.reserve_storage(23).unwrap();
            let result = query().validate_core_v18(&blocks, &sources,
                &mut ProjectionAllocationV18::Source(&mut SourceAssertionMeterV18(&mut budget)));
            if work == 3 {
                assert!(result.is_err());
                assert_eq!(budget.storage(), 23 + storage);
                drop(budget);
                assert_eq!(ledger.failed_work(), Some(4));
            } else if limit < storage {
                assert!(result.is_err());
                assert_eq!(budget.failed_storage(), Some(23 + storage));
                assert_eq!(budget.work(), 2);
            } else {
                result.unwrap();
                assert_eq!((budget.work(), budget.storage()), (4, 23 + storage));
                budget.release_storage(storage).unwrap();
                assert_eq!(budget.storage(), 23);
            }
        }
    }

    #[test]
    fn live_slice_census_keeps_missing_duplicate_and_wrong_actual_view_refusals() {
        for mode in 0..4 {
            let mut query = query();
            let mut blocks = blocks();
            let mut sources = vec![ProductionRankedAccessSourceV1::new(3, Some(7), 2, 0, 0)];
            match mode {
                0 => sources.clear(),
                1 => sources.push(ProductionRankedAccessSourceV1::new(3, Some(7), 2, 0, 0)),
                2 => {
                    blocks[0] = ProductionRankedBlockV1::new(vec![ProductionRankedOperationV1::Access {
                        kind: AccessKindAttr::Read,
                        view: ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(1)),
                        indices: vec![],
                    }], ProductionRankedTerminatorV1::Return);
                }
                3 => query.0.push(QueriedSliceV1 {
                    site: ProjectedSemanticAccessSiteV1 { block: 3, statement: Some(7) },
                    ordinal: 2, view: ProductionRankedValueIdV1::new(0),
                }),
                _ => unreachable!(),
            }
            let mut ledger = Work::new(100);
            let mut budget = Budget::new(&mut ledger, 100_000);
            assert!(query.validate_core_v18(&blocks, &sources,
                &mut ProjectionAllocationV18::Source(&mut SourceAssertionMeterV18(&mut budget))).is_err());
            assert_eq!(budget.failed_storage(), None);
            drop(budget);
            assert_eq!(ledger.failed_work(), None);
        }
    }
}
