#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

use std::error::Error;
use std::fmt;

use fe2o3_kernel_ir::{AddressSpace, BlockId, FunctionId, OperationKind, ScalarType};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, IndexWidthV1, SimulationArgumentV1, SimulationErrorV1,
    SimulationEventKindV1, SimulationEventSinkControlV1, SimulationEventSinkErrorV1,
    SimulationEventSinkV1, SimulationEventSiteV1, SimulationEventV1, SimulationExecutionV1,
    SimulationInvocationV1, SimulationLimitsV1, SimulationRequestV1, SimulationSiteV1,
    SimulationTargetV1,
};
use fe2o3_semantic_trace::{
    ActiveMaskV1, AddressSpaceV1, AllocationEventV1, BarrierActionV1, BarrierEventV1,
    BarrierScopeV1, CaptureBoundariesV1, DiagnosticEventV1, DiagnosticKindV1, DispatchEventV1,
    DispatchIdentityDomainV1, DispatchIdentityV1, DispatchOutcomeV1, DroppedEventCountV1,
    ExecutionKindV1, ExecutionScopeV1, FactProvenanceV1, InvocationEventV1,
    KernelIrIdentityClaimV1, KirSiteClaimV1, KirSitePointV1, LaunchGeometryV1,
    MAX_TRACE_RESIDENT_BYTES_V1, MemoryAccessKindV1, MemoryEventV1, MemoryOutcomeV1,
    OpaqueIdentityV1, OperationEventV1, OperationOccurrenceIdV1, ProducerIdentityV1,
    ProducerKindV1, ProducerTextV1, TimestampV1, TraceAllocationIdV1, TraceBoundsV1,
    TraceCompletenessV1, TraceEventKindV1, TraceEventV1, TraceHeaderV1, TraceV1,
    TraceValidationErrorV1, TruncationReasonV1, WaveWidthV1, encoded_event_len_v1,
    encoded_trace_prefix_len_v1,
};
use sha2::{Digest, Sha256};

const CONFIGURATION_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3-kir-sim-trace/configuration/v1\0";

/// Caller-selected trace visualization and storage profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SimulationTraceProfileV1 {
    pub wave_width: WaveWidthV1,
    pub bounds: TraceBoundsV1,
    /// Caller-owned identity for this one simulation occurrence.
    pub dispatch_occurrence: OpaqueIdentityV1,
}

/// Numeric site catalog bound to one admitted module inspection view.
#[derive(Debug, Eq, PartialEq)]
pub struct KirSiteCatalogV1 {
    functions: Vec<FunctionCatalogV1>,
    function_positions_by_ordinal: Vec<usize>,
    resident_bytes: u64,
}

#[derive(Debug, Eq, PartialEq)]
struct FunctionCatalogV1 {
    id: String,
    ordinal: u64,
    blocks: Vec<BlockCatalogV1>,
}

#[derive(Debug, Eq, PartialEq)]
struct BlockCatalogV1 {
    id: BlockId,
    ordinal: u64,
    operations: Vec<OperationCatalogV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OperationCatalogV1 {
    address_space: Option<AddressSpaceV1>,
    workgroup_barrier: Option<u32>,
}

impl KirSiteCatalogV1 {
    /// Builds ordinal bindings from canonical vector order, never identifier text.
    pub fn from_admitted(module: &AdmittedSimulationModuleV1) -> Result<Self, TraceAdapterErrorV1> {
        let mut resident = AdapterResidentLedgerV1::new(MAX_TRACE_RESIDENT_BYTES_V1);
        Self::from_admitted_with_resident(module, &mut resident)
    }

    fn from_admitted_with_resident(
        module: &AdmittedSimulationModuleV1,
        resident: &mut AdapterResidentLedgerV1,
    ) -> Result<Self, TraceAdapterErrorV1> {
        let mut functions = Vec::new();
        let mut next_workgroup_barrier = 1_u32;
        resident.try_reserve_vec(&mut functions, module.module().functions.len())?;
        for (function_index, function) in module.module().functions.iter().enumerate() {
            let id = resident.copy_string(function.id.as_str())?;
            let mut blocks = Vec::new();
            if let Some(body) = &function.body {
                resident.try_reserve_vec(&mut blocks, body.blocks.len())?;
                for (block_index, block) in body.blocks.iter().enumerate() {
                    let mut operations = Vec::new();
                    resident.try_reserve_vec(&mut operations, block.operations.len())?;
                    for operation in &block.operations {
                        let address_space = match &operation.kind {
                            OperationKind::Load { access, .. }
                            | OperationKind::Store { access, .. } => {
                                Some(map_address_space(access.address_space))
                            }
                            _ => None,
                        };
                        let workgroup_barrier =
                            if matches!(operation.kind, OperationKind::WorkgroupBarrier(_)) {
                                let barrier = next_workgroup_barrier;
                                next_workgroup_barrier = next_workgroup_barrier
                                    .checked_add(1)
                                    .ok_or(TraceAdapterErrorV1::UnrepresentableCatalog)?;
                                Some(barrier)
                            } else {
                                None
                            };
                        operations.push(OperationCatalogV1 {
                            address_space,
                            workgroup_barrier,
                        });
                    }
                    blocks.push(BlockCatalogV1 {
                        id: block.id,
                        ordinal: u64::try_from(block_index)
                            .map_err(|_| TraceAdapterErrorV1::UnrepresentableCatalog)?,
                        operations,
                    });
                }
                blocks.sort_unstable_by_key(|entry| entry.id);
            }
            functions.push(FunctionCatalogV1 {
                id,
                ordinal: u64::try_from(function_index)
                    .map_err(|_| TraceAdapterErrorV1::UnrepresentableCatalog)?,
                blocks,
            });
        }
        functions.sort_unstable_by(|left, right| left.id.cmp(&right.id));
        let mut function_positions_by_ordinal = Vec::new();
        resident.try_reserve_vec(&mut function_positions_by_ordinal, functions.len())?;
        function_positions_by_ordinal.resize(functions.len(), 0);
        for (position, function) in functions.iter().enumerate() {
            let ordinal = usize::try_from(function.ordinal)
                .map_err(|_| TraceAdapterErrorV1::UnrepresentableCatalog)?;
            *function_positions_by_ordinal
                .get_mut(ordinal)
                .ok_or(TraceAdapterErrorV1::UnrepresentableCatalog)? = position;
        }
        Ok(Self {
            functions,
            function_positions_by_ordinal,
            resident_bytes: resident.used(),
        })
    }

