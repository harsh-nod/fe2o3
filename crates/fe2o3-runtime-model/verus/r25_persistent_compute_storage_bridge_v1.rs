// Independent finite R25 persistent-compute storage-bridge model. This is not
// a refinement of executable Rust, runtime/KFD code, firmware, or hardware.
use vstd::prelude::*;

verus! {

pub open spec fn max_storage_bytes_v1() -> nat { 256 * 1024 * 1024 }
pub open spec fn max_generation_v1() -> nat { 18_446_744_073_709_551_615 }

#[derive(PartialEq, Eq)]
pub struct StorageIdentityV1 {
    pub device: nat,
    pub vm: nat,
    pub allocation: nat,
    pub storage_generation: nat,
}

#[derive(PartialEq, Eq)]
pub struct FullRangeV1 {
    pub logical_offset: nat,
    pub logical_bytes: nat,
    pub physical_offset: nat,
    pub physical_bytes: nat,
}

#[derive(PartialEq, Eq)]
pub struct EffectsV1 {
    pub reads: bool,
    pub writes: bool,
}

#[derive(PartialEq, Eq)]
pub enum AuthorizationV1 { None, Read, Write, ReadWrite }

#[derive(PartialEq, Eq)]
pub enum PhaseV1 {
    FullH2dReady,
    PreparedCompute,
    Published,
    Completed,
    Restored,
    Device,
    Quarantined,
}

#[derive(PartialEq, Eq)]
pub enum QuarantineReasonV1 {
    None,
    AmbiguousPublication,
    PostRetentionFault,
    CompletionAuthenticationFailed,
    AmbiguousRestore,
}

#[derive(PartialEq, Eq)]
pub struct BridgeKeyV1 {
    pub storage: StorageIdentityV1,
    pub operation_generation: nat,
}

#[derive(PartialEq, Eq)]
pub struct PrepareRequestV1 {
    pub expected_storage: StorageIdentityV1,
    pub expected_frontier: nat,
    pub range: FullRangeV1,
    pub effects: EffectsV1,
}

#[derive(PartialEq, Eq)]
pub struct CompletionV1 {
    pub key: BridgeKeyV1,
    pub range: FullRangeV1,
    pub authorization: AuthorizationV1,
}

#[derive(PartialEq, Eq)]
pub enum PublishDispositionV1 {
    Published,
    RetryableNoEffect,
    AmbiguousFailure,
    PostRetentionFault,
}

#[derive(PartialEq, Eq)]
pub enum CompletionDispositionV1 {
    Pending,
    Completed(CompletionV1),
    AmbiguousFailure,
    PostRetentionFault,
}

#[derive(PartialEq, Eq)]
pub enum RestoreDispositionV1 {
    Restored,
    RetryableNoEffect,
    AmbiguousFailure,
    PostRetentionFault,
}

#[derive(PartialEq, Eq)]
pub struct BridgeStateV1 {
    pub storage: StorageIdentityV1,
    pub storage_bytes: nat,
    pub phase: PhaseV1,
    pub initialized: bool,
    pub fast_path_selected: bool,
    pub active_generation: nat,
    pub retired_frontier: nat,
    pub range: Option<FullRangeV1>,
    pub authorization: AuthorizationV1,
    pub completion: Option<CompletionV1>,
    pub quarantine_reason: QuarantineReasonV1,
    pub generic_materializations: nat,
}

pub open spec fn valid_storage_v1(storage: StorageIdentityV1) -> bool {
    storage.device > 0 && storage.vm > 0 && storage.allocation > 0
        && 0 < storage.storage_generation <= max_generation_v1()
}

pub open spec fn exact_full_range_v1(range: FullRangeV1, bytes: nat) -> bool {
    range.logical_offset == 0 && range.physical_offset == 0
        && range.logical_bytes == bytes && range.physical_bytes == bytes
}

pub open spec fn derived_authorization_v1(effects: EffectsV1) -> AuthorizationV1 {
    if effects.reads && effects.writes { AuthorizationV1::ReadWrite }
    else if effects.reads { AuthorizationV1::Read }
    else if effects.writes { AuthorizationV1::Write }
    else { AuthorizationV1::None }
}

pub open spec fn authorization_reads_v1(authorization: AuthorizationV1) -> bool {
    authorization == AuthorizationV1::Read || authorization == AuthorizationV1::ReadWrite
}

pub open spec fn authorization_writes_v1(authorization: AuthorizationV1) -> bool {
    authorization == AuthorizationV1::Write || authorization == AuthorizationV1::ReadWrite
}

pub open spec fn active_key_v1(state: BridgeStateV1) -> BridgeKeyV1 {
    BridgeKeyV1 { storage: state.storage, operation_generation: state.active_generation }
}

pub open spec fn valid_state_v1(state: BridgeStateV1) -> bool {
    &&& valid_storage_v1(state.storage)
    &&& 0 < state.storage_bytes <= max_storage_bytes_v1()
    &&& state.active_generation <= max_generation_v1()
    &&& state.retired_frontier <= max_generation_v1()
    &&& state.generic_materializations == 0
    &&& match state.phase {
        PhaseV1::FullH2dReady =>
            state.initialized && !state.fast_path_selected && state.active_generation == 0
                && state.range.is_none() && state.authorization == AuthorizationV1::None
                && state.completion.is_none() && state.quarantine_reason == QuarantineReasonV1::None,
        PhaseV1::Device =>
            !state.fast_path_selected && state.active_generation == 0
                && state.range.is_none() && state.authorization == AuthorizationV1::None
                && state.completion.is_none() && state.quarantine_reason == QuarantineReasonV1::None,
        PhaseV1::PreparedCompute | PhaseV1::Published =>
            state.fast_path_selected && state.active_generation > state.retired_frontier
                && state.range.is_some() && state.authorization != AuthorizationV1::None
                && state.completion.is_none() && state.quarantine_reason == QuarantineReasonV1::None,
        PhaseV1::Completed | PhaseV1::Restored =>
            state.fast_path_selected && state.active_generation > state.retired_frontier
                && state.range.is_some() && state.authorization != AuthorizationV1::None
                && state.completion.is_some() && state.quarantine_reason == QuarantineReasonV1::None,
        PhaseV1::Quarantined =>
            state.fast_path_selected && state.active_generation > state.retired_frontier
                && state.range.is_some() && state.authorization != AuthorizationV1::None
                && state.quarantine_reason != QuarantineReasonV1::None,
    }
    &&& match state.range {
        Some(range) => exact_full_range_v1(range, state.storage_bytes),
        None => true,
    }
    &&& (!authorization_reads_v1(state.authorization) || state.initialized)
    &&& match state.completion {
        Some(completion) => completion.key == active_key_v1(state)
            && Some(completion.range) == state.range
            && completion.authorization == state.authorization,
        None => true,
    }
}

pub open spec fn prepare_allowed_v1(state: BridgeStateV1, request: PrepareRequestV1) -> bool {
    &&& valid_state_v1(state)
    &&& (state.phase == PhaseV1::FullH2dReady || state.phase == PhaseV1::Device)
    &&& request.expected_storage == state.storage
    &&& request.expected_frontier == state.retired_frontier
    &&& state.retired_frontier < max_generation_v1()
    &&& exact_full_range_v1(request.range, state.storage_bytes)
    &&& derived_authorization_v1(request.effects) != AuthorizationV1::None
    &&& (!request.effects.reads || state.initialized)
}

pub open spec fn prepare_v1(state: BridgeStateV1, request: PrepareRequestV1) -> BridgeStateV1 {
    if prepare_allowed_v1(state, request) {
        BridgeStateV1 {
            phase: PhaseV1::PreparedCompute,
            fast_path_selected: true,
            active_generation: state.retired_frontier + 1,
            range: Some(request.range),
            authorization: derived_authorization_v1(request.effects),
            completion: None,
            ..state
        }
    } else { state }
}

pub open spec fn matching_active_key_v1(state: BridgeStateV1, key: BridgeKeyV1) -> bool {
    key == active_key_v1(state) && state.active_generation > 0
}

pub open spec fn quarantine_v1(state: BridgeStateV1, reason: QuarantineReasonV1)
    -> BridgeStateV1
{
    if state.phase == PhaseV1::Quarantined { state }
    else { BridgeStateV1 { phase: PhaseV1::Quarantined, quarantine_reason: reason, ..state } }
}

pub open spec fn publish_v1(state: BridgeStateV1, key: BridgeKeyV1,
    disposition: PublishDispositionV1) -> BridgeStateV1
{
    if state.phase == PhaseV1::Quarantined { state }
    else if state.phase != PhaseV1::PreparedCompute || !matching_active_key_v1(state, key) { state }
    else { match disposition {
        PublishDispositionV1::Published => BridgeStateV1 { phase: PhaseV1::Published, ..state },
        PublishDispositionV1::RetryableNoEffect => state,
        PublishDispositionV1::AmbiguousFailure =>
            quarantine_v1(state, QuarantineReasonV1::AmbiguousPublication),
        PublishDispositionV1::PostRetentionFault =>
            quarantine_v1(state, QuarantineReasonV1::PostRetentionFault),
    }}
}

pub open spec fn exact_completion_v1(state: BridgeStateV1,
    completion: CompletionV1) -> bool
{
    completion.key == active_key_v1(state) && Some(completion.range) == state.range
        && completion.authorization == state.authorization
}

pub open spec fn authenticated_completion_v1(state: BridgeStateV1) -> bool {
    match state.completion {
        Some(completion) => exact_completion_v1(state, completion),
        None => false,
    }
}

pub open spec fn observe_completion_v1(state: BridgeStateV1, key: BridgeKeyV1,
    disposition: CompletionDispositionV1) -> BridgeStateV1
{
    if state.phase == PhaseV1::Quarantined { state }
    else if state.phase != PhaseV1::Published || !matching_active_key_v1(state, key) { state }
    else { match disposition {
        CompletionDispositionV1::Pending => state,
        CompletionDispositionV1::Completed(completion) => {
            if exact_completion_v1(state, completion) {
                BridgeStateV1 { phase: PhaseV1::Completed, completion: Some(completion), ..state }
            } else {
                quarantine_v1(state, QuarantineReasonV1::CompletionAuthenticationFailed)
            }
        },
        CompletionDispositionV1::AmbiguousFailure | CompletionDispositionV1::PostRetentionFault =>
            quarantine_v1(state, QuarantineReasonV1::PostRetentionFault),
    }}
}

pub open spec fn restore_v1(state: BridgeStateV1, key: BridgeKeyV1,
    disposition: RestoreDispositionV1) -> BridgeStateV1
{
    if state.phase == PhaseV1::Quarantined { state }
    else if state.phase != PhaseV1::Completed || !matching_active_key_v1(state, key)
        || !authenticated_completion_v1(state) { state }
    else { match disposition {
        RestoreDispositionV1::Restored => BridgeStateV1 { phase: PhaseV1::Restored, ..state },
        RestoreDispositionV1::RetryableNoEffect => state,
        RestoreDispositionV1::AmbiguousFailure =>
            quarantine_v1(state, QuarantineReasonV1::AmbiguousRestore),
        RestoreDispositionV1::PostRetentionFault =>
            quarantine_v1(state, QuarantineReasonV1::PostRetentionFault),
    }}
}

pub open spec fn retire_frontier_v1(state: BridgeStateV1, key: BridgeKeyV1)
    -> BridgeStateV1
{
    if state.phase == PhaseV1::Quarantined { state }
    else if state.phase != PhaseV1::Restored || !matching_active_key_v1(state, key)
        || !authenticated_completion_v1(state)
        || key.operation_generation >= max_generation_v1() { state }
    else { BridgeStateV1 {
        phase: PhaseV1::Device,
        initialized: state.initialized || authorization_writes_v1(state.authorization),
        fast_path_selected: false,
        active_generation: 0,
        retired_frontier: key.operation_generation,
        range: None,
        authorization: AuthorizationV1::None,
        completion: None,
        ..state
    }}
}

pub open spec fn attempt_generic_materialization_v1(state: BridgeStateV1)
    -> BridgeStateV1 { state }

pub open spec fn sample_storage_v1() -> StorageIdentityV1 {
    StorageIdentityV1 { device: 25, vm: 31, allocation: 41, storage_generation: 7 }
}

pub open spec fn sample_range_v1() -> FullRangeV1 {
    FullRangeV1 { logical_offset: 0, logical_bytes: 4096,
        physical_offset: 0, physical_bytes: 4096 }
}

pub open spec fn sample_state_v1() -> BridgeStateV1 {
    BridgeStateV1 { storage: sample_storage_v1(), storage_bytes: 4096,
        phase: PhaseV1::FullH2dReady, initialized: true, fast_path_selected: false,
        active_generation: 0, retired_frontier: 0, range: None,
        authorization: AuthorizationV1::None, completion: None,
        quarantine_reason: QuarantineReasonV1::None, generic_materializations: 0 }
}

pub open spec fn sample_request_v1() -> PrepareRequestV1 {
    PrepareRequestV1 { expected_storage: sample_storage_v1(), expected_frontier: 0,
        range: sample_range_v1(), effects: EffectsV1 { reads: true, writes: true } }
}

pub open spec fn sample_key_v1() -> BridgeKeyV1 {
    BridgeKeyV1 { storage: sample_storage_v1(), operation_generation: 1 }
}

pub open spec fn sample_completion_v1() -> CompletionV1 {
    CompletionV1 { key: sample_key_v1(), range: sample_range_v1(),
        authorization: AuthorizationV1::ReadWrite }
}

pub proof fn constants_are_exact_v1()
    ensures max_storage_bytes_v1() == 268435456,
        max_generation_v1() == 18446744073709551615, {}

pub proof fn sample_values_are_valid_v1()
    ensures valid_storage_v1(sample_storage_v1()), valid_state_v1(sample_state_v1()),
        prepare_allowed_v1(sample_state_v1(), sample_request_v1()), {}

pub proof fn read_authorization_is_derived_v1()
    ensures derived_authorization_v1(EffectsV1 { reads: true, writes: false })
        == AuthorizationV1::Read, {}

pub proof fn write_authorization_is_derived_v1()
    ensures derived_authorization_v1(EffectsV1 { reads: false, writes: true })
        == AuthorizationV1::Write, {}

pub proof fn readwrite_authorization_is_derived_v1()
    ensures derived_authorization_v1(EffectsV1 { reads: true, writes: true })
        == AuthorizationV1::ReadWrite, {}

pub proof fn empty_effects_are_rejected_v1(state: BridgeStateV1,
    request: PrepareRequestV1)
    requires request.effects == (EffectsV1 { reads: false, writes: false }),
    ensures prepare_v1(state, request) == state, {}

pub proof fn uninitialized_read_is_rejected_v1(state: BridgeStateV1,
    request: PrepareRequestV1)
    requires !state.initialized, request.effects.reads,
    ensures prepare_v1(state, request) == state, {}

pub proof fn initialized_read_may_be_admitted_v1(state: BridgeStateV1,
    request: PrepareRequestV1)
    requires prepare_allowed_v1(state, request), request.effects.reads,
    ensures prepare_v1(state, request).authorization
        == derived_authorization_v1(request.effects), {}

pub proof fn nonfull_range_is_rejected_atomically_v1(state: BridgeStateV1,
    request: PrepareRequestV1)
    requires !exact_full_range_v1(request.range, state.storage_bytes),
    ensures prepare_v1(state, request) == state, {}

pub proof fn storage_substitution_is_rejected_atomically_v1(state: BridgeStateV1,
    request: PrepareRequestV1)
    requires request.expected_storage != state.storage,
    ensures prepare_v1(state, request) == state, {}

pub proof fn frontier_aba_is_rejected_atomically_v1(state: BridgeStateV1,
    request: PrepareRequestV1)
    requires request.expected_frontier != state.retired_frontier,
    ensures prepare_v1(state, request) == state, {}

pub proof fn exhausted_generation_is_rejected_atomically_v1(state: BridgeStateV1,
    request: PrepareRequestV1)
    requires state.retired_frontier == max_generation_v1(),
    ensures prepare_v1(state, request) == state, {}

pub proof fn successful_prepare_retains_exact_storage_v1(state: BridgeStateV1,
    request: PrepareRequestV1)
    requires prepare_allowed_v1(state, request),
    ensures prepare_v1(state, request).storage == state.storage,
        prepare_v1(state, request).range == Some(request.range), {}

pub proof fn successful_prepare_selects_fast_path_v1(state: BridgeStateV1,
    request: PrepareRequestV1)
    requires prepare_allowed_v1(state, request),
    ensures prepare_v1(state, request).phase == PhaseV1::PreparedCompute,
        prepare_v1(state, request).fast_path_selected,
        prepare_v1(state, request).generic_materializations == 0, {}

pub proof fn retryable_publish_is_no_effect_v1(state: BridgeStateV1, key: BridgeKeyV1)
    ensures publish_v1(state, key, PublishDispositionV1::RetryableNoEffect) == state, {}

pub proof fn stale_publish_key_is_rejected_atomically_v1(state: BridgeStateV1,
    key: BridgeKeyV1, disposition: PublishDispositionV1)
    requires !matching_active_key_v1(state, key),
    ensures publish_v1(state, key, disposition) == state, {}

pub proof fn successful_publish_retains_exact_custody_v1(state: BridgeStateV1,
    key: BridgeKeyV1)
    requires state.phase == PhaseV1::PreparedCompute, matching_active_key_v1(state, key),
    ensures {
        let published = publish_v1(state, key, PublishDispositionV1::Published);
        &&& published.phase == PhaseV1::Published
        &&& published.storage == state.storage && published.range == state.range
        &&& published.authorization == state.authorization
    }, {}

pub proof fn ambiguous_publish_quarantines_v1(state: BridgeStateV1, key: BridgeKeyV1)
    requires state.phase == PhaseV1::PreparedCompute, matching_active_key_v1(state, key),
    ensures publish_v1(state, key, PublishDispositionV1::AmbiguousFailure).phase
        == PhaseV1::Quarantined, {}

pub proof fn post_retention_publish_fault_quarantines_v1(state: BridgeStateV1,
    key: BridgeKeyV1)
    requires state.phase == PhaseV1::PreparedCompute, matching_active_key_v1(state, key),
    ensures publish_v1(state, key, PublishDispositionV1::PostRetentionFault).phase
        == PhaseV1::Quarantined, {}

pub proof fn pending_completion_retains_published_state_v1(state: BridgeStateV1,
    key: BridgeKeyV1)
    ensures observe_completion_v1(state, key, CompletionDispositionV1::Pending) == state, {}

pub proof fn substituted_completion_quarantines_v1(state: BridgeStateV1,
    key: BridgeKeyV1, completion: CompletionV1)
    requires state.phase == PhaseV1::Published, matching_active_key_v1(state, key),
        !exact_completion_v1(state, completion),
    ensures observe_completion_v1(state, key, CompletionDispositionV1::Completed(completion)).phase
        == PhaseV1::Quarantined, {}

pub proof fn exact_completion_enters_completed_v1(state: BridgeStateV1,
    key: BridgeKeyV1, completion: CompletionV1)
    requires state.phase == PhaseV1::Published, matching_active_key_v1(state, key),
        exact_completion_v1(state, completion),
    ensures {
        let completed = observe_completion_v1(state, key,
            CompletionDispositionV1::Completed(completion));
        completed.phase == PhaseV1::Completed && completed.completion == Some(completion)
    }, {}

pub proof fn restore_before_completion_is_rejected_v1(state: BridgeStateV1,
    key: BridgeKeyV1, disposition: RestoreDispositionV1)
    requires state.phase != PhaseV1::Completed,
    ensures restore_v1(state, key, disposition) == state, {}

pub proof fn retryable_restore_is_no_effect_v1(state: BridgeStateV1, key: BridgeKeyV1)
    ensures restore_v1(state, key, RestoreDispositionV1::RetryableNoEffect) == state, {}

pub proof fn ambiguous_restore_quarantines_v1(state: BridgeStateV1, key: BridgeKeyV1)
    requires state.phase == PhaseV1::Completed, matching_active_key_v1(state, key),
        authenticated_completion_v1(state),
    ensures restore_v1(state, key, RestoreDispositionV1::AmbiguousFailure).phase
        == PhaseV1::Quarantined, {}

pub proof fn successful_restore_retains_completion_v1(state: BridgeStateV1,
    key: BridgeKeyV1)
    requires state.phase == PhaseV1::Completed, matching_active_key_v1(state, key),
        authenticated_completion_v1(state),
    ensures {
        let restored = restore_v1(state, key, RestoreDispositionV1::Restored);
        restored.phase == PhaseV1::Restored && restored.completion == state.completion
            && restored.storage == state.storage
    }, {}

pub proof fn retirement_requires_completed_restore_v1(state: BridgeStateV1,
    key: BridgeKeyV1)
    requires state.phase != PhaseV1::Restored,
    ensures retire_frontier_v1(state, key) == state, {}

pub proof fn stale_frontier_retirement_is_atomic_v1(state: BridgeStateV1,
    key: BridgeKeyV1)
    requires !matching_active_key_v1(state, key),
    ensures retire_frontier_v1(state, key) == state, {}

pub proof fn exhausted_frontier_retirement_is_atomic_v1(state: BridgeStateV1,
    key: BridgeKeyV1)
    requires key.operation_generation >= max_generation_v1(),
    ensures retire_frontier_v1(state, key) == state, {}

pub proof fn exact_frontier_retirement_returns_quiescent_device_v1(state: BridgeStateV1,
    key: BridgeKeyV1)
    requires state.phase == PhaseV1::Restored, matching_active_key_v1(state, key),
        authenticated_completion_v1(state), key.operation_generation < max_generation_v1(),
    ensures {
        let retired = retire_frontier_v1(state, key);
        &&& retired.phase == PhaseV1::Device && !retired.fast_path_selected
        &&& retired.active_generation == 0
        &&& retired.retired_frontier == key.operation_generation
        &&& retired.storage == state.storage && retired.range.is_none()
        &&& retired.completion.is_none()
    }, {}

pub proof fn write_retirement_initializes_storage_v1(state: BridgeStateV1,
    key: BridgeKeyV1)
    requires state.phase == PhaseV1::Restored, matching_active_key_v1(state, key),
        authenticated_completion_v1(state), key.operation_generation < max_generation_v1(),
        authorization_writes_v1(state.authorization),
    ensures retire_frontier_v1(state, key).initialized, {}

pub proof fn read_retirement_preserves_initialization_v1(state: BridgeStateV1,
    key: BridgeKeyV1)
    requires state.phase == PhaseV1::Restored, matching_active_key_v1(state, key),
        authenticated_completion_v1(state), key.operation_generation < max_generation_v1(),
        state.initialized,
    ensures retire_frontier_v1(state, key).initialized, {}

pub proof fn generic_materialization_is_zero_and_no_effect_v1(state: BridgeStateV1)
    requires state.generic_materializations == 0,
    ensures attempt_generic_materialization_v1(state) == state,
        attempt_generic_materialization_v1(state).generic_materializations == 0, {}

pub proof fn quarantine_is_absorbing_for_publish_v1(state: BridgeStateV1,
    key: BridgeKeyV1, disposition: PublishDispositionV1)
    requires state.phase == PhaseV1::Quarantined,
    ensures publish_v1(state, key, disposition) == state, {}

pub proof fn quarantine_is_absorbing_for_completion_v1(state: BridgeStateV1,
    key: BridgeKeyV1, disposition: CompletionDispositionV1)
    requires state.phase == PhaseV1::Quarantined,
    ensures observe_completion_v1(state, key, disposition) == state, {}

pub proof fn quarantine_is_absorbing_for_restore_and_retirement_v1(state: BridgeStateV1,
    key: BridgeKeyV1, disposition: RestoreDispositionV1)
    requires state.phase == PhaseV1::Quarantined,
    ensures restore_v1(state, key, disposition) == state,
        retire_frontier_v1(state, key) == state, {}

pub proof fn exact_sample_chain_reaches_quiescent_device_v1()
    ensures {
        let prepared = prepare_v1(sample_state_v1(), sample_request_v1());
        let published = publish_v1(prepared, sample_key_v1(), PublishDispositionV1::Published);
        let completed = observe_completion_v1(published, sample_key_v1(),
            CompletionDispositionV1::Completed(sample_completion_v1()));
        let restored = restore_v1(completed, sample_key_v1(), RestoreDispositionV1::Restored);
        let retired = retire_frontier_v1(restored, sample_key_v1());
        retired.phase == PhaseV1::Device && retired.storage == sample_storage_v1()
            && retired.retired_frontier == 1 && retired.generic_materializations == 0
    }, {}

fn main() {}

}
