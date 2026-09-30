//! Fixed-size, authority-free descriptions for an initial distributed contract.
//!
//! Decoding or accepting a receipt does not authenticate its producer or prove
//! its publication claim. This module owns no native resources and has no
//! conversion to runtime submission, completion, cancellation or retry authority.
//! It is not connected to a runtime, worker, transport or participant session.

use crate::{AuthorityDomainV1, IdentityDigestV1, RuntimeArtifactIdV1, RuntimeModelIdV1};

macro_rules! distributed_contract_declarations_v1 {
    ($($tokens:tt)*) => { $($tokens)* };
}
include!("distributed_publication_contract/declarations.rs");

#[macro_use]
mod classifier_body;
#[cfg(test)]
mod tests;

pub const DISTRIBUTED_PUBLICATION_CONTRACT_SCHEMA_V1: u16 = 1;
pub const DISTRIBUTED_OPERATION_DESCRIPTION_DOMAIN_V1: &[u8] =
    b"FE2O3/DISTRIBUTED-OPERATION-DESCRIPTION/V1\0";
pub const DISTRIBUTED_PUBLICATION_RECEIPT_DOMAIN_V1: &[u8] =
    b"FE2O3/DISTRIBUTED-PUBLICATION-RECEIPT/V1\0";
const COORDINATE_BYTES: usize = 11 * 32 + 4 * 8;
pub const DISTRIBUTED_OPERATION_DESCRIPTION_BYTES_V1: usize =
    DISTRIBUTED_OPERATION_DESCRIPTION_DOMAIN_V1.len() + 4 + COORDINATE_BYTES;
pub const DISTRIBUTED_PUBLICATION_RECEIPT_BYTES_V1: usize =
    DISTRIBUTED_PUBLICATION_RECEIPT_DOMAIN_V1.len()
        + 4
        + DISTRIBUTED_OPERATION_DESCRIPTION_BYTES_V1
        + 8
        + 4;

macro_rules! descriptive_identity {
    ($($name:ident),+ $(,)?) => {$(
        /// Caller-supplied identity description, never authenticated authority.
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(IdentityDigestV1);
        impl $name {
            pub const fn from_untrusted_digest(digest: IdentityDigestV1) -> Self {
                Self(digest)
            }
            pub const fn digest(self) -> IdentityDigestV1 { self.0 }
        }
    )+};
}

descriptive_identity!(
    DistributedRuntimeInstanceIdV1,
    DistributedParticipantIdV1,
    DistributedMembershipIdV1,
    DistributedRunIdV1,
    DistributedOperationIdV1,
    DistributedExecutionPlanIdV1,
    DistributedPlacementPlanIdV1,
    DistributedTargetDescriptionIdV1,
);

impl ModelDistributedOperationBindingV1 {
    pub fn from_untrusted_coordinates(
        coordinates: UntrustedDistributedOperationCoordinatesV1,
    ) -> Result<Self, DistributedPublicationContractErrorV1> {
        let c = coordinates;
        for digest in [
            c.runtime_instance.digest(),
            c.participant.digest(),
            c.coordinator.digest(),
            c.membership.digest(),
            c.run.digest(),
            c.operation.digest(),
            c.artifact.digest(),
            c.execution_plan.digest(),
            c.placement_plan.digest(),
            c.target.digest(),
            c.runtime_model.digest(),
        ] {
            if digest.as_bytes().iter().all(|byte| *byte == 0) {
                return Err(DistributedPublicationContractErrorV1::ZeroIdentity);
            }
        }
        if [
            c.participant_incarnation,
            c.coordinator_epoch,
            c.membership_epoch,
            c.attempt,
        ]
        .contains(&0)
        {
            return Err(DistributedPublicationContractErrorV1::ZeroEpochOrAttempt);
        }
        Ok(Self { coordinates })
    }

    pub const fn authority_domain(self) -> AuthorityDomainV1 {
        AuthorityDomainV1::ModelOnly
    }
    pub const fn coordinates(self) -> UntrustedDistributedOperationCoordinatesV1 {
        self.coordinates
    }