    fn function(&self, id: &FunctionId) -> Option<&FunctionCatalogV1> {
        let position = self
            .functions
            .binary_search_by(|entry| entry.id.as_str().cmp(id.as_str()))
            .ok()?;
        self.functions.get(position)
    }

    fn function_by_ordinal(&self, ordinal: usize) -> Option<&FunctionCatalogV1> {
        let position = *self.function_positions_by_ordinal.get(ordinal)?;
        self.functions.get(position)
    }

    fn block(function: &FunctionCatalogV1, id: BlockId) -> Option<&BlockCatalogV1> {
        let position = function
            .blocks
            .binary_search_by_key(&id, |entry| entry.id)
            .ok()?;
        function.blocks.get(position)
    }

    /// Resolves an ephemeral site to an unresolved durable ordinal claim.
    pub fn claim(&self, site: &SimulationSiteV1) -> Option<KirSiteClaimV1> {
        let function = self.function(&site.function)?;
        Self::claim_in_function(function, site.block, site.operation)
    }

    fn claim_event(&self, site: &SimulationEventSiteV1) -> Option<KirSiteClaimV1> {
        let function = self.function_by_ordinal(site.function_ordinal)?;
        Self::claim_in_function(function, site.block, site.operation)
    }

    fn claim_in_function(
        function: &FunctionCatalogV1,
        block_id: BlockId,
        operation: Option<u32>,
    ) -> Option<KirSiteClaimV1> {
        let block = Self::block(function, block_id)?;
        let point = match operation {
            Some(operation) => {
                block.operations.get(usize::try_from(operation).ok()?)?;
                KirSitePointV1::Operation(u64::from(operation))
            }
            None => KirSitePointV1::Terminator,
        };
        Some(KirSiteClaimV1::new(function.ordinal, block.ordinal, point))
    }

    fn address_space_event(&self, site: &SimulationEventSiteV1) -> Option<AddressSpaceV1> {
        let operation = usize::try_from(site.operation?).ok()?;
        let function = self.function_by_ordinal(site.function_ordinal)?;
        Self::block(function, site.block)?
            .operations
            .get(operation)
            .and_then(|operation| operation.address_space)
    }

    fn workgroup_barrier_event(&self, site: &SimulationEventSiteV1) -> Option<u32> {
        let operation = usize::try_from(site.operation?).ok()?;
        let function = self.function_by_ordinal(site.function_ordinal)?;
        Self::block(function, site.block)?
            .operations
            .get(operation)?
            .workgroup_barrier
    }

    fn block_entry_event(&self, site: &SimulationEventSiteV1) -> Option<KirSiteClaimV1> {
        let function = self.function_by_ordinal(site.function_ordinal)?;
        let block = Self::block(function, site.block)?;
        Some(KirSiteClaimV1::new(
            function.ordinal,
            block.ordinal,
            KirSitePointV1::BlockEntry,
        ))
    }

    fn block_ordinal_event(&self, site: &SimulationEventSiteV1, block: BlockId) -> Option<u64> {
        let function = self.function_by_ordinal(site.function_ordinal)?;
        Some(Self::block(function, block)?.ordinal)
    }
}

#[derive(Clone, Copy)]
struct AdapterResidentLedgerV1 {
    used: u64,
    limit: u64,
}

impl AdapterResidentLedgerV1 {
    const fn new(limit: u64) -> Self {
        Self { used: 0, limit }
    }

    const fn used(self) -> u64 {
        self.used
    }

    fn charge(&mut self, bytes: u64) -> Result<(), TraceAdapterErrorV1> {
        self.ensure_available(bytes)?;
        self.used = self
            .used
            .checked_add(bytes)
            .ok_or(TraceAdapterErrorV1::ResidentSizeOverflow)?;
        Ok(())
    }

    fn try_reserve_vec<T>(
        &mut self,
        values: &mut Vec<T>,
        additional: usize,
    ) -> Result<(), TraceAdapterErrorV1> {
        self.try_reserve_vec_bounded(values, additional, usize::MAX)
    }

    fn try_reserve_vec_bounded<T>(
        &mut self,
        values: &mut Vec<T>,
        additional: usize,
        max_capacity: usize,
    ) -> Result<(), TraceAdapterErrorV1> {
        let before = values.capacity();
        let required = values
            .len()
            .checked_add(additional)
            .ok_or(TraceAdapterErrorV1::ResidentSizeOverflow)?;
        if required <= before {
            return Ok(());
        }
        if required > max_capacity {
            return Err(TraceAdapterErrorV1::ResidentLimitExceeded {
                actual: u64::try_from(required)
                    .map_err(|_| TraceAdapterErrorV1::ResidentSizeOverflow)?,
                max: u64::try_from(max_capacity)
                    .map_err(|_| TraceAdapterErrorV1::ResidentSizeOverflow)?,
            });
        }
        let geometric = before.max(4).saturating_mul(2).max(required);
        let element_bytes = std::mem::size_of::<T>();
        let replacement_capacity = if element_bytes == 0 {
            max_capacity
        } else {
            let remaining = self
                .limit
                .checked_sub(self.used)
                .ok_or(TraceAdapterErrorV1::ResidentSizeOverflow)?;
            usize::try_from(remaining / element_bytes as u64).unwrap_or(usize::MAX)
        };
        let target = geometric.min(max_capacity).min(replacement_capacity);
        if target < required {
            let actual = self
                .used
                .checked_add(capacity_bytes::<T>(required)?)
                .ok_or(TraceAdapterErrorV1::ResidentSizeOverflow)?;
            return Err(TraceAdapterErrorV1::ResidentLimitExceeded {
                actual,
                max: self.limit,
            });
        }

        // Reserve into a replacement so allocator over-reservation can be
        // rejected without mutating or under-accounting the retained vector.
        let mut replacement = Vec::new();
        replacement
            .try_reserve_exact(target)
            .map_err(|_| TraceAdapterErrorV1::AllocationFailure)?;
        let replacement_bytes = capacity_bytes::<T>(replacement.capacity())?;
        self.ensure_available(replacement_bytes)?;
        let final_used = self
            .used
            .checked_sub(capacity_bytes::<T>(before)?)
            .and_then(|used| used.checked_add(replacement_bytes))
            .ok_or(TraceAdapterErrorV1::ResidentSizeOverflow)?;
        replacement.append(values);
        *values = replacement;
        self.used = final_used;
        Ok(())
    }

