//! Shared checked-reference data preparation. Paid results remain UNJOINED:
//! no owner/inventory join, actual GuardedAccess roster, readiness or admission.
//! The separate S5A continuation below joins the same-owner guard roster only;
//! it still provides no memory sites, CheckedReferencesV1 or normal admission.
use super::bf16_nominal_preparation_resources_v1::{PreparationResourcesV1, resource};
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, CanonicalKernelIrWorkLedgerIdentityV1,
};
use std::mem::size_of;

/// Plain intermediate data, not CheckedReferencesV1. Its source-call ordinals
/// must be joined to the actual constructed guarded-access vector later.
pub(super) struct UnjoinedReferenceOriginPayloadV1 {
    pub(super) definitions: Vec<u8>,
    pub(super) origins: Vec<Option<CheckedReferenceOriginV1>>,
    pub(super) fifo: Vec<usize>,
    pub(super) cursor: usize,
}
impl UnjoinedReferenceOriginPayloadV1 {
    const fn empty() -> Self {
        Self {
            definitions: Vec::new(),
            origins: Vec::new(),
            fifo: Vec::new(),
            cursor: 0,
        }
    }
}
/// OUTER physical owner. Keep through all enclosing facts/rich postflights and
/// drop payload/error/panic values BEFORE releasing only its accepted credits.
/// No refund, retry, replacement or checked-ready constructor.
pub(super) struct PendingUnjoinedReferenceOriginsV1 {
    payload: Option<UnjoinedReferenceOriginPayloadV1>,
    ledger: Option<(usize, CanonicalKernelIrWorkLedgerIdentityV1)>,
    completed: bool,
}
impl PendingUnjoinedReferenceOriginsV1 {
    pub(super) const fn new() -> Self {
        Self {
            payload: None,
            ledger: None,
            completed: false,
        }
    }
    #[cfg(test)]
    pub(super) fn payload_for_test(&self) -> Option<&UnjoinedReferenceOriginPayloadV1> {
        self.payload.as_ref()
    }
    #[cfg(test)]
    pub(super) fn completed_for_test(&self) -> bool {
        self.completed
    }
    #[cfg(test)]
    pub(super) fn has_ledger_for_test(&self) -> bool {
        self.ledger.is_some()
    }
}

/// Ordinary queue retains its original VecDeque allocation behavior.
/// Paid queue retains every consumed entry in OUTER storage, preserving FIFO
/// order while never freeing/refunding partial queue payload on refusal.
enum OriginQueueV1<'a> {
    Legacy(&'a mut VecDeque<usize>),
    Paid {
        values: &'a mut Vec<usize>,
        cursor: &'a mut usize,
    },
}
impl OriginQueueV1<'_> {
    fn push(
        &mut self,
        value: usize,
        resources: &mut PreparationResourcesV1<'_, '_>,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        match self {
            Self::Legacy(queue) => queue.push_back(value),
            Self::Paid { values, .. } => {
                resources.work(16)?;
                resources.push(values, value)?;
            }
        }
        Ok(())
    }
    fn pop(
        &mut self,
        resources: &mut PreparationResourcesV1<'_, '_>,
    ) -> Result<Option<usize>, ProductionRankedProjectionErrorV1> {
        match self {
            Self::Legacy(queue) => Ok(queue.pop_front()),
            Self::Paid { values, cursor } => {
                resources.work(16)?;
                let value = values.get(**cursor).copied();
                if value.is_some() {
                    **cursor += 1;
                }
                Ok(value)
            }
        }
    }
}

pub(super) fn local_definition_counts_legacy_v1(function: &SemanticFunctionDeclV1) -> Vec<u8> {
    // Same ordinary allocation expression. The legacy adapter cannot deny work,
    // and this loop has no arithmetic/allocation/fallible semantic operation.
    let mut definitions = vec![0_u8; function.locals().len()];
    populate_definition_counts_v1(
        function,
        &mut definitions,
        &mut PreparationResourcesV1::unmetered(),
    )
    .expect("unmetered definition visitor cannot refuse");
    definitions
}

