//! Pure finite lifecycle specification. Input events are caller-authored
//! descriptions, never evidence of ioctls, acknowledgments, queue state or cleanup.
//! No retained native owner accepts this model or its result as a capability.

/// Missing evidence is enumerated, rather than accepted as caller booleans.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx950DebugNativeRequirementV1 {
    CurrentOwnedProcessDeviceAndDependencyClosure,
    LifetimeExclusiveNoSamplingAndNoForeignRuntime,
    BoundDebugtrapFixtureAndDispatchAbi,
    ActualTrapRegistrationAndTtmpSetup,
    SameClientRuntimeLoadedAcknowledgment,
    OwnedEventQueueDoorbellAndAllEightCwsrHeaders,
    SameQueuePacketPublicationAndStoppedWaveIdentity,
    SameStopAllocatedRegisterAndAllocationRelativeMemory,
    OrderedQueueEventRuntimeTrapDoorbellTeardownAcknowledgments,
}
static REQUIRED: [Gfx950DebugNativeRequirementV1; 9] = [
    Gfx950DebugNativeRequirementV1::CurrentOwnedProcessDeviceAndDependencyClosure,
    Gfx950DebugNativeRequirementV1::LifetimeExclusiveNoSamplingAndNoForeignRuntime,
    Gfx950DebugNativeRequirementV1::BoundDebugtrapFixtureAndDispatchAbi,
    Gfx950DebugNativeRequirementV1::ActualTrapRegistrationAndTtmpSetup,
    Gfx950DebugNativeRequirementV1::SameClientRuntimeLoadedAcknowledgment,
    Gfx950DebugNativeRequirementV1::OwnedEventQueueDoorbellAndAllEightCwsrHeaders,
    Gfx950DebugNativeRequirementV1::SameQueuePacketPublicationAndStoppedWaveIdentity,
    Gfx950DebugNativeRequirementV1::SameStopAllocatedRegisterAndAllocationRelativeMemory,
    Gfx950DebugNativeRequirementV1::OrderedQueueEventRuntimeTrapDoorbellTeardownAcknowledgments,
];

/// A closed descriptive refusal. There is no success variant, token constructor,
/// native address, activation method, or operation accepting this as authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx950DebugNativeUnavailableV1 {
    private: (),
}
impl Gfx950DebugNativeUnavailableV1 {
    pub(super) const PREPARATION_ONLY: Self = Self { private: () };
    pub const fn missing_requirements(self) -> &'static [Gfx950DebugNativeRequirementV1] {
        &REQUIRED
    }
    pub const fn reason(self) -> &'static str {
        "gfx950 debug execution preparation only; native queue lifecycle unavailable"
    }
}

/// Inert policy symbols. They are NOT runtime observations or teardown receipts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx950DebugQueueLifecycleEventV1 {
    TrapInstalled,
    RuntimeEnableBegan,
    RuntimeEnableAcknowledged,
    EventCreated,
    QueueCreated,
    PacketPublished,
    WaveStopped,
    WaveResumed,
    PacketCompleted,
    QueueDestroyed,
    EventDestroyed,
    RuntimeDisableBegan,
    RuntimeDisableAcknowledged,
    TrapClearAcknowledged,
    DoorbellReleased,
    BackingReleased,
    NativeOutcomeUncertain,
}
/// Describes a valid prefix of the policy, not a live native state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx950DebugQueueLifecyclePhaseV1 {
    Prepared,
    TrapInstalled,
    EnablingRuntime,
    RuntimeAcknowledged,
    EventReady,
    EmptyQueue,
    PacketLive,
    Stopped,
    Resumed,
    CompletedPacket,
    DestroyedQueue,
    DestroyedEvent,
    DisablingRuntime,
    RuntimeDisabled,
    TrapCleared,
    DoorbellReleased,
    ModelTeardownComplete,
    RetainUntilProcessExit,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx950DebugQueueLifecycleErrorV1 {
    EventBound,
    Transition {
        ordinal: usize,
        phase: Gfx950DebugQueueLifecyclePhaseV1,
        event: Gfx950DebugQueueLifecycleEventV1,
    },
}

/// Validates ≤32 inert rows without allocation. Successful validation is ONLY a
/// policy-prefix fact, never native transition/cleanup evidence. Neither this
/// result nor any event can be supplied to the retained preparation owner.
pub fn validate_gfx950_debug_queue_lifecycle_v1(
    events: &[Gfx950DebugQueueLifecycleEventV1],
) -> Result<Gfx950DebugQueueLifecyclePhaseV1, Gfx950DebugQueueLifecycleErrorV1> {
    use Gfx950DebugQueueLifecycleEventV1 as E;
    use Gfx950DebugQueueLifecyclePhaseV1 as P;
    if events.len() > 32 {
        return Err(Gfx950DebugQueueLifecycleErrorV1::EventBound);
    }
    let mut phase = P::Prepared;
    for (ordinal, event) in events.iter().copied().enumerate() {
        phase = match (phase, event) {
            (P::Prepared, E::TrapInstalled) => P::TrapInstalled,
            (P::TrapInstalled, E::RuntimeEnableBegan) => P::EnablingRuntime,
            (P::EnablingRuntime, E::RuntimeEnableAcknowledged) => P::RuntimeAcknowledged,
            (P::RuntimeAcknowledged, E::EventCreated) => P::EventReady,
            (P::EventReady, E::QueueCreated) => P::EmptyQueue,
            (P::EmptyQueue, E::PacketPublished) => P::PacketLive,
            (P::PacketLive, E::WaveStopped) => P::Stopped,
            (P::Stopped, E::WaveResumed) => P::Resumed,
            (P::Resumed, E::PacketCompleted) => P::CompletedPacket,
            (P::EmptyQueue | P::CompletedPacket, E::QueueDestroyed) => P::DestroyedQueue,
            (P::DestroyedQueue, E::EventDestroyed) => P::DestroyedEvent,
            (P::DestroyedEvent, E::RuntimeDisableBegan) => P::DisablingRuntime,
            (P::DisablingRuntime, E::RuntimeDisableAcknowledged) => P::RuntimeDisabled,
            (P::RuntimeDisabled, E::TrapClearAcknowledged) => P::TrapCleared,
            (P::TrapCleared, E::DoorbellReleased) => P::DoorbellReleased,
            (P::DoorbellReleased, E::BackingReleased) => P::ModelTeardownComplete,
            (p, E::NativeOutcomeUncertain)
                if !matches!(p, P::ModelTeardownComplete | P::RetainUntilProcessExit) =>
            {
                P::RetainUntilProcessExit
            }
            _ => {
                return Err(Gfx950DebugQueueLifecycleErrorV1::Transition {
                    ordinal,
                    phase,
                    event,
                });
            }
        }
    }
    Ok(phase)
}
