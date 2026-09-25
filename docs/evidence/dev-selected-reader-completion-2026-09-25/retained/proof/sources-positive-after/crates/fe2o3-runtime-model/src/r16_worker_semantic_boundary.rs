//! Abstract Runtime Worker V5 semantic-transport and sidecar boundary.
//!
//! This caller-constructible, `no_std` model summarizes already-decoded fields.
//! Request acceptance is a composed pre-custody boundary: it combines bounded
//! worker-wire checks with binding and dependency admission, but does not say
//! which production layer performs a check or whether a backend method was
//! invoked before rejection. Attempted requests, in-flight/indeterminate
//! custody, and accepted nonzero-success responses are tracked separately;
//! none is a concrete parser-success or backend-call counter.
//!
//! This model does not parse worker bytes, run a subprocess, serialize with
//! serde, compute or verify SHA-256, authenticate a worker, refine the
//! production Rust implementation, or establish compiler, KFD, native atomic,
//! collective, subprocess, transport, timing, or GPU behavior.

use alloc::vec::Vec;

pub const MAX_R16_WORKER_FRAME_BYTES_V1: usize = 65 * 1024 * 1024;
pub const MAX_R16_EXPLICIT_KERNARG_BYTES_V1: usize = 1024 * 1024;
pub const MAX_R16_BINDINGS_V1: usize = 128;
pub const MAX_R16_DEPENDENCIES_V1: usize = 256;
pub const R16_DEVICE_POINTER_BYTES_V1: usize = 8;
pub const MAX_R16_SEMANTIC_SIDECAR_BYTES_V1: usize = 16 * 1024 * 1024;
pub const MAX_R16_SEMANTIC_SIDECAR_RECORDS_V1: usize = 16_384;