pub(super) fn checked_reference_origins_legacy_v1(
    function: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
    guarded_access_count: usize,
    edges_by_source: &[Vec<CapabilityEdgeV1>],
    option_dominance: &SemanticOptionDominanceV1,
    enum_payload_dominance: &SemanticEnumPayloadDominanceV1,
) -> Result<Vec<Option<CheckedReferenceOriginV1>>, ProductionRankedProjectionErrorV1> {
    let definitions = local_definition_counts_legacy_v1(function);
    let mut origins = vec![None; function.locals().len()];
    let mut worklist = VecDeque::new();
    populate_origins_v1(
        function,
        callables,
        guarded_access_count,
        edges_by_source,
        option_dominance,
        enum_payload_dominance,
        &definitions,
        &mut origins,
        &mut OriginQueueV1::Legacy(&mut worklist),
        &mut PreparationResourcesV1::unmetered(),
    )?;
    Ok(origins)
}

/// Paid component DATA only. Raw callable/count/graph inputs confer no source or
/// guarded-access identity. Do not feed this payload into a checked-ready
/// consumer: actual owner/function/graph/guard-roster joins are still absent.
/// The exact count consistency check is retained to share the ordinary algorithm.
#[allow(clippy::too_many_arguments)]
pub(super) fn prepare_unjoined_reference_origins_v1(
    function: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
    guarded_access_count: usize,
    edges_by_source: &[Vec<CapabilityEdgeV1>],
    option_dominance: &SemanticOptionDominanceV1,
    enum_payload_dominance: &SemanticEnumPayloadDominanceV1,
    pending: &mut PendingUnjoinedReferenceOriginsV1,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    if resources.has_denial() {
        return Err(resource(Resource::Accounting));
    }
    let ledger =
        resources
            .original_ledger_v1()
            .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                "unjoined reference data requires an original ledger",
            ))?;
    resources.work(64)?;
    if pending.ledger.is_some_and(|saved| saved != ledger) {
        return Err(resource(Resource::Accounting));
    }
    if pending.payload.is_some() || pending.ledger.is_some() || pending.completed {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "unjoined reference pending owner cannot be replaced or retried",
        ));
    }
    let frame = 8192usize
        .checked_add(size_of::<PendingUnjoinedReferenceOriginsV1>())
        .and_then(|n| n.checked_add(size_of::<UnjoinedReferenceOriginPayloadV1>()))
        .and_then(|n| n.checked_add(size_of::<OriginQueueV1<'static>>()))
        .ok_or_else(|| resource(Resource::Arithmetic))?;
    resources.work(frame)?;
    resources.reserve_storage(frame)?;
    // Install empty OUTER storage before the first allocation. Any later partial
    // vector remains retained on error/unwind until its owner's final postflight.
    pending.ledger = Some(ledger);
    pending.payload = Some(UnjoinedReferenceOriginPayloadV1::empty());
    let payload = pending
        .payload
        .as_mut()
        .expect("outer payload just installed");
    let locals = function.locals().len();
    resources.work(locals)?;
    resources.reserve(&mut payload.definitions, locals)?;
    payload.definitions.resize(locals, 0);
    populate_definition_counts_v1(function, &mut payload.definitions, resources)?;
    resources.work(locals)?;
    resources.reserve(&mut payload.origins, locals)?;
    payload.origins.resize(locals, None);
    populate_origins_v1(
        function,
        callables,
        guarded_access_count,
        edges_by_source,
        option_dominance,
        enum_payload_dominance,
        &payload.definitions,
        &mut payload.origins,
        &mut OriginQueueV1::Paid {
            values: &mut payload.fifo,
            cursor: &mut payload.cursor,
        },
        resources,
    )?;
    if resources.has_denial() || resources.original_ledger_v1() != Some(ledger) {
        return Err(resource(Resource::Accounting));
    }
    pending.completed = true; // Data completion, NEVER readiness/admission.
    Ok(())
}

fn populate_definition_counts_v1(
    function: &SemanticFunctionDeclV1,
    definitions: &mut [u8],
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    let mut record = |place: &SemanticPlaceV1| {
        if let Some(slot) =
            local_definition_index(place).and_then(|local| definitions.get_mut(local))
        {
            *slot = slot.saturating_add(1);
        }
    };
    for block in function.blocks() {
        resources.work(32)?;
        for statement in block.statements() {
            // At most two constant-time definition visits, including atomic address.
            resources.work(64)?;
            visit_statement_definition_places(statement.kind(), &mut record);
        }
        if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind()
            && let Some(destination) = call.destination()
        {
            resources.work(64)?;
            record(destination.place());
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn populate_origins_v1(
    function: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
    guarded_access_count: usize,
    edges_by_source: &[Vec<CapabilityEdgeV1>],
    option_dominance: &SemanticOptionDominanceV1,
    enum_payload_dominance: &SemanticEnumPayloadDominanceV1,
    definitions: &[u8],
    origins: &mut [Option<CheckedReferenceOriginV1>],
    queue: &mut OriginQueueV1<'_>,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    for block in function.blocks() {
        resources.work(32)?;
        for statement in block.statements() {
            resources.work(64)?;
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                continue;
            };
            let destination = assignment.destination();
            if !destination.projections().is_empty()
                || definitions
                    .get(destination.local().index() as usize)
                    .copied()
                    != Some(1)
            {
                continue;
            }
            let SemanticRvalueKindV1::Borrow { kind, place } = assignment.value().kind() else {
                continue;
            };
            if resources.is_metered() {
                let units = place
                    .projections()
                    .len()
                    .checked_mul(4)
                    .and_then(|n| n.checked_add(16))
                    .ok_or_else(|| resource(Resource::Arithmetic))?;
                resources.work(units)?;
            }
            if !matches!(kind, SemanticBorrowKindV1::Shared)
                || !place.projections().iter().any(|projection| {
                    matches!(
                        projection.kind(),
                        SemanticProjectionKindV1::Index(_)
                            | SemanticProjectionKindV1::ConstantIndex { .. }
                    )
                })
            {
                continue;
            }
            let destination = destination.local().index() as usize;
            origins[destination] = Some(CheckedReferenceOriginV1 {
                source: CheckedReferenceSourceV1::ProjectedSharedBorrow,
                availability: None,
            });
            queue.push(destination, resources)?;
        }
    }

    let mut access = 0_usize;
    for block in function.blocks() {
        resources.work(32)?;
        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
            continue;
        };
        resources.work(128)?; // Fixed callable/destination and Option lookup.
        if !matches!(
            callables.get(call.callee().index() as usize),
            Some(SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut { .. }
                    | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetDisjointMut { .. }
                    | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMutExclusive { .. }
                    | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetBlockMut { .. }
                    | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetTiled2dMut { .. }
                    | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetRowStriped2dMut { .. },
                ..
            })
        ) {
            continue;
        }
        let destination = simple_call_destination(call)?;
        if definitions.get(destination.index() as usize).copied() != Some(1) {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a checked disjoint result without one exact definition",
            ));
        }
        let destination = destination.index() as usize;
        let availability = option_dominance
            .availability(SemanticLocalIdV1::from_index(destination as u32))
            .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                "a checked disjoint result without exact Option Some availability",
            ))?;
        if origins[destination].is_some() {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a checked disjoint result with a conflicting reference origin",
            ));
        }
        origins[destination] = Some(CheckedReferenceOriginV1 {
            source: CheckedReferenceSourceV1::GuardedAccess(access),
            availability: Some(CapabilityAvailabilityV1::Option(availability)),
        });
        queue.push(destination, resources)?;
        access += 1;
    }
    if access != guarded_access_count {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "checked disjoint access inventory changed during projection",
        ));
    }
    while let Some(source) = queue.pop(resources)? {
        resources.work(32)?;
        let Some(origin) = origins[source] else {
            continue;
        };
        let edges =
            edges_by_source
                .get(source)
                .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                    "a checked reference source outside the capability graph",
                ))?;
        for edge in edges {
            resources.work(128)?; // Kind, interval-based availability and two bounds lookups.
            if !matches!(
                edge.kind,
                CapabilityEdgeKindV1::Alias
                    | CapabilityEdgeKindV1::AuthenticatedOptionPayload
                    | CapabilityEdgeKindV1::AuthenticatedEnumPayload { .. }
            ) {
                continue;
            }
            let authorization_block = match edge.kind {
                CapabilityEdgeKindV1::AuthenticatedEnumPayload {
                    construction_block, ..
                } => construction_block,
                _ => edge.use_block,
            };
            if !origin.availability.is_none_or(|availability| {
                capability_availability_allows(
                    option_dominance,
                    enum_payload_dominance,
                    availability,
                    SemanticBlockIdV1::from_index(authorization_block as u32),
                )
            }) {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "a checked reference is transported outside its authenticated payload region",
                ));
            }
            if definitions.get(edge.destination).copied() != Some(1) {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "a checked reference destination without one exact definition",
                ));
            }
            let projected = match edge.kind {
                CapabilityEdgeKindV1::AuthenticatedEnumPayload { availability, .. } => {
                    CheckedReferenceOriginV1 {
                        availability: Some(CapabilityAvailabilityV1::EnumPayload(availability)),
                        ..origin
                    }
                }
                _ => origin,
            };
            let slot = origins.get_mut(edge.destination).ok_or(
                ProductionRankedProjectionErrorV1::Unsupported(
                    "a checked reference destination outside the semantic local table",
                ),
            )?;
            if slot.is_none() {
                *slot = Some(projected);
                queue.push(edge.destination, resources)?;
            } else if *slot != Some(projected) {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "a checked disjoint reference with conflicting origins",
                ));
            }
        }
    }
    Ok(())
}