    /// Canonical descriptive preimage, not a signature, digest or authority token.
    pub fn canonical_description(self) -> [u8; DISTRIBUTED_OPERATION_DESCRIPTION_BYTES_V1] {
        let mut bytes = [0; DISTRIBUTED_OPERATION_DESCRIPTION_BYTES_V1];
        let mut writer = Writer::new(&mut bytes);
        writer.header(DISTRIBUTED_OPERATION_DESCRIPTION_DOMAIN_V1);
        let c = self.coordinates;
        writer.digest(c.runtime_instance.digest());
        writer.digest(c.participant.digest());
        writer.u64(c.participant_incarnation);
        writer.digest(c.coordinator.digest());
        writer.u64(c.coordinator_epoch);
        writer.digest(c.membership.digest());
        writer.u64(c.membership_epoch);
        writer.digest(c.run.digest());
        writer.digest(c.operation.digest());
        writer.u64(c.attempt);
        writer.digest(c.artifact.digest());
        writer.digest(c.execution_plan.digest());
        writer.digest(c.placement_plan.digest());
        writer.digest(c.target.digest());
        writer.digest(c.runtime_model.digest());
        debug_assert_eq!(writer.offset, DISTRIBUTED_OPERATION_DESCRIPTION_BYTES_V1);
        bytes
    }

    pub fn decode_untrusted_description(
        bytes: &[u8],
    ) -> Result<Self, DistributedPublicationContractErrorV1> {
        if bytes.len() != DISTRIBUTED_OPERATION_DESCRIPTION_BYTES_V1 {
            return Err(DistributedPublicationContractErrorV1::WrongLength);
        }
        let mut r = Reader::new(bytes);
        r.header(DISTRIBUTED_OPERATION_DESCRIPTION_DOMAIN_V1)?;
        let coordinates = UntrustedDistributedOperationCoordinatesV1 {
            runtime_instance: DistributedRuntimeInstanceIdV1::from_untrusted_digest(r.digest()?),
            participant: DistributedParticipantIdV1::from_untrusted_digest(r.digest()?),
            participant_incarnation: r.u64()?,
            coordinator: DistributedParticipantIdV1::from_untrusted_digest(r.digest()?),
            coordinator_epoch: r.u64()?,
            membership: DistributedMembershipIdV1::from_untrusted_digest(r.digest()?),
            membership_epoch: r.u64()?,
            run: DistributedRunIdV1::from_untrusted_digest(r.digest()?),
            operation: DistributedOperationIdV1::from_untrusted_digest(r.digest()?),
            attempt: r.u64()?,
            artifact: RuntimeArtifactIdV1::from_untrusted_digest(r.digest()?),
            execution_plan: DistributedExecutionPlanIdV1::from_untrusted_digest(r.digest()?),
            placement_plan: DistributedPlacementPlanIdV1::from_untrusted_digest(r.digest()?),
            target: DistributedTargetDescriptionIdV1::from_untrusted_digest(r.digest()?),
            runtime_model: RuntimeModelIdV1::from_untrusted_digest(r.digest()?),
        };
        r.finish()?;
        Self::from_untrusted_coordinates(coordinates)
    }
}

impl UntrustedDistributedPublicationReceiptV1 {
    pub fn new(
        binding: ModelDistributedOperationBindingV1,
        sequence: u64,
        outcome: ReportedDistributedPublicationV1,
    ) -> Result<Self, DistributedPublicationContractErrorV1> {
        if sequence == 0 {
            return Err(DistributedPublicationContractErrorV1::ZeroSequence);
        }
        Ok(Self {
            binding,
            description: ReceiptDescription { sequence, outcome },
        })
    }
    pub const fn binding(self) -> ModelDistributedOperationBindingV1 {
        self.binding
    }
    pub const fn sequence(self) -> u64 {
        self.description.sequence
    }
    pub const fn reported_outcome(self) -> ReportedDistributedPublicationV1 {
        self.description.outcome
    }
    pub fn canonical_description(self) -> [u8; DISTRIBUTED_PUBLICATION_RECEIPT_BYTES_V1] {
        let mut bytes = [0; DISTRIBUTED_PUBLICATION_RECEIPT_BYTES_V1];
        let mut writer = Writer::new(&mut bytes);
        writer.header(DISTRIBUTED_PUBLICATION_RECEIPT_DOMAIN_V1);
        writer.put(&self.binding.canonical_description());
        writer.u64(self.description.sequence);
        writer.put(&[self.description.outcome as u8, 0, 0, 0]);
        debug_assert_eq!(writer.offset, DISTRIBUTED_PUBLICATION_RECEIPT_BYTES_V1);
        bytes
    }
    pub fn decode_untrusted_description(
        bytes: &[u8],
    ) -> Result<Self, DistributedPublicationContractErrorV1> {
        if bytes.len() != DISTRIBUTED_PUBLICATION_RECEIPT_BYTES_V1 {
            return Err(DistributedPublicationContractErrorV1::WrongLength);
        }
        let mut r = Reader::new(bytes);
        r.header(DISTRIBUTED_PUBLICATION_RECEIPT_DOMAIN_V1)?;
        let binding = ModelDistributedOperationBindingV1::decode_untrusted_description(
            r.take(DISTRIBUTED_OPERATION_DESCRIPTION_BYTES_V1)?,
        )?;
        let sequence = r.u64()?;
        let [tag, a, b, c] = r.fixed()?;
        if [a, b, c] != [0; 3] {
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
        r.finish()?;
        Self::new(binding, sequence, outcome)
    }
}

impl ModelDistributedPublicationRecordV1 {
    pub const fn new(binding: ModelDistributedOperationBindingV1) -> Self {
        Self {
            binding,
            last: None,
            interrupted: false,
        }
    }
    pub const fn authority_domain(&self) -> AuthorityDomainV1 {
        AuthorityDomainV1::ModelOnly
    }
    pub const fn binding(&self) -> ModelDistributedOperationBindingV1 {
        self.binding
    }
    pub fn last_receipt(&self) -> Option<UntrustedDistributedPublicationReceiptV1> {
        self.last
            .map(|description| UntrustedDistributedPublicationReceiptV1 {
                binding: self.binding,
                description,
            })
    }
    pub const fn connection_interrupted(&self) -> bool {
        self.interrupted
    }

