// Private to source-replayed translation validation. This records one actual
// conditional memory consumer, never authority to remove its true-path read.
#[derive(Clone, Copy)]
struct AuthenticatedConditionalReadV1 {
    location: FunctionOperationLocation,
    operation_access_ordinal: u32,
    site: SemanticAccessSiteV1,
}

impl AuthenticatedConditionalReadV1 {
    fn key(self) -> Option<(u32, u64)> {
        let operation = u32::try_from(self.location.operation_index).ok()?;
        Some((
            self.location.block.0,
            (u64::from(operation) << 32) | u64::from(self.operation_access_ordinal),
        ))
    }

    fn matches(self, block: u32, order: u64, site: SemanticAccessSiteV1) -> bool {
        self.site == site && self.key() == Some((block, order))
    }
}

fn public_effect_counts_by_call_v1(
    kir: &KirCorrelationIndexV1<'_>,
    sites: &BTreeMap<(FunctionOperationLocation, u32), SemanticAccessSiteV1>,
    budget: &mut UnsupportedIndexCorrelationBudgetV1,
) -> Result<BTreeMap<(u32, Option<u32>), usize>, ProductionMirPlironTranslationErrorV1> {
    let mut counts = BTreeMap::new();
    for consumer in &kir.memory_consumers {
        budget
            .charge()
            .ok_or(ProductionMirPlironTranslationErrorV1::ResourceLimit)?;
        if consumer.memory_space == dialect_kernel::MemorySpaceAttr::Private {
            continue;
        }
        let Some(site) = sites.get(&(consumer.location, consumer.operation_access_ordinal)) else {
            // The existing attribution check rejects this consumer below.
            continue;
        };
        let count = counts
            .entry((site.block, site.statement))
            .or_insert(0_usize);
        *count = count
            .checked_add(1)
            .ok_or(ProductionMirPlironTranslationErrorV1::ResourceLimit)?;
    }
    Ok(counts)
}

fn authenticate_conditional_total_read_v1(
    semantic: Option<&AdmittedInertSemanticMirV1>,
    semantic_function: SemanticFunctionIdV1,
    site: SemanticAccessSiteV1,
    logical_site: SemanticAccessSiteV1,
    consumer: KirMemoryConsumerV1,
    operation: &Operation,
    call_effect_count: usize,
) -> Option<AuthenticatedConditionalReadV1> {
    if site != logical_site
        || site.statement.is_some()
        || site.ordinal != 0
        || consumer.operation_access_ordinal != 0
        || call_effect_count != 1
        || consumer.access != dialect_kernel::AccessKindAttr::Read
        || consumer.memory_space != dialect_kernel::MemorySpaceAttr::Global
        || consumer.atomic.is_some()
        || operation.results.len() != 1
    {
        return None;
    }
    let semantic = semantic?;
    let function = semantic
        .functions()
        .get(semantic_function.index() as usize)?;
    let block = function.blocks().get(site.block as usize)?;
    let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
        return None;
    };
    if call.arguments().len() != 4 || call.destination().is_none() {
        return None;
    }
    if !matches!(
        semantic.callables().get(call.callee().index() as usize),
        Some(SemanticCallableDeclV1::CompilerIntrinsic {
            operation: SemanticCompilerIntrinsicOperationV1::StridedReadView2DLoadOr { .. },
            ..
        })
    ) {
        return None;
    }
    let OperationKind::GuardedLoad {
        pointer, access, ..
    } = &operation.kind
    else {
        return None;
    };
    if *pointer != consumer.pointer || access.address_space != AddressSpace::Global {
        return None;
    }
    // The sole production callers either just generated this complete KIR
    // from verified source SSA or compared module/correspondence/canonical
    // bytes with that exact re-lowering. Predicate, address and fallback are
    // authenticated there, not inferred from the opcode or a nominal site.
    Some(AuthenticatedConditionalReadV1 {
        location: consumer.location,
        operation_access_ordinal: consumer.operation_access_ordinal,
        site,
    })
}

// Collect every possible first event in a block suffix. An authenticated
// conditional read contributes its true event AND its no-read continuation.
// The first mandatory event stops traversal of that path.
fn append_effect_prefix_v1(
    block: u32,
    after_operation: Option<u64>,
    events: &BTreeMap<u32, Vec<(u64, SemanticAccessSiteV1)>>,
    conditional_reads: &BTreeMap<(u32, u64), AuthenticatedConditionalReadV1>,
    found: &mut BTreeSet<SemanticAccessSiteV1>,
    budget: &mut UnsupportedIndexCorrelationBudgetV1,
) -> Option<bool> {
    let Some(events) = events.get(&block) else {
        return Some(true);
    };
    for &(order, site) in events {
        budget.charge()?;
        if after_operation.is_some_and(|after| order <= after) {
            continue;
        }
        found.insert(site);
        if !conditional_reads
            .get(&(block, order))
            .is_some_and(|witness| witness.matches(block, order, site))
        {
            return Some(false);
        }
    }
    Some(true)
}
