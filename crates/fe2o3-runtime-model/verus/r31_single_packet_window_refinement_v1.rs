// Independent R31 refinement model for the bounded R19 single-packet path and
// its normalized one-element R22 window. This does not refine executable Rust,
// KFD, HSA, HIP, firmware, hardware, liveness, SHA-256, or DMA visibility.
use vstd::prelude::*;

verus! {

pub open spec fn max_packet_bytes_v1() -> nat { 0x003f_ffe0 }

#[derive(PartialEq, Eq)] pub enum DirectionV1 { HostToDevice, DeviceToHost }
#[derive(PartialEq, Eq)] pub enum PhaseV1 {
    Ready, Published, Completed, ComputeReady, TerminalAbsorbed,
}
#[derive(PartialEq, Eq)] pub enum CustodyV1 {
    ReadyPair, PublishedSingle, CompletedSingle, ComputeReadyAndHost, OpaqueTerminal,
}
#[derive(PartialEq, Eq)] pub enum TerminalStageV1 {
    SubmitOpening, SubmitPrepare, SubmitClosing,
    PollOpening, PollClosing, PromotionOpening, PromotionClosing,
}
#[derive(PartialEq, Eq)] pub enum SubmitDispositionV1 {
    RetryableBeforeRequest, OpeningAmbiguous, PrepareRetryable,
    PrepareAmbiguous, PublicationRetryable, ClosingAmbiguous, Published,
}
#[derive(PartialEq, Eq)] pub enum PollDispositionV1 {
    Pending, Completed, OpeningAmbiguous, ClosingAmbiguous,
}
#[derive(PartialEq, Eq)] pub enum PromotionDispositionV1 {
    Current, OpeningAmbiguous, ClosingAmbiguous,
}

#[derive(PartialEq, Eq)] pub struct RequestV1 {
    pub transfer_id: nat,
    pub queue_id: nat,
    pub queue_generation: nat,
    pub host_storage_id: nat,
    pub host_storage_generation: nat,
    pub pool_generation: nat,
    pub host_extent: nat,
    pub device_extent: nat,
    pub host_offset: nat,
    pub device_offset: nat,
    pub copy_bytes: nat,
    pub direction: DirectionV1,
}

#[derive(PartialEq, Eq)] pub struct CertificateV1 {
    pub queue_id: nat,
    pub queue_generation: nat,
    pub host_storage_id: nat,
    pub host_storage_generation: nat,
    pub pool_generation: nat,
    pub extent: nat,
    pub digest: nat,
}

#[derive(PartialEq, Eq)] pub struct CompletionV1 {
    pub transfer_id: nat,
    pub direction: DirectionV1,
    pub host_offset: nat,
    pub device_offset: nat,
    pub copy_bytes: nat,
    pub packet_count: nat,
}

pub struct SingleStateV1 {
    pub request: RequestV1,
    pub phase: PhaseV1,
    pub custody: CustodyV1,
    pub packet_count: nat,
    pub ticket_count: nat,
    pub authority_count: nat,
    pub lease_count: nat,
    pub directional_checks: nat,
    pub queue_checks: nat,
    pub completion: Option<CompletionV1>,
    pub host_certificate: Option<CertificateV1>,
    pub host_certificate_invalidated: bool,
    pub host_destination_may_have_mutated: bool,
    pub retired_frontiers: nat,
    pub ready_digest: Option<nat>,
    pub terminal_stage: Option<TerminalStageV1>,
}

pub struct WindowStateV1 {
    pub request_count: nat,
    pub request: RequestV1,
    pub phase: PhaseV1,
    pub custody: CustodyV1,
    pub packet_count: nat,
    pub ticket_count: nat,
    pub authority_count: nat,
    pub lease_count: nat,
    pub directional_checks: nat,
    pub queue_checks: nat,
    pub completion: Option<CompletionV1>,
    pub host_certificate: Option<CertificateV1>,
    pub host_certificate_invalidated: bool,
    pub host_destination_may_have_mutated: bool,
    pub retired_frontiers: nat,
    pub ready_digest: Option<nat>,
    pub terminal_stage: Option<TerminalStageV1>,
}

pub open spec fn valid_request_v1(request: RequestV1) -> bool {
    &&& request.transfer_id > 0
    &&& request.queue_id > 0
    &&& request.queue_generation > 0
    &&& request.host_storage_id > 0
    &&& request.host_storage_generation > 0
    &&& request.pool_generation > 0
    &&& request.host_extent > 0
    &&& request.device_extent > 0
    &&& 0 < request.copy_bytes <= max_packet_bytes_v1()
    &&& request.host_offset + request.copy_bytes <= request.host_extent
    &&& request.device_offset + request.copy_bytes <= request.device_extent
}