    /// Loss preserves the last receipt and refuses new ones; exact duplicates
    /// remain idempotent. Timeout/Drop do nothing. No participant-loss or device
    /// fact is inferred from any of these observation events.
    pub fn observe(&mut self, event: DistributedObservationEventV1) {
        distributed_observation_body_v1!(self, event)
    }
    pub fn record_untrusted_receipt(
        &mut self,
        receipt: UntrustedDistributedPublicationReceiptV1,
    ) -> Result<ModelDistributedReceiptDispositionV1, DistributedPublicationContractErrorV1> {
        distributed_receipt_record_body_v1!(self, receipt)
    }
}

fn classify_receipt(
    record: &ModelDistributedPublicationRecordV1,
    receipt: &UntrustedDistributedPublicationReceiptV1,
) -> Result<ModelDistributedReceiptDispositionV1, DistributedPublicationContractErrorV1> {
    distributed_receipt_classifier_body_v1!(record, receipt)
}

struct Writer<'a> {
    bytes: &'a mut [u8],
    offset: usize,
}
impl<'a> Writer<'a> {
    fn new(bytes: &'a mut [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    fn put(&mut self, value: &[u8]) {
        self.bytes[self.offset..self.offset + value.len()].copy_from_slice(value);
        self.offset += value.len();
    }
    fn header(&mut self, domain: &[u8]) {
        self.put(domain);
        self.put(&DISTRIBUTED_PUBLICATION_CONTRACT_SCHEMA_V1.to_le_bytes());
        self.put(&[0; 2]);
    }
    fn u64(&mut self, value: u64) {
        self.put(&value.to_le_bytes());
    }
    fn digest(&mut self, value: IdentityDigestV1) {
        self.put(value.as_bytes());
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    fn take(&mut self, count: usize) -> Result<&'a [u8], DistributedPublicationContractErrorV1> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(DistributedPublicationContractErrorV1::WrongLength)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(DistributedPublicationContractErrorV1::WrongLength)?;
        self.offset = end;
        Ok(value)
    }
    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], DistributedPublicationContractErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| DistributedPublicationContractErrorV1::WrongLength)
    }
    fn header(&mut self, domain: &[u8]) -> Result<(), DistributedPublicationContractErrorV1> {
        if self.take(domain.len())? != domain {
            return Err(DistributedPublicationContractErrorV1::WrongDomain);
        }
        if u16::from_le_bytes(self.fixed()?) != DISTRIBUTED_PUBLICATION_CONTRACT_SCHEMA_V1 {
            return Err(DistributedPublicationContractErrorV1::WrongSchema);
        }
        if self.fixed::<2>()? != [0; 2] {
            return Err(DistributedPublicationContractErrorV1::NonzeroReserved);
        }
        Ok(())
    }
    fn u64(&mut self) -> Result<u64, DistributedPublicationContractErrorV1> {
        Ok(u64::from_le_bytes(self.fixed()?))
    }
    fn digest(&mut self) -> Result<IdentityDigestV1, DistributedPublicationContractErrorV1> {
        Ok(IdentityDigestV1::from_untrusted_bytes(self.fixed()?))
    }
    fn finish(&self) -> Result<(), DistributedPublicationContractErrorV1> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(DistributedPublicationContractErrorV1::WrongLength)
        }
    }
}