    fn copy_string(&mut self, value: &str) -> Result<String, TraceAdapterErrorV1> {
        let requested =
            u64::try_from(value.len()).map_err(|_| TraceAdapterErrorV1::ResidentSizeOverflow)?;
        self.ensure_available(requested)?;
        let mut copy = String::new();
        copy.try_reserve_exact(value.len())
            .map_err(|_| TraceAdapterErrorV1::AllocationFailure)?;
        self.charge(
            u64::try_from(copy.capacity())
                .map_err(|_| TraceAdapterErrorV1::ResidentSizeOverflow)?,
        )?;
        copy.push_str(value);
        Ok(copy)
    }

    fn ensure_available(self, bytes: u64) -> Result<(), TraceAdapterErrorV1> {
        let actual = self
            .used
            .checked_add(bytes)
            .ok_or(TraceAdapterErrorV1::ResidentSizeOverflow)?;
        if actual > self.limit {
            return Err(TraceAdapterErrorV1::ResidentLimitExceeded {
                actual,
                max: self.limit,
            });
        }
        Ok(())
    }
}

fn capacity_bytes<T>(capacity: usize) -> Result<u64, TraceAdapterErrorV1> {
    u64::try_from(capacity)
        .ok()
        .and_then(|count| count.checked_mul(std::mem::size_of::<T>() as u64))
        .ok_or(TraceAdapterErrorV1::ResidentSizeOverflow)
}

/// Simulation result paired with its bounded semantic observation.
#[derive(Debug)]
pub struct TracedSimulationOutcomeV1 {
    pub execution: Result<SimulationExecutionV1, SimulationErrorV1>,
    pub trace: TraceV1,
    pub catalog: KirSiteCatalogV1,
    /// Sensitive deterministic correlation digest over KIR, target, launch, and values.
    pub configuration_identity: OpaqueIdentityV1,
}

impl TracedSimulationOutcomeV1 {
    pub const fn grants_execution_authority(&self) -> bool {
        false
    }
}

/// Adapter setup failure. Runtime simulation failures remain in the outcome.
#[derive(Debug)]
pub enum TraceAdapterErrorV1 {
    Trace(TraceValidationErrorV1),
    AllocationFailure,
    ResidentSizeOverflow,
    ResidentLimitExceeded { actual: u64, max: u64 },
    UnrepresentableGeometry,
    UnrepresentableCatalog,
    ByteBudgetTooSmall,
    InsufficientClosureBudget,
    ZeroWorkgroupDimension { axis: usize },
    Preflight(fe2o3_kir_sim::SimulationPreflightErrorV1),
}

impl fmt::Display for TraceAdapterErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "semantic trace adapter failed: {self:?}")
    }
}

impl Error for TraceAdapterErrorV1 {}

impl From<TraceValidationErrorV1> for TraceAdapterErrorV1 {
    fn from(error: TraceValidationErrorV1) -> Self {
        Self::Trace(error)
    }
}

/// Runs the deterministic cooperative CPU simulator and records a bounded semantic trace.
pub fn simulate_with_semantic_trace_v1(
    module: &AdmittedSimulationModuleV1,
    request: &SimulationRequestV1,
    target: SimulationTargetV1,
    limits: SimulationLimitsV1,
    profile: SimulationTraceProfileV1,
) -> Result<TracedSimulationOutcomeV1, TraceAdapterErrorV1> {
    for (axis, extent) in request.workgroup.0.into_iter().enumerate() {
        if extent == 0 {
            return Err(TraceAdapterErrorV1::ZeroWorkgroupDimension { axis });
        }
    }
    module
        .preflight(request, target, limits)
        .map_err(TraceAdapterErrorV1::Preflight)?;
    let mut resident = AdapterResidentLedgerV1::new(profile.bounds.max_resident_bytes());
    let catalog = KirSiteCatalogV1::from_admitted_with_resident(module, &mut resident)?;
    let launch = launch_geometry(request, profile.wave_width)?;
    let dispatch = DispatchIdentityV1::new(
        DispatchIdentityDomainV1::TraceLocal,
        profile.dispatch_occurrence,
    );
    let configuration_identity = configuration_identity(module, request, target)?;
    let claim = KernelIrIdentityClaimV1::canonical_v7_claim(
        OpaqueIdentityV1::new(*module.identity().digest())?,
        module.identity().canonical_length(),
    )?;
    let budget_header = TraceHeaderV1::new(
        producer_identity()?,
        ExecutionKindV1::CpuKirSimulation,
        claim,
        None,
        None,
        None,
        dispatch,
        launch,
        profile.bounds,
        TraceCompletenessV1::Truncated {
            reason: TruncationReasonV1::ByteLimit,
            emitted_events: 0,
            dropped_events: DroppedEventCountV1::Unknown,
        },
        CaptureBoundariesV1::FULL_DISPATCH,
    )?;
    let prefix_bytes = encoded_trace_prefix_len_v1(&budget_header)
        .map_err(|_| TraceAdapterErrorV1::ByteBudgetTooSmall)?;
    drop(budget_header);
    let footer_probe = TraceEventV1::new(
        0,
        TimestampV1::LogicalStep(0),
        FactProvenanceV1::Observed,
        ExecutionScopeV1::dispatch(dispatch),
        None,
        TraceEventKindV1::Dispatch(DispatchEventV1::End(DispatchOutcomeV1::Completed)),
        Vec::new(),
    )?;
    let footer_bytes =
        encoded_event_len_v1(&footer_probe).map_err(|_| TraceAdapterErrorV1::ByteBudgetTooSmall)?;
    let mut collector = TraceCollectorV1::new(
        dispatch,
        launch,
        profile.bounds,
        prefix_bytes,
        footer_bytes,
        resident,
        &catalog,
    )?;
    collector.emit_dispatch(DispatchEventV1::Begin);
    if collector.truncation.is_some() {
        return Err(TraceAdapterErrorV1::InsufficientClosureBudget);
    }

    let execution = module.simulate_observed_with_sink(request, target, limits, &mut collector);
    if let Err(SimulationErrorV1::Execution(error)) = &execution {
        collector.record_failure(error.invocation, error.site.as_ref());
    }
    collector.finish_execution(execution.is_ok());
    let (events, truncation) = collector.into_parts();
    let (completeness, boundaries) = match truncation {
        Some(reason) => (
            TraceCompletenessV1::Truncated {
                reason,
                emitted_events: u64::try_from(events.len())
                    .map_err(|_| TraceAdapterErrorV1::UnrepresentableCatalog)?,
                dropped_events: DroppedEventCountV1::Unknown,
            },
            CaptureBoundariesV1::FULL_DISPATCH,
        ),
        None => (
            TraceCompletenessV1::Complete,
            CaptureBoundariesV1::FULL_DISPATCH,
        ),
    };
    let header = TraceHeaderV1::new(
        producer_identity()?,
        ExecutionKindV1::CpuKirSimulation,
        claim,
        None,
        None,
        None,
        dispatch,
        launch,
        profile.bounds,
        completeness,
        boundaries,
    )?;
    let trace = TraceV1::new_with_resident_reservation(header, events, catalog.resident_bytes)?;
    Ok(TracedSimulationOutcomeV1 {
        execution,
        trace,
        catalog,
        configuration_identity,
    })
}

