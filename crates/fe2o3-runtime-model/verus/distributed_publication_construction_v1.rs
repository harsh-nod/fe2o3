// Actual model construction and fixed trailer decisions only. The unchanged
// classifier root supplies the exact declarations and full-payload equality
// bridge; its classifier obligations remain distinct from these new functions.
// These accessor bodies match source-checked real identity projections. Their
// representation bridge, Rust/compiler, and pinned vstd are explicit trust
// boundaries, not authentication or validity assumptions. No wire cursor,
// full encoder/decoder, transport, native execution or lifecycle fact is proved.
include!("distributed_publication_contract_v1.rs");
include!("../src/distributed_publication_contract/construction_body.rs");

macro_rules! digest_accessor_bridge_v1 {
    ($($name:ident),+ $(,)?) => { $(verus! {
        impl $name {
            fn digest(self) -> (result: IdentityDigestV1)
                ensures result == self.0,
            { self.0 }
        }
    })+ };
}

digest_accessor_bridge_v1!(
    DistributedRuntimeInstanceIdV1, DistributedParticipantIdV1,
    DistributedMembershipIdV1, DistributedRunIdV1, DistributedOperationIdV1,
    DistributedExecutionPlanIdV1, DistributedPlacementPlanIdV1,
    DistributedTargetDescriptionIdV1, RuntimeArtifactIdV1, RuntimeModelIdV1,
);

verus! {
impl IdentityDigestV1 {
    fn from_untrusted_bytes(bytes: [u8; 32]) -> (result: Self)
        ensures result.0 == bytes,
    { Self(bytes) }
}

spec fn zero_identity(c: UntrustedDistributedOperationCoordinatesV1) -> bool {
    c.runtime_instance.0.0 == vstd::array::spec_array_fill_for_copy_type::<u8, 32>(0)
    || c.participant.0.0 == vstd::array::spec_array_fill_for_copy_type::<u8, 32>(0)
    || c.coordinator.0.0 == vstd::array::spec_array_fill_for_copy_type::<u8, 32>(0)
    || c.membership.0.0 == vstd::array::spec_array_fill_for_copy_type::<u8, 32>(0)
    || c.run.0.0 == vstd::array::spec_array_fill_for_copy_type::<u8, 32>(0)
    || c.operation.0.0 == vstd::array::spec_array_fill_for_copy_type::<u8, 32>(0)
    || c.artifact.0.0 == vstd::array::spec_array_fill_for_copy_type::<u8, 32>(0)
    || c.execution_plan.0.0 == vstd::array::spec_array_fill_for_copy_type::<u8, 32>(0)
    || c.placement_plan.0.0 == vstd::array::spec_array_fill_for_copy_type::<u8, 32>(0)
    || c.target.0.0 == vstd::array::spec_array_fill_for_copy_type::<u8, 32>(0)
    || c.runtime_model.0.0 == vstd::array::spec_array_fill_for_copy_type::<u8, 32>(0)
}

spec fn zero_integer(c: UntrustedDistributedOperationCoordinatesV1) -> bool {
    c.participant_incarnation == 0 || c.coordinator_epoch == 0
    || c.membership_epoch == 0 || c.attempt == 0
}

impl ModelDistributedOperationBindingV1 {
    fn from_untrusted_coordinates(coordinates: UntrustedDistributedOperationCoordinatesV1)
        -> (result: Result<Self, E>)
        ensures result == if zero_identity(coordinates) { Err(E::ZeroIdentity) }
            else if zero_integer(coordinates) { Err(E::ZeroEpochOrAttempt) }
            else { Ok(Self { coordinates }) },
    {
        distributed_binding_constructor_body_v1!(coordinates)
    }
}

impl UntrustedDistributedPublicationReceiptV1 {
    fn new(binding: ModelDistributedOperationBindingV1, sequence: u64, outcome: P)
        -> (result: Result<Self, E>)
        ensures result == if sequence == 0 { Err(E::ZeroSequence) }
            else { Ok(Self { binding, description: ReceiptDescription { sequence, outcome } }) },
    {
        distributed_receipt_constructor_body_v1!(binding, sequence, outcome)
    }
}

spec fn trailer_decision(bytes: [u8; 4]) -> Result<P, E> {
    if bytes[1] != 0 || bytes[2] != 0 || bytes[3] != 0 { Err(E::NonzeroReserved) }
    else {
        match bytes[0] {
            1 => Ok(P::DefinitelyNotPublished),
            2 => Ok(P::Published),
            3 => Ok(P::Completed),
            4 => Ok(P::FailedBeforePublication),
            5 => Ok(P::FailedMayStillExecute),
            6 => Ok(P::ParticipantLostWithUnknownPublication),
            _ => Err(E::InvalidOutcome),
        }
    }
}

fn decode_outcome_trailer(bytes: [u8; 4]) -> (result: Result<P, E>)
    ensures result == trailer_decision(bytes),
{
    distributed_outcome_trailer_body_v1!(bytes)
}
}
