// Abstract, decoded-summary model only. It separates worker-wire validity from
// a narrower predicate that composes binding/dependency admission before
// abstract custody; it does not assign checks to parser or backend call sites.
// Excluded: Rust/native refinement, byte parsing, serde, SHA-256, subprocess
// and transport behavior, compiler/ISA behavior, KFD/GPU semantics, and timing.

use vstd::prelude::*;

verus! {

pub open spec fn max_worker_frame_bytes_v1() -> nat { 65 * 1024 * 1024 }
pub open spec fn max_explicit_kernarg_bytes_v1() -> nat { 1024 * 1024 }
pub open spec fn max_bindings_v1() -> nat { 128 }
pub open spec fn max_dependencies_v1() -> nat { 256 }
pub open spec fn max_sidecar_bytes_v1() -> nat { 16 * 1024 * 1024 }
pub open spec fn max_sidecar_records_v1() -> nat { 16_384 }
pub open spec fn max_u32_v1() -> nat { 0xffff_ffff }
pub open spec fn max_u64_v1() -> nat { 0xffff_ffff_ffff_ffff }

#[derive(PartialEq, Eq)]
pub enum HandshakeV1 {
    RuntimeV1,
    RuntimeV4,
    ExactRuntimeV5,
    Other,
}

#[derive(PartialEq, Eq)]
pub enum WorkerPhaseV1 {
    AwaitingHandshake,
    ReadyV5,
    AwaitingResponse,
    Terminal,
}

#[derive(PartialEq, Eq)]
pub enum MemoryScopeV1 {
    Workgroup,
    Device,
    System,
}

#[derive(PartialEq, Eq)]
pub enum MemoryOrderV1 {
    Relaxed,
    Acquire,
    Release,
    AcquireRelease,
    SequentiallyConsistent,
}

#[derive(PartialEq, Eq)]
pub enum AtomicOperationV1 {
    Add,
    Minimum,
    Maximum,
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    Exchange,
    CompareExchange,
}

#[derive(PartialEq, Eq)]
pub enum CollectiveOperationV1 {
    Barrier,
    Broadcast,
    ReduceSum,
    ReduceMinimum,
    ReduceMaximum,
    AllReduceSum,
    InclusiveScanSum,
}

#[derive(PartialEq, Eq)]
pub struct GeometryV1 {
    pub grid_x: nat,
    pub grid_y: nat,
    pub grid_z: nat,
    pub group_x: nat,
    pub group_y: nat,
    pub group_z: nat,
    pub dynamic_shared_bytes: nat,
}

pub open spec fn valid_geometry_v1(geometry: GeometryV1) -> bool {
    &&& geometry.grid_x > 0 && geometry.grid_y > 0 && geometry.grid_z > 0
    &&& geometry.group_x > 0 && geometry.group_y > 0 && geometry.group_z > 0
    &&& geometry.grid_x <= max_u32_v1()
    &&& geometry.grid_y <= max_u32_v1()
    &&& geometry.grid_z <= max_u32_v1()
    &&& geometry.group_x <= max_u32_v1()
    &&& geometry.group_y <= max_u32_v1()
    &&& geometry.group_z <= max_u32_v1()
    &&& geometry.group_x * geometry.group_y * geometry.group_z <= max_u32_v1()
    &&& geometry.dynamic_shared_bytes <= max_u32_v1()
}

pub open spec fn complete_workgroups_v1(geometry: GeometryV1) -> bool {
    &&& valid_geometry_v1(geometry)
    &&& geometry.grid_x >= geometry.group_x
    &&& geometry.grid_y >= geometry.group_y
    &&& geometry.grid_z >= geometry.group_z
    &&& geometry.grid_x % geometry.group_x == 0
    &&& geometry.grid_y % geometry.group_y == 0
    &&& geometry.grid_z % geometry.group_z == 0
}

#[derive(PartialEq, Eq)]
pub struct AtomicContractV1 {
    pub operation: AtomicOperationV1,
    pub scope: MemoryScopeV1,
    pub order: MemoryOrderV1,
    pub failure_order: Option<MemoryOrderV1>,
    pub weak: bool,
    pub geometry: GeometryV1,
}

#[derive(PartialEq, Eq)]
pub struct CollectiveContractV1 {
    pub operation: CollectiveOperationV1,
    pub scope: MemoryScopeV1,
    pub order: MemoryOrderV1,
    pub participants: nat,
    pub geometry: GeometryV1,
}

#[derive(PartialEq, Eq)]
pub enum SemanticContractV1 {
    Atomic(AtomicContractV1),
    Collective(CollectiveContractV1),
}

pub open spec fn compare_exchange_orders_are_legal_v1(
    success: MemoryOrderV1,
    failure: MemoryOrderV1,
) -> bool {
    match success {
        MemoryOrderV1::Relaxed => failure == MemoryOrderV1::Relaxed,
        MemoryOrderV1::Acquire =>
            failure == MemoryOrderV1::Relaxed || failure == MemoryOrderV1::Acquire,
        MemoryOrderV1::Release => failure == MemoryOrderV1::Relaxed,
        MemoryOrderV1::AcquireRelease =>
            failure == MemoryOrderV1::Relaxed || failure == MemoryOrderV1::Acquire,
        MemoryOrderV1::SequentiallyConsistent =>
            failure == MemoryOrderV1::Relaxed
                || failure == MemoryOrderV1::Acquire
                || failure == MemoryOrderV1::SequentiallyConsistent,
    }
}

pub open spec fn atomic_contract_is_legal_v1(contract: AtomicContractV1) -> bool {
    if contract.operation == AtomicOperationV1::CompareExchange {
        match contract.failure_order {
            Some(failure) => compare_exchange_orders_are_legal_v1(contract.order, failure),
            None => false,
        }
    } else {
        contract.failure_order.is_none() && !contract.weak
    }
}

pub open spec fn collective_participants_v1(contract: CollectiveContractV1) -> Option<nat> {
    match contract.scope {
        MemoryScopeV1::Workgroup => Some(
            contract.geometry.group_x
                * contract.geometry.group_y
                * contract.geometry.group_z,
        ),
        MemoryScopeV1::Device => Some(
            contract.geometry.grid_x
                * contract.geometry.grid_y
                * contract.geometry.grid_z,
        ),
        MemoryScopeV1::System => None,
    }
}

pub open spec fn collective_participant_product_fits_u64_v1(
    contract: CollectiveContractV1,
) -> bool {
    &&& contract.participants <= max_u64_v1()
    &&& match collective_participants_v1(contract) {
        Some(participants) => participants <= max_u64_v1(),
        None => false,
    }
}

pub open spec fn worker_contract_valid_for_v1(
    contract: SemanticContractV1,
    launch: GeometryV1,
) -> bool {
    match contract {
        SemanticContractV1::Atomic(atomic) => {
            atomic.geometry == launch
                && valid_geometry_v1(launch)
                && atomic_contract_is_legal_v1(atomic)
        }
        SemanticContractV1::Collective(collective) => {
            collective.geometry == launch
                && complete_workgroups_v1(launch)
                && collective_participant_product_fits_u64_v1(collective)
                && collective.participants > 0
                && collective_participants_v1(collective) == Some(collective.participants)
        }
    }
}

pub open spec fn direct_kfd_sidecar_contract_valid_for_v1(
    contract: SemanticContractV1,
    launch: GeometryV1,
) -> bool {
    worker_contract_valid_for_v1(contract, launch)
        && match contract {
            SemanticContractV1::Atomic(atomic) => atomic.scope != MemoryScopeV1::System,
            SemanticContractV1::Collective(collective) =>
                collective.scope == MemoryScopeV1::Workgroup,
        }
}

#[derive(PartialEq, Eq)]
pub enum SemanticKindV1 {
    Atomic,
    Collective,
    Unknown,
}

pub open spec fn expected_kind_v1(contract: SemanticContractV1) -> SemanticKindV1 {
    match contract {
        SemanticContractV1::Atomic(_) => SemanticKindV1::Atomic,
        SemanticContractV1::Collective(_) => SemanticKindV1::Collective,
    }
}

pub open spec fn atomic_request_bytes_v1(
    kernarg_bytes: nat,
    binding_count: nat,
    dependency_count: nat,
) -> nat {
    63 + kernarg_bytes + 29 * binding_count + 8 * dependency_count
}

pub open spec fn collective_request_bytes_v1(
    kernarg_bytes: nat,
    binding_count: nat,
    dependency_count: nat,
) -> nat {
    69 + kernarg_bytes + 29 * binding_count + 8 * dependency_count
}

#[derive(PartialEq, Eq)]
pub struct SemanticRequestV1 {
    pub opcode: SemanticKindV1,
    pub variant: SemanticKindV1,
    pub contract: SemanticContractV1,
    pub launch: GeometryV1,
    pub frame_bytes: nat,
    pub kernarg_bytes: nat,
    pub binding_count: nat,
    pub dependency_count: nat,
    pub binding_layout_valid: bool,
    pub dependencies_unique: bool,
    pub canonical_end: bool,
}

pub open spec fn encoded_request_bytes_v1(request: SemanticRequestV1) -> nat {
    match request.opcode {
        SemanticKindV1::Atomic => atomic_request_bytes_v1(
            request.kernarg_bytes,
            request.binding_count,
            request.dependency_count,
        ),
        SemanticKindV1::Collective => collective_request_bytes_v1(
            request.kernarg_bytes,
            request.binding_count,
            request.dependency_count,
        ),
        SemanticKindV1::Unknown => max_worker_frame_bytes_v1() + 1,
    }
}

pub open spec fn bounded_request_payload_v1(request: SemanticRequestV1) -> bool {
    &&& request.kernarg_bytes <= max_explicit_kernarg_bytes_v1()
    &&& request.binding_count <= max_bindings_v1()
    &&& request.dependency_count <= max_dependencies_v1()
    &&& request.canonical_end
}

pub open spec fn worker_wire_request_valid_v1(request: SemanticRequestV1) -> bool {
    &&& bounded_request_payload_v1(request)
    &&& request.opcode == expected_kind_v1(request.contract)
    &&& request.variant == expected_kind_v1(request.contract)
    &&& request.frame_bytes == encoded_request_bytes_v1(request)
    &&& request.frame_bytes <= max_worker_frame_bytes_v1()
    &&& worker_contract_valid_for_v1(request.contract, request.launch)
}

pub open spec fn semantic_request_valid_v1(request: SemanticRequestV1) -> bool {
    &&& worker_wire_request_valid_v1(request)
    &&& request.binding_layout_valid
    &&& request.dependencies_unique
}

pub struct WorkerStateV1 {
    pub phase: WorkerPhaseV1,
    // Composed admission attempts, not parser or backend invocation count.
    pub attempted_requests: nat,
    // Accepted only after a nonzero success response.
    pub accepted_backend_custodies: nat,
    pub request_custody: Option<RequestCustodyV1>,
    pub last_successful_contract: Option<SemanticContractV1>,
    pub last_successful_launch: Option<GeometryV1>,
}

#[derive(PartialEq, Eq)]
pub enum RequestCustodyV1 {
    InFlight(SemanticContractV1, GeometryV1),
    Indeterminate(SemanticContractV1, GeometryV1),
}

pub open spec fn initial_worker_v1() -> WorkerStateV1 {
    WorkerStateV1 {
        phase: WorkerPhaseV1::AwaitingHandshake,
        attempted_requests: 0,
        accepted_backend_custodies: 0,
        request_custody: None,
        last_successful_contract: None,
        last_successful_launch: None,
    }
}

pub open spec fn negotiate_v1(
    state: WorkerStateV1,
    handshake: HandshakeV1,
) -> WorkerStateV1 {
    if state.phase != WorkerPhaseV1::AwaitingHandshake {
        state
    } else if handshake == HandshakeV1::ExactRuntimeV5 {
        WorkerStateV1 { phase: WorkerPhaseV1::ReadyV5, ..state }
    } else {
        WorkerStateV1 { phase: WorkerPhaseV1::Terminal, ..state }
    }
}

pub open spec fn receive_request_v1(
    state: WorkerStateV1,
    request: SemanticRequestV1,
) -> WorkerStateV1 {
    if state.phase != WorkerPhaseV1::ReadyV5 {
        state
    } else if semantic_request_valid_v1(request) {
        WorkerStateV1 {
            phase: WorkerPhaseV1::AwaitingResponse,
            attempted_requests: state.attempted_requests + 1,
            request_custody: Some(RequestCustodyV1::InFlight(request.contract, request.launch)),
            ..state
        }
    } else {
        WorkerStateV1 { phase: WorkerPhaseV1::Terminal, ..state }
    }
}

#[derive(PartialEq, Eq)]
pub enum ResponseV1 {
    SuccessNonzero,
    Rejected,
    Quiescent,
    SuccessZero,
    Terminal,
    Malformed,
    Timeout,
    EndOfFile,
}

pub open spec fn response_seals_v1(response: ResponseV1) -> bool {
    response == ResponseV1::SuccessZero
        || response == ResponseV1::Terminal
        || response == ResponseV1::Malformed
        || response == ResponseV1::Timeout
        || response == ResponseV1::EndOfFile
}

pub open spec fn observe_response_v1(
    state: WorkerStateV1,
    response: ResponseV1,
) -> WorkerStateV1 {
    if state.phase != WorkerPhaseV1::AwaitingResponse {
        state
    } else if response == ResponseV1::SuccessNonzero {
        WorkerStateV1 {
            phase: WorkerPhaseV1::ReadyV5,
            accepted_backend_custodies: state.accepted_backend_custodies + 1,
            request_custody: None,
            last_successful_contract: match state.request_custody {
                Some(RequestCustodyV1::InFlight(contract, _)) => Some(contract),
                _ => state.last_successful_contract,
            },
            last_successful_launch: match state.request_custody {
                Some(RequestCustodyV1::InFlight(_, launch)) => Some(launch),
                _ => state.last_successful_launch,
            },
            ..state
        }
    } else if response == ResponseV1::Rejected || response == ResponseV1::Quiescent {
        WorkerStateV1 {
            phase: WorkerPhaseV1::ReadyV5,
            request_custody: None,
            ..state
        }
    } else {
        WorkerStateV1 {
            phase: WorkerPhaseV1::Terminal,
            request_custody: match state.request_custody {
                Some(RequestCustodyV1::InFlight(contract, launch)) =>
                    Some(RequestCustodyV1::Indeterminate(contract, launch)),
                custody => custody,
            },
            ..state
        }
    }
}

pub open spec fn worker_state_reachable_invariant_v1(state: WorkerStateV1) -> bool {
    &&& state.accepted_backend_custodies <= state.attempted_requests
    &&& (state.accepted_backend_custodies > 0
        <==> state.last_successful_contract.is_some())
    &&& state.last_successful_contract.is_some() == state.last_successful_launch.is_some()
    &&& match (state.last_successful_contract, state.last_successful_launch) {
        (Some(contract), Some(launch)) => worker_contract_valid_for_v1(contract, launch),
        (None, None) => true,
        _ => false,
    }
    &&& match state.request_custody {
        Some(RequestCustodyV1::InFlight(contract, launch))
        | Some(RequestCustodyV1::Indeterminate(contract, launch)) =>
            worker_contract_valid_for_v1(contract, launch),
        None => true,
    }
    &&& match state.phase {
        WorkerPhaseV1::AwaitingHandshake => {
            &&& state.attempted_requests == 0
            &&& state.accepted_backend_custodies == 0
            &&& state.request_custody.is_none()
            &&& state.last_successful_contract.is_none()
        }
        WorkerPhaseV1::ReadyV5 => state.request_custody.is_none(),
        WorkerPhaseV1::AwaitingResponse => {
            &&& state.accepted_backend_custodies < state.attempted_requests
            &&& match state.request_custody {
                Some(RequestCustodyV1::InFlight(_, _)) => true,
                _ => false,
            }
        }
        WorkerPhaseV1::Terminal => {
            &&& match state.request_custody {
                Some(RequestCustodyV1::InFlight(_, _)) => false,
                Some(RequestCustodyV1::Indeterminate(_, _)) =>
                    state.accepted_backend_custodies < state.attempted_requests,
                None => true,
            }
        }
    }
}

pub struct SemanticPublicationV1 {
    pub runtime_event: nat,
    pub runtime_event_sequence: nat,
    pub dispatch: nat,
    pub dispatch_shape: nat,
    pub launch: GeometryV1,
}

pub struct SemanticObservationV1 {
    pub dispatch: nat,
    pub semantic_contract: Option<SemanticContractV1>,
}

pub struct SemanticSidecarRecordV1 {
    pub runtime_event: nat,
    pub runtime_event_sequence: nat,
    pub dispatch: nat,
    pub dispatch_shape: nat,
    pub launch: GeometryV1,
    pub semantic_contract: Option<SemanticContractV1>,
}

#[derive(PartialEq, Eq)]
pub enum SidecarSchemaV1 {
    ExactV1,
    Other,
}

pub struct SidecarSummaryV1 {
    pub schema: SidecarSchemaV1,
    pub schema_version: nat,
    pub encoded_byte_len: nat,
    pub runtime_profile: nat,
    pub runtime_capture_scope: nat,
    pub runtime_profile_dispatches: nat,
    pub typed_semantic_contracts: nat,
    pub ordinary_dispatches: nat,
    pub complete_retained_dispatch_classification: bool,
    pub complete_runtime_operation_history: bool,
    pub runtime_profile_complete_runtime_operation_history: bool,
}

pub open spec fn typed_record_count_v1(records: Seq<SemanticSidecarRecordV1>) -> nat
    decreases records.len(),
{
    if records.len() == 0 {
        0
    } else {
        let last = records[records.len() as int - 1];
        typed_record_count_v1(records.subrange(0, records.len() as int - 1))
            + if last.semantic_contract.is_some() { 1nat } else { 0nat }
    }
}

pub open spec fn sidecar_summary_valid_for_v1(
    summary: SidecarSummaryV1,
    records: Seq<SemanticSidecarRecordV1>,
) -> bool {
    &&& summary.schema == SidecarSchemaV1::ExactV1
    &&& summary.schema_version == 1
    &&& 0 < summary.encoded_byte_len <= max_sidecar_bytes_v1()
    &&& summary.runtime_profile > 0
    &&& summary.runtime_capture_scope > 0
    &&& records.len() <= max_sidecar_records_v1()
    &&& summary.runtime_profile_dispatches == records.len()
    &&& summary.typed_semantic_contracts == typed_record_count_v1(records)
    &&& summary.ordinary_dispatches
        == records.len() - typed_record_count_v1(records)
    &&& summary.complete_retained_dispatch_classification
    &&& summary.complete_runtime_operation_history
        == summary.runtime_profile_complete_runtime_operation_history
}

pub open spec fn profileable_publication_v1(publication: SemanticPublicationV1) -> bool {
    &&& publication.runtime_event > 0
    &&& publication.dispatch > 0
    &&& publication.dispatch_shape > 0
    &&& valid_geometry_v1(publication.launch)
}

pub open spec fn canonical_sidecar_record_v1(
    publication: SemanticPublicationV1,
    semantic_contract: Option<SemanticContractV1>,
) -> SemanticSidecarRecordV1 {
    SemanticSidecarRecordV1 {
        runtime_event: publication.runtime_event,
        runtime_event_sequence: publication.runtime_event_sequence,
        dispatch: publication.dispatch,
        dispatch_shape: publication.dispatch_shape,
        launch: publication.launch,
        semantic_contract,
    }
}

pub open spec fn ordered_unique_publications_v1(
    publications: Seq<SemanticPublicationV1>,
) -> bool {
    forall |i: int, j: int| 0 <= i < j < publications.len() ==> {
        &&& publications[i].runtime_event_sequence < publications[j].runtime_event_sequence
        &&& publications[i].dispatch != publications[j].dispatch
    }
}

pub open spec fn exact_sidecar_sequence_join_v1(
    summary: SidecarSummaryV1,
    publications: Seq<SemanticPublicationV1>,
    observations: Seq<SemanticObservationV1>,
    records: Seq<SemanticSidecarRecordV1>,
) -> bool {
    &&& sidecar_summary_valid_for_v1(summary, records)
    &&& publications.len() == records.len()
    &&& observations.len() == records.len()
    &&& ordered_unique_publications_v1(publications)
    &&& forall |i: int| 0 <= i < records.len() ==> {
        &&& profileable_publication_v1(publications[i])
        &&& observations[i].dispatch == publications[i].dispatch
        &&& match observations[i].semantic_contract {
            Some(contract) =>
                direct_kfd_sidecar_contract_valid_for_v1(contract, publications[i].launch),
            None => true,
        }
        &&& records[i] == canonical_sidecar_record_v1(
            publications[i],
            observations[i].semantic_contract,
        )
    }
}

pub proof fn exact_v5_handshake_opens_ready_worker_v1()
    ensures {
        let next = negotiate_v1(initial_worker_v1(), HandshakeV1::ExactRuntimeV5);
        &&& next.phase == WorkerPhaseV1::ReadyV5
        &&& next.attempted_requests == 0
        &&& next.accepted_backend_custodies == 0
    },
{
}

pub proof fn non_v5_handshake_seals_without_forwarding_v1(handshake: HandshakeV1)
    requires handshake != HandshakeV1::ExactRuntimeV5,
    ensures {
        let next = negotiate_v1(initial_worker_v1(), handshake);
        &&& next.phase == WorkerPhaseV1::Terminal
        &&& next.attempted_requests == 0
        &&& next.accepted_backend_custodies == 0
        &&& next.request_custody.is_none()
    },
{
}

pub proof fn bounded_semantic_request_formulas_fit_frame_v1(
    kernarg_bytes: nat,
    binding_count: nat,
    dependency_count: nat,
)
    requires
        kernarg_bytes <= max_explicit_kernarg_bytes_v1(),
        binding_count <= max_bindings_v1(),
        dependency_count <= max_dependencies_v1(),
    ensures
        atomic_request_bytes_v1(kernarg_bytes, binding_count, dependency_count)
            == 63 + kernarg_bytes + 29 * binding_count + 8 * dependency_count,
        collective_request_bytes_v1(kernarg_bytes, binding_count, dependency_count)
            == 69 + kernarg_bytes + 29 * binding_count + 8 * dependency_count,
        atomic_request_bytes_v1(kernarg_bytes, binding_count, dependency_count)
            <= max_worker_frame_bytes_v1(),
        collective_request_bytes_v1(kernarg_bytes, binding_count, dependency_count)
            <= max_worker_frame_bytes_v1(),
{
    assert(69 + 1024 * 1024 + 29 * 128 + 8 * 256 < 65 * 1024 * 1024) by (compute);
}

pub proof fn geometry_and_collective_products_have_exact_integer_widths_v1(
    geometry: GeometryV1,
    collective: CollectiveContractV1,
)
    requires
        valid_geometry_v1(geometry),
        worker_contract_valid_for_v1(
            SemanticContractV1::Collective(collective),
            geometry,
        ),
    ensures
        geometry.grid_x <= max_u32_v1(),
        geometry.grid_y <= max_u32_v1(),
        geometry.grid_z <= max_u32_v1(),
        geometry.group_x <= max_u32_v1(),
        geometry.group_y <= max_u32_v1(),
        geometry.group_z <= max_u32_v1(),
        geometry.dynamic_shared_bytes <= max_u32_v1(),
        collective.participants <= max_u64_v1(),
        match collective_participants_v1(collective) {
            Some(participants) => participants <= max_u64_v1(),
            None => false,
        },
{
}

pub proof fn composed_pre_custody_admission_is_narrower_than_worker_wire_v1(
    request: SemanticRequestV1,
)
    requires
        worker_wire_request_valid_v1(request),
        !request.binding_layout_valid || !request.dependencies_unique,
    ensures
        worker_wire_request_valid_v1(request),
        !semantic_request_valid_v1(request),
{
}

pub proof fn malformed_request_is_pre_custody_and_terminal_v1(
    state: WorkerStateV1,
    request: SemanticRequestV1,
)
    requires
        state.phase == WorkerPhaseV1::ReadyV5,
        !semantic_request_valid_v1(request),
    ensures {
        let next = receive_request_v1(state, request);
        &&& next.phase == WorkerPhaseV1::Terminal
        &&& next.attempted_requests == state.attempted_requests
        &&& next.accepted_backend_custodies == state.accepted_backend_custodies
        &&& next.request_custody == state.request_custody
    },
{
}

pub proof fn valid_request_starts_exact_in_flight_attempt_without_acceptance_v1(
    state: WorkerStateV1,
    request: SemanticRequestV1,
)
    requires
        state.phase == WorkerPhaseV1::ReadyV5,
        semantic_request_valid_v1(request),
    ensures {
        let next = receive_request_v1(state, request);
        &&& next.phase == WorkerPhaseV1::AwaitingResponse
        &&& next.attempted_requests == state.attempted_requests + 1
        &&& next.accepted_backend_custodies == state.accepted_backend_custodies
        &&& next.request_custody
            == Some(RequestCustodyV1::InFlight(request.contract, request.launch))
    },
{
}

pub proof fn worker_wire_and_direct_sidecar_validity_are_distinct_v1(
    system_atomic: AtomicContractV1,
    device_collective: CollectiveContractV1,
    system_collective: CollectiveContractV1,
)
    requires
        system_atomic.scope == MemoryScopeV1::System,
        valid_geometry_v1(system_atomic.geometry),
        atomic_contract_is_legal_v1(system_atomic),
        device_collective.scope == MemoryScopeV1::Device,
        complete_workgroups_v1(device_collective.geometry),
        collective_participant_product_fits_u64_v1(device_collective),
        device_collective.participants > 0,
        device_collective.participants
            == device_collective.geometry.grid_x
                * device_collective.geometry.grid_y
                * device_collective.geometry.grid_z,
        system_collective.scope == MemoryScopeV1::System,
    ensures
        worker_contract_valid_for_v1(
            SemanticContractV1::Atomic(system_atomic),
            system_atomic.geometry,
        ),
        !direct_kfd_sidecar_contract_valid_for_v1(
            SemanticContractV1::Atomic(system_atomic),
            system_atomic.geometry,
        ),
        worker_contract_valid_for_v1(
            SemanticContractV1::Collective(device_collective),
            device_collective.geometry,
        ),
        !direct_kfd_sidecar_contract_valid_for_v1(
            SemanticContractV1::Collective(device_collective),
            device_collective.geometry,
        ),
        !worker_contract_valid_for_v1(
            SemanticContractV1::Collective(system_collective),
            system_collective.geometry,
        ),
{
}

pub proof fn recoverable_response_restores_ready_without_success_v1(
    state: WorkerStateV1,
    response: ResponseV1,
)
    requires
        state.phase == WorkerPhaseV1::AwaitingResponse,
        response == ResponseV1::Rejected || response == ResponseV1::Quiescent,
    ensures {
        let next = observe_response_v1(state, response);
        &&& next.phase == WorkerPhaseV1::ReadyV5
        &&& next.attempted_requests == state.attempted_requests
        &&& next.accepted_backend_custodies == state.accepted_backend_custodies
        &&& next.request_custody.is_none()
        &&& next.last_successful_contract == state.last_successful_contract
    },
{
}

pub proof fn nonzero_success_accepts_exact_in_flight_contract_once_v1(
    state: WorkerStateV1,
    contract: SemanticContractV1,
    launch: GeometryV1,
)
    requires
        state.phase == WorkerPhaseV1::AwaitingResponse,
        state.request_custody == Some(RequestCustodyV1::InFlight(contract, launch)),
    ensures {
        let next = observe_response_v1(state, ResponseV1::SuccessNonzero);
        &&& next.phase == WorkerPhaseV1::ReadyV5
        &&& next.attempted_requests == state.attempted_requests
        &&& next.accepted_backend_custodies == state.accepted_backend_custodies + 1
        &&& next.request_custody.is_none()
        &&& next.last_successful_contract == Some(contract)
        &&& next.last_successful_launch == Some(launch)
    },
{
}

pub proof fn terminal_response_seals_without_fabricating_success_v1(
    state: WorkerStateV1,
    contract: SemanticContractV1,
    launch: GeometryV1,
    response: ResponseV1,
)
    requires
        state.phase == WorkerPhaseV1::AwaitingResponse,
        state.request_custody == Some(RequestCustodyV1::InFlight(contract, launch)),
        response_seals_v1(response),
    ensures {
        let next = observe_response_v1(state, response);
        &&& next.phase == WorkerPhaseV1::Terminal
        &&& next.attempted_requests == state.attempted_requests
        &&& next.accepted_backend_custodies == state.accepted_backend_custodies
        &&& next.request_custody
            == Some(RequestCustodyV1::Indeterminate(contract, launch))
        &&& next.last_successful_contract == state.last_successful_contract
    },
{
}

pub proof fn terminal_worker_is_absorbing_v1(
    state: WorkerStateV1,
    request: SemanticRequestV1,
    response: ResponseV1,
)
    requires state.phase == WorkerPhaseV1::Terminal,
    ensures
        receive_request_v1(state, request) == state,
        observe_response_v1(state, response) == state,
{
}

pub proof fn initial_worker_satisfies_reachable_invariant_v1()
    ensures worker_state_reachable_invariant_v1(initial_worker_v1()),
{
}

pub proof fn negotiation_preserves_reachable_invariant_v1(
    state: WorkerStateV1,
    handshake: HandshakeV1,
)
    requires
        worker_state_reachable_invariant_v1(state),
        state.phase == WorkerPhaseV1::AwaitingHandshake,
    ensures worker_state_reachable_invariant_v1(negotiate_v1(state, handshake)),
{
}

pub proof fn request_transition_preserves_reachable_invariant_v1(
    state: WorkerStateV1,
    request: SemanticRequestV1,
)
    requires
        worker_state_reachable_invariant_v1(state),
        state.phase == WorkerPhaseV1::ReadyV5,
    ensures worker_state_reachable_invariant_v1(receive_request_v1(state, request)),
{
}

pub proof fn response_transition_preserves_reachable_invariant_v1(
    state: WorkerStateV1,
    response: ResponseV1,
)
    requires
        worker_state_reachable_invariant_v1(state),
        state.phase == WorkerPhaseV1::AwaitingResponse,
    ensures worker_state_reachable_invariant_v1(observe_response_v1(state, response)),
{
}

pub proof fn composed_success_preserves_exact_contract_and_reachability_v1(
    request: SemanticRequestV1,
)
    requires semantic_request_valid_v1(request),
    ensures {
        let ready = negotiate_v1(initial_worker_v1(), HandshakeV1::ExactRuntimeV5);
        let attempted = receive_request_v1(ready, request);
        let accepted = observe_response_v1(attempted, ResponseV1::SuccessNonzero);
        &&& accepted.phase == WorkerPhaseV1::ReadyV5
        &&& accepted.attempted_requests == 1
        &&& accepted.accepted_backend_custodies == 1
        &&& accepted.request_custody.is_none()
        &&& accepted.last_successful_contract == Some(request.contract)
        &&& accepted.last_successful_launch == Some(request.launch)
        &&& worker_state_reachable_invariant_v1(accepted)
    },
{
}

pub proof fn bounded_sidecar_summary_matches_records_v1(
    summary: SidecarSummaryV1,
    records: Seq<SemanticSidecarRecordV1>,
)
    requires
        summary.schema == SidecarSchemaV1::ExactV1,
        summary.schema_version == 1,
        0 < summary.encoded_byte_len <= max_sidecar_bytes_v1(),
        summary.runtime_profile > 0,
        summary.runtime_capture_scope > 0,
        records.len() <= max_sidecar_records_v1(),
        summary.runtime_profile_dispatches == records.len(),
        summary.typed_semantic_contracts == typed_record_count_v1(records),
        summary.ordinary_dispatches == records.len() - typed_record_count_v1(records),
        summary.complete_retained_dispatch_classification,
        summary.complete_runtime_operation_history
            == summary.runtime_profile_complete_runtime_operation_history,
    ensures sidecar_summary_valid_for_v1(summary, records),
{
}

pub proof fn exact_sidecar_sequence_join_is_ordered_bijection_v1(
    summary: SidecarSummaryV1,
    publications: Seq<SemanticPublicationV1>,
    observations: Seq<SemanticObservationV1>,
    records: Seq<SemanticSidecarRecordV1>,
)
    requires exact_sidecar_sequence_join_v1(summary, publications, observations, records),
    ensures
        publications.len() == observations.len(),
        observations.len() == records.len(),
        ordered_unique_publications_v1(publications),
        forall |i: int| 0 <= i < records.len() ==> {
            &&& observations[i].dispatch == publications[i].dispatch
            &&& records[i] == canonical_sidecar_record_v1(
                publications[i],
                observations[i].semantic_contract,
            )
        },
{
}

pub proof fn substituted_sidecar_record_cannot_join_sequence_v1(
    summary: SidecarSummaryV1,
    publications: Seq<SemanticPublicationV1>,
    observations: Seq<SemanticObservationV1>,
    records: Seq<SemanticSidecarRecordV1>,
    index: int,
)
    requires
        0 <= index < records.len(),
        publications.len() == records.len(),
        observations.len() == records.len(),
        records[index] != canonical_sidecar_record_v1(
            publications[index],
            observations[index].semantic_contract,
        ),
    ensures !exact_sidecar_sequence_join_v1(summary, publications, observations, records),
{
    if exact_sidecar_sequence_join_v1(summary, publications, observations, records) {
        assert(records[index] == canonical_sidecar_record_v1(
            publications[index],
            observations[index].semantic_contract,
        ));
    }
}

}