struct TraceCollectorV1<'a> {
    dispatch: DispatchIdentityV1,
    launch: LaunchGeometryV1,
    event_limit: u64,
    byte_limit: u64,
    encoded_bytes: u64,
    footer_bytes: u64,
    resident: AdapterResidentLedgerV1,
    catalog: &'a KirSiteCatalogV1,
    events: Vec<TraceEventV1>,
    invocations: Vec<InvocationTraceStateV1>,
    next_invocation_state: usize,
    next_frame: u64,
    next_occurrence: u64,
    allocations: Vec<AllocationMappingV1>,
    workgroup_checkpoint: Option<(usize, u64)>,
    truncation: Option<TruncationReasonV1>,
}

struct OperationStateV1 {
    site: KirSiteClaimV1,
    scope: ExecutionScopeV1,
    occurrence: OperationOccurrenceIdV1,
}

struct InvocationTraceStateV1 {
    invocation: SimulationInvocationV1,
    scope: ExecutionScopeV1,
    operations: Vec<OperationStateV1>,
    frames: Vec<u64>,
    ended: bool,
}

struct AllocationMappingV1 {
    raw: u64,
    trace: TraceAllocationIdV1,
    address_space: AddressSpaceV1,
    released: bool,
}

impl<'a> TraceCollectorV1<'a> {
    fn new(
        dispatch: DispatchIdentityV1,
        launch: LaunchGeometryV1,
        bounds: TraceBoundsV1,
        prefix_bytes: u64,
        footer_bytes: u64,
        mut resident: AdapterResidentLedgerV1,
        catalog: &'a KirSiteCatalogV1,
    ) -> Result<Self, TraceAdapterErrorV1> {
        let mut events = Vec::new();
        let max_events = usize::try_from(bounds.max_events())
            .map_err(|_| TraceAdapterErrorV1::ResidentSizeOverflow)?;
        if max_events < 2 {
            return Err(TraceAdapterErrorV1::InsufficientClosureBudget);
        }
        resident.try_reserve_vec_bounded(&mut events, 2, max_events)?;
        Ok(Self {
            dispatch,
            launch,
            event_limit: bounds.max_events(),
            byte_limit: bounds.max_encoded_bytes(),
            encoded_bytes: prefix_bytes,
            footer_bytes,
            resident,
            catalog,
            events,
            invocations: Vec::new(),
            next_invocation_state: 0,
            next_frame: 1,
            next_occurrence: 1,
            allocations: Vec::new(),
            workgroup_checkpoint: None,
            truncation: None,
        })
    }

    fn emit_dispatch(&mut self, event: DispatchEventV1) {
        self.emit(
            ExecutionScopeV1::dispatch(self.dispatch),
            None,
            TraceEventKindV1::Dispatch(event),
        );
    }

    fn emit(
        &mut self,
        scope: ExecutionScopeV1,
        site: Option<KirSiteClaimV1>,
        kind: TraceEventKindV1,
    ) {
        if self.truncation.is_some() {
            return;
        }
        let event = match TraceEventV1::new(
            self.events.len() as u64,
            TimestampV1::LogicalStep(self.events.len() as u64),
            FactProvenanceV1::Observed,
            scope,
            site,
            kind,
            Vec::new(),
        ) {
            Ok(event) => event,
            Err(_) => {
                self.truncate(TruncationReasonV1::ProducerFailure);
                return;
            }
        };
        let event_bytes = match encoded_event_len_v1(&event) {
            Ok(bytes) => bytes,
            Err(_) => {
                self.truncate(TruncationReasonV1::ProducerFailure);
                return;
            }
        };
        if (self.events.len() as u64)
            .checked_add(2)
            .is_none_or(|needed| needed > self.event_limit)
        {
            self.truncate(TruncationReasonV1::EventLimit);
            return;
        }
        if self
            .encoded_bytes
            .checked_add(event_bytes)
            .and_then(|bytes| bytes.checked_add(self.footer_bytes))
            .is_none_or(|needed| needed > self.byte_limit)
        {
            self.truncate(TruncationReasonV1::ByteLimit);
            return;
        }
        let max_events = usize::try_from(self.event_limit).unwrap_or(usize::MAX);
        if self
            .resident
            .try_reserve_vec_bounded(&mut self.events, 2, max_events)
            .is_err()
        {
            self.truncate(TruncationReasonV1::ProducerFailure);
            return;
        }
        self.encoded_bytes += event_bytes;
        self.events.push(event);
    }

    fn truncate(&mut self, reason: TruncationReasonV1) {
        if self.truncation.is_none() {
            self.truncation = Some(reason);
        }
        if let Some((events, bytes)) = self.workgroup_checkpoint.take() {
            self.events.truncate(events);
            self.encoded_bytes = bytes;
        }
        self.invocations.clear();
    }

    fn emit_footer(&mut self, event: DispatchEventV1) {
        let sequence = self.events.len() as u64;
        let Ok(event) = TraceEventV1::new(
            sequence,
            TimestampV1::LogicalStep(sequence),
            FactProvenanceV1::Observed,
            ExecutionScopeV1::dispatch(self.dispatch),
            None,
            TraceEventKindV1::Dispatch(event),
            Vec::new(),
        ) else {
            self.truncation = Some(TruncationReasonV1::ProducerFailure);
            return;
        };
        // The constructor reserves both dispatch boundary slots before any
        // simulated effects, and body growth never consumes the final slot.
        self.events.push(event);
    }

