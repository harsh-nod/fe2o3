fn index_ranked_correlation(
    lowering: &ProductionRankedKernelLoweringInputV1,
    sources: &[ProductionRankedAccessSourceV1],
    max_operations: usize,
    budget: &mut UnsupportedIndexCorrelationBudgetV1,
) -> Option<RankedCorrelationIndexV1> {
    if sources.len() > DEFAULT_MAX_OPERATIONS_V1 {
        return None;
    }
    let mut operation_count = 0_usize;
    let mut view_definitions = BTreeMap::new();
    let mut semantic_expressions = BTreeMap::new();
    for block in lowering.kernel().blocks() {
        budget.charge()?;
        for operation in block.operations() {
            operation_count = operation_count.checked_add(1)?;
            if operation_count > max_operations {
                return None;
            }
            budget.charge()?;
            let definition = match operation {
                ProductionRankedOperationV1::View {
                    result,
                    allocation_origin,
                    ..
                } => Some((
                    *result,
                    RankedViewDefinitionV1 {
                        allocation_origin: *allocation_origin,
                        memory_space: dialect_kernel::MemorySpaceAttr::Global,
                        noalias_class: 0,
                    },
                )),
                ProductionRankedOperationV1::ViewInSpace {
                    result,
                    memory_space,
                    allocation_origin,
                    ..
                } => Some((
                    *result,
                    RankedViewDefinitionV1 {
                        allocation_origin: *allocation_origin,
                        memory_space: *memory_space,
                        noalias_class: 0,
                    },
                )),
                _ => None,
            };
            if let Some((result, definition)) = definition {
                budget.charge()?;
                if view_definitions.insert(result, definition).is_some() {
                    return None;
                }
            }
            if let ProductionRankedOperationV1::SemanticExpression {
                result,
                expression,
                numerical_contract,
            } = operation
            {
                budget.charge()?;
                if semantic_expressions
                    .insert(*result, (expression.clone(), *numerical_contract))
                    .is_some()
                {
                    return None;
                }
            }
        }
    }

    let mut ranked_locations = BTreeSet::new();
    let mut sites_by_ranked_location = BTreeMap::new();
    let mut source_ordinals = BTreeMap::<(u32, Option<u32>), BTreeSet<u32>>::new();
    let mut sources_by_site = BTreeMap::new();
    let mut conservative_sources_by_statement = BTreeMap::new();
    let mut ambiguous_conservative_statements = BTreeSet::new();
    for source in sources {
        budget.charge()?;
        let operation = lowering
            .kernel()
            .blocks()
            .get(source.ranked_block as usize)?
            .operations()
            .get(source.ranked_operation as usize)?;
        let (access, allocation, value, atomic) = match operation {
            ProductionRankedOperationV1::Access { kind, view, .. } => {
                (*kind, IndexedRankedAllocationV1::View(*view), None, None)
            }
            ProductionRankedOperationV1::PredicatedAccess { kind, view, .. } => {
                (*kind, IndexedRankedAllocationV1::View(*view), None, None)
            }
            ProductionRankedOperationV1::ValueAccess {
                kind, view, value, ..
            } => (
                *kind,
                IndexedRankedAllocationV1::View(*view),
                Some(*value),
                None,
            ),
            ProductionRankedOperationV1::AtomicAccess {
                kind,
                ordering,
                scope,
                view,
                ..
            } => (
                *kind,
                IndexedRankedAllocationV1::View(*view),
                None,
                Some(normalize_ranked_atomic_contract_v1(*ordering, *scope)),
            ),
            ProductionRankedOperationV1::PublicationAtomicStoreU32 { view, .. } => (
                dialect_kernel::AccessKindAttr::AtomicWrite,
                IndexedRankedAllocationV1::View(*view),
                None,
                Some(normalize_ranked_atomic_contract_v1(
                    dialect_kernel::AtomicOrderingAttr::Release,
                    dialect_kernel::AtomicScopeAttr::System,
                )),
            ),
            ProductionRankedOperationV1::PublicationAtomicLoadU32 { view, .. } => (
                dialect_kernel::AccessKindAttr::AtomicRead,
                IndexedRankedAllocationV1::View(*view),
                None,
                Some(normalize_ranked_atomic_contract_v1(
                    dialect_kernel::AtomicOrderingAttr::Acquire,
                    dialect_kernel::AtomicScopeAttr::System,
                )),
            ),
            ProductionRankedOperationV1::AtomicValueAccess {
                kind,
                ordering,
                scope,
                view,
                value,
                ..
            } => (
                *kind,
                IndexedRankedAllocationV1::View(*view),
                Some(*value),
                Some(normalize_ranked_atomic_contract_v1(*ordering, *scope)),
            ),
            ProductionRankedOperationV1::AllocationEffect {
                kind,
                memory_space,
                allocation_origin,
                noalias_class,
            } => (
                *kind,
                IndexedRankedAllocationV1::Direct(RankedViewDefinitionV1 {
                    allocation_origin: *allocation_origin,
                    memory_space: *memory_space,
                    noalias_class: *noalias_class,
                }),
                None,
                None,
            ),
            _ => return None,
        };
        if !ranked_locations.insert((source.ranked_block, source.ranked_operation))
            || !source_ordinals
                .entry((source.semantic_block, source.semantic_statement))
                .or_default()
                .insert(source.semantic_access_ordinal)
        {
            return None;
        }
        let site = SemanticAccessSiteV1 {
            block: source.semantic_block,
            statement: source.semantic_statement,
            ordinal: source.semantic_access_ordinal,
        };
        if sites_by_ranked_location
            .insert((source.ranked_block, source.ranked_operation), site)
            .is_some()
        {
            return None;
        }
        let indexed = IndexedRankedAccessSourceV1 {
            ranked_block: source.ranked_block,
            ranked_operation: source.ranked_operation,
            access,
            allocation,
            value,
            atomic,
        };
        if matches!(allocation, IndexedRankedAllocationV1::Direct(_)) {
            let key = (site.block, site.statement);
            if !ambiguous_conservative_statements.contains(&key)
                && conservative_sources_by_statement
                    .insert(key, indexed)
                    .is_some()
            {
                conservative_sources_by_statement.remove(&key);
                ambiguous_conservative_statements.insert(key);
            }
        }
        if sources_by_site.insert(site, indexed).is_some() {
            return None;
        }
    }
    for ordinals in source_ordinals.values() {
        budget.charge()?;
        if !ordinals
            .iter()
            .copied()
            .eq(0..u32::try_from(ordinals.len()).unwrap_or(u32::MAX))
        {
            return None;
        }
    }
    Some(RankedCorrelationIndexV1 {
        sources_by_site,
        conservative_sources_by_statement,
        sites_by_ranked_location,
        view_definitions,
        semantic_expressions,
    })
}
