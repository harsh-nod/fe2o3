// Bounded R21 scripted failure-seam model. This is not a refinement of the
// executable Rust model, R20, KFD code, native execution, hardware, liveness,
// HIP/HSA behavior, or performance.
use vstd::prelude::*;

verus! {

pub open spec fn max_packet_bytes_v1() -> nat { 0x003f_ffe0 }
pub open spec fn max_u32_v1() -> nat { 0xffff_ffff }

#[derive(PartialEq, Eq)]
pub enum PhaseV1 {
    HostReady,
    DeviceReady,
    Ready,
    Published,
    TerminalObserved,
    RecyclePending,
    Completed,
    QuiescentWithoutResult,
    DemotedDeviceCleanup,
    Released,
    ProcessTeardown,
}

#[derive(PartialEq, Eq)]
pub enum CustodyV1 {
    Host,
    Device,
    Ready,
    Published,
    Terminal,
    Recycle,
    DemotedDevice,
    Opaque,
    None,
}

#[derive(PartialEq, Eq)]
pub enum FailurePointV1 {
    Promotion,
    Demotion,
    Submission,
    Poll,
    CompletionMetadata,
    Retirement,
    Recycle,
    HiddenCleanup,
}

#[derive(PartialEq, Eq)]
pub enum OperationDispositionV1 { Succeeded, Retryable, ProcessTeardown }

#[derive(PartialEq, Eq)]
pub enum DemotionDispositionV1 {
    Succeeded, RetryableBeforeDemotion, RecoveredDemotedNeedsCleanup, ProcessTeardown,
}

#[derive(PartialEq, Eq)]
pub enum SubmitDispositionV1 { Published, DependenciesPending, Retryable, ProcessTeardown }

pub struct StateV1 {
    pub phase: PhaseV1,
    pub custody: CustodyV1,
    pub authority_count: nat,
    pub allocation: nat,
    pub pair_occurrence: nat,
    pub attachment_generation: nat,
    pub pool_generation: nat,
    pub transfer_id: nat,
    pub total_bytes: nat,
    pub completed_bytes: nat,
    pub packet_offset: nat,
    pub packet_bytes: nat,
    pub ticket_generation: nat,
    pub slot_generation: nat,
    pub target_retained: bool,
    pub terminal_success: bool,
    pub result_code: int,
    pub dirty_through: nat,
    pub host_destination: bool,
    pub host_dirty_through: nat,
    pub opaque_point: Option<FailurePointV1>,
    pub current: bool,
}

pub open spec fn base_valid_v1(s: StateV1) -> bool {
    &&& s.allocation > 0
    &&& s.pair_occurrence > 0
    &&& s.attachment_generation > 0
    &&& s.pool_generation > 0
    &&& s.slot_generation > 0
    &&& s.completed_bytes <= s.total_bytes
    &&& s.dirty_through == s.completed_bytes
    &&& if s.host_destination {
        s.host_dirty_through == s.completed_bytes
    } else {
        s.host_dirty_through == 0
    }
    &&& s.ticket_generation <= max_u32_v1()
    &&& if s.phase == PhaseV1::Released {
        s.authority_count == 0 && s.custody == CustodyV1::None
    } else {
        s.authority_count == 1 && s.custody != CustodyV1::None
    }
}

pub open spec fn valid_state_v1(s: StateV1) -> bool {
    &&& base_valid_v1(s)
    &&& match s.phase {
        PhaseV1::HostReady => s.custody == CustodyV1::Host
            && !s.target_retained && s.current && s.packet_bytes == 0,
        PhaseV1::DeviceReady => s.custody == CustodyV1::Device
            && !s.target_retained && s.current && s.packet_bytes == 0,
        PhaseV1::Ready => s.custody == CustodyV1::Ready
            && s.target_retained && s.current && s.transfer_id > 0
            && s.completed_bytes < s.total_bytes && s.packet_bytes == 0
            && s.packet_offset == s.completed_bytes && s.result_code == 0,
        PhaseV1::Published => s.custody == CustodyV1::Published
            && s.target_retained && s.current && s.transfer_id > 0
            && s.completed_bytes < s.total_bytes
            && s.packet_offset == s.completed_bytes
            && 0 < s.packet_bytes <= max_packet_bytes_v1()
            && s.packet_offset + s.packet_bytes <= s.total_bytes
            && s.ticket_generation > 0 && s.result_code == 0,
        PhaseV1::TerminalObserved => s.custody == CustodyV1::Terminal
            && s.target_retained && s.current && s.transfer_id > 0
            && s.packet_offset == s.completed_bytes && s.packet_bytes > 0
            && s.packet_offset + s.packet_bytes <= s.total_bytes
            && ((s.terminal_success && s.result_code == 0)
                || (!s.terminal_success && s.result_code < 0)),
        PhaseV1::RecyclePending => s.custody == CustodyV1::Recycle
            && s.target_retained && s.current && s.transfer_id > 0
            && s.packet_bytes > 0 && s.completed_bytes <= s.total_bytes
            && ((s.terminal_success && s.result_code == 0)
                || (!s.terminal_success && s.result_code < 0
                    && s.completed_bytes < s.total_bytes)),
        PhaseV1::Completed => s.custody == CustodyV1::Device
            && s.target_retained && s.current && s.transfer_id > 0
            && s.packet_bytes == 0
            && ((s.result_code == 0 && s.completed_bytes == s.total_bytes)
                || (s.result_code < 0 && s.completed_bytes < s.total_bytes)),
        PhaseV1::QuiescentWithoutResult => s.custody == CustodyV1::Device
            && s.target_retained && s.current && s.transfer_id > 0
            && 0 < s.completed_bytes < s.total_bytes
            && s.packet_bytes == 0 && s.result_code == 0,
        PhaseV1::DemotedDeviceCleanup => s.custody == CustodyV1::DemotedDevice
            && !s.target_retained && s.current && s.packet_bytes == 0,
        PhaseV1::Released => !s.target_retained && s.current && s.packet_bytes == 0,
        PhaseV1::ProcessTeardown => s.custody == CustodyV1::Opaque
            && s.authority_count == 1 && !s.current && s.opaque_point.is_some(),
    }
}

pub open spec fn initial_v1(allocation: nat, pair_occurrence: nat,
    attachment_generation: nat, pool_generation: nat) -> StateV1
{
    StateV1 {
        phase: PhaseV1::HostReady,
        custody: CustodyV1::Host,
        authority_count: 1,
        allocation,
        pair_occurrence,
        attachment_generation,
        pool_generation,
        transfer_id: 0,
        total_bytes: 0,
        completed_bytes: 0,
        packet_offset: 0,
        packet_bytes: 0,
        ticket_generation: 0,
        slot_generation: 1,
        target_retained: false,
        terminal_success: false,
        result_code: 0,
        dirty_through: 0,
        host_destination: false,
        host_dirty_through: 0,
        opaque_point: None,
        current: true,
    }
}

pub open spec fn teardown_v1(s: StateV1, point: FailurePointV1) -> StateV1 {
    StateV1 { phase: PhaseV1::ProcessTeardown, custody: CustodyV1::Opaque,
        authority_count: 1, opaque_point: Some(point), current: false, ..s }
}

pub open spec fn promote_v1(s: StateV1, disposition: OperationDispositionV1) -> StateV1 {
    if s.phase != PhaseV1::HostReady { s } else {
        match disposition {
            OperationDispositionV1::Succeeded => StateV1 {
                phase: PhaseV1::DeviceReady, custody: CustodyV1::Device, ..s },
            OperationDispositionV1::Retryable => s,
            OperationDispositionV1::ProcessTeardown => teardown_v1(s, FailurePointV1::Promotion),
        }
    }
}

pub open spec fn demote_v1(s: StateV1, disposition: DemotionDispositionV1) -> StateV1 {
    if s.phase != PhaseV1::DeviceReady { s } else {
        match disposition {
            DemotionDispositionV1::Succeeded => StateV1 {
                phase: PhaseV1::HostReady, custody: CustodyV1::Host, ..s },
            DemotionDispositionV1::RetryableBeforeDemotion => s,
            DemotionDispositionV1::RecoveredDemotedNeedsCleanup => StateV1 {
                phase: PhaseV1::DemotedDeviceCleanup,
                custody: CustodyV1::DemotedDevice, ..s },
            DemotionDispositionV1::ProcessTeardown => teardown_v1(s, FailurePointV1::Demotion),
        }
    }
}

pub open spec fn hidden_cleanup_v1(s: StateV1,
    disposition: OperationDispositionV1) -> StateV1
{
    if s.phase != PhaseV1::DemotedDeviceCleanup { s } else {
        match disposition {
            OperationDispositionV1::Succeeded => StateV1 {
                phase: PhaseV1::HostReady, custody: CustodyV1::Host, ..s },
            OperationDispositionV1::Retryable => s,
            OperationDispositionV1::ProcessTeardown =>
                teardown_v1(s, FailurePointV1::HiddenCleanup),
        }
    }
}

pub open spec fn begin_v1(s: StateV1, transfer_id: nat, total_bytes: nat,
    host_destination: bool) -> StateV1 {
    if s.phase == PhaseV1::DeviceReady && transfer_id > 0 && total_bytes > 0 {
        StateV1 { phase: PhaseV1::Ready, custody: CustodyV1::Ready,
            transfer_id, total_bytes, completed_bytes: 0, packet_offset: 0,
            packet_bytes: 0, target_retained: true, terminal_success: false,
            result_code: 0, dirty_through: 0, host_destination,
            host_dirty_through: 0, ..s }
    } else { s }
}

pub open spec fn chunk_bytes_v1(s: StateV1) -> nat {
    if s.total_bytes - s.completed_bytes <= max_packet_bytes_v1() {
        (s.total_bytes - s.completed_bytes) as nat
    } else { max_packet_bytes_v1() }
}

pub open spec fn submit_v1(s: StateV1, disposition: SubmitDispositionV1) -> StateV1 {
    if s.phase != PhaseV1::Ready { s } else {
        match disposition {
            SubmitDispositionV1::Published => if s.ticket_generation < max_u32_v1() {
                StateV1 { phase: PhaseV1::Published, custody: CustodyV1::Published,
                    packet_offset: s.completed_bytes, packet_bytes: chunk_bytes_v1(s),
                    ticket_generation: s.ticket_generation + 1, ..s }
            } else { s },
            SubmitDispositionV1::DependenciesPending => s,
            SubmitDispositionV1::Retryable => if s.completed_bytes == 0 {
                StateV1 { phase: PhaseV1::Completed, custody: CustodyV1::Device,
                    packet_bytes: 0, result_code: -1, ..s }
            } else {
                StateV1 { phase: PhaseV1::QuiescentWithoutResult,
                    custody: CustodyV1::Device, packet_bytes: 0, result_code: 0, ..s }
            },
            SubmitDispositionV1::ProcessTeardown =>
                teardown_v1(s, FailurePointV1::Submission),
        }
    }
}

pub open spec fn poll_pending_v1(s: StateV1) -> StateV1 { s }
pub open spec fn poll_retryable_v1(s: StateV1) -> StateV1 { s }
pub open spec fn poll_timeout_v1(s: StateV1) -> StateV1 { s }
pub open spec fn poll_teardown_v1(s: StateV1) -> StateV1 {
    if s.phase == PhaseV1::Published { teardown_v1(s, FailurePointV1::Poll) } else { s }
}

pub open spec fn complete_v1(s: StateV1, metadata_matches: bool,
    terminal_success: bool, failure_code: int) -> StateV1
{
    if s.phase != PhaseV1::Published { s }
    else if !metadata_matches { teardown_v1(s, FailurePointV1::CompletionMetadata) }
    else { StateV1 { phase: PhaseV1::TerminalObserved, custody: CustodyV1::Terminal,
        terminal_success, result_code: if terminal_success { 0 } else { failure_code }, ..s } }
}

pub open spec fn retire_v1(s: StateV1, frontier_matches: bool,
    disposition: OperationDispositionV1) -> StateV1
{
    if s.phase != PhaseV1::TerminalObserved { s }
    else if !frontier_matches { teardown_v1(s, FailurePointV1::Retirement) }
    else {
        match disposition {
            OperationDispositionV1::Succeeded => {
                let through = if s.terminal_success {
                    s.completed_bytes + s.packet_bytes
                } else { s.completed_bytes };
                StateV1 { phase: PhaseV1::RecyclePending, custody: CustodyV1::Recycle,
                    completed_bytes: through, dirty_through: through,
                    host_dirty_through: if s.host_destination { through } else { 0 }, ..s }
            },
            OperationDispositionV1::Retryable => s,
            OperationDispositionV1::ProcessTeardown =>
                teardown_v1(s, FailurePointV1::Retirement),
        }
    }
}

pub open spec fn recycle_v1(s: StateV1, recycle_matches: bool,
    disposition: OperationDispositionV1) -> StateV1
{
    if s.phase != PhaseV1::RecyclePending { s }
    else if !recycle_matches { teardown_v1(s, FailurePointV1::Recycle) }
    else {
        match disposition {
            OperationDispositionV1::Succeeded => if s.terminal_success
                && s.completed_bytes < s.total_bytes {
                StateV1 { phase: PhaseV1::Ready, custody: CustodyV1::Ready,
                    packet_offset: s.completed_bytes, packet_bytes: 0,
                    ticket_generation: s.ticket_generation,
                    slot_generation: s.slot_generation + 1,
                    terminal_success: false, result_code: 0, ..s }
            } else {
                StateV1 { phase: PhaseV1::Completed, custody: CustodyV1::Device,
                    packet_offset: s.completed_bytes, packet_bytes: 0,
                    slot_generation: s.slot_generation + 1, ..s }
            },
            OperationDispositionV1::Retryable => s,
            OperationDispositionV1::ProcessTeardown =>
                teardown_v1(s, FailurePointV1::Recycle),
        }
    }
}

pub open spec fn release_terminal_v1(s: StateV1, transfer_id: nat) -> StateV1 {
    if (s.phase == PhaseV1::Completed || s.phase == PhaseV1::QuiescentWithoutResult)
        && s.transfer_id == transfer_id && s.target_retained {
        StateV1 { phase: PhaseV1::DeviceReady, custody: CustodyV1::Device,
            transfer_id: 0, total_bytes: 0, completed_bytes: 0,
            packet_offset: 0, packet_bytes: 0, target_retained: false,
            terminal_success: false, result_code: 0, dirty_through: 0,
            host_destination: false, host_dirty_through: 0, ..s }
    } else { s }
}

pub open spec fn release_allocation_v1(s: StateV1) -> StateV1 {
    if s.phase == PhaseV1::HostReady && !s.target_retained {
        StateV1 { phase: PhaseV1::Released, custody: CustodyV1::None,
            authority_count: 0, ..s }
    } else { s }
}

pub proof fn initial_is_valid_v1()
    ensures valid_state_v1(initial_v1(1, 2, 3, 4)),
{}

pub proof fn promotion_success_preserves_validity_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::HostReady,
    ensures valid_state_v1(promote_v1(s, OperationDispositionV1::Succeeded)),
{}

pub proof fn promotion_retry_is_atomic_v1(s: StateV1)
    ensures promote_v1(s, OperationDispositionV1::Retryable) == s,
{}

pub proof fn promotion_teardown_retains_authority_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::HostReady,
    ensures valid_state_v1(promote_v1(s, OperationDispositionV1::ProcessTeardown)),
            promote_v1(s, OperationDispositionV1::ProcessTeardown).authority_count == 1,
{}

pub proof fn demotion_success_preserves_validity_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::DeviceReady,
    ensures valid_state_v1(demote_v1(s, DemotionDispositionV1::Succeeded)),
{}

pub proof fn demotion_retry_is_atomic_v1(s: StateV1)
    ensures demote_v1(s, DemotionDispositionV1::RetryableBeforeDemotion) == s,
{}

pub proof fn recovered_demoted_owner_enters_cleanup_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::DeviceReady,
    ensures valid_state_v1(demote_v1(s, DemotionDispositionV1::RecoveredDemotedNeedsCleanup)),
            demote_v1(s, DemotionDispositionV1::RecoveredDemotedNeedsCleanup).phase
                == PhaseV1::DemotedDeviceCleanup,
{}

pub proof fn demotion_teardown_retains_authority_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::DeviceReady,
    ensures valid_state_v1(demote_v1(s, DemotionDispositionV1::ProcessTeardown)),
{}

pub proof fn hidden_cleanup_retry_is_atomic_v1(s: StateV1)
    ensures hidden_cleanup_v1(s, OperationDispositionV1::Retryable) == s,
{}

pub proof fn hidden_cleanup_success_preserves_validity_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::DemotedDeviceCleanup,
    ensures valid_state_v1(hidden_cleanup_v1(s, OperationDispositionV1::Succeeded)),
{}

pub proof fn hidden_cleanup_teardown_retains_authority_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::DemotedDeviceCleanup,
    ensures valid_state_v1(hidden_cleanup_v1(s, OperationDispositionV1::ProcessTeardown)),
{}

pub proof fn begin_preserves_validity_v1(s: StateV1, transfer_id: nat, total_bytes: nat,
    host_destination: bool)
    requires valid_state_v1(s), s.phase == PhaseV1::DeviceReady,
        transfer_id > 0, total_bytes > 0,
    ensures valid_state_v1(begin_v1(s, transfer_id, total_bytes, host_destination)),
{}

pub proof fn submit_pending_is_atomic_v1(s: StateV1)
    ensures submit_v1(s, SubmitDispositionV1::DependenciesPending) == s,
{}

pub proof fn submit_publication_preserves_validity_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::Ready,
        s.ticket_generation < max_u32_v1(),
    ensures valid_state_v1(submit_v1(s, SubmitDispositionV1::Published)),
{}

pub proof fn initial_retry_is_conclusive_failure_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::Ready, s.completed_bytes == 0,
    ensures valid_state_v1(submit_v1(s, SubmitDispositionV1::Retryable)),
        submit_v1(s, SubmitDispositionV1::Retryable).phase == PhaseV1::Completed,
        submit_v1(s, SubmitDispositionV1::Retryable).result_code == -1,
{}

pub proof fn partial_retry_is_quiescent_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::Ready, s.completed_bytes > 0,
    ensures valid_state_v1(submit_v1(s, SubmitDispositionV1::Retryable)),
        submit_v1(s, SubmitDispositionV1::Retryable).phase
            == PhaseV1::QuiescentWithoutResult,
        submit_v1(s, SubmitDispositionV1::Retryable).completed_bytes == s.completed_bytes,
{}

pub proof fn partial_host_mutation_retry_is_quiescent_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::Ready,
        s.host_destination, s.completed_bytes > 0,
    ensures valid_state_v1(submit_v1(s, SubmitDispositionV1::Retryable)),
        submit_v1(s, SubmitDispositionV1::Retryable).phase
            == PhaseV1::QuiescentWithoutResult,
        submit_v1(s, SubmitDispositionV1::Retryable).host_dirty_through
            == s.host_dirty_through,
        submit_v1(s, SubmitDispositionV1::Retryable).host_dirty_through > 0,
{}

pub proof fn submission_teardown_retains_authority_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::Ready,
    ensures valid_state_v1(submit_v1(s, SubmitDispositionV1::ProcessTeardown)),
{}

pub proof fn pending_poll_is_observation_only_v1(s: StateV1)
    ensures poll_pending_v1(s) == s,
{}

pub proof fn retryable_poll_is_observation_only_v1(s: StateV1)
    ensures poll_retryable_v1(s) == s,
{}

pub proof fn timeout_is_observation_only_v1(s: StateV1)
    ensures poll_timeout_v1(s) == s,
{}

pub proof fn poll_teardown_retains_authority_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::Published,
    ensures valid_state_v1(poll_teardown_v1(s)),
        poll_teardown_v1(s).authority_count == 1,
{}

pub proof fn exact_completion_preserves_validity_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::Published,
    ensures valid_state_v1(complete_v1(s, true, true, 0)),
{}

pub proof fn completion_metadata_mismatch_tears_down_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::Published,
    ensures valid_state_v1(complete_v1(s, false, true, 0)),
        complete_v1(s, false, true, 0).phase == PhaseV1::ProcessTeardown,
{}

pub proof fn retirement_retry_is_atomic_v1(s: StateV1)
    ensures retire_v1(s, true, OperationDispositionV1::Retryable) == s,
{}

pub proof fn retirement_success_preserves_validity_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::TerminalObserved,
    ensures valid_state_v1(retire_v1(s, true, OperationDispositionV1::Succeeded)),
{}

pub proof fn retirement_mismatch_tears_down_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::TerminalObserved,
    ensures valid_state_v1(retire_v1(s, false, OperationDispositionV1::Succeeded)),
{}

pub proof fn retirement_teardown_retains_authority_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::TerminalObserved,
    ensures valid_state_v1(retire_v1(s, true, OperationDispositionV1::ProcessTeardown)),
{}

pub proof fn recycle_retry_is_atomic_v1(s: StateV1)
    ensures recycle_v1(s, true, OperationDispositionV1::Retryable) == s,
{}

pub proof fn recycle_success_preserves_validity_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::RecyclePending,
    ensures valid_state_v1(recycle_v1(s, true, OperationDispositionV1::Succeeded)),
        recycle_v1(s, true, OperationDispositionV1::Succeeded).slot_generation
            == s.slot_generation + 1,
{}

pub proof fn recycle_mismatch_tears_down_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::RecyclePending,
    ensures valid_state_v1(recycle_v1(s, false, OperationDispositionV1::Succeeded)),
{}

pub proof fn recycle_teardown_retains_authority_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::RecyclePending,
    ensures valid_state_v1(recycle_v1(s, true, OperationDispositionV1::ProcessTeardown)),
{}

pub proof fn continuation_waits_for_recycle_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::TerminalObserved,
        s.terminal_success, s.completed_bytes + s.packet_bytes < s.total_bytes,
    ensures retire_v1(s, true, OperationDispositionV1::Succeeded).phase
            == PhaseV1::RecyclePending,
        recycle_v1(retire_v1(s, true, OperationDispositionV1::Succeeded), true,
            OperationDispositionV1::Succeeded).phase == PhaseV1::Ready,
{}

pub proof fn exact_terminal_release_preserves_validity_v1(s: StateV1)
    requires valid_state_v1(s),
        s.phase == PhaseV1::Completed || s.phase == PhaseV1::QuiescentWithoutResult,
    ensures valid_state_v1(release_terminal_v1(s, s.transfer_id)),
        release_terminal_v1(s, s.transfer_id).phase == PhaseV1::DeviceReady,
{}

pub proof fn foreign_terminal_release_is_atomic_v1(s: StateV1, foreign: nat)
    requires foreign != s.transfer_id,
    ensures release_terminal_v1(s, foreign) == s,
{}

pub proof fn teardown_blocks_release_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::ProcessTeardown,
    ensures release_terminal_v1(s, s.transfer_id) == s,
        release_allocation_v1(s) == s,
{}

pub proof fn host_release_discharges_exact_authority_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::HostReady,
    ensures valid_state_v1(release_allocation_v1(s)),
        release_allocation_v1(s).authority_count == 0,
{}

} // verus!