    fn begin_invocation(&mut self, invocation: SimulationInvocationV1) {
        let starts_workgroup = self.invocations.iter().all(|state| state.ended);
        if starts_workgroup {
            self.next_invocation_state = 0;
            self.workgroup_checkpoint = Some((self.events.len(), self.encoded_bytes));
        }
        if self
            .invocations
            .iter()
            .any(|state| !state.ended && state.invocation == invocation)
        {
            self.truncation = Some(TruncationReasonV1::ProducerFailure);
            return;
        }
        let frame = self.next_frame;
        let Some(next_frame) = self.next_frame.checked_add(1) else {
            self.truncation = Some(TruncationReasonV1::ProducerFailure);
            return;
        };
        let Ok(scope) = lane_scope(self.dispatch, self.launch, invocation) else {
            self.truncation = Some(TruncationReasonV1::ProducerFailure);
            return;
        };
        let position = self.next_invocation_state;
        if position == self.invocations.len()
            && self
                .resident
                .try_reserve_vec(&mut self.invocations, 1)
                .is_err()
        {
            self.truncation = Some(TruncationReasonV1::ProducerFailure);
            return;
        }
        let mut new_frames = Vec::new();
        let reserve = if position < self.invocations.len() {
            let (resident, invocations) = (&mut self.resident, &mut self.invocations);
            resident.try_reserve_vec(&mut invocations[position].frames, 1)
        } else {
            self.resident.try_reserve_vec(&mut new_frames, 1)
        };
        if reserve.is_err() {
            self.truncation = Some(TruncationReasonV1::ProducerFailure);
            return;
        }
        self.emit(
            scope,
            None,
            TraceEventKindV1::Invocation(InvocationEventV1::Begin),
        );
        if self.truncation.is_none() {
            self.next_frame = next_frame;
            self.next_invocation_state += 1;
            if position < self.invocations.len() {
                let state = &mut self.invocations[position];
                state.invocation = invocation;
                state.scope = scope;
                state.operations.clear();
                state.frames.push(frame);
                state.ended = false;
            } else {
                new_frames.push(frame);
                self.invocations.push(InvocationTraceStateV1 {
                    invocation,
                    scope,
                    operations: Vec::new(),
                    frames: new_frames,
                    ended: false,
                });
            }
        }
    }

    fn current_scope(&self, invocation: SimulationInvocationV1) -> Option<ExecutionScopeV1> {
        self.invocations
            .iter()
            .rev()
            .find(|state| state.invocation == invocation)
            .map(|state| state.scope)
    }

    fn invocation_position(&self, invocation: SimulationInvocationV1) -> Option<usize> {
        self.invocations
            .iter()
            .rposition(|state| state.invocation == invocation)
    }