pub open spec fn exact_full_h2d_v1(request: RequestV1) -> bool {
    &&& request.direction == DirectionV1::HostToDevice
    &&& request.host_offset == 0
    &&& request.device_offset == 0
    &&& request.copy_bytes == request.host_extent
    &&& request.copy_bytes == request.device_extent
}

pub open spec fn exact_certificate_v1(
    certificate: CertificateV1, request: RequestV1) -> bool
{
    &&& certificate.queue_id == request.queue_id
    &&& certificate.queue_generation == request.queue_generation
    &&& certificate.host_storage_id == request.host_storage_id
    &&& certificate.host_storage_generation == request.host_storage_generation
    &&& certificate.pool_generation == request.pool_generation
    &&& certificate.extent == request.host_extent
    &&& request.host_offset == 0
    &&& request.copy_bytes == certificate.extent
}

pub open spec fn completion_for_v1(request: RequestV1) -> CompletionV1 {
    CompletionV1 {
        transfer_id: request.transfer_id,
        direction: request.direction,
        host_offset: request.host_offset,
        device_offset: request.device_offset,
        copy_bytes: request.copy_bytes,
        packet_count: 1,
    }
}

pub open spec fn initial_single_v1(
    request: RequestV1, certificate: Option<CertificateV1>) -> SingleStateV1
{
    SingleStateV1 {
        request, phase: PhaseV1::Ready, custody: CustodyV1::ReadyPair,
        packet_count: 0, ticket_count: 0, authority_count: 1, lease_count: 0,
        directional_checks: 0, queue_checks: 0, completion: None,
        host_certificate: certificate, host_certificate_invalidated: false,
        host_destination_may_have_mutated: false, retired_frontiers: 0,
        ready_digest: None, terminal_stage: None,
    }
}

pub open spec fn initial_window_v1(
    request: RequestV1, certificate: Option<CertificateV1>) -> WindowStateV1
{
    WindowStateV1 {
        request_count: 1, request, phase: PhaseV1::Ready,
        custody: CustodyV1::ReadyPair, packet_count: 0, ticket_count: 0,
        authority_count: 1, lease_count: 0, directional_checks: 0,
        queue_checks: 0, completion: None, host_certificate: certificate,
        host_certificate_invalidated: false,
        host_destination_may_have_mutated: false, retired_frontiers: 0,
        ready_digest: None, terminal_stage: None,
    }
}

