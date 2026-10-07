// Conditional source refinement of model-only descriptions, not peer authority.
// The exact record/receipt declarations and three executable bodies are shared.
// The digest equality bridge is explicit: each named digest wrapper denotes its
// full 32-byte payload. The checker binds the real derived-equality declarations
// in identity.rs and the model module; no digest preimage/authentication premise
// or precomputed binding_matches Boolean enters this proof. Constructors and
// wire encode/decode are deliberately outside this executable proof closure.
use vstd::prelude::*;
use vstd::prelude::verus as distributed_contract_declarations_v1;

verus! {
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IdentityDigestV1([u8; 32]);
}

macro_rules! digest_identity_bridge_v1 {
    ($($name:ident),+ $(,)?) => { $(verus! {
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub struct $name(IdentityDigestV1);
    })+ };
}

digest_identity_bridge_v1!(
    DistributedRuntimeInstanceIdV1, DistributedParticipantIdV1,
    DistributedMembershipIdV1, DistributedRunIdV1, DistributedOperationIdV1,
    DistributedExecutionPlanIdV1, DistributedPlacementPlanIdV1,
    DistributedTargetDescriptionIdV1, RuntimeArtifactIdV1, RuntimeModelIdV1,
);

include!("../src/distributed_publication_contract/declarations.rs");
include!("../src/distributed_publication_contract/classifier_body.rs");

// This bridge specifies Rust's source-checked structural derives, not a claim
// about external identity validity, cryptography, ABI, compiler or machine code.
macro_rules! structural_equality_bridge_v1 {
    ($($name:ident),+ $(,)?) => { $(verus! {
        impl vstd::std_specs::cmp::PartialEqSpecImpl for $name {
            open spec fn obeys_eq_spec() -> bool { true }
            open spec fn eq_spec(&self, other: &Self) -> bool { *self == *other }
        }
    })+ };
}

structural_equality_bridge_v1!(
    IdentityDigestV1, DistributedRuntimeInstanceIdV1, DistributedParticipantIdV1,
    DistributedMembershipIdV1, DistributedRunIdV1, DistributedOperationIdV1,
    DistributedExecutionPlanIdV1, DistributedPlacementPlanIdV1,
    DistributedTargetDescriptionIdV1, RuntimeArtifactIdV1, RuntimeModelIdV1,
    UntrustedDistributedOperationCoordinatesV1, ModelDistributedOperationBindingV1,
    DistributedPublicationContractErrorV1, ReportedDistributedPublicationV1,
    ReceiptDescription, UntrustedDistributedPublicationReceiptV1,
    ModelDistributedReceiptDispositionV1, DistributedObservationEventV1,
    ModelDistributedPublicationRecordV1,
);

verus! {
type E = DistributedPublicationContractErrorV1;
type D = ModelDistributedReceiptDispositionV1;
type P = ReportedDistributedPublicationV1;

spec fn all_coordinates_same(
    left: UntrustedDistributedOperationCoordinatesV1,
    right: UntrustedDistributedOperationCoordinatesV1,
) -> bool {
    left.runtime_instance.0.0 == right.runtime_instance.0.0
    && left.participant.0.0 == right.participant.0.0
    && left.participant_incarnation == right.participant_incarnation
    && left.coordinator.0.0 == right.coordinator.0.0
    && left.coordinator_epoch == right.coordinator_epoch
    && left.membership.0.0 == right.membership.0.0
    && left.membership_epoch == right.membership_epoch
    && left.run.0.0 == right.run.0.0
    && left.operation.0.0 == right.operation.0.0
    && left.attempt == right.attempt
    && left.artifact.0.0 == right.artifact.0.0
    && left.execution_plan.0.0 == right.execution_plan.0.0
    && left.placement_plan.0.0 == right.placement_plan.0.0
    && left.target.0.0 == right.target.0.0
    && left.runtime_model.0.0 == right.runtime_model.0.0
}

proof fn binding_equality_is_all_fifteen_coordinates(
    left: ModelDistributedOperationBindingV1,
    right: ModelDistributedOperationBindingV1,
)
    ensures (left == right) == all_coordinates_same(left.coordinates, right.coordinates),
{}

spec fn decision(
    record: ModelDistributedPublicationRecordV1,
    receipt: UntrustedDistributedPublicationReceiptV1,
) -> Result<D, E> {
    if record.binding != receipt.binding { Err(E::BindingMismatch) }
    else {
        match record.last {
            Some(previous) => {
                if receipt.description.sequence == previous.sequence {
                    if receipt.description == previous { Ok(D::ExactDuplicate) }
                    else { Err(E::ConflictingDuplicate) }
                } else if receipt.description.sequence < previous.sequence { Err(E::StaleReceipt) }
                else if record.interrupted { Err(E::Interrupted) }
                else if previous.sequence == u64::MAX { Err(E::SequenceExhausted) }
                else if receipt.description.sequence != previous.sequence + 1 { Err(E::SequenceGap) }
                else if previous.outcome != P::Published
                    || !(receipt.description.outcome == P::Completed
                         || receipt.description.outcome == P::FailedMayStillExecute)
                { Err(E::InvalidTransition) }
                else { Ok(D::Recorded) }
            },
            None => {
                if record.interrupted { Err(E::Interrupted) }
                else if receipt.description.sequence != 1 { Err(E::SequenceGap) }
                else if receipt.description.outcome == P::Completed { Err(E::InvalidTransition) }
                else { Ok(D::Recorded) }
            },
        }
    }
}

fn classify_receipt(
    record: &ModelDistributedPublicationRecordV1,
    receipt: &UntrustedDistributedPublicationReceiptV1,
) -> (result: Result<D, E>)
    ensures
        result == decision(*record, *receipt),
        result != Err(E::SequenceExhausted),
        record.binding != receipt.binding ==> result == Err(E::BindingMismatch),
{
    distributed_receipt_classifier_body_v1!(record, receipt)
}

impl ModelDistributedPublicationRecordV1 {
    fn record_untrusted_receipt(
        &mut self,
        receipt: UntrustedDistributedPublicationReceiptV1,
    ) -> (result: Result<D, E>)
        ensures
            result == decision(*old(self), receipt),
            final(self).binding == old(self).binding,
            final(self).interrupted == old(self).interrupted,
            final(self).last == if result == Ok(D::Recorded) {
                Some(receipt.description)
            } else { old(self).last },
            (result.is_err() || result == Ok(D::ExactDuplicate)) ==> *final(self) == *old(self),
            result != Err(E::SequenceExhausted),
    {
        distributed_receipt_record_body_v1!(self, receipt)
    }

    fn observe(&mut self, event: DistributedObservationEventV1)
        ensures
            final(self).binding == old(self).binding,
            final(self).last == old(self).last,
            final(self).interrupted == (old(self).interrupted
                || event == DistributedObservationEventV1::ConnectionLost),
    {
        distributed_observation_body_v1!(self, event)
    }
}
}
