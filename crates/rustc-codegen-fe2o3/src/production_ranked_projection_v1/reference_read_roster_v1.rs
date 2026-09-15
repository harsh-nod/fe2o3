//! Read facts retained by the existing source replay, independent of RHS use.

use super::*;
use std::cell::Cell;

type JoinError = crate::production_reference_effect_join_v2::ProductionReferenceEffectJoinErrorV2;
type Write = crate::production_reference_effect_join_v2::RankedGpuWriteV2;
type Access = ProductionRankedAccessSourceV1;
type Generated = ProductionRankedExecutableEffectSourceV1;

#[derive(Default)]
pub(crate) struct ReferenceReadRosterV1 {
    reads: Vec<(ProjectedAccessSourceV1, ProductionSemanticLoadV2)>,
    work: Cell<usize>,
    reserved: Option<usize>,
}

impl ReferenceReadRosterV1 {
    pub(super) fn reserve(
        &mut self,
        count: usize,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        if self.reserved.is_some() {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "reference read source reservation cannot be replaced",
            ));
        }
        self.charge(count)
            .map_err(ProductionRankedProjectionErrorV1::Incomplete)?;
        self.reads.try_reserve_exact(count).map_err(|_| {
            ProductionRankedProjectionErrorV1::Incomplete(
                "reference read storage cannot be reserved",
            )
        })?;
        self.reserved = Some(count);
        Ok(())
    }

    pub(crate) fn charge(&self, amount: usize) -> Result<(), &'static str> {
        let next = self
            .work
            .get()
            .checked_add(amount)
            .ok_or("reference read roster exceeds the existing reference work limit")?;
        if next > crate::reference_effect_v1::MAX_REFERENCE_STATEMENTS_V1 {
            return Err("reference read roster exceeds the existing reference work limit");
        }
        self.work.set(next);
        Ok(())
    }

    pub(super) fn record(
        &mut self,
        source: &ProjectedAccessSourceV1,
        load: &ProductionSemanticLoadV2,
        readonly: bool,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        // Mutable loads keep their existing reaching-version resolver. They are
        // never introduced as initial-memory leaves by this new lookup.
        if !readonly {
            return Ok(());
        }
        self.charge(1 + load.indices.len())
            .map_err(ProductionRankedProjectionErrorV1::Incomplete)?;
        if source.access != AccessKindAttr::Read
            || source.memory_space != MemorySpaceAttr::Global
            || source.semantic_site.is_none()
            || (source.block, source.operation) != (load.block as usize, load.operation as usize)
        {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "reference read lost its exact projected source occurrence",
            ));
        }
        if self.reserved.is_none_or(|limit| self.reads.len() >= limit) {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "reference reads exceed their source reservation",
            ));
        }
        self.reads.push((*source, load.clone()));
        Ok(())
    }

    pub(crate) fn loads(&self) -> impl Iterator<Item = &ProductionSemanticLoadV2> {
        self.reads.iter().map(|(_, load)| load)
    }

    fn validate_original_sources(
        &self,
        sources: &[ProjectedAccessSourceV1],
    ) -> Result<(), &'static str> {
        // Index only retained read sites. Non-observable source rows keep their
        // existing treatment; equality below still checks the entire provenance.
        let mut expected = BTreeMap::new();
        for (source, _) in &self.reads {
            self.charge(2)?; // One potential entry and one indexed insertion.
            if expected
                .insert((source.block, source.operation), (source, 0_usize))
                .is_some()
            {
                return Err("reference read roster contains a duplicate occurrence");
            }
        }
        for candidate in sources {
            self.charge(1)?;
            if let Some((source, count)) = expected.get_mut(&(candidate.block, candidate.operation))
                && **source == *candidate
            {
                // Only zero, one, or duplicate matters; no unbounded counter.
                *count = (*count + 1).min(2);
            }
        }
        self.charge(expected.len())?;
        if expected.values().any(|(_, count)| *count != 1) {
            return Err("reference read provenance is absent or duplicated in this root");
        }
        Ok(())
    }

    fn validate(
        &self,
        kernel: &ProductionRankedKernelV1,
        accesses: &[Access],
        generated: &[Generated],
    ) -> Result<(), &'static str> {
        // The complete pre-remap roster has one owner and one occurrence key.
        let mut sites = BTreeMap::new();
        let mut origins = BTreeSet::new();
        for access in accesses {
            self.charge(4)?; // Two potential entries and two indexed insertions.
            let site = (access.ranked_block(), access.ranked_operation());
            if sites.insert(site, (Some(access), false)).is_some()
                || !origins.insert((
                    access.semantic_block(),
                    access.semantic_statement(),
                    access.semantic_access_ordinal(),
                ))
            {
                return Err("reference source roster contains a duplicate occurrence");
            }
        }
        for source in generated {
            self.charge(2)?;
            if sites
                .insert(
                    (source.ranked_block(), source.ranked_operation()),
                    (None, false),
                )
                .is_some()
            {
                return Err("reference source roster contains a duplicate occurrence");
            }
        }
        let mut views = BTreeMap::new();
        let mut private_views = BTreeSet::new();
        // Classify views before accesses: stored block order is not CFG order.
        for block in kernel.blocks() {
            self.charge(block.operations().len())?;
            for operation in block.operations() {
                match operation {
                    ProductionRankedOperationV1::View {
                        result,
                        allocation_origin,
                        writable,
                        ..
                    }
                    | ProductionRankedOperationV1::ViewInSpace {
                        result,
                        allocation_origin,
                        writable,
                        memory_space: MemorySpaceAttr::Global,
                        ..
                    } => {
                        self.charge(2)?;
                        if views
                            .insert(
                                ProductionRankedValueV1::Local(*result),
                                (*allocation_origin, *writable),
                            )
                            .is_some()
                        {
                            return Err("reference read view is multiply defined");
                        }
                    }
                    ProductionRankedOperationV1::ViewInSpace {
                        result,
                        memory_space: MemorySpaceAttr::Private,
                        ..
                    } => {
                        self.charge(2)?;
                        private_views.insert(ProductionRankedValueV1::Local(*result));
                    }
                    _ => {}
                }
            }
        }
        for (block_index, block) in kernel.blocks().iter().enumerate() {
            self.charge(block.operations().len())?;
            for (operation_index, operation) in block.operations().iter().enumerate() {
                if matches!(
                    operation,
                    ProductionRankedOperationV1::Access { .. }
                        | ProductionRankedOperationV1::PredicatedAccess { .. }
                ) {
                    self.charge(1)?; // Private-view membership query.
                }
                if reference_observable_access(operation, &private_views) {
                    self.charge(1)?;
                    let (_, seen) = sites
                        .get_mut(&(block_index as u32, operation_index as u32))
                        .ok_or("reference source roster is incomplete")?;
                    if *seen {
                        return Err("reference source roster contains a duplicate occurrence");
                    }
                    *seen = true;
                }
            }
        }
        self.charge(sites.len())?;
        if sites.values().any(|(_, seen)| !seen) {
            return Err("reference source roster contains an absent access");
        }
        let mut seen = BTreeSet::new();
        for (source, load) in &self.reads {
            // Seen-entry storage/insertion, source lookup, view lookup, direct
            // ranked operation lookup, then exact index comparison.
            self.charge(5 + load.indices.len())?;
            if !seen.insert((load.block, load.operation)) {
                return Err("reference read roster contains a duplicate occurrence");
            }
            let site = source
                .semantic_site
                .ok_or("reference read source site is absent")?;
            let access = sites
                .get(&(load.block, load.operation))
                .and_then(|(access, _)| *access)
                .ok_or("reference read has no retained access correspondence")?;
            if access.semantic_block() as usize != site.block
                || access.semantic_statement().map(|value| value as usize) != site.statement
                || (source.block, source.operation)
                    != (load.block as usize, load.operation as usize)
                || views.get(&load.view) != Some(&(load.allocation_origin, false))
                || !matches!(kernel.blocks().get(load.block as usize)
                    .and_then(|block| block.operations().get(load.operation as usize)),
                    Some(ProductionRankedOperationV1::Access { kind: AccessKindAttr::Read, view, indices })
                        if *view == load.view && indices.as_slice() == load.indices.as_ref())
            {
                return Err("reference read differs from its retained source or ranked access");
            }
        }
        Ok(())
    }
}