    fn record_event(&mut self, event: &SimulationEventV1) {
        if self.truncation.is_some() {
            return;
        }
        if matches!(event.kind, SimulationEventKindV1::InvocationBegin) {
            self.begin_invocation(event.invocation);
            return;
        }
        let Some(scope) = self.current_scope(event.invocation) else {
            self.truncation = Some(TruncationReasonV1::ProducerFailure);
            return;
        };
        let Some(invocation_position) = self.invocation_position(event.invocation) else {
            self.truncation = Some(TruncationReasonV1::ProducerFailure);
            return;
        };
        if self.invocations[invocation_position].ended
            && !matches!(event.kind, SimulationEventKindV1::AllocationReleased { .. })
        {
            self.truncation = Some(TruncationReasonV1::ProducerFailure);
            return;
        }
        let Some(site) = self.catalog.claim_event(&event.site) else {
            self.truncation = Some(TruncationReasonV1::ProducerFailure);
            return;
        };
        match &event.kind {
            SimulationEventKindV1::InvocationBegin => {
                self.truncate(TruncationReasonV1::ProducerFailure);
            }
            SimulationEventKindV1::InvocationEnd { .. } => {
                if !self.invocations[invocation_position].operations.is_empty() {
                    self.truncation = Some(TruncationReasonV1::ProducerFailure);
                    return;
                }
                self.emit(
                    scope,
                    None,
                    TraceEventKindV1::Invocation(InvocationEventV1::End),
                );
                if self.truncation.is_none() {
                    self.invocations[invocation_position].ended = true;
                    self.invocations[invocation_position].frames.clear();
                }
            }
            SimulationEventKindV1::BlockEnter => {
                let Some(entry) = self.catalog.block_entry_event(&event.site) else {
                    self.truncation = Some(TruncationReasonV1::ProducerFailure);
                    return;
                };
                self.emit(scope, Some(entry), TraceEventKindV1::BlockEnter);
            }
            SimulationEventKindV1::OperationBegin => {
                let Some(frame) = self.invocations[invocation_position].frames.last().copied()
                else {
                    self.truncation = Some(TruncationReasonV1::ProducerFailure);
                    return;
                };
                let Ok(occurrence) = OperationOccurrenceIdV1::new(frame, self.next_occurrence)
                else {
                    self.truncation = Some(TruncationReasonV1::ProducerFailure);
                    return;
                };
                let Some(next_occurrence) = self.next_occurrence.checked_add(1) else {
                    self.truncation = Some(TruncationReasonV1::ProducerFailure);
                    return;
                };
                let reserve = {
                    let (resident, invocations) = (&mut self.resident, &mut self.invocations);
                    resident.try_reserve_vec(&mut invocations[invocation_position].operations, 1)
                };
                if reserve.is_err() {
                    self.truncation = Some(TruncationReasonV1::ProducerFailure);
                    return;
                }
                self.emit(
                    scope,
                    Some(site),
                    TraceEventKindV1::Operation(OperationEventV1::Begin(occurrence)),
                );
                if self.truncation.is_none() {
                    self.next_occurrence = next_occurrence;
                    self.invocations[invocation_position]
                        .operations
                        .push(OperationStateV1 {
                            site,
                            scope,
                            occurrence,
                        });
                }
            }
            SimulationEventKindV1::OperationEnd { .. } => {
                let Some(operation) = self.invocations[invocation_position].operations.pop() else {
                    self.truncation = Some(TruncationReasonV1::ProducerFailure);
                    return;
                };
                if operation.site != site || operation.scope != scope {
                    self.truncation = Some(TruncationReasonV1::ProducerFailure);
                    return;
                }
                self.emit(
                    scope,
                    Some(site),
                    TraceEventKindV1::Operation(OperationEventV1::End(operation.occurrence)),
                );
            }
            SimulationEventKindV1::Call { .. } => {
                let frame = self.next_frame;
                let reserve = {
                    let (resident, invocations) = (&mut self.resident, &mut self.invocations);
                    resident.try_reserve_vec(&mut invocations[invocation_position].frames, 1)
                };
                if reserve.is_err() {
                    self.truncation = Some(TruncationReasonV1::ProducerFailure);
                    return;
                }
                let Some(next_frame) = self.next_frame.checked_add(1) else {
                    self.truncation = Some(TruncationReasonV1::ProducerFailure);
                    return;
                };
                self.next_frame = next_frame;
                self.invocations[invocation_position].frames.push(frame);
            }
            SimulationEventKindV1::Return => {
                self.invocations[invocation_position].frames.pop();
            }
            SimulationEventKindV1::Terminator => {}
            SimulationEventKindV1::Branch { target } => {
                let Some(target_block_ordinal) =
                    self.catalog.block_ordinal_event(&event.site, *target)
                else {
                    self.truncation = Some(TruncationReasonV1::ProducerFailure);
                    return;
                };
                self.emit(
                    scope,
                    Some(site),
                    TraceEventKindV1::Branch {
                        target_block_ordinal,
                    },
                );
            }
            SimulationEventKindV1::MemoryRead {
                allocation,
                offset,
                bytes,
            } => self.record_memory(
                scope,
                site,
                *allocation,
                *offset,
                *bytes,
                MemoryAccessKindV1::Read,
                &event.site,
            ),
            SimulationEventKindV1::MemoryWrite {
                allocation,
                offset,
                bytes,
            } => self.record_memory(
                scope,
                site,
                *allocation,
                *offset,
                *bytes,
                MemoryAccessKindV1::Write,
                &event.site,
            ),
            SimulationEventKindV1::WorkgroupBarrierArrive { phase } => {
                let Some(barrier) = self.catalog.workgroup_barrier_event(&event.site) else {
                    self.truncation = Some(TruncationReasonV1::ProducerFailure);
                    return;
                };
                self.emit(
                    scope,
                    Some(site),
                    TraceEventKindV1::Barrier(BarrierEventV1::new(
                        barrier,
                        *phase,
                        BarrierScopeV1::Workgroup,
                        BarrierActionV1::Arrive,
                    )),
                );
            }
            SimulationEventKindV1::WorkgroupBarrierRelease {
                phase,
                participants: _,
            } => {
                let Some(barrier) = self.catalog.workgroup_barrier_event(&event.site) else {
                    self.truncation = Some(TruncationReasonV1::ProducerFailure);
                    return;
                };
                let Some(workgroup_scope) = workgroup_scope(self.dispatch, event.invocation) else {
                    self.truncation = Some(TruncationReasonV1::ProducerFailure);
                    return;
                };
                self.emit(
                    workgroup_scope,
                    Some(site),
                    TraceEventKindV1::Barrier(BarrierEventV1::new(
                        barrier,
                        *phase,
                        BarrierScopeV1::Workgroup,
                        BarrierActionV1::Release,
                    )),
                );
            }
            SimulationEventKindV1::AllocationPreexisting {
                allocation,
                address_space,
                bytes,
            } => self.record_allocation(
                scope,
                Some(site),
                *allocation,
                map_address_space(*address_space),
                *bytes,
                true,
            ),
            SimulationEventKindV1::AllocationCreated {
                allocation,
                address_space,
                bytes,
            } => {
                let address_space = map_address_space(*address_space);
                let allocation_scope = if address_space == AddressSpaceV1::Workgroup {
                    let Some(scope) = workgroup_scope(self.dispatch, event.invocation) else {
                        self.truncation = Some(TruncationReasonV1::ProducerFailure);
                        return;
                    };
                    scope
                } else {
                    scope
                };
                self.record_allocation(
                    allocation_scope,
                    Some(site),
                    *allocation,
                    address_space,
                    *bytes,
                    false,
                );
            }
            SimulationEventKindV1::AllocationReleased { allocation } => {
                let Ok(position) = self
                    .allocations
                    .binary_search_by_key(allocation, |mapping| mapping.raw)
                else {
                    self.truncation = Some(TruncationReasonV1::ProducerFailure);
                    return;
                };
                let mapping = &self.allocations[position];
                if mapping.released {
                    self.truncation = Some(TruncationReasonV1::ProducerFailure);
                    return;
                }
                let trace = mapping.trace;
                let release_scope = if mapping.address_space == AddressSpaceV1::Workgroup {
                    let Some(scope) = workgroup_scope(self.dispatch, event.invocation) else {
                        self.truncation = Some(TruncationReasonV1::ProducerFailure);
                        return;
                    };
                    scope
                } else {
                    scope
                };
                self.emit(
                    release_scope,
                    Some(site),
                    TraceEventKindV1::Allocation(AllocationEventV1::Release { allocation: trace }),
                );
                if self.truncation.is_none() {
                    self.allocations[position].released = true;
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn record_allocation(
        &mut self,
        scope: ExecutionScopeV1,
        site: Option<KirSiteClaimV1>,
        raw: u64,
        address_space: AddressSpaceV1,
        bytes: usize,
        preexisting: bool,
    ) {
        let position = self
            .allocations
            .binary_search_by_key(&raw, |mapping| mapping.raw);
        let (ordinal, generation) = match position {
            Ok(position) if !self.allocations[position].released => {
                self.truncation = Some(TruncationReasonV1::ProducerFailure);
                return;
            }
            Ok(position) => (
                self.allocations[position].trace.ordinal(),
                match self.allocations[position].trace.generation().checked_add(1) {
                    Some(generation) => generation,
                    None => {
                        self.truncation = Some(TruncationReasonV1::ProducerFailure);
                        return;
                    }
                },
            ),
            Err(position) if position == self.allocations.len() => {
                let Some(ordinal) = self.allocations.len().checked_add(1) else {
                    self.truncation = Some(TruncationReasonV1::ProducerFailure);
                    return;
                };
                let Ok(ordinal) = u64::try_from(ordinal) else {
                    self.truncation = Some(TruncationReasonV1::ProducerFailure);
                    return;
                };
                (ordinal, 0)
            }
            Err(_) => {
                // The simulator allocates fresh raw IDs monotonically. Reject a
                // producer stream that would require quadratic middle inserts.
                self.truncation = Some(TruncationReasonV1::ProducerFailure);
                return;
            }
        };
        let (Ok(allocation), Ok(byte_len)) = (
            TraceAllocationIdV1::new(ordinal, generation),
            u64::try_from(bytes),
        ) else {
            self.truncation = Some(TruncationReasonV1::ProducerFailure);
            return;
        };
        if position.is_err()
            && self
                .resident
                .try_reserve_vec(&mut self.allocations, 1)
                .is_err()
        {
            self.truncation = Some(TruncationReasonV1::ProducerFailure);
            return;
        }
        let event = if preexisting {
            AllocationEventV1::Preexisting {
                allocation,
                byte_len,
                address_space,
            }
        } else {
            AllocationEventV1::Create {
                allocation,
                byte_len,
                address_space,
            }
        };
        self.emit(scope, site, TraceEventKindV1::Allocation(event));
        if self.truncation.is_none() {
            match position {
                Ok(position) => {
                    self.allocations[position].trace = allocation;
                    self.allocations[position].address_space = address_space;
                    self.allocations[position].released = false;
                }
                Err(_) => self.allocations.push(AllocationMappingV1 {
                    raw,
                    trace: allocation,
                    address_space,
                    released: false,
                }),
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn record_memory(
        &mut self,
        scope: ExecutionScopeV1,
        site: KirSiteClaimV1,
        raw_allocation: u64,
        offset: usize,
        bytes: usize,
        kind: MemoryAccessKindV1,
        raw_site: &SimulationEventSiteV1,
    ) {
        let Ok(position) = self
            .allocations
            .binary_search_by_key(&raw_allocation, |mapping| mapping.raw)
        else {
            self.truncation = Some(TruncationReasonV1::ProducerFailure);
            return;
        };
        let mapping = &self.allocations[position];
        if mapping.released {
            self.truncation = Some(TruncationReasonV1::ProducerFailure);
            return;
        }
        let allocation = mapping.trace;
        let Some(address_space) = self.catalog.address_space_event(raw_site) else {
            self.truncation = Some(TruncationReasonV1::ProducerFailure);
            return;
        };
        let (Ok(offset), Ok(bytes)) = (u64::try_from(offset), u64::try_from(bytes)) else {
            self.truncation = Some(TruncationReasonV1::ProducerFailure);
            return;
        };
        let Ok(memory) = MemoryEventV1::new(
            kind,
            allocation,
            offset,
            bytes,
            address_space,
            MemoryOutcomeV1::Completed,
        ) else {
            self.truncation = Some(TruncationReasonV1::ProducerFailure);
            return;
        };
        self.emit(scope, Some(site), TraceEventKindV1::Memory(memory));
    }

    fn finish_execution(&mut self, succeeded: bool) {
        if self.invocations.iter().any(|state| !state.ended) {
            self.truncate(TruncationReasonV1::ProducerFailure);
        }
        self.emit_footer(DispatchEventV1::End(if succeeded {
            DispatchOutcomeV1::Completed
        } else {
            DispatchOutcomeV1::Failed
        }));
    }

    fn record_failure(
        &mut self,
        _invocation: Option<SimulationInvocationV1>,
        raw_site: Option<&SimulationSiteV1>,
    ) {
        if self.truncation.is_some() {
            return;
        }
        let scope = ExecutionScopeV1::dispatch(self.dispatch);
        let site = match raw_site {
            Some(raw_site) => match self.catalog.claim(raw_site) {
                Some(site) => Some(site),
                None => {
                    self.truncation = Some(TruncationReasonV1::ProducerFailure);
                    return;
                }
            },
            None => None,
        };
        self.emit(
            scope,
            site,
            TraceEventKindV1::Diagnostic(DiagnosticEventV1::new(DiagnosticKindV1::Fault, 1)),
        );
    }

    fn into_parts(self) -> (Vec<TraceEventV1>, Option<TruncationReasonV1>) {
        (self.events, self.truncation)
    }
}

impl SimulationEventSinkV1 for TraceCollectorV1<'_> {
    fn record(&mut self, event: &SimulationEventV1) -> Result<(), SimulationEventSinkErrorV1> {
        self.record_event(event);
        Ok(())
    }

    fn record_controlled(
        &mut self,
        event: &SimulationEventV1,
    ) -> Result<SimulationEventSinkControlV1, SimulationEventSinkErrorV1> {
        let retained_before = self.events.len();
        self.record_event(event);
        Ok(match self.truncation {
            Some(_) if self.events.len() > retained_before => SimulationEventSinkControlV1::Stop,
            Some(_) => SimulationEventSinkControlV1::DropAndStop,
            None => SimulationEventSinkControlV1::Continue,
        })
    }
}

fn producer_identity() -> Result<ProducerIdentityV1, TraceAdapterErrorV1> {
    Ok(ProducerIdentityV1::new(
        ProducerKindV1::CpuKirSimulator,
        ProducerTextV1::new("fe2o3-kir-sim-trace")?,
        ProducerTextV1::new(env!("CARGO_PKG_VERSION"))?,
        None,
    ))
}

fn launch_geometry(
    request: &SimulationRequestV1,
    wave: WaveWidthV1,
) -> Result<LaunchGeometryV1, TraceAdapterErrorV1> {
    let mut grid_workgroups = [0_u32; 3];
    for (axis, grid_workgroup) in grid_workgroups.iter_mut().enumerate() {
        let workgroup = u64::from(request.workgroup.0[axis]);
        let count = request.grid.0[axis]
            .checked_add(workgroup.saturating_sub(1))
            .ok_or(TraceAdapterErrorV1::UnrepresentableGeometry)?
            / workgroup;
        *grid_workgroup =
            u32::try_from(count).map_err(|_| TraceAdapterErrorV1::UnrepresentableGeometry)?;
    }
    LaunchGeometryV1::new_exact(request.grid.0, grid_workgroups, request.workgroup.0, wave)
        .map_err(Into::into)
}

fn lane_scope(
    dispatch: DispatchIdentityV1,
    launch: LaunchGeometryV1,
    invocation: SimulationInvocationV1,
) -> Result<ExecutionScopeV1, TraceAdapterErrorV1> {
    let workgroup = [
        u32::try_from(invocation.workgroup[0])
            .map_err(|_| TraceAdapterErrorV1::UnrepresentableGeometry)?,
        u32::try_from(invocation.workgroup[1])
            .map_err(|_| TraceAdapterErrorV1::UnrepresentableGeometry)?,
        u32::try_from(invocation.workgroup[2])
            .map_err(|_| TraceAdapterErrorV1::UnrepresentableGeometry)?,
    ];
    let linear = launch
        .linear_local_workitem(invocation.local)
        .ok_or(TraceAdapterErrorV1::UnrepresentableGeometry)?;
    let width = u64::from(launch.wave_width().lanes());
    let wave =
        u32::try_from(linear / width).map_err(|_| TraceAdapterErrorV1::UnrepresentableGeometry)?;
    let lane =
        u16::try_from(linear % width).map_err(|_| TraceAdapterErrorV1::UnrepresentableGeometry)?;
    let mut mask = 0_u64;
    let first = u64::from(wave) * width;
    for candidate in 0..width {
        let local_linear = first + candidate;
        if local_linear >= launch.workitems_per_workgroup() {
            break;
        }
        let x = local_linear % u64::from(invocation.workgroup_size[0]);
        let yz = local_linear / u64::from(invocation.workgroup_size[0]);
        let y = yz % u64::from(invocation.workgroup_size[1]);
        let z = yz / u64::from(invocation.workgroup_size[1]);
        let locals = [x, y, z];
        let active = (0..3).all(|axis| {
            u64::from(workgroup[axis]) * u64::from(invocation.workgroup_size[axis]) + locals[axis]
                < invocation.launch_extent[axis]
        });
        if active {
            mask |= 1_u64 << candidate;
        }
    }
    let mask = ActiveMaskV1::new(launch.wave_width(), mask)?;
    Ok(ExecutionScopeV1::lane(
        dispatch,
        workgroup,
        wave,
        lane,
        invocation.global,
        mask,
    ))
}

fn workgroup_scope(
    dispatch: DispatchIdentityV1,
    invocation: SimulationInvocationV1,
) -> Option<ExecutionScopeV1> {
    Some(ExecutionScopeV1::workgroup(
        dispatch,
        [
            u32::try_from(invocation.workgroup[0]).ok()?,
            u32::try_from(invocation.workgroup[1]).ok()?,
            u32::try_from(invocation.workgroup[2]).ok()?,
        ],
    ))
}

fn configuration_identity(
    module: &AdmittedSimulationModuleV1,
    request: &SimulationRequestV1,
    target: SimulationTargetV1,
) -> Result<OpaqueIdentityV1, TraceAdapterErrorV1> {
    let mut digest = Sha256::new();
    digest.update(CONFIGURATION_IDENTITY_DOMAIN_V1);
    digest.update(module.identity().digest());
    digest.update(module.identity().canonical_length().to_le_bytes());
    hash_bytes(&mut digest, request.kernel.as_str().as_bytes());
    for extent in request.grid.0 {
        digest.update(extent.to_le_bytes());
    }
    for extent in request.workgroup.0 {
        digest.update(extent.to_le_bytes());
    }
    digest.update([match target.index_width() {
        IndexWidthV1::Bits32 => 32,
        IndexWidthV1::Bits64 => 64,
    }]);
    digest.update((request.arguments.len() as u64).to_le_bytes());
    for argument in &request.arguments {
        match argument {
            SimulationArgumentV1::Scalar(value) => {
                digest.update([0, scalar_tag(value.ty())]);
                digest.update(value.bits().to_le_bytes());
            }
            SimulationArgumentV1::Buffer(buffer) => {
                digest.update([1, scalar_tag(buffer.element()), access_tag(buffer.access())]);
                digest.update(buffer.alignment().to_le_bytes());
                digest.update((buffer.bytes().len() as u64).to_le_bytes());
                digest.update(buffer.bytes());
                for initialized in buffer.initialized() {
                    digest.update([u8::from(*initialized)]);
                }
            }
            SimulationArgumentV1::BufferView(view) => {
                digest.update([2, scalar_tag(view.element()), access_tag(view.access())]);
                digest.update(view.backing().0.to_le_bytes());
                digest.update(view.alignment().to_le_bytes());
                digest.update((view.byte_offset() as u64).to_le_bytes());
                digest.update((view.elements() as u64).to_le_bytes());
            }
        }
    }
    digest.update((request.shared_buffers.len() as u64).to_le_bytes());
    for shared in &request.shared_buffers {
        digest.update(shared.id.0.to_le_bytes());
        digest.update([
            scalar_tag(shared.buffer.element()),
            access_tag(shared.buffer.access()),
        ]);
        digest.update(shared.buffer.alignment().to_le_bytes());
        digest.update((shared.buffer.bytes().len() as u64).to_le_bytes());
        digest.update(shared.buffer.bytes());
        for initialized in shared.buffer.initialized() {
            digest.update([u8::from(*initialized)]);
        }
    }
    let mut bytes: [u8; 32] = digest.finalize().into();
    if bytes == [0; 32] {
        bytes[0] = 1;
    }
    OpaqueIdentityV1::new(bytes).map_err(Into::into)
}

fn hash_bytes(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
}

fn map_address_space(space: AddressSpace) -> AddressSpaceV1 {
    match space {
        AddressSpace::Private => AddressSpaceV1::Private,
        AddressSpace::Workgroup => AddressSpaceV1::Workgroup,
        AddressSpace::Global => AddressSpaceV1::Global,
        AddressSpace::Constant => AddressSpaceV1::Constant,
        AddressSpace::Generic => AddressSpaceV1::Generic,
    }
}

fn access_tag(access: fe2o3_kernel_ir::AccessMode) -> u8 {
    match access {
        fe2o3_kernel_ir::AccessMode::ReadOnly => 0,
        fe2o3_kernel_ir::AccessMode::ReadWrite => 1,
    }
}

fn scalar_tag(ty: ScalarType) -> u8 {
    match ty {
        ScalarType::Bool => 0,
        ScalarType::I8 => 1,
        ScalarType::I16 => 2,
        ScalarType::I32 => 3,
        ScalarType::I64 => 4,
        ScalarType::I128 => 5,
        ScalarType::U8 => 6,
        ScalarType::U16 => 7,
        ScalarType::U32 => 8,
        ScalarType::U64 => 9,
        ScalarType::U128 => 10,
        ScalarType::Index => 11,
        ScalarType::F16 => 12,
        ScalarType::Bf16 => 13,
        ScalarType::F32 => 14,
        ScalarType::F64 => 15,
    }
}