pub open spec fn project_single_v1(single: SingleStateV1) -> WindowStateV1 {
    WindowStateV1 {
        request_count: 1,
        request: single.request,
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

pub open spec fn refines_v1(single: SingleStateV1, window: WindowStateV1) -> bool {
    window == project_single_v1(single)
}

pub open spec fn valid_single_v1(state: SingleStateV1) -> bool {
    &&& valid_request_v1(state.request)
    &&& state.authority_count == 1
    &&& match state.host_certificate {
        Some(certificate) => exact_certificate_v1(certificate, state.request),
        None => true,
    }
    &&& (state.request.direction == DirectionV1::HostToDevice ==>
        !state.host_certificate_invalidated && !state.host_destination_may_have_mutated)
    &&& (state.host_destination_may_have_mutated ==>
        state.request.direction == DirectionV1::DeviceToHost
            && state.host_certificate_invalidated && state.host_certificate.is_none())
    &&& match state.phase {
        PhaseV1::Ready => state.custody == CustodyV1::ReadyPair
            && state.packet_count == 0 && state.ticket_count == 0
            && state.lease_count == 0 && state.completion.is_none()
            && state.retired_frontiers == 0 && state.ready_digest.is_none()
            && state.terminal_stage.is_none(),
        PhaseV1::Published => state.custody == CustodyV1::PublishedSingle
            && state.packet_count == 1 && state.ticket_count == 1
            && state.lease_count == 1 && state.completion.is_none()
            && state.retired_frontiers == 0 && state.ready_digest.is_none()
            && state.terminal_stage.is_none(),
        PhaseV1::Completed => state.custody == CustodyV1::CompletedSingle
            && state.packet_count == 1 && state.ticket_count == 1
            && state.lease_count == 1
            && state.completion == Some(completion_for_v1(state.request))
            && state.retired_frontiers == 0 && state.ready_digest.is_none()
            && state.terminal_stage.is_none(),
        PhaseV1::ComputeReady => exact_full_h2d_v1(state.request)
            && state.custody == CustodyV1::ComputeReadyAndHost
            && state.packet_count == 0 && state.ticket_count == 0
            && state.lease_count == 0 && state.completion.is_none()
            && state.retired_frontiers == 1 && state.ready_digest.is_some()
            && state.terminal_stage.is_none(),
        PhaseV1::TerminalAbsorbed => state.custody == CustodyV1::OpaqueTerminal
            && state.retired_frontiers == 0 && state.ready_digest.is_none()
            && state.terminal_stage.is_some(),
    }
}

pub open spec fn valid_window_v1(state: WindowStateV1) -> bool {
    state.request_count == 1 && valid_single_v1(SingleStateV1 {
        request: state.request, phase: state.phase, custody: state.custody,
        packet_count: state.packet_count, ticket_count: state.ticket_count,
        authority_count: state.authority_count, lease_count: state.lease_count,
        directional_checks: state.directional_checks, queue_checks: state.queue_checks,
        completion: state.completion, host_certificate: state.host_certificate,
        host_certificate_invalidated: state.host_certificate_invalidated,
        host_destination_may_have_mutated: state.host_destination_may_have_mutated,
        retired_frontiers: state.retired_frontiers, ready_digest: state.ready_digest,
        terminal_stage: state.terminal_stage,
    })
}

pub open spec fn request_constructed_v1(disposition: SubmitDispositionV1) -> bool {
    disposition != SubmitDispositionV1::RetryableBeforeRequest
        && disposition != SubmitDispositionV1::OpeningAmbiguous
}

pub open spec fn directional_submit_checks_v1(disposition: SubmitDispositionV1) -> nat {
    if disposition == SubmitDispositionV1::RetryableBeforeRequest { 0 }
    else if disposition == SubmitDispositionV1::OpeningAmbiguous { 1 }
    else if disposition == SubmitDispositionV1::PrepareRetryable
        || disposition == SubmitDispositionV1::PrepareAmbiguous { 2 }
    else { 3 }
}

pub open spec fn queue_submit_checks_v1(disposition: SubmitDispositionV1) -> nat {
    if disposition == SubmitDispositionV1::PublicationRetryable
        || disposition == SubmitDispositionV1::ClosingAmbiguous
        || disposition == SubmitDispositionV1::Published { 1 } else { 0 }
}

pub open spec fn submit_terminal_stage_v1(
    disposition: SubmitDispositionV1) -> Option<TerminalStageV1>
{
    if disposition == SubmitDispositionV1::OpeningAmbiguous {
        Some(TerminalStageV1::SubmitOpening)
    } else if disposition == SubmitDispositionV1::PrepareAmbiguous {
        Some(TerminalStageV1::SubmitPrepare)
    } else if disposition == SubmitDispositionV1::ClosingAmbiguous {
        Some(TerminalStageV1::SubmitClosing)
    } else { None }
}

pub open spec fn single_submit_v1(
    state: SingleStateV1, disposition: SubmitDispositionV1) -> SingleStateV1
{
    if state.phase != PhaseV1::Ready { state } else {
        let constructed = request_constructed_v1(disposition);
        let d2h = state.request.direction == DirectionV1::DeviceToHost;
        let base = SingleStateV1 {
            directional_checks: state.directional_checks
                + directional_submit_checks_v1(disposition),
            queue_checks: state.queue_checks + queue_submit_checks_v1(disposition),
            host_certificate: if constructed && d2h { None } else { state.host_certificate },
            host_certificate_invalidated:
                if constructed && d2h { true } else { state.host_certificate_invalidated },
            ..state
        };
        if disposition == SubmitDispositionV1::Published {
            SingleStateV1 {
                phase: PhaseV1::Published, custody: CustodyV1::PublishedSingle,
                packet_count: 1, ticket_count: 1, lease_count: 1,
                host_destination_may_have_mutated: d2h, ..base
            }
        } else if submit_terminal_stage_v1(disposition).is_some() {
            SingleStateV1 {
                phase: PhaseV1::TerminalAbsorbed, custody: CustodyV1::OpaqueTerminal,
                host_destination_may_have_mutated:
                    disposition == SubmitDispositionV1::ClosingAmbiguous && d2h,
                terminal_stage: submit_terminal_stage_v1(disposition), ..base
            }
        } else { base }
    }
}

pub open spec fn window_submit_v1(
    state: WindowStateV1, disposition: SubmitDispositionV1) -> WindowStateV1
{
    if state.phase != PhaseV1::Ready { state } else {
        let constructed = request_constructed_v1(disposition);
        let d2h = state.request.direction == DirectionV1::DeviceToHost;
        let base = WindowStateV1 {
            directional_checks: state.directional_checks
                + directional_submit_checks_v1(disposition),
            queue_checks: state.queue_checks + queue_submit_checks_v1(disposition),
            host_certificate: if constructed && d2h { None } else { state.host_certificate },
            host_certificate_invalidated:
                if constructed && d2h { true } else { state.host_certificate_invalidated },
            ..state
        };
        if disposition == SubmitDispositionV1::Published {
            WindowStateV1 {
                phase: PhaseV1::Published, custody: CustodyV1::PublishedSingle,
                packet_count: 1, ticket_count: 1, lease_count: 1,
                host_destination_may_have_mutated: d2h, ..base
            }
        } else if submit_terminal_stage_v1(disposition).is_some() {
            WindowStateV1 {
                phase: PhaseV1::TerminalAbsorbed, custody: CustodyV1::OpaqueTerminal,
                host_destination_may_have_mutated:
                    disposition == SubmitDispositionV1::ClosingAmbiguous && d2h,
                terminal_stage: submit_terminal_stage_v1(disposition), ..base
            }
        } else { base }
    }
}

pub open spec fn poll_checks_v1(disposition: PollDispositionV1) -> nat {
    if disposition == PollDispositionV1::OpeningAmbiguous { 1 } else { 2 }
}

pub open spec fn poll_terminal_stage_v1(
    disposition: PollDispositionV1) -> Option<TerminalStageV1>
{
    if disposition == PollDispositionV1::OpeningAmbiguous {
        Some(TerminalStageV1::PollOpening)
    } else if disposition == PollDispositionV1::ClosingAmbiguous {
        Some(TerminalStageV1::PollClosing)
    } else { None }
}

pub open spec fn single_poll_v1(
    state: SingleStateV1, disposition: PollDispositionV1) -> SingleStateV1
{
    if state.phase != PhaseV1::Published { state } else {
        let base = SingleStateV1 {
            queue_checks: state.queue_checks + poll_checks_v1(disposition), ..state
        };
        if disposition == PollDispositionV1::Completed {
            SingleStateV1 {
                phase: PhaseV1::Completed, custody: CustodyV1::CompletedSingle,
                completion: Some(completion_for_v1(state.request)), ..base
            }
        } else if poll_terminal_stage_v1(disposition).is_some() {
            SingleStateV1 {
                phase: PhaseV1::TerminalAbsorbed, custody: CustodyV1::OpaqueTerminal,
                terminal_stage: poll_terminal_stage_v1(disposition), ..base
            }
        } else { base }
    }
}

pub open spec fn window_poll_v1(
    state: WindowStateV1, disposition: PollDispositionV1) -> WindowStateV1
{
    if state.phase != PhaseV1::Published { state } else {
        let base = WindowStateV1 {
            queue_checks: state.queue_checks + poll_checks_v1(disposition), ..state
        };
        if disposition == PollDispositionV1::Completed {
            WindowStateV1 {
                phase: PhaseV1::Completed, custody: CustodyV1::CompletedSingle,
                completion: Some(completion_for_v1(state.request)), ..base
            }
        } else if poll_terminal_stage_v1(disposition).is_some() {
            WindowStateV1 {
                phase: PhaseV1::TerminalAbsorbed, custody: CustodyV1::OpaqueTerminal,
                terminal_stage: poll_terminal_stage_v1(disposition), ..base
            }
        } else { base }
    }
}

pub open spec fn promotion_checks_v1(disposition: PromotionDispositionV1) -> nat {
    if disposition == PromotionDispositionV1::OpeningAmbiguous { 1 } else { 2 }
}

pub open spec fn promotion_terminal_stage_v1(
    disposition: PromotionDispositionV1) -> Option<TerminalStageV1>
{
    if disposition == PromotionDispositionV1::OpeningAmbiguous {
        Some(TerminalStageV1::PromotionOpening)
    } else if disposition == PromotionDispositionV1::ClosingAmbiguous {
        Some(TerminalStageV1::PromotionClosing)
    } else { None }
}

pub open spec fn single_promote_v1(
    state: SingleStateV1, candidate: CertificateV1,
    disposition: PromotionDispositionV1) -> SingleStateV1
{
    if state.phase != PhaseV1::Completed || !exact_full_h2d_v1(state.request) { state }
    else {
        let base = SingleStateV1 {
            queue_checks: state.queue_checks + promotion_checks_v1(disposition), ..state
        };
        if promotion_terminal_stage_v1(disposition).is_some() {
            SingleStateV1 {
                phase: PhaseV1::TerminalAbsorbed, custody: CustodyV1::OpaqueTerminal,
                terminal_stage: promotion_terminal_stage_v1(disposition), ..base
            }
        } else if state.host_certificate != Some(candidate)
            || !exact_certificate_v1(candidate, state.request) { base }
        else {
            SingleStateV1 {
                phase: PhaseV1::ComputeReady, custody: CustodyV1::ComputeReadyAndHost,
                packet_count: 0, ticket_count: 0, lease_count: 0, completion: None,
                retired_frontiers: 1, ready_digest: Some(candidate.digest), ..base
            }
        }
    }
}

pub open spec fn window_promote_v1(
    state: WindowStateV1, candidate: CertificateV1,
    disposition: PromotionDispositionV1) -> WindowStateV1
{
    if state.phase != PhaseV1::Completed || !exact_full_h2d_v1(state.request) { state }
    else {
        let base = WindowStateV1 {
            queue_checks: state.queue_checks + promotion_checks_v1(disposition), ..state
        };
        if promotion_terminal_stage_v1(disposition).is_some() {
            WindowStateV1 {
                phase: PhaseV1::TerminalAbsorbed, custody: CustodyV1::OpaqueTerminal,
                terminal_stage: promotion_terminal_stage_v1(disposition), ..base
            }
        } else if state.host_certificate != Some(candidate)
            || !exact_certificate_v1(candidate, state.request) { base }
        else {
            WindowStateV1 {
                phase: PhaseV1::ComputeReady, custody: CustodyV1::ComputeReadyAndHost,
                packet_count: 0, ticket_count: 0, lease_count: 0, completion: None,
                retired_frontiers: 1, ready_digest: Some(candidate.digest), ..base
            }
        }
    }
}

pub proof fn initial_single_is_valid_v1(request: RequestV1, certificate: Option<CertificateV1>)
    requires valid_request_v1(request),
        certificate.is_some() ==> exact_certificate_v1(certificate.unwrap(), request),
    ensures valid_single_v1(initial_single_v1(request, certificate)), {}

pub proof fn initial_window_is_valid_v1(request: RequestV1, certificate: Option<CertificateV1>)
    requires valid_request_v1(request),
        certificate.is_some() ==> exact_certificate_v1(certificate.unwrap(), request),
    ensures valid_window_v1(initial_window_v1(request, certificate)), {}

pub proof fn initial_states_refine_v1(request: RequestV1, certificate: Option<CertificateV1>)
    ensures refines_v1(initial_single_v1(request, certificate),
        initial_window_v1(request, certificate)), {}

pub proof fn projection_has_exactly_one_request_v1(single: SingleStateV1)
    ensures project_single_v1(single).request_count == 1,
        project_single_v1(single).request == single.request, {}

pub proof fn maximum_packet_is_admitted_v1(request: RequestV1)
    requires valid_request_v1(request), request.copy_bytes == max_packet_bytes_v1(),
    ensures request.copy_bytes <= max_packet_bytes_v1(), {}

pub proof fn maximum_plus_one_is_rejected_v1(request: RequestV1)
    requires request.copy_bytes == max_packet_bytes_v1() + 1,
    ensures !valid_request_v1(request), {}

pub proof fn single_submit_preserves_validity_v1(
    state: SingleStateV1, disposition: SubmitDispositionV1)
    requires valid_single_v1(state), state.phase == PhaseV1::Ready,
    ensures valid_single_v1(single_submit_v1(state, disposition)), {}

pub proof fn window_submit_preserves_validity_v1(
    state: WindowStateV1, disposition: SubmitDispositionV1)
    requires valid_window_v1(state), state.phase == PhaseV1::Ready,
    ensures valid_window_v1(window_submit_v1(state, disposition)), {}

pub proof fn submit_transition_refines_v1(
    single: SingleStateV1, window: WindowStateV1, disposition: SubmitDispositionV1)
    requires refines_v1(single, window),
    ensures refines_v1(single_submit_v1(single, disposition),
        window_submit_v1(window, disposition)), {}

pub proof fn published_single_has_one_packet_v1(state: SingleStateV1)
    requires state.phase == PhaseV1::Ready,
    ensures single_submit_v1(state, SubmitDispositionV1::Published).packet_count == 1, {}

pub proof fn published_single_has_one_ticket_and_lease_v1(state: SingleStateV1)
    requires state.phase == PhaseV1::Ready,
    ensures single_submit_v1(state, SubmitDispositionV1::Published).ticket_count == 1,
        single_submit_v1(state, SubmitDispositionV1::Published).lease_count == 1, {}

pub proof fn published_single_uses_four_checks_v1(state: SingleStateV1)
    requires state.phase == PhaseV1::Ready,
    ensures single_submit_v1(state, SubmitDispositionV1::Published).directional_checks
            == state.directional_checks + 3,
        single_submit_v1(state, SubmitDispositionV1::Published).queue_checks
            == state.queue_checks + 1, {}

pub proof fn h2d_publication_preserves_certificate_v1(state: SingleStateV1)
    requires valid_single_v1(state), state.phase == PhaseV1::Ready,
        state.request.direction == DirectionV1::HostToDevice,
    ensures single_submit_v1(state, SubmitDispositionV1::Published).host_certificate
            == state.host_certificate,
        !single_submit_v1(state, SubmitDispositionV1::Published)
            .host_certificate_invalidated, {}

pub proof fn d2h_publication_invalidates_certificate_v1(state: SingleStateV1)
    requires state.phase == PhaseV1::Ready,
        state.request.direction == DirectionV1::DeviceToHost,
    ensures single_submit_v1(state, SubmitDispositionV1::Published).host_certificate.is_none(),
        single_submit_v1(state, SubmitDispositionV1::Published)
            .host_certificate_invalidated,
        single_submit_v1(state, SubmitDispositionV1::Published)
            .host_destination_may_have_mutated,
        single_submit_v1(state, SubmitDispositionV1::ClosingAmbiguous)
            .host_certificate.is_none(),
        single_submit_v1(state, SubmitDispositionV1::ClosingAmbiguous)
            .host_certificate_invalidated,
        single_submit_v1(state, SubmitDispositionV1::ClosingAmbiguous)
            .host_destination_may_have_mutated,
        single_submit_v1(state, SubmitDispositionV1::ClosingAmbiguous).retired_frontiers
            == state.retired_frontiers, {}

pub proof fn pre_request_retry_preserves_d2h_certificate_v1(state: SingleStateV1)
    requires state.phase == PhaseV1::Ready,
        state.request.direction == DirectionV1::DeviceToHost,
    ensures single_submit_v1(state, SubmitDispositionV1::RetryableBeforeRequest)
            .host_certificate == state.host_certificate,
        single_submit_v1(state, SubmitDispositionV1::RetryableBeforeRequest)
            .host_certificate_invalidated == state.host_certificate_invalidated, {}

pub proof fn post_request_retry_invalidates_d2h_certificate_v1(state: SingleStateV1)
    requires state.phase == PhaseV1::Ready,
        state.request.direction == DirectionV1::DeviceToHost,
    ensures single_submit_v1(state, SubmitDispositionV1::PrepareRetryable)
            .host_certificate.is_none(),
        single_submit_v1(state, SubmitDispositionV1::PrepareRetryable)
            .host_certificate_invalidated,
        single_submit_v1(state, SubmitDispositionV1::PublicationRetryable)
            .host_certificate.is_none(),
        single_submit_v1(state, SubmitDispositionV1::PublicationRetryable)
            .host_certificate_invalidated, {}

pub proof fn opening_submit_ambiguity_has_exact_stage_v1(state: SingleStateV1)
    requires state.phase == PhaseV1::Ready,
    ensures single_submit_v1(state, SubmitDispositionV1::OpeningAmbiguous).terminal_stage
        == Some(TerminalStageV1::SubmitOpening), {}

pub proof fn prepare_submit_ambiguity_has_exact_stage_v1(state: SingleStateV1)
    requires state.phase == PhaseV1::Ready,
    ensures single_submit_v1(state, SubmitDispositionV1::PrepareAmbiguous).terminal_stage
        == Some(TerminalStageV1::SubmitPrepare), {}

pub proof fn closing_submit_ambiguity_has_exact_stage_v1(state: SingleStateV1)
    requires state.phase == PhaseV1::Ready,
    ensures single_submit_v1(state, SubmitDispositionV1::ClosingAmbiguous).terminal_stage
        == Some(TerminalStageV1::SubmitClosing), {}

pub proof fn retryable_submit_does_not_publish_v1(state: SingleStateV1)
    requires state.phase == PhaseV1::Ready,
    ensures single_submit_v1(state, SubmitDispositionV1::RetryableBeforeRequest).phase
            == PhaseV1::Ready,
        single_submit_v1(state, SubmitDispositionV1::PrepareRetryable).phase
            == PhaseV1::Ready,
        single_submit_v1(state, SubmitDispositionV1::PublicationRetryable).phase
            == PhaseV1::Ready, {}

pub proof fn single_poll_preserves_validity_v1(
    state: SingleStateV1, disposition: PollDispositionV1)
    requires valid_single_v1(state), state.phase == PhaseV1::Published,
    ensures valid_single_v1(single_poll_v1(state, disposition)), {}

pub proof fn window_poll_preserves_validity_v1(
    state: WindowStateV1, disposition: PollDispositionV1)
    requires valid_window_v1(state), state.phase == PhaseV1::Published,
    ensures valid_window_v1(window_poll_v1(state, disposition)), {}

pub proof fn poll_transition_refines_v1(
    single: SingleStateV1, window: WindowStateV1, disposition: PollDispositionV1)
    requires refines_v1(single, window),
    ensures refines_v1(single_poll_v1(single, disposition),
        window_poll_v1(window, disposition)), {}

pub proof fn pending_poll_retains_published_custody_v1(state: SingleStateV1)
    requires valid_single_v1(state), state.phase == PhaseV1::Published,
    ensures single_poll_v1(state, PollDispositionV1::Pending).phase == PhaseV1::Published,
        single_poll_v1(state, PollDispositionV1::Pending).custody
            == CustodyV1::PublishedSingle,
        single_poll_v1(state, PollDispositionV1::Pending).ticket_count
            == state.ticket_count, {}

pub proof fn poll_has_two_operational_checks_v1(state: SingleStateV1)
    requires state.phase == PhaseV1::Published,
    ensures single_poll_v1(state, PollDispositionV1::Pending).queue_checks
            == state.queue_checks + 2,
        single_poll_v1(state, PollDispositionV1::Completed).queue_checks
            == state.queue_checks + 2, {}

pub proof fn completed_single_retains_exact_metadata_v1(state: SingleStateV1)
    requires state.phase == PhaseV1::Published,
    ensures single_poll_v1(state, PollDispositionV1::Completed).completion
            == Some(completion_for_v1(state.request)),
        single_poll_v1(state, PollDispositionV1::Completed)
            .completion.unwrap().packet_count == 1, {}

pub proof fn completion_projection_retains_offsets_v1(state: SingleStateV1)
    requires state.phase == PhaseV1::Published,
    ensures {
        let completed = single_poll_v1(state, PollDispositionV1::Completed);
        let projected = project_single_v1(completed);
        &&& projected.completion.unwrap().host_offset == state.request.host_offset
        &&& projected.completion.unwrap().device_offset == state.request.device_offset
        &&& projected.completion.unwrap().copy_bytes == state.request.copy_bytes
        &&& projected.completion.unwrap().packet_count == 1
    }, {}

pub proof fn opening_poll_ambiguity_retires_nothing_v1(state: SingleStateV1)
    requires valid_single_v1(state), state.phase == PhaseV1::Published,
    ensures single_poll_v1(state, PollDispositionV1::OpeningAmbiguous).terminal_stage
            == Some(TerminalStageV1::PollOpening),
        single_poll_v1(state, PollDispositionV1::OpeningAmbiguous).completion.is_none(),
        single_poll_v1(state, PollDispositionV1::OpeningAmbiguous).retired_frontiers
            == state.retired_frontiers, {}

pub proof fn closing_poll_ambiguity_retires_nothing_v1(state: SingleStateV1)
    requires valid_single_v1(state), state.phase == PhaseV1::Published,
    ensures single_poll_v1(state, PollDispositionV1::ClosingAmbiguous).terminal_stage
            == Some(TerminalStageV1::PollClosing),
        single_poll_v1(state, PollDispositionV1::ClosingAmbiguous).completion.is_none(),
        single_poll_v1(state, PollDispositionV1::ClosingAmbiguous).retired_frontiers
            == state.retired_frontiers, {}

pub proof fn promotion_transition_refines_v1(
    single: SingleStateV1, window: WindowStateV1, candidate: CertificateV1,
    disposition: PromotionDispositionV1)
    requires refines_v1(single, window),
    ensures refines_v1(single_promote_v1(single, candidate, disposition),
        window_promote_v1(window, candidate, disposition)), {}

pub proof fn promotion_mismatch_retires_nothing_v1(
    state: SingleStateV1, candidate: CertificateV1)
    requires state.phase == PhaseV1::Completed, exact_full_h2d_v1(state.request),
        state.host_certificate != Some(candidate) || !exact_certificate_v1(candidate, state.request),
    ensures single_promote_v1(state, candidate, PromotionDispositionV1::Current).phase
            == PhaseV1::Completed,
        single_promote_v1(state, candidate, PromotionDispositionV1::Current).retired_frontiers
            == state.retired_frontiers,
        single_promote_v1(state, candidate, PromotionDispositionV1::Current).completion
            == state.completion, {}

pub proof fn exact_promotion_retires_once_and_mints_digest_v1(
    state: SingleStateV1, certificate: CertificateV1)
    requires state.phase == PhaseV1::Completed, exact_full_h2d_v1(state.request),
        state.host_certificate == Some(certificate),
        exact_certificate_v1(certificate, state.request),
    ensures single_promote_v1(state, certificate, PromotionDispositionV1::Current).phase
            == PhaseV1::ComputeReady,
        single_promote_v1(state, certificate, PromotionDispositionV1::Current).retired_frontiers
            == 1,
        single_promote_v1(state, certificate, PromotionDispositionV1::Current).ready_digest
            == Some(certificate.digest),
        single_promote_v1(state, certificate, PromotionDispositionV1::Current)
            .completion.is_none(), {}

pub proof fn opening_promotion_ambiguity_precedes_mismatch_v1(
    state: SingleStateV1, candidate: CertificateV1)
    requires state.phase == PhaseV1::Completed, exact_full_h2d_v1(state.request),
    ensures single_promote_v1(state, candidate, PromotionDispositionV1::OpeningAmbiguous).phase
            == PhaseV1::TerminalAbsorbed,
        single_promote_v1(state, candidate, PromotionDispositionV1::OpeningAmbiguous)
            .terminal_stage == Some(TerminalStageV1::PromotionOpening),
        single_promote_v1(state, candidate, PromotionDispositionV1::OpeningAmbiguous)
            .retired_frontiers == state.retired_frontiers,
        single_promote_v1(state, candidate, PromotionDispositionV1::OpeningAmbiguous).completion
            == state.completion, {}

pub proof fn closing_promotion_ambiguity_precedes_mismatch_v1(
    state: SingleStateV1, candidate: CertificateV1)
    requires state.phase == PhaseV1::Completed, exact_full_h2d_v1(state.request),
    ensures single_promote_v1(state, candidate, PromotionDispositionV1::ClosingAmbiguous).phase
            == PhaseV1::TerminalAbsorbed,
        single_promote_v1(state, candidate, PromotionDispositionV1::ClosingAmbiguous)
            .terminal_stage == Some(TerminalStageV1::PromotionClosing),
        single_promote_v1(state, candidate, PromotionDispositionV1::ClosingAmbiguous)
            .retired_frontiers == state.retired_frontiers,
        single_promote_v1(state, candidate, PromotionDispositionV1::ClosingAmbiguous).completion
            == state.completion, {}

pub proof fn exact_promotion_preserves_validity_v1(
    state: SingleStateV1, certificate: CertificateV1)
    requires valid_single_v1(state), state.phase == PhaseV1::Completed,
        exact_full_h2d_v1(state.request), state.host_certificate == Some(certificate),
        exact_certificate_v1(certificate, state.request),
    ensures valid_single_v1(single_promote_v1(
        state, certificate, PromotionDispositionV1::Current)), {}

pub proof fn promotion_ambiguity_preserves_validity_v1(
    state: SingleStateV1, candidate: CertificateV1,
    disposition: PromotionDispositionV1)
    requires valid_single_v1(state), state.phase == PhaseV1::Completed,
        exact_full_h2d_v1(state.request), disposition != PromotionDispositionV1::Current,
    ensures valid_single_v1(single_promote_v1(state, candidate, disposition)), {}

pub proof fn full_h2d_trace_refines_v1(request: RequestV1, certificate: CertificateV1)
    requires valid_request_v1(request), exact_full_h2d_v1(request),
        exact_certificate_v1(certificate, request),
    ensures {
        let single0 = initial_single_v1(request, Some(certificate));
        let window0 = initial_window_v1(request, Some(certificate));
        let single1 = single_submit_v1(single0, SubmitDispositionV1::Published);
        let window1 = window_submit_v1(window0, SubmitDispositionV1::Published);
        let single2 = single_poll_v1(single1, PollDispositionV1::Completed);
        let window2 = window_poll_v1(window1, PollDispositionV1::Completed);
        let single3 = single_promote_v1(single2, certificate, PromotionDispositionV1::Current);
        let window3 = window_promote_v1(window2, certificate, PromotionDispositionV1::Current);
        &&& refines_v1(single1, window1)
        &&& refines_v1(single2, window2)
        &&& refines_v1(single3, window3)
        &&& single3.phase == PhaseV1::ComputeReady
        &&& single3.ready_digest == Some(certificate.digest)
    }, {}

pub proof fn d2h_completion_trace_refines_v1(request: RequestV1, certificate: CertificateV1)
    requires valid_request_v1(request), request.direction == DirectionV1::DeviceToHost,
        exact_certificate_v1(certificate, request),
    ensures {
        let single0 = initial_single_v1(request, Some(certificate));
        let window0 = initial_window_v1(request, Some(certificate));
        let single1 = single_submit_v1(single0, SubmitDispositionV1::Published);
        let window1 = window_submit_v1(window0, SubmitDispositionV1::Published);
        let single2 = single_poll_v1(single1, PollDispositionV1::Completed);
        let window2 = window_poll_v1(window1, PollDispositionV1::Completed);
        &&& refines_v1(single1, window1)
        &&& refines_v1(single2, window2)
        &&& single2.host_certificate.is_none()
        &&& single2.host_certificate_invalidated
        &&& single2.host_destination_may_have_mutated
        &&& single2.completion == Some(completion_for_v1(request))
    }, {}

pub proof fn non_full_or_d2h_promotion_is_no_effect_v1(
    state: SingleStateV1, candidate: CertificateV1, disposition: PromotionDispositionV1)
    requires state.phase == PhaseV1::Completed, !exact_full_h2d_v1(state.request),
    ensures single_promote_v1(state, candidate, disposition) == state, {}

pub proof fn terminal_submit_is_absorbing_v1(
    state: SingleStateV1, disposition: SubmitDispositionV1)
    requires state.phase == PhaseV1::Ready, submit_terminal_stage_v1(disposition).is_some(),
    ensures {
        let terminal = single_submit_v1(state, disposition);
        &&& terminal.phase == PhaseV1::TerminalAbsorbed
        &&& single_submit_v1(terminal, SubmitDispositionV1::Published) == terminal
        &&& single_poll_v1(terminal, PollDispositionV1::Completed) == terminal
    }, {}

fn main() {}
}