fn reference_observable_access(
    operation: &ProductionRankedOperationV1,
    private_views: &BTreeSet<ProductionRankedValueV1>,
) -> bool {
    if matches!(operation,
        ProductionRankedOperationV1::Access { view, .. }
        | ProductionRankedOperationV1::PredicatedAccess { view, .. } if private_views.contains(view))
    {
        return false;
    }
    matches!(
        operation,
        ProductionRankedOperationV1::Access { .. }
            | ProductionRankedOperationV1::PredicatedAccess { .. }
            | ProductionRankedOperationV1::ValueAccess { .. }
            | ProductionRankedOperationV1::AtomicAccess { .. }
            | ProductionRankedOperationV1::AtomicValueAccess { .. }
            | ProductionRankedOperationV1::AllocationEffect { .. }
    )
}

/// Move-only custody: no separately supplied graph or source roster at the join.
pub(crate) struct ProjectedReferenceInputV2 {
    kernel: ProductionRankedKernelV1,
    source_function: SemanticFunctionIdentityV1,
    writes: Vec<Write>,
    reads: ReferenceReadRosterV1,
    accesses: Vec<Access>,
    generated: Vec<Generated>,
}

impl ProjectedReferenceInputV2 {
    pub(super) fn new(
        kernel: ProductionRankedKernelV1,
        source_function: SemanticFunctionIdentityV1,
        writes: Vec<Write>,
        reads: ReferenceReadRosterV1,
        sources: &[ProjectedAccessSourceV1],
        accesses: Vec<Access>,
        generated: Vec<Generated>,
    ) -> Result<Self, ProductionRankedProjectionErrorV1> {
        reads
            .charge(sources.len())
            .map_err(ProductionRankedProjectionErrorV1::Incomplete)?;
        if production_access_sources(kernel.blocks(), sources)? != accesses {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "reference source roster differs from the original access projection",
            ));
        }
        reads
            .validate_original_sources(sources)
            .map_err(ProductionRankedProjectionErrorV1::Incomplete)?;
        reads
            .validate(&kernel, &accesses, &generated)
            .map_err(ProductionRankedProjectionErrorV1::Incomplete)?;
        Ok(Self {
            kernel,
            source_function,
            writes,
            reads,
            accesses,
            generated,
        })
    }

    pub(crate) fn into_parts(
        self,
        bindings: &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1,
    ) -> Result<
        (
            ProductionRankedKernelV1,
            Vec<Write>,
            ReferenceReadRosterV1,
            Vec<Access>,
            Vec<Generated>,
        ),
        JoinError,
    > {
        let [binding] = bindings.as_slice() else {
            return Err(JoinError::BindingCount(bindings.as_slice().len()));
        };
        if !binding.kernel.has_function_identity_v1(self.source_function) {
            return Err(JoinError::UnsupportedReference(
                "reference read roster belongs to another kernel root",
            ));
        }
        Ok((
            self.kernel,
            self.writes,
            self.reads,
            self.accesses,
            self.generated,
        ))
    }

    #[cfg(test)]
    pub(crate) fn fixture(
        kernel: ProductionRankedKernelV1,
        source_function: [u8; 32],
        writes: Vec<Write>,
        loads: Vec<ProductionSemanticLoadV2>,
        accesses: Vec<Access>,
    ) -> Result<Self, ProductionRankedProjectionErrorV1> {
        let mut reads = ReferenceReadRosterV1::default();
        reads.reserve(loads.len())?;
        for load in &loads {
            let access = accesses
                .iter()
                .find(|access| {
                    (access.ranked_block(), access.ranked_operation())
                        == (load.block, load.operation)
                })
                .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                    "fixture read correspondence missing",
                ))?;
            reads.record(
                &ProjectedAccessSourceV1 {
                    block: load.block as usize,
                    operation: load.operation as usize,
                    access: AccessKindAttr::Read,
                    memory_space: MemorySpaceAttr::Global,
                    source: SemanticSourceProvenanceV1::unavailable(),
                    semantic_site: Some(ProjectedSemanticAccessSiteV1 {
                        block: access.semantic_block() as usize,
                        statement: access.semantic_statement().map(|value| value as usize),
                    }),
                },
                load,
                true,
            )?;
        }
        let sources = accesses
            .iter()
            .map(|access| ProjectedAccessSourceV1 {
                block: access.ranked_block() as usize,
                operation: access.ranked_operation() as usize,
                access: if loads.iter().any(|load| {
                    (load.block, load.operation)
                        == (access.ranked_block(), access.ranked_operation())
                }) {
                    AccessKindAttr::Read
                } else {
                    match &kernel.blocks()[access.ranked_block() as usize].operations()
                        [access.ranked_operation() as usize]
                    {
                        ProductionRankedOperationV1::Access { kind, .. } => *kind,
                        _ => panic!("component fixture only supplies plain accesses"),
                    }
                },
                memory_space: MemorySpaceAttr::Global,
                source: SemanticSourceProvenanceV1::unavailable(),
                semantic_site: Some(ProjectedSemanticAccessSiteV1 {
                    block: access.semantic_block() as usize,
                    statement: access.semantic_statement().map(|value| value as usize),
                }),
            })
            .collect::<Vec<_>>();
        Self::new(
            kernel,
            SemanticFunctionIdentityV1::from_sha256(source_function),
            writes,
            reads,
            &sources,
            accesses,
            Vec::new(),
        )
    }
}

#[cfg(test)]
#[path = "reference_read_index_tests.rs"]
mod index_tests;