/// Same-assembly DATA storage, never CheckedReferencesV1 or an admission token.
/// Both the unjoined FIFO and its later source/guard joins remain OUTER owned.
pub(super) struct ActualRootReferenceOriginsStorageV1 {
    pending: PendingUnjoinedReferenceOriginsV1,
    ledger: Option<(usize, CanonicalKernelIrWorkLedgerIdentityV1)>,
    started: bool,
    completed: bool,
    pub(super) frame_credits: usize,
}
impl ActualRootReferenceOriginsStorageV1 {
    pub(super) const fn empty() -> Self {
        Self {
            pending: PendingUnjoinedReferenceOriginsV1::new(),
            ledger: None,
            started: false,
            completed: false,
            frame_credits: 0,
        }
    }
    pub(super) fn payload(&self) -> Option<&UnjoinedReferenceOriginPayloadV1> {
        if self.completed {
            self.pending.payload.as_ref()
        } else {
            None
        }
    }
    pub(super) fn completed(&self) -> bool {
        self.completed
    }
}

/// Content join only. Only the private same-owner factory lends the resulting
/// data. Raw fixtures cannot turn this helper into source-custody authority.
fn validate_actual_guarded_source_calls_v1(
    function: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
    guarded: &super::root_guarded_access_preparation_v1::RootGuardedAccessStorageV1,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    resources.work(64)?;
    if guarded.source_calls.len() != guarded.accesses.len() {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "actual reference source-call/guard roster differs",
        ));
    }
    let mut source_call_ordinal = 0usize;
    let mut guarded_access = 0usize;
    for (block_index, block) in function.blocks().iter().enumerate() {
        resources.work(32)?;
        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
            continue;
        };
        resources.work(128)?;
        let ordinal = source_call_ordinal;
        source_call_ordinal = source_call_ordinal
            .checked_add(1)
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        let Some(callable) = callables.get(call.callee().index() as usize) else {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "actual reference source callable absent",
            ));
        };
        match callable {
            SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut { .. },
                ..
            } => {}
            SemanticCallableDeclV1::CompilerIntrinsic {
                operation:
                    SemanticCompilerIntrinsicOperationV1::DisjointSliceGetDisjointMut { .. }
                    | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMutExclusive { .. }
                    | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetBlockMut { .. }
                    | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetTiled2dMut { .. }
                    | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetRowStriped2dMut { .. },
                ..
            } => {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "actual reference source exceeds identity-access profile",
                ));
            }
            _ => continue,
        }
        let destination = simple_call_destination(call)?;
        let expected = super::root_guarded_access_preparation_v1::RootGuardedSourceCallV1 {
            source_call_ordinal: ordinal,
            block: block_index,
            callee: call.callee(),
            destination,
            guarded_access,
        };
        if guarded.source_calls.get(guarded_access) != Some(&expected) {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "actual reference source-call identity differs",
            ));
        }
        let access = guarded.accesses.get(guarded_access).ok_or(
            ProductionRankedProjectionErrorV1::Incomplete("actual reference guard absent"),
        )?;
        if access.source != block.terminator().source() || access.semantic_site.is_some() {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "actual reference guard provenance or pending site differs",
            ));
        }
        guarded_access = guarded_access
            .checked_add(1)
            .ok_or_else(|| resource(Resource::Arithmetic))?;
    }
    if guarded_access != guarded.source_calls.len() {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "actual reference unobserved source-call association",
        ));
    }
    Ok(())
}

