//! Independent executable R31 model for bounded native-Single SDMA.
//!
//! Caller-supplied observations, certificates, and digests are contracted
//! inputs. This finite model performs no I/O and does not refine executable
//! Rust, KFD, HSA, HIP, firmware, hardware, progress, SHA-256, coherent memory,
//! or DMA visibility.

use crate::IdentityDigestV1;

pub const R31_SDMA_MAX_LINEAR_COPY_BYTES_V1: u64 = 0x003f_ffe0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R31DirectionV1 {
    HostToDevice,
    DeviceToHost,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R31CertificateV1 {
    pub queue_id: u64,
    pub queue_generation: u64,
    pub host_storage_id: u64,
    pub host_storage_generation: u64,
    pub pool_generation: u64,
    pub extent_bytes: u64,
    pub digest: IdentityDigestV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R31SingleRequestV1 {
    pub transfer_id: u64,
    pub queue_id: u64,
    pub queue_generation: u64,
    pub host_storage_id: u64,
    pub host_storage_generation: u64,
    pub pool_generation: u64,
    pub host_extent_bytes: u64,
    pub device_extent_bytes: u64,
    pub host_offset: u64,
    pub device_offset: u64,
    pub copy_bytes: u64,
    pub direction: R31DirectionV1,
}

impl R31SingleRequestV1 {
    pub fn is_bounded_and_in_range(self) -> bool {
        self.transfer_id != 0
            && self.queue_id != 0
            && self.queue_generation != 0
            && self.host_storage_id != 0
            && self.host_storage_generation != 0
            && self.pool_generation != 0
            && self.host_extent_bytes != 0
            && self.device_extent_bytes != 0
            && self.copy_bytes != 0
            && self.copy_bytes <= R31_SDMA_MAX_LINEAR_COPY_BYTES_V1
            && matches!(self.host_offset.checked_add(self.copy_bytes), Some(end)
                if end <= self.host_extent_bytes)
            && matches!(self.device_offset.checked_add(self.copy_bytes), Some(end)
                if end <= self.device_extent_bytes)
    }

    pub const fn is_exact_full_h2d(self) -> bool {
        matches!(self.direction, R31DirectionV1::HostToDevice)
            && self.host_offset == 0
            && self.device_offset == 0
            && self.copy_bytes == self.host_extent_bytes
            && self.copy_bytes == self.device_extent_bytes
    }
}

impl R31CertificateV1 {
    pub const fn is_exact_for(self, request: R31SingleRequestV1) -> bool {
        self.queue_id == request.queue_id
            && self.queue_generation == request.queue_generation
            && self.host_storage_id == request.host_storage_id
            && self.host_storage_generation == request.host_storage_generation
            && self.pool_generation == request.pool_generation
            && self.extent_bytes == request.host_extent_bytes
            && request.host_offset == 0
            && request.copy_bytes == self.extent_bytes
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R31CompletionV1 {
    pub transfer_id: u64,
    pub direction: R31DirectionV1,
    pub host_offset: u64,
    pub device_offset: u64,
    pub copy_bytes: u64,
    pub packet_count: u8,
}

impl R31CompletionV1 {
    pub const fn exact_for(request: R31SingleRequestV1) -> Self {
        Self {
            transfer_id: request.transfer_id,
            direction: request.direction,
            host_offset: request.host_offset,
            device_offset: request.device_offset,
            copy_bytes: request.copy_bytes,
            packet_count: 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R31PhaseV1 {
    Ready,
    Published,
    Completed,
    ComputeReady,
    TerminalAbsorbed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R31CustodyV1 {
    ReadyPair,
    PublishedSingle,
    CompletedSingle,
    ComputeReadyAndHost,
    OpaqueTerminal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R31TerminalStageV1 {
    SubmitOpening,
    SubmitPrepare,
    SubmitClosing,
    PollOpening,
    PollClosing,
    PromotionOpening,
    PromotionClosing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R31SubmitDispositionV1 {
    RetryableBeforeRequest,
    OpeningAmbiguous,
    PrepareRetryable,
    PrepareAmbiguous,
    PublicationRetryable,
    ClosingAmbiguous,
    Published,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R31SubmitOutcomeV1 {
    Retryable,
    Published,
    TerminalAbsorbed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R31PollDispositionV1 {
    Pending,
    Completed,
    OpeningAmbiguous,
    ClosingAmbiguous,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R31PollOutcomeV1 {
    Pending,
    Completed,
    TerminalAbsorbed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R31PromotionDispositionV1 {
    Current,
    OpeningAmbiguous,
    ClosingAmbiguous,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R31PromotionOutcomeV1 {
    Ready,
    Retryable,
    TerminalAbsorbed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R31ErrorV1 {
    InvalidRequest,
    InvalidCertificate,
    IllegalPhase,
    DirectionMismatch,
    InvariantViolation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R31SingleStateV1 {
    pub request: R31SingleRequestV1,
    pub phase: R31PhaseV1,
    pub custody: R31CustodyV1,
    pub packet_count: u8,
    pub ticket_count: u8,
    pub authority_count: u8,
    pub lease_count: u8,
    pub directional_checks: u8,
    pub queue_checks: u8,
    pub completion: Option<R31CompletionV1>,
    pub host_certificate: Option<R31CertificateV1>,
    pub host_certificate_invalidated: bool,
    pub host_destination_may_have_mutated: bool,
    pub retired_frontiers: u8,
    pub ready_digest: Option<IdentityDigestV1>,
    pub terminal_stage: Option<R31TerminalStageV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R31WindowStateV1 {
    pub requests: [R31SingleRequestV1; 1],
    pub phase: R31PhaseV1,
    pub custody: R31CustodyV1,
    pub packet_count: u8,
    pub ticket_count: u8,
    pub authority_count: u8,
    pub lease_count: u8,
    pub directional_checks: u8,
    pub queue_checks: u8,
    pub completion: Option<R31CompletionV1>,
    pub host_certificate: Option<R31CertificateV1>,
    pub host_certificate_invalidated: bool,
    pub host_destination_may_have_mutated: bool,
    pub retired_frontiers: u8,
    pub ready_digest: Option<IdentityDigestV1>,
    pub terminal_stage: Option<R31TerminalStageV1>,
}

pub const fn r31_project_single_to_window_v1(single: R31SingleStateV1) -> R31WindowStateV1 {
    R31WindowStateV1 {
        requests: [single.request],
        phase: single.phase,
        custody: single.custody,
        packet_count: single.packet_count,
        ticket_count: single.ticket_count,
        authority_count: single.authority_count,
        lease_count: single.lease_count,
        directional_checks: single.directional_checks,
        queue_checks: single.queue_checks,
        completion: single.completion,
        host_certificate: single.host_certificate,
        host_certificate_invalidated: single.host_certificate_invalidated,
        host_destination_may_have_mutated: single.host_destination_may_have_mutated,
        retired_frontiers: single.retired_frontiers,
        ready_digest: single.ready_digest,
        terminal_stage: single.terminal_stage,
    }
}

fn initial_single_state_v1(
    request: R31SingleRequestV1,
    host_certificate: Option<R31CertificateV1>,
) -> R31SingleStateV1 {
    R31SingleStateV1 {
        request,
        phase: R31PhaseV1::Ready,
        custody: R31CustodyV1::ReadyPair,
        packet_count: 0,
        ticket_count: 0,
        authority_count: 1,
        lease_count: 0,
        directional_checks: 0,
        queue_checks: 0,
        completion: None,
        host_certificate,
        host_certificate_invalidated: false,
        host_destination_may_have_mutated: false,
        retired_frontiers: 0,
        ready_digest: None,
        terminal_stage: None,
    }
}

fn initial_window_state_v1(
    request: R31SingleRequestV1,
    host_certificate: Option<R31CertificateV1>,
) -> R31WindowStateV1 {
    R31WindowStateV1 {
        requests: [request],
        phase: R31PhaseV1::Ready,
        custody: R31CustodyV1::ReadyPair,
        packet_count: 0,
        ticket_count: 0,
        authority_count: 1,
        lease_count: 0,
        directional_checks: 0,
        queue_checks: 0,
        completion: None,
        host_certificate,
        host_certificate_invalidated: false,
        host_destination_may_have_mutated: false,
        retired_frontiers: 0,
        ready_digest: None,
        terminal_stage: None,
    }
}

fn checked_add_checks(value: u8, add: u8) -> Result<u8, R31ErrorV1> {
    value.checked_add(add).ok_or(R31ErrorV1::InvariantViolation)
}

fn submit_check_counts(disposition: R31SubmitDispositionV1) -> (u8, u8) {
    match disposition {
        R31SubmitDispositionV1::RetryableBeforeRequest => (0, 0),
        R31SubmitDispositionV1::OpeningAmbiguous => (1, 0),
        R31SubmitDispositionV1::PrepareRetryable | R31SubmitDispositionV1::PrepareAmbiguous => {
            (2, 0)
        }
        R31SubmitDispositionV1::PublicationRetryable
        | R31SubmitDispositionV1::ClosingAmbiguous
        | R31SubmitDispositionV1::Published => (3, 1),
    }
}

fn request_was_constructed(disposition: R31SubmitDispositionV1) -> bool {
    !matches!(
        disposition,
        R31SubmitDispositionV1::RetryableBeforeRequest | R31SubmitDispositionV1::OpeningAmbiguous
    )
}

fn submit_outcome(disposition: R31SubmitDispositionV1) -> R31SubmitOutcomeV1 {
    match disposition {
        R31SubmitDispositionV1::RetryableBeforeRequest
        | R31SubmitDispositionV1::PrepareRetryable
        | R31SubmitDispositionV1::PublicationRetryable => R31SubmitOutcomeV1::Retryable,
        R31SubmitDispositionV1::Published => R31SubmitOutcomeV1::Published,
        R31SubmitDispositionV1::OpeningAmbiguous
        | R31SubmitDispositionV1::PrepareAmbiguous
        | R31SubmitDispositionV1::ClosingAmbiguous => R31SubmitOutcomeV1::TerminalAbsorbed,
    }
}

fn submit_terminal_stage(disposition: R31SubmitDispositionV1) -> Option<R31TerminalStageV1> {
    match disposition {
        R31SubmitDispositionV1::OpeningAmbiguous => Some(R31TerminalStageV1::SubmitOpening),
        R31SubmitDispositionV1::PrepareAmbiguous => Some(R31TerminalStageV1::SubmitPrepare),
        R31SubmitDispositionV1::ClosingAmbiguous => Some(R31TerminalStageV1::SubmitClosing),
        _ => None,
    }
}

fn single_submit_v1(
    mut state: R31SingleStateV1,
    disposition: R31SubmitDispositionV1,
) -> Result<(R31SingleStateV1, R31SubmitOutcomeV1), R31ErrorV1> {
    if state.phase != R31PhaseV1::Ready {
        return Err(R31ErrorV1::IllegalPhase);
    }
    let (directional, queue) = submit_check_counts(disposition);
    state.directional_checks = checked_add_checks(state.directional_checks, directional)?;
    state.queue_checks = checked_add_checks(state.queue_checks, queue)?;
    if request_was_constructed(disposition)
        && state.request.direction == R31DirectionV1::DeviceToHost
    {
        state.host_certificate = None;
        state.host_certificate_invalidated = true;
    }
    let outcome = submit_outcome(disposition);
    match outcome {
        R31SubmitOutcomeV1::Retryable => {}
        R31SubmitOutcomeV1::Published => {
            state.phase = R31PhaseV1::Published;
            state.custody = R31CustodyV1::PublishedSingle;
            state.packet_count = 1;
            state.ticket_count = 1;
            state.lease_count = 1;
            if state.request.direction == R31DirectionV1::DeviceToHost {
                state.host_destination_may_have_mutated = true;
            }
        }
        R31SubmitOutcomeV1::TerminalAbsorbed => {
            state.phase = R31PhaseV1::TerminalAbsorbed;
            state.custody = R31CustodyV1::OpaqueTerminal;
            state.terminal_stage = submit_terminal_stage(disposition);
            if disposition == R31SubmitDispositionV1::ClosingAmbiguous
                && state.request.direction == R31DirectionV1::DeviceToHost
            {
                state.host_destination_may_have_mutated = true;
            }
        }
    }
    Ok((state, outcome))
}

fn window_submit_v1(
    mut state: R31WindowStateV1,
    disposition: R31SubmitDispositionV1,
) -> Result<(R31WindowStateV1, R31SubmitOutcomeV1), R31ErrorV1> {
    if state.phase != R31PhaseV1::Ready {
        return Err(R31ErrorV1::IllegalPhase);
    }
    let request = state.requests[0];
    let (directional, queue) = submit_check_counts(disposition);
    state.directional_checks = checked_add_checks(state.directional_checks, directional)?;
    state.queue_checks = checked_add_checks(state.queue_checks, queue)?;
    if request_was_constructed(disposition) && request.direction == R31DirectionV1::DeviceToHost {
        state.host_certificate = None;
        state.host_certificate_invalidated = true;
    }
    let outcome = submit_outcome(disposition);
    match outcome {
        R31SubmitOutcomeV1::Retryable => {}
        R31SubmitOutcomeV1::Published => {
            state.phase = R31PhaseV1::Published;
            state.custody = R31CustodyV1::PublishedSingle;
            state.packet_count = 1;
            state.ticket_count = 1;
            state.lease_count = 1;
            if request.direction == R31DirectionV1::DeviceToHost {
                state.host_destination_may_have_mutated = true;
            }
        }
        R31SubmitOutcomeV1::TerminalAbsorbed => {
            state.phase = R31PhaseV1::TerminalAbsorbed;
            state.custody = R31CustodyV1::OpaqueTerminal;
            state.terminal_stage = submit_terminal_stage(disposition);
            if disposition == R31SubmitDispositionV1::ClosingAmbiguous
                && request.direction == R31DirectionV1::DeviceToHost
            {
                state.host_destination_may_have_mutated = true;
            }
        }
    }
    Ok((state, outcome))
}

fn poll_checks(disposition: R31PollDispositionV1) -> u8 {
    if disposition == R31PollDispositionV1::OpeningAmbiguous {
        1
    } else {
        2
    }
}

fn poll_terminal_stage(disposition: R31PollDispositionV1) -> Option<R31TerminalStageV1> {
    match disposition {
        R31PollDispositionV1::OpeningAmbiguous => Some(R31TerminalStageV1::PollOpening),
        R31PollDispositionV1::ClosingAmbiguous => Some(R31TerminalStageV1::PollClosing),
        _ => None,
    }
}

fn single_poll_v1(
    mut state: R31SingleStateV1,
    disposition: R31PollDispositionV1,
) -> Result<(R31SingleStateV1, R31PollOutcomeV1), R31ErrorV1> {
    if state.phase != R31PhaseV1::Published {
        return Err(R31ErrorV1::IllegalPhase);
    }
    state.queue_checks = checked_add_checks(state.queue_checks, poll_checks(disposition))?;
    let outcome = match disposition {
        R31PollDispositionV1::Pending => R31PollOutcomeV1::Pending,
        R31PollDispositionV1::Completed => R31PollOutcomeV1::Completed,
        _ => R31PollOutcomeV1::TerminalAbsorbed,
    };
    match outcome {
        R31PollOutcomeV1::Pending => {}
        R31PollOutcomeV1::Completed => {
            state.phase = R31PhaseV1::Completed;
            state.custody = R31CustodyV1::CompletedSingle;
            state.completion = Some(R31CompletionV1::exact_for(state.request));
        }
        R31PollOutcomeV1::TerminalAbsorbed => {
            state.phase = R31PhaseV1::TerminalAbsorbed;
            state.custody = R31CustodyV1::OpaqueTerminal;
            state.terminal_stage = poll_terminal_stage(disposition);
        }
    }
    Ok((state, outcome))
}

fn window_poll_v1(
    mut state: R31WindowStateV1,
    disposition: R31PollDispositionV1,
) -> Result<(R31WindowStateV1, R31PollOutcomeV1), R31ErrorV1> {
    if state.phase != R31PhaseV1::Published {
        return Err(R31ErrorV1::IllegalPhase);
    }
    state.queue_checks = checked_add_checks(state.queue_checks, poll_checks(disposition))?;
    let outcome = match disposition {
        R31PollDispositionV1::Pending => R31PollOutcomeV1::Pending,
        R31PollDispositionV1::Completed => R31PollOutcomeV1::Completed,
        _ => R31PollOutcomeV1::TerminalAbsorbed,
    };
    match outcome {
        R31PollOutcomeV1::Pending => {}
        R31PollOutcomeV1::Completed => {
            state.phase = R31PhaseV1::Completed;
            state.custody = R31CustodyV1::CompletedSingle;
            state.completion = Some(R31CompletionV1::exact_for(state.requests[0]));
        }
        R31PollOutcomeV1::TerminalAbsorbed => {
            state.phase = R31PhaseV1::TerminalAbsorbed;
            state.custody = R31CustodyV1::OpaqueTerminal;
            state.terminal_stage = poll_terminal_stage(disposition);
        }
    }
    Ok((state, outcome))
}

fn promotion_terminal_stage(disposition: R31PromotionDispositionV1) -> Option<R31TerminalStageV1> {
    match disposition {
        R31PromotionDispositionV1::OpeningAmbiguous => Some(R31TerminalStageV1::PromotionOpening),
        R31PromotionDispositionV1::ClosingAmbiguous => Some(R31TerminalStageV1::PromotionClosing),
        R31PromotionDispositionV1::Current => None,
    }
}

fn single_promote_v1(
    mut state: R31SingleStateV1,
    candidate: R31CertificateV1,
    disposition: R31PromotionDispositionV1,
) -> Result<(R31SingleStateV1, R31PromotionOutcomeV1), R31ErrorV1> {
    if state.phase != R31PhaseV1::Completed {
        return Err(R31ErrorV1::IllegalPhase);
    }
    if !state.request.is_exact_full_h2d() {
        return Err(R31ErrorV1::DirectionMismatch);
    }
    let checks = if disposition == R31PromotionDispositionV1::OpeningAmbiguous {
        1
    } else {
        2
    };
    state.queue_checks = checked_add_checks(state.queue_checks, checks)?;
    if disposition != R31PromotionDispositionV1::Current {
        state.phase = R31PhaseV1::TerminalAbsorbed;
        state.custody = R31CustodyV1::OpaqueTerminal;
        state.terminal_stage = promotion_terminal_stage(disposition);
        return Ok((state, R31PromotionOutcomeV1::TerminalAbsorbed));
    }
    if state.host_certificate != Some(candidate) || !candidate.is_exact_for(state.request) {
        return Ok((state, R31PromotionOutcomeV1::Retryable));
    }
    state.phase = R31PhaseV1::ComputeReady;
    state.custody = R31CustodyV1::ComputeReadyAndHost;
    state.packet_count = 0;
    state.ticket_count = 0;
    state.lease_count = 0;
    state.completion = None;
    state.retired_frontiers = 1;
    state.ready_digest = Some(candidate.digest);
    Ok((state, R31PromotionOutcomeV1::Ready))
}

fn window_promote_v1(
    mut state: R31WindowStateV1,
    candidate: R31CertificateV1,
    disposition: R31PromotionDispositionV1,
) -> Result<(R31WindowStateV1, R31PromotionOutcomeV1), R31ErrorV1> {
    if state.phase != R31PhaseV1::Completed {
        return Err(R31ErrorV1::IllegalPhase);
    }
    let request = state.requests[0];
    if !request.is_exact_full_h2d() {
        return Err(R31ErrorV1::DirectionMismatch);
    }
    let checks = if disposition == R31PromotionDispositionV1::OpeningAmbiguous {
        1
    } else {
        2
    };
    state.queue_checks = checked_add_checks(state.queue_checks, checks)?;
    if disposition != R31PromotionDispositionV1::Current {
        state.phase = R31PhaseV1::TerminalAbsorbed;
        state.custody = R31CustodyV1::OpaqueTerminal;
        state.terminal_stage = promotion_terminal_stage(disposition);
        return Ok((state, R31PromotionOutcomeV1::TerminalAbsorbed));
    }
    if state.host_certificate != Some(candidate) || !candidate.is_exact_for(request) {
        return Ok((state, R31PromotionOutcomeV1::Retryable));
    }
    state.phase = R31PhaseV1::ComputeReady;
    state.custody = R31CustodyV1::ComputeReadyAndHost;
    state.packet_count = 0;
    state.ticket_count = 0;
    state.lease_count = 0;
    state.completion = None;
    state.retired_frontiers = 1;
    state.ready_digest = Some(candidate.digest);
    Ok((state, R31PromotionOutcomeV1::Ready))
}

fn validate_single_state_v1(state: R31SingleStateV1) -> bool {
    if !state.request.is_bounded_and_in_range() || state.authority_count != 1 {
        return false;
    }
    if state
        .host_certificate
        .is_some_and(|certificate| !certificate.is_exact_for(state.request))
    {
        return false;
    }
    if state.request.direction == R31DirectionV1::HostToDevice
        && (state.host_certificate_invalidated || state.host_destination_may_have_mutated)
    {
        return false;
    }
    if state.request.direction == R31DirectionV1::DeviceToHost
        && state.host_destination_may_have_mutated
        && (!state.host_certificate_invalidated || state.host_certificate.is_some())
    {
        return false;
    }
    match state.phase {
        R31PhaseV1::Ready => {
            state.custody == R31CustodyV1::ReadyPair
                && state.packet_count == 0
                && state.ticket_count == 0
                && state.lease_count == 0
                && state.completion.is_none()
                && state.retired_frontiers == 0
                && state.ready_digest.is_none()
                && state.terminal_stage.is_none()
        }
        R31PhaseV1::Published => {
            state.custody == R31CustodyV1::PublishedSingle
                && state.packet_count == 1
                && state.ticket_count == 1
                && state.lease_count == 1
                && state.completion.is_none()
                && state.retired_frontiers == 0
                && state.ready_digest.is_none()
                && state.terminal_stage.is_none()
        }
        R31PhaseV1::Completed => {
            state.custody == R31CustodyV1::CompletedSingle
                && state.packet_count == 1
                && state.ticket_count == 1
                && state.lease_count == 1
                && state.completion == Some(R31CompletionV1::exact_for(state.request))
                && state.retired_frontiers == 0
                && state.ready_digest.is_none()
                && state.terminal_stage.is_none()
        }
        R31PhaseV1::ComputeReady => {
            state.request.is_exact_full_h2d()
                && state.custody == R31CustodyV1::ComputeReadyAndHost
                && state.packet_count == 0
                && state.ticket_count == 0
                && state.lease_count == 0
                && state.completion.is_none()
                && state.retired_frontiers == 1
                && state.ready_digest.is_some()
                && state.terminal_stage.is_none()
        }
        R31PhaseV1::TerminalAbsorbed => {
            state.custody == R31CustodyV1::OpaqueTerminal
                && state.retired_frontiers == 0
                && state.ready_digest.is_none()
                && state.terminal_stage.is_some()
        }
    }
}

fn validate_window_state_v1(state: R31WindowStateV1) -> bool {
    validate_single_state_v1(R31SingleStateV1 {
        request: state.requests[0],
        phase: state.phase,
        custody: state.custody,
        packet_count: state.packet_count,
        ticket_count: state.ticket_count,
        authority_count: state.authority_count,
        lease_count: state.lease_count,
        directional_checks: state.directional_checks,
        queue_checks: state.queue_checks,
        completion: state.completion,
        host_certificate: state.host_certificate,
        host_certificate_invalidated: state.host_certificate_invalidated,
        host_destination_may_have_mutated: state.host_destination_may_have_mutated,
        retired_frontiers: state.retired_frontiers,
        ready_digest: state.ready_digest,
        terminal_stage: state.terminal_stage,
    })
}

pub struct R31SingleWindowModelV1 {
    single: R31SingleStateV1,
    window: R31WindowStateV1,
}

impl R31SingleWindowModelV1 {
    pub fn new_model_only(
        request: R31SingleRequestV1,
        host_certificate: Option<R31CertificateV1>,
    ) -> Result<Self, R31ErrorV1> {
        if !request.is_bounded_and_in_range() {
            return Err(R31ErrorV1::InvalidRequest);
        }
        if host_certificate.is_some_and(|certificate| !certificate.is_exact_for(request)) {
            return Err(R31ErrorV1::InvalidCertificate);
        }
        let model = Self {
            single: initial_single_state_v1(request, host_certificate),
            window: initial_window_state_v1(request, host_certificate),
        };
        model.validate_global_invariants()?;
        Ok(model)
    }

    pub const fn single_snapshot(&self) -> R31SingleStateV1 {
        self.single
    }
    pub const fn window_snapshot(&self) -> R31WindowStateV1 {
        self.window
    }

    pub fn validate_global_invariants(&self) -> Result<(), R31ErrorV1> {
        if !validate_single_state_v1(self.single)
            || !validate_window_state_v1(self.window)
            || r31_project_single_to_window_v1(self.single) != self.window
        {
            return Err(R31ErrorV1::InvariantViolation);
        }
        Ok(())
    }

    pub fn submit_model_only(
        &mut self,
        disposition: R31SubmitDispositionV1,
    ) -> Result<R31SubmitOutcomeV1, R31ErrorV1> {
        let (single, single_outcome) = single_submit_v1(self.single, disposition)?;
        let (window, window_outcome) = window_submit_v1(self.window, disposition)?;
        if single_outcome != window_outcome || r31_project_single_to_window_v1(single) != window {
            return Err(R31ErrorV1::InvariantViolation);
        }
        self.single = single;
        self.window = window;
        self.validate_global_invariants()?;
        Ok(single_outcome)
    }

    pub fn poll_model_only(
        &mut self,
        disposition: R31PollDispositionV1,
    ) -> Result<R31PollOutcomeV1, R31ErrorV1> {
        let (single, single_outcome) = single_poll_v1(self.single, disposition)?;
        let (window, window_outcome) = window_poll_v1(self.window, disposition)?;
        if single_outcome != window_outcome || r31_project_single_to_window_v1(single) != window {
            return Err(R31ErrorV1::InvariantViolation);
        }
        self.single = single;
        self.window = window;
        self.validate_global_invariants()?;
        Ok(single_outcome)
    }

    pub fn promote_model_only(
        &mut self,
        candidate: R31CertificateV1,
        disposition: R31PromotionDispositionV1,
    ) -> Result<R31PromotionOutcomeV1, R31ErrorV1> {
        let (single, single_outcome) = single_promote_v1(self.single, candidate, disposition)?;
        let (window, window_outcome) = window_promote_v1(self.window, candidate, disposition)?;
        if single_outcome != window_outcome || r31_project_single_to_window_v1(single) != window {
            return Err(R31ErrorV1::InvariantViolation);
        }
        self.single = single;
        self.window = window;
        self.validate_global_invariants()?;
        Ok(single_outcome)
    }
}
