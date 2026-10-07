// Actual constructor and fixed trailer decisions, shared with their proof.
macro_rules! distributed_binding_constructor_body_v1 {
    ($coordinates:ident) => {{
        let zero = IdentityDigestV1::from_untrusted_bytes([0; 32]);
        if $coordinates.runtime_instance.digest() == zero
            || $coordinates.participant.digest() == zero
            || $coordinates.coordinator.digest() == zero
            || $coordinates.membership.digest() == zero
            || $coordinates.run.digest() == zero
            || $coordinates.operation.digest() == zero
            || $coordinates.artifact.digest() == zero
            || $coordinates.execution_plan.digest() == zero
            || $coordinates.placement_plan.digest() == zero
            || $coordinates.target.digest() == zero
            || $coordinates.runtime_model.digest() == zero
        {
            return Err(DistributedPublicationContractErrorV1::ZeroIdentity);
        }
        if $coordinates.participant_incarnation == 0
            || $coordinates.coordinator_epoch == 0
            || $coordinates.membership_epoch == 0
            || $coordinates.attempt == 0
        {
            return Err(DistributedPublicationContractErrorV1::ZeroEpochOrAttempt);
        }
        Ok(Self {
            coordinates: $coordinates,
        })
    }};
}

macro_rules! distributed_receipt_constructor_body_v1 {
    ($binding:ident, $sequence:ident, $outcome:ident) => {{
        if $sequence == 0 {
            return Err(DistributedPublicationContractErrorV1::ZeroSequence);
        }
        Ok(Self {
            binding: $binding,
            description: ReceiptDescription {
                sequence: $sequence,
                outcome: $outcome,
            },
        })
    }};
}

macro_rules! distributed_outcome_trailer_body_v1 {
    ($bytes:ident) => {{
        let (tag, a, b, c) = ($bytes[0], $bytes[1], $bytes[2], $bytes[3]);
        if a != 0 || b != 0 || c != 0 {
            return Err(DistributedPublicationContractErrorV1::NonzeroReserved);
        }
        use ReportedDistributedPublicationV1 as P;
        let outcome = match tag {
            1 => P::DefinitelyNotPublished,
            2 => P::Published,
            3 => P::Completed,
            4 => P::FailedBeforePublication,
            5 => P::FailedMayStillExecute,
            6 => P::ParticipantLostWithUnknownPublication,
            _ => return Err(DistributedPublicationContractErrorV1::InvalidOutcome),
        };
        Ok(outcome)
    }};
}