const R16_ATOMIC_FIXED_FRAME_BYTES_V1: usize = 63;
const R16_COLLECTIVE_FIXED_FRAME_BYTES_V1: usize = 69;
const R16_BINDING_FRAME_BYTES_V1: usize = 29;
const R16_DEPENDENCY_FRAME_BYTES_V1: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R16WorkerHandshakeV1 {
    RuntimeV1,
    RuntimeV4,
    ExactRuntimeV5,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R16WorkerPhaseV1 {
    AwaitingHandshake,
    ReadyV5,
    AwaitingResponse,
    Terminal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R16MemoryScopeV1 {
    Workgroup,
    Device,
    System,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R16MemoryOrderV1 {
    Relaxed,
    Acquire,
    Release,
    AcquireRelease,
    SequentiallyConsistent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R16AtomicOperationV1 {
    Add,
    Minimum,
    Maximum,
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    Exchange,
    CompareExchange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R16CollectiveOperationV1 {
    Barrier,
    Broadcast,
    ReduceSum,
    ReduceMinimum,
    ReduceMaximum,
    AllReduceSum,
    InclusiveScanSum,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R16LaunchGeometryV1 {
    pub grid: [u32; 3],
    pub workgroup: [u32; 3],
    pub dynamic_shared_bytes: u32,
}

impl R16LaunchGeometryV1 {
    pub fn is_worker_valid(self) -> bool {
        !self.grid.contains(&0)
            && !self.workgroup.contains(&0)
            && self
                .workgroup
                .into_iter()
                .try_fold(1_u32, u32::checked_mul)
                .is_some()
    }

    pub fn has_complete_workgroups(self) -> bool {
        self.grid
            .into_iter()
            .zip(self.workgroup)
            .all(|(grid, workgroup)| grid >= workgroup && grid.is_multiple_of(workgroup))
    }

    fn participant_count(self, scope: R16MemoryScopeV1) -> Option<u64> {
        let dimensions = match scope {
            R16MemoryScopeV1::Workgroup => self.workgroup,
            R16MemoryScopeV1::Device => self.grid,
            R16MemoryScopeV1::System => return None,
        };
        dimensions.into_iter().try_fold(1_u64, |product, value| {
            product.checked_mul(u64::from(value))
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R16AtomicContractV1 {
    pub operation: R16AtomicOperationV1,
    pub scope: R16MemoryScopeV1,
    pub order: R16MemoryOrderV1,
    pub failure_order: Option<R16MemoryOrderV1>,
    pub weak: bool,
    pub geometry: R16LaunchGeometryV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R16CollectiveContractV1 {
    pub operation: R16CollectiveOperationV1,
    pub scope: R16MemoryScopeV1,
    pub order: R16MemoryOrderV1,
    pub participants: u64,
    pub geometry: R16LaunchGeometryV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R16SemanticContractV1 {
    Atomic(R16AtomicContractV1),
    Collective(R16CollectiveContractV1),
}

impl R16SemanticContractV1 {
    /// Worker-wire validity deliberately permits System-scope atomics and
    /// Device-scope collectives. Backend authority remains a later boundary.
    pub fn is_worker_wire_valid_for(self, launch: R16LaunchGeometryV1) -> bool {
        match self {
            Self::Atomic(contract) => {
                contract.geometry == launch
                    && launch.is_worker_valid()
                    && atomic_contract_is_legal_v1(contract)
            }
            Self::Collective(contract) => {
                contract.geometry == launch
                    && launch.is_worker_valid()
                    && launch.has_complete_workgroups()
                    && contract.participants != 0
                    && launch.participant_count(contract.scope) == Some(contract.participants)
            }
        }
    }

    /// The direct-KFD semantic sidecar is intentionally narrower than Worker
    /// V5's generic wire contract.
    pub fn is_direct_kfd_sidecar_valid_for(self, launch: R16LaunchGeometryV1) -> bool {
        self.is_worker_wire_valid_for(launch)
            && match self {
                Self::Atomic(contract) => contract.scope != R16MemoryScopeV1::System,
                Self::Collective(contract) => contract.scope == R16MemoryScopeV1::Workgroup,
            }
    }
}

fn atomic_contract_is_legal_v1(contract: R16AtomicContractV1) -> bool {
    match (contract.operation, contract.failure_order) {
        (R16AtomicOperationV1::CompareExchange, Some(failure)) => {
            compare_exchange_orders_are_legal_v1(contract.order, failure)
        }
        (R16AtomicOperationV1::CompareExchange, None) => false,
        (_, None) => !contract.weak,
        (_, Some(_)) => false,
    }
}

fn compare_exchange_orders_are_legal_v1(
    success: R16MemoryOrderV1,
    failure: R16MemoryOrderV1,
) -> bool {
    match success {
        R16MemoryOrderV1::Relaxed => failure == R16MemoryOrderV1::Relaxed,
        R16MemoryOrderV1::Acquire => matches!(
            failure,
            R16MemoryOrderV1::Relaxed | R16MemoryOrderV1::Acquire
        ),
        R16MemoryOrderV1::Release => failure == R16MemoryOrderV1::Relaxed,
        R16MemoryOrderV1::AcquireRelease => matches!(
            failure,
            R16MemoryOrderV1::Relaxed | R16MemoryOrderV1::Acquire
        ),
        R16MemoryOrderV1::SequentiallyConsistent => matches!(
            failure,
            R16MemoryOrderV1::Relaxed
                | R16MemoryOrderV1::Acquire
                | R16MemoryOrderV1::SequentiallyConsistent
        ),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R16SemanticOperationKindV1 {
    Atomic,
    Collective,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R16BindingSummaryV1 {
    pub kernarg_byte_offset: u32,
    pub kernarg_patch_is_zero: bool,
    pub region_byte_offset: u64,
    pub region_byte_len: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct R16SemanticRequestV1 {
    pub opcode: R16SemanticOperationKindV1,
    pub variant: R16SemanticOperationKindV1,
    pub contract: R16SemanticContractV1,
    pub launch: R16LaunchGeometryV1,
    pub explicit_kernarg_bytes: usize,
    pub bindings: Vec<R16BindingSummaryV1>,
    pub dependencies: Vec<u64>,
    pub trailing_bytes: usize,
}

impl R16SemanticRequestV1 {
    pub fn encoded_frame_bytes(&self) -> Option<usize> {
        let fixed = match self.opcode {
            R16SemanticOperationKindV1::Atomic => R16_ATOMIC_FIXED_FRAME_BYTES_V1,
            R16SemanticOperationKindV1::Collective => R16_COLLECTIVE_FIXED_FRAME_BYTES_V1,
            R16SemanticOperationKindV1::Unknown => return None,
        };
        fixed
            .checked_add(self.explicit_kernarg_bytes)?
            .checked_add(
                self.bindings
                    .len()
                    .checked_mul(R16_BINDING_FRAME_BYTES_V1)?,
            )?
            .checked_add(
                self.dependencies
                    .len()
                    .checked_mul(R16_DEPENDENCY_FRAME_BYTES_V1)?,
            )?
            .checked_add(self.trailing_bytes)
    }

    pub fn is_worker_wire_valid(&self) -> bool {
        let expected = match self.contract {
            R16SemanticContractV1::Atomic(_) => R16SemanticOperationKindV1::Atomic,
            R16SemanticContractV1::Collective(_) => R16SemanticOperationKindV1::Collective,
        };
        self.opcode == expected
            && self.variant == expected
            && self.trailing_bytes == 0
            && self.explicit_kernarg_bytes <= MAX_R16_EXPLICIT_KERNARG_BYTES_V1
            && self.bindings.len() <= MAX_R16_BINDINGS_V1
            && self.dependencies.len() <= MAX_R16_DEPENDENCIES_V1
            && self
                .encoded_frame_bytes()
                .is_some_and(|bytes| bytes <= MAX_R16_WORKER_FRAME_BYTES_V1)
            && self.contract.is_worker_wire_valid_for(self.launch)
    }

    /// Composes the decoded Worker frame with backend/facade binding and
    /// dependency admission without assigning those checks to a call site.
    pub fn is_composed_pre_custody_valid(&self) -> bool {
        self.is_worker_wire_valid()
            && bindings_are_valid_v1(self.explicit_kernarg_bytes, &self.bindings)
            && dependencies_are_unique_v1(&self.dependencies)
    }
}

fn bindings_are_valid_v1(explicit_kernarg_bytes: usize, bindings: &[R16BindingSummaryV1]) -> bool {
    bindings.iter().enumerate().all(|(index, binding)| {
        let start = binding.kernarg_byte_offset as usize;
        let Some(end) = start.checked_add(R16_DEVICE_POINTER_BYTES_V1) else {
            return false;
        };
        binding
            .kernarg_byte_offset
            .is_multiple_of(R16_DEVICE_POINTER_BYTES_V1 as u32)
            && end <= explicit_kernarg_bytes
            && binding.kernarg_patch_is_zero
            && binding.region_byte_len != 0
            && binding
                .region_byte_offset
                .checked_add(binding.region_byte_len)
                .is_some()
            && bindings[..index].iter().all(|prior| {
                let prior_start = prior.kernarg_byte_offset as usize;
                let prior_end = prior_start + R16_DEVICE_POINTER_BYTES_V1;
                end <= prior_start || prior_end <= start
            })
    })
}

fn dependencies_are_unique_v1(dependencies: &[u64]) -> bool {
    dependencies
        .iter()
        .enumerate()
        .all(|(index, dependency)| !dependencies[..index].contains(dependency))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R16WorkerResponseV1 {
    Success { handle: u64 },
    Rejected,
    Quiescent,
    Terminal,
    Malformed,
    Timeout,
    EndOfFile,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R16WorkerOutcomeV1 {
    Success { handle: u64 },
    Rejected,
    Quiescent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R16WorkerModelErrorV1 {
    HandshakeMismatch,
    InvalidRequestBeforeCustody,
    IllegalTransition,
    Terminal,
    InvariantViolation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum R16RequestCustodyV1 {
    InFlight(R16SemanticRequestV1),
    Indeterminate(R16SemanticRequestV1),
}

impl R16RequestCustodyV1 {
    pub const fn request(&self) -> &R16SemanticRequestV1 {
        match self {
            Self::InFlight(request) | Self::Indeterminate(request) => request,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct R16WorkerSemanticBoundaryModelV1 {
    phase: R16WorkerPhaseV1,
    attempted_requests: u64,
    accepted_backend_custodies: u64,
    request_custody: Option<R16RequestCustodyV1>,
    last_successful_request: Option<R16SemanticRequestV1>,
}

impl Default for R16WorkerSemanticBoundaryModelV1 {
    fn default() -> Self {
        Self::new_model_only()
    }
}

impl R16WorkerSemanticBoundaryModelV1 {
    pub const fn new_model_only() -> Self {
        Self {
            phase: R16WorkerPhaseV1::AwaitingHandshake,
            attempted_requests: 0,
            accepted_backend_custodies: 0,
            request_custody: None,
            last_successful_request: None,
        }
    }

    pub const fn phase(&self) -> R16WorkerPhaseV1 {
        self.phase
    }

    /// Number of well-formed requests whose composed admission was attempted.
    pub const fn attempted_requests(&self) -> u64 {
        self.attempted_requests
    }

    /// Number of requests classified as accepted by a nonzero success.
    /// This is not a count of decoder successes or backend method invocations.
    pub const fn accepted_backend_custodies(&self) -> u64 {
        self.accepted_backend_custodies
    }

    pub const fn request_custody(&self) -> Option<&R16RequestCustodyV1> {
        self.request_custody.as_ref()
    }

    pub const fn pending_request(&self) -> Option<&R16SemanticRequestV1> {
        match self.request_custody.as_ref() {
            Some(R16RequestCustodyV1::InFlight(request)) => Some(request),
            Some(R16RequestCustodyV1::Indeterminate(_)) | None => None,
        }
    }

    pub const fn indeterminate_request(&self) -> Option<&R16SemanticRequestV1> {
        match self.request_custody.as_ref() {
            Some(R16RequestCustodyV1::Indeterminate(request)) => Some(request),
            Some(R16RequestCustodyV1::InFlight(_)) | None => None,
        }
    }

    pub const fn last_successful_request(&self) -> Option<&R16SemanticRequestV1> {
        self.last_successful_request.as_ref()
    }

    pub const fn is_terminal(&self) -> bool {
        matches!(self.phase, R16WorkerPhaseV1::Terminal)
    }

    pub fn negotiate_model_only(
        &mut self,
        handshake: R16WorkerHandshakeV1,
    ) -> Result<(), R16WorkerModelErrorV1> {
        if self.is_terminal() {
            return Err(R16WorkerModelErrorV1::Terminal);
        }
        if self.phase != R16WorkerPhaseV1::AwaitingHandshake {
            return Err(R16WorkerModelErrorV1::IllegalTransition);
        }
        if handshake == R16WorkerHandshakeV1::ExactRuntimeV5 {
            self.phase = R16WorkerPhaseV1::ReadyV5;
            Ok(())
        } else {
            self.phase = R16WorkerPhaseV1::Terminal;
            Err(R16WorkerModelErrorV1::HandshakeMismatch)
        }
    }

    /// Models the composed worker-wire and backend-admission boundary after
    /// decoding into a summary. A rejected request never enters custody; this
    /// makes no claim about whether concrete dispatch invoked a backend method.
    pub fn receive_request_model_only(
        &mut self,
        request: R16SemanticRequestV1,
    ) -> Result<(), R16WorkerModelErrorV1> {
        if self.is_terminal() {
            return Err(R16WorkerModelErrorV1::Terminal);
        }
        if self.phase != R16WorkerPhaseV1::ReadyV5 {
            return Err(R16WorkerModelErrorV1::IllegalTransition);
        }
        if !request.is_composed_pre_custody_valid() {
            self.phase = R16WorkerPhaseV1::Terminal;
            return Err(R16WorkerModelErrorV1::InvalidRequestBeforeCustody);
        }
        self.attempted_requests = self
            .attempted_requests
            .checked_add(1)
            .ok_or(R16WorkerModelErrorV1::InvariantViolation)?;
        self.request_custody = Some(R16RequestCustodyV1::InFlight(request));
        self.phase = R16WorkerPhaseV1::AwaitingResponse;
        Ok(())
    }

    pub fn observe_response_model_only(
        &mut self,
        response: R16WorkerResponseV1,
    ) -> Result<R16WorkerOutcomeV1, R16WorkerModelErrorV1> {
        if self.is_terminal() {
            return Err(R16WorkerModelErrorV1::Terminal);
        }
        if self.phase != R16WorkerPhaseV1::AwaitingResponse
            || !matches!(self.request_custody, Some(R16RequestCustodyV1::InFlight(_)))
        {
            return Err(R16WorkerModelErrorV1::IllegalTransition);
        }
        match response {
            R16WorkerResponseV1::Success { handle } if handle != 0 => {
                let accepted_backend_custodies = self
                    .accepted_backend_custodies
                    .checked_add(1)
                    .ok_or(R16WorkerModelErrorV1::InvariantViolation)?;
                let Some(R16RequestCustodyV1::InFlight(request)) = self.request_custody.take()
                else {
                    return Err(R16WorkerModelErrorV1::InvariantViolation);
                };
                self.accepted_backend_custodies = accepted_backend_custodies;
                self.last_successful_request = Some(request);
                self.phase = R16WorkerPhaseV1::ReadyV5;
                Ok(R16WorkerOutcomeV1::Success { handle })
            }
            R16WorkerResponseV1::Rejected => {
                self.request_custody = None;
                self.phase = R16WorkerPhaseV1::ReadyV5;
                Ok(R16WorkerOutcomeV1::Rejected)
            }
            R16WorkerResponseV1::Quiescent => {
                self.request_custody = None;
                self.phase = R16WorkerPhaseV1::ReadyV5;
                Ok(R16WorkerOutcomeV1::Quiescent)
            }
            R16WorkerResponseV1::Success { handle: 0 }
            | R16WorkerResponseV1::Terminal
            | R16WorkerResponseV1::Malformed
            | R16WorkerResponseV1::Timeout
            | R16WorkerResponseV1::EndOfFile => {
                let Some(R16RequestCustodyV1::InFlight(request)) = self.request_custody.take()
                else {
                    return Err(R16WorkerModelErrorV1::InvariantViolation);
                };
                self.request_custody = Some(R16RequestCustodyV1::Indeterminate(request));
                self.phase = R16WorkerPhaseV1::Terminal;
                Err(R16WorkerModelErrorV1::Terminal)
            }
            R16WorkerResponseV1::Success { .. } => unreachable!(),
        }
    }

    pub fn validate_global_invariants(&self) -> Result<(), R16WorkerModelErrorV1> {
        let phase_shape_valid = match self.phase {
            R16WorkerPhaseV1::AwaitingHandshake => {
                self.attempted_requests == 0
                    && self.accepted_backend_custodies == 0
                    && self.request_custody.is_none()
                    && self.last_successful_request.is_none()
            }
            R16WorkerPhaseV1::ReadyV5 => self.request_custody.is_none(),
            R16WorkerPhaseV1::AwaitingResponse => {
                matches!(self.request_custody, Some(R16RequestCustodyV1::InFlight(_)))
                    && self.accepted_backend_custodies < self.attempted_requests
            }
            R16WorkerPhaseV1::Terminal => {
                !matches!(self.request_custody, Some(R16RequestCustodyV1::InFlight(_)))
                    && (self.request_custody.is_none()
                        || self.accepted_backend_custodies < self.attempted_requests)
            }
        };
        if !phase_shape_valid
            || self.accepted_backend_custodies > self.attempted_requests
            || self
                .request_custody
                .as_ref()
                .is_some_and(|custody| !custody.request().is_composed_pre_custody_valid())
            || self
                .last_successful_request
                .as_ref()
                .is_some_and(|request| !request.is_composed_pre_custody_valid())
            || (self.last_successful_request.is_some() != (self.accepted_backend_custodies != 0))
        {
            return Err(R16WorkerModelErrorV1::InvariantViolation);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R16SemanticPublicationV1 {
    pub runtime_event: u64,
    pub runtime_event_sequence: u64,
    pub dispatch: u64,
    pub dispatch_shape: u64,
    pub launch: R16LaunchGeometryV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R16SemanticObservationV1 {
    pub dispatch: u64,
    pub semantic_contract: Option<R16SemanticContractV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R16SemanticSidecarRecordV1 {
    pub runtime_event: u64,
    pub runtime_event_sequence: u64,
    pub dispatch: u64,
    pub dispatch_shape: u64,
    pub launch: R16LaunchGeometryV1,
    pub semantic_contract: Option<R16SemanticContractV1>,
}

impl R16SemanticSidecarRecordV1 {
    pub const fn from_publication(
        publication: R16SemanticPublicationV1,
        semantic_contract: Option<R16SemanticContractV1>,
    ) -> Self {
        Self {
            runtime_event: publication.runtime_event,
            runtime_event_sequence: publication.runtime_event_sequence,
            dispatch: publication.dispatch,
            dispatch_shape: publication.dispatch_shape,
            launch: publication.launch,
            semantic_contract,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R16SemanticSidecarSchemaV1 {
    ExactV1,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R16SemanticSidecarSummaryV1 {
    pub schema: R16SemanticSidecarSchemaV1,
    pub schema_version: u16,
    /// Length of the canonical encoded sidecar supplied to this abstract check.
    pub encoded_byte_len: usize,
    pub runtime_profile: u64,
    pub runtime_capture_scope: u64,
    pub runtime_profile_dispatches: usize,
    pub typed_semantic_contracts: usize,
    pub ordinary_dispatches: usize,
    pub complete_retained_dispatch_classification: bool,
    pub complete_runtime_operation_history: bool,
    pub runtime_profile_complete_runtime_operation_history: bool,
}

impl R16SemanticSidecarSummaryV1 {
    pub fn is_valid_for(
        self,
        publications: &[R16SemanticPublicationV1],
        observations: &[R16SemanticObservationV1],
        records: &[R16SemanticSidecarRecordV1],
    ) -> bool {
        let typed = records
            .iter()
            .filter(|record| record.semantic_contract.is_some())
            .count();
        self.schema == R16SemanticSidecarSchemaV1::ExactV1
            && self.schema_version == 1
            && self.encoded_byte_len != 0
            && self.encoded_byte_len <= MAX_R16_SEMANTIC_SIDECAR_BYTES_V1
            && self.runtime_profile != 0
            && self.runtime_capture_scope != 0
            && records.len() <= MAX_R16_SEMANTIC_SIDECAR_RECORDS_V1
            && publications.len() == records.len()
            && observations.len() == records.len()
            && self.runtime_profile_dispatches == records.len()
            && self.typed_semantic_contracts == typed
            && self.ordinary_dispatches == records.len() - typed
            && self.complete_retained_dispatch_classification
            && self.complete_runtime_operation_history
                == self.runtime_profile_complete_runtime_operation_history
    }
}

pub fn semantic_observation_matches_request_model_only(
    request: &R16SemanticRequestV1,
    publication: R16SemanticPublicationV1,
    observation: R16SemanticObservationV1,
) -> bool {
    request.is_composed_pre_custody_valid()
        && request
            .contract
            .is_direct_kfd_sidecar_valid_for(request.launch)
        && observation.dispatch == publication.dispatch
        && publication.launch == request.launch
        && observation.semantic_contract == Some(request.contract)
}

pub fn semantic_sidecar_sequence_joins_exactly_model_only(
    summary: R16SemanticSidecarSummaryV1,
    publications: &[R16SemanticPublicationV1],
    observations: &[R16SemanticObservationV1],
    records: &[R16SemanticSidecarRecordV1],
) -> bool {
    summary.is_valid_for(publications, observations, records)
        && publications.iter().enumerate().all(|(index, publication)| {
            publication.runtime_event != 0
                && publication.dispatch != 0
                && publication.dispatch_shape != 0
                && publication.launch.is_worker_valid()
                && observations[index].dispatch == publication.dispatch
                && observations[index]
                    .semantic_contract
                    .is_none_or(|contract| {
                        contract.is_direct_kfd_sidecar_valid_for(publication.launch)
                    })
                && records[index]
                    == R16SemanticSidecarRecordV1::from_publication(
                        *publication,
                        observations[index].semantic_contract,
                    )
                && publications[..index].iter().all(|prior| {
                    prior.runtime_event_sequence < publication.runtime_event_sequence
                        && prior.dispatch != publication.dispatch
                })
        })
}