/// Strict S5A: source-call association -> actual guard -> original-ledger FIFO.
/// No memory-use semantic site, operation cursor, CFG/Final or normal admission.
#[allow(clippy::too_many_arguments)]
pub(super) fn prepare_actual_root_reference_origins_v1(
    function: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
    guarded: &super::root_guarded_access_preparation_v1::RootGuardedAccessStorageV1,
    edges_by_source: &[Vec<CapabilityEdgeV1>],
    option_dominance: &SemanticOptionDominanceV1,
    enum_payload_dominance: &SemanticEnumPayloadDominanceV1,
    rows: &mut ActualRootReferenceOriginsStorageV1,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    if resources.has_denial() {
        return Err(resource(Resource::Accounting));
    }
    let ledger = resources
        .original_ledger_v1()
        .ok_or_else(|| resource(Resource::Accounting))?;
    resources.work(64)?;
    if guarded.ledger != Some(ledger) || rows.ledger.is_some_and(|saved| saved != ledger) {
        return Err(resource(Resource::Accounting));
    }
    if !guarded.completed() || guarded.frame_credits == 0 {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "actual reference guarded stage unfinished",
        ));
    }
    if rows.started
        || rows.completed
        || rows.ledger.is_some()
        || rows.frame_credits != 0
        || rows.pending.payload.is_some()
        || rows.pending.ledger.is_some()
        || rows.pending.completed
    {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "actual reference origins cannot be replaced or retried",
        ));
    }
    let frame = 4096usize
        .checked_add(size_of::<ActualRootReferenceOriginsStorageV1>())
        .and_then(|n| {
            n.checked_add(size_of::<
                super::root_guarded_access_preparation_v1::RootGuardedSourceCallV1,
            >())
        })
        .ok_or_else(|| resource(Resource::Arithmetic))?;
    resources.work(frame)?;
    resources.reserve_storage(frame)?;
    rows.frame_credits = frame;
    rows.ledger = Some(ledger);
    rows.started = true;
    if edges_by_source.len() != function.locals().len() {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "actual reference complete graph local roster differs",
        ));
    }
    validate_actual_guarded_source_calls_v1(function, callables, guarded, resources)?;
    prepare_unjoined_reference_origins_v1(
        function,
        callables,
        guarded.accesses.len(),
        edges_by_source,
        option_dominance,
        enum_payload_dominance,
        &mut rows.pending,
        resources,
    )?;
    let payload =
        rows.pending
            .payload
            .as_ref()
            .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                "actual reference FIFO payload absent",
            ))?;
    resources.work(64)?;
    if !rows.pending.completed
        || rows.pending.ledger != Some(ledger)
        || payload.definitions.len() != function.locals().len()
        || payload.origins.len() != function.locals().len()
        || payload.cursor != payload.fifo.len()
    {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "actual reference FIFO completion differs",
        ));
    }
    for binding in &guarded.source_calls {
        resources.work(128)?;
        let availability = option_dominance.availability(binding.destination).ok_or(
            ProductionRankedProjectionErrorV1::Incomplete(
                "actual reference seed availability absent",
            ),
        )?;
        let expected = CheckedReferenceOriginV1 {
            source: CheckedReferenceSourceV1::GuardedAccess(binding.guarded_access),
            availability: Some(CapabilityAvailabilityV1::Option(availability)),
        };
        if payload
            .definitions
            .get(binding.destination.index() as usize)
            != Some(&1)
            || payload.origins.get(binding.destination.index() as usize) != Some(&Some(expected))
        {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "actual reference seed/guard association differs",
            ));
        }
    }
    for origin in &payload.origins {
        resources.work(32)?;
        if let Some(CheckedReferenceOriginV1 {
            source: CheckedReferenceSourceV1::GuardedAccess(index),
            ..
        }) = origin
            && (guarded
                .source_calls
                .get(*index)
                .is_none_or(|row| row.guarded_access != *index)
                || guarded
                    .accesses
                    .get(*index)
                    .is_none_or(|row| row.semantic_site.is_some()))
        {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "actual reference transported guard association differs",
            ));
        }
    }
    if resources.has_denial() || resources.original_ledger_v1() != Some(ledger) {
        return Err(resource(Resource::Accounting));
    }
    rows.completed = true; // Joined S5A DATA only, never CheckedReferencesV1.
    Ok(())
}
