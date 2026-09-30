// Rust and Verus compile the same complete record and receipt declarations.
distributed_contract_declarations_v1! {
/// Untrusted coordinates of one operation attempt, independent of connections.
/// Digests describe external objects; their preimages, authentication and
/// admissibility are not established here. Epochs are names, not clock readings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UntrustedDistributedOperationCoordinatesV1 {
    pub runtime_instance: DistributedRuntimeInstanceIdV1,
    pub participant: DistributedParticipantIdV1,
    pub participant_incarnation: u64,
    pub coordinator: DistributedParticipantIdV1,
    pub coordinator_epoch: u64,
    pub membership: DistributedMembershipIdV1,
    pub membership_epoch: u64,
    pub run: DistributedRunIdV1,
    pub operation: DistributedOperationIdV1,
    pub attempt: u64,
    pub artifact: RuntimeArtifactIdV1,
    pub execution_plan: DistributedExecutionPlanIdV1,
    pub placement_plan: DistributedPlacementPlanIdV1,
    pub target: DistributedTargetDescriptionIdV1,
    pub runtime_model: RuntimeModelIdV1,
}

/// Structurally checked coordinates. Still a caller-constructible model value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ModelDistributedOperationBindingV1 {
    coordinates: UntrustedDistributedOperationCoordinatesV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DistributedPublicationContractErrorV1 {
    ZeroIdentity,
    ZeroEpochOrAttempt,
    WrongLength,
    WrongDomain,
    WrongSchema,
    NonzeroReserved,
    InvalidOutcome,
    ZeroSequence,
    BindingMismatch,
    Interrupted,
    ConflictingDuplicate,
    StaleReceipt,
    SequenceExhausted,
    SequenceGap,
    InvalidTransition,
}

/// A peer's descriptive claim, not evidence that the claimed event occurred.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ReportedDistributedPublicationV1 {
    /// Peer-reported final nonpublication for this attempt, not a snapshot from
    /// which the same attempt may subsequently publish. Not retry authority.
    DefinitelyNotPublished = 1,
    Published = 2,
    Completed = 3,
    FailedBeforePublication = 4,
    FailedMayStillExecute = 5,
    ParticipantLostWithUnknownPublication = 6,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ReceiptDescription {
    sequence: u64,
    outcome: ReportedDistributedPublicationV1,
}

/// Exact structurally checked wire description, still untrusted after decoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UntrustedDistributedPublicationReceiptV1 {
    binding: ModelDistributedOperationBindingV1,
    description: ReceiptDescription,
}

/// Descriptive record update, never a permission to execute or release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelDistributedReceiptDispositionV1 {
    Recorded,
    ExactDuplicate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DistributedObservationEventV1 {
    ConnectionLost,
    ObservationTimedOut,
    ObserverDropped,
}

/// One fixed-size operation record. No resource ownership, registry or history.
/// The first receipt must be sequence 1; only Published may advance, to
/// Completed or FailedMayStillExecute. Other outcomes close this exact attempt.
/// Reconnect and receipt-gap reconciliation are deliberately unsupported.
#[derive(Debug, Eq, PartialEq)]
pub struct ModelDistributedPublicationRecordV1 {
    binding: ModelDistributedOperationBindingV1,
    last: Option<ReceiptDescription>,
    interrupted: bool,
}
}
