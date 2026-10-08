//! Opt-in historical custody evidence, not execution or physical-overlap authority.

use super::*;
use sha2::{Digest, Sha256};

/// The two native publication classes admitted by this qualification profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KfdGeneratedCopyPublicationKindV1 {
    GeneratedCompute,
    DirectionalCopy,
}

/// Fixed diagnostics. None authorizes publication, completion, or resource release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KfdGeneratedCopyCoexistenceFailureV1 {
    ContextUnavailable,
    NotArmed,
    AlreadyArmed,
    UnsupportedProfile,
    Terminal,
    Busy,
    MissingPublication,
    PublicationOverflow,
    RepeatedPublicationClass,
    PublicationIdentity,
    UnsupportedCompute,
    UnsupportedCopy,
    GeneratedSource,
    GeneratedLease,
    NativeComputeReceipt,
    NativeCopyReceipt,
    AliasedOwners,
    UnaccountedCustody,
}

impl fmt::Display for KfdGeneratedCopyCoexistenceFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for KfdGeneratedCopyCoexistenceFailureV1 {}

/// Historical evidence that two exact native receipts coexisted before either
/// successful host completion transition consumed its published receipt.
///
/// The profile is one primary-lane Worker V3 generated write-only DATA owner and
/// one independent same-device directional SDMA owner. Digests contain no raw
/// handles or addresses. They are qualification data, never proof, admission,
/// completion, currentness, cleanup, or physical GPU overlap authority. Either
/// operation may already have finished on hardware when this is recorded.
///
/// Successful results, canaries, drain and refund must be checked separately.
/// This value records history and cannot establish later execution success.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KfdGeneratedCopyCoexistenceWitnessV1 {
    device_unique_id: u64,
    first_publication: KfdGeneratedCopyPublicationKindV1,
    compute_receipt: [u8; 32],
    compute_membership: [u8; 32],
    copy_receipt: [u8; 32],
    copy_membership: [u8; 32],
    copy_packets: usize,
}

impl KfdGeneratedCopyCoexistenceWitnessV1 {
    pub const fn device_unique_id(&self) -> u64 {
        self.device_unique_id
    }
    pub const fn first_publication(&self) -> KfdGeneratedCopyPublicationKindV1 {
        self.first_publication
    }
    pub const fn compute_receipt_sha256(&self) -> [u8; 32] {
        self.compute_receipt
    }
    pub const fn compute_membership_sha256(&self) -> [u8; 32] {
        self.compute_membership
    }
    pub const fn copy_receipt_sha256(&self) -> [u8; 32] {
        self.copy_receipt
    }
    pub const fn copy_membership_sha256(&self) -> [u8; 32] {
        self.copy_membership
    }
    pub const fn copy_packets(&self) -> usize {
        self.copy_packets
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PublicationV1 {
    kind: KfdGeneratedCopyPublicationKindV1,
    submission: u64,
    native: [u8; 32],
    membership: [u8; 32],
}

pub(in crate::kfd_backend) struct RecorderV1 {
    first: Option<PublicationV1>,
    count: u8,
    witness: Option<KfdGeneratedCopyCoexistenceWitnessV1>,
    failure: Option<KfdGeneratedCopyCoexistenceFailureV1>,
    taken: bool,
}

impl RecorderV1 {
    fn new() -> Self {
        Self {
            first: None,
            count: 0,
            witness: None,
            failure: None,
            taken: false,
        }
    }

    fn reject(&mut self, failure: KfdGeneratedCopyCoexistenceFailureV1) {
        if !self.taken {
            self.failure.get_or_insert(failure);
            self.witness = None;
        }
    }

    fn next(&mut self) -> bool {
        if self.taken || self.failure.is_some() {
            return false;
        }
        if self.count == 2 {
            self.reject(KfdGeneratedCopyCoexistenceFailureV1::PublicationOverflow);
            return false;
        }
        self.count += 1;
        true
    }

    fn record(
        &mut self,
        publication: PublicationV1,
        simultaneous: Option<(PublicationV1, PublicationV1, usize)>,
        device_unique_id: u64,
    ) {
        use KfdGeneratedCopyCoexistenceFailureV1 as Failure;
        if publication.submission == 0 || device_unique_id == 0 {
            self.reject(Failure::PublicationIdentity);
            return;
        }
        let Some(first) = self.first else {
            self.first = Some(publication);
            return;
        };
        if first.kind == publication.kind {
            self.reject(Failure::RepeatedPublicationClass);
            return;
        }
        let Some((compute, copy, copy_packets)) = simultaneous else {
            self.reject(Failure::MissingPublication);
            return;
        };
        let expected = match first.kind {
            KfdGeneratedCopyPublicationKindV1::GeneratedCompute => (compute, copy),
            KfdGeneratedCopyPublicationKindV1::DirectionalCopy => (copy, compute),
        };
        if expected != (first, publication) || copy_packets == 0 {
            self.reject(Failure::PublicationIdentity);
            return;
        }
        self.witness = Some(KfdGeneratedCopyCoexistenceWitnessV1 {
            device_unique_id,
            first_publication: first.kind,
            compute_receipt: compute.native,
            compute_membership: compute.membership,
            copy_receipt: copy.native,
            copy_membership: copy.membership,
            copy_packets,
        });
    }

    fn take(
        &mut self,
    ) -> Result<KfdGeneratedCopyCoexistenceWitnessV1, KfdGeneratedCopyCoexistenceFailureV1> {
        if self.taken {
            return Err(KfdGeneratedCopyCoexistenceFailureV1::NotArmed);
        }
        self.taken = true;
        if let Some(failure) = self.failure {
            return Err(failure);
        }
        self.witness
            .take()
            .ok_or(KfdGeneratedCopyCoexistenceFailureV1::MissingPublication)
    }
}

struct ComputeV1 {
    publication: PublicationV1,
    allocation: u64,
    stream: u64,
}

impl KfdRuntimeBackendV1 {
    fn generated_copy_qualification_scope_v1(
        &self,
    ) -> Result<(), KfdGeneratedCopyCoexistenceFailureV1> {
        use KfdGeneratedCopyCoexistenceFailureV1 as Failure;
        if self.terminal
            || self.queue_retired
            || self.terminal_memory.is_some()
            || self.terminal_sdma_custody.is_some()
        {
            return Err(Failure::Terminal);
        }
        if !matches!(
            self.launch_gate,
            KfdRuntimeLaunchGateV1::WorkerV3GeneratedOnly
        ) || self.description.target != "gfx942:xnack-"
            || self.description.backend_device == 0
        {
            return Err(Failure::UnsupportedProfile);
        }
        if self.active.is_some()
            || !self.pending_compute.is_empty()
            || !self.compute_pipeline.is_empty()
            || self
                .auxiliary_compute_lanes
                .iter()
                .any(|lane| lane.active.is_some() || !lane.pipeline.is_empty())
            || !self.modules.is_empty()
            || !self.kernels.is_empty()
        {
            return Err(Failure::UnsupportedCompute);
        }
        Ok(())
    }

    /// Arms one fixed two-publication capture on this backend, without I/O or allocation.
    ///
    /// Warm queues/buffers first. Arming requires no submitted work. The first two
    /// successful native work publications must be one generated write-only dispatch
    /// and one independent directional copy. Further publication before taking
    /// the witness invalidates the capture. This cannot be rearmed or enable any
    /// launch capability. Synchronous or same-device copies are outside this
    /// profile and invalidate capture. No reset, poll, progress, or scheduling change occurs.
    pub fn arm_generated_copy_coexistence_qualification_v1(
        &mut self,
    ) -> Result<(), KfdGeneratedCopyCoexistenceFailureV1> {
        use KfdGeneratedCopyCoexistenceFailureV1 as Failure;
        self.generated_copy_qualification_scope_v1()?;
        if self.generated_copy_coexistence.is_some() {
            return Err(Failure::AlreadyArmed);
        }
        if !self.generated_submissions.is_empty()
            || !self.active_sdma.is_empty()
            || !self.published_sdma_submissions.is_empty()
            || !self.allocation_custody.is_empty()
            || self
                .submissions
                .values()
                .any(|entry| entry.status == BackendPollV1::Pending)
        {
            return Err(Failure::Busy);
        }
        self.generated_copy_coexistence = Some(RecorderV1::new());
        Ok(())
    }

    /// Takes historical qualification evidence once; never polls or releases work.
    ///
    /// An ambiguous/terminal owner cannot return successful evidence. The caller
    /// must still settle both operations and validate outputs, cleanup and refunds.
    pub fn take_generated_copy_coexistence_qualification_v1(
        &mut self,
    ) -> Result<KfdGeneratedCopyCoexistenceWitnessV1, KfdGeneratedCopyCoexistenceFailureV1> {
        self.generated_copy_qualification_scope_v1()?;
        self.generated_copy_coexistence
            .as_mut()
            .ok_or(KfdGeneratedCopyCoexistenceFailureV1::NotArmed)?
            .take()
    }

    pub(in crate::kfd_backend) fn reject_generated_copy_witness_v1(
        &mut self,
        failure: KfdGeneratedCopyCoexistenceFailureV1,
    ) {
        if let Some(recorder) = &mut self.generated_copy_coexistence {
            recorder.reject(failure);
        }
    }

    pub(in crate::kfd_backend) fn record_generated_copy_publication_v1(
        &mut self,
        kind: KfdGeneratedCopyPublicationKindV1,
        submission: u64,
    ) {
        let Some(recorder) = &mut self.generated_copy_coexistence else {
            return;
        };
        if !recorder.next() {
            return;
        }
        let second = recorder.count == 2;
        let result = (|| {
            self.generated_copy_qualification_scope_v1()?;
            let publication = match kind {
                KfdGeneratedCopyPublicationKindV1::GeneratedCompute => {
                    self.generated_compute_observation_v1()?.publication
                }
                KfdGeneratedCopyPublicationKindV1::DirectionalCopy => {
                    self.generated_copy_observation_v1()?.0
                }
            };
            if publication.submission != submission {
                return Err(KfdGeneratedCopyCoexistenceFailureV1::PublicationIdentity);
            }
            let simultaneous = if second {
                let compute = self.generated_compute_observation_v1()?;
                let (copy, packets) = self.generated_copy_observation_v1()?;
                self.generated_copy_disjoint_custody_v1(&compute, copy.submission)?;
                Some((compute.publication, copy, packets))
            } else {
                None
            };
            Ok((publication, simultaneous))
        })();
        let recorder = self
            .generated_copy_coexistence
            .as_mut()
            .expect("retained recorder");
        match result {
            Ok((publication, simultaneous)) => {
                recorder.record(publication, simultaneous, self.description.backend_device)
            }
            Err(failure) => recorder.reject(failure),
        }
    }

    fn generated_compute_observation_v1(
        &self,
    ) -> Result<ComputeV1, KfdGeneratedCopyCoexistenceFailureV1> {
        use KfdGeneratedCopyCoexistenceFailureV1 as Failure;
        if self.generated_submissions.len() != 1 || self.generated_shells.len() != 1 {
            return Err(Failure::GeneratedSource);
        }
        let (&submission, &key) = self
            .generated_submissions
            .iter()
            .next()
            .ok_or(Failure::GeneratedSource)?;
        let record = self
            .generated_shells
            .get(&key)
            .ok_or(Failure::GeneratedSource)?;
        let plan = &record.plan;
        if !self.generated_submission_owner_matches_v1(submission, plan)
            || !self.generated_lease_matches_v1(plan)
            || plan.count != 1
            || plan.binding.backend_device != self.description.backend_device
            || self.submissions.contains_key(&submission)
            || self.active_sdma.contains_key(&submission)
        {
            return Err(Failure::GeneratedSource);
        }
        let native = record.native.as_ref().ok_or(Failure::GeneratedLease)?;
        if native.phase != PhaseV1::Adopted
            || native.lane != 0
            || !native.data.is_empty()
            || native.returned.remaining.is_some()
            || native.returned.completed != 0
            || native.returned.handed_to_lower.is_some()
            || record.control.is_some()
        {
            return Err(Failure::GeneratedLease);
        }
        let owner = native.submission.as_ref().ok_or(Failure::GeneratedSource)?;
        let member = plan.members[0].ok_or(Failure::GeneratedSource)?;
        let slot = owner.roster.buffers[0].ok_or(Failure::GeneratedSource)?;
        if slot.access != crate::Gfx942RuntimeBufferAccessV1::WriteOnly
            || owner.roster.readback_bytes != member.description.byte_len
            || owner.roster.fixup_count != 1
            || self.allocations.generated_count_for_adoption(key) != 1
        {
            return Err(Failure::UnsupportedCompute);
        }
        let NativeReceiptV1::Singleton(ReceiptV1::Published(batch)) = &owner.receipt else {
            return Err(Failure::NativeComputeReceipt);
        };
        let identity = self
            .queue
            .as_ref()
            .and_then(|queue| queue.observe_retained_fixed_dispatch_v1(native.native_lane?, batch))
            .ok_or(Failure::NativeComputeReceipt)?;
        let mut hash = Sha256::new();
        hash.update(b"fe2o3.generated-copy-coexistence-membership.v1\0");
        hash.update(identity);
        owner
            .roster
            .dispatch_contract_sha256
            .update_qualification_hash(&mut hash);
        for value in [
            plan.binding.context_generation,
            plan.binding.hold,
            plan.binding.backend_device,
            plan.binding.backend_stream,
            plan.key,
            submission,
            member.backend,
            member.description.byte_len,
        ] {
            hash.update(value.to_le_bytes());
        }
        Ok(ComputeV1 {
            publication: PublicationV1 {
                kind: KfdGeneratedCopyPublicationKindV1::GeneratedCompute,
                submission,
                native: identity,
                membership: hash.finalize().into(),
            },
            allocation: member.backend,
            stream: plan.binding.backend_stream,
        })
    }

    fn generated_copy_observation_v1(
        &self,
    ) -> Result<(PublicationV1, usize), KfdGeneratedCopyCoexistenceFailureV1> {
        use KfdGeneratedCopyCoexistenceFailureV1 as Failure;
        if self.active_sdma.len() != 1 {
            return Err(Failure::UnsupportedCopy);
        }
        let (&id, copy) = self
            .active_sdma
            .iter()
            .next()
            .ok_or(Failure::UnsupportedCopy)?;
        if !copy.dependencies.is_empty()
            || copy.dependency_cursor != 0
            || copy.peer_access.is_some()
            || copy.source == copy.destination
            || copy.byte_len == 0
        {
            return Err(Failure::UnsupportedCopy);
        }
        let observation = self
            .r66_copy_observation_v1(id, copy)
            .map_err(|_| Failure::NativeCopyReceipt)?;
        Ok((
            PublicationV1 {
                kind: KfdGeneratedCopyPublicationKindV1::DirectionalCopy,
                submission: id,
                native: observation.copy.ok_or(Failure::NativeCopyReceipt)?,
                membership: observation
                    .copy_membership
                    .ok_or(Failure::NativeCopyReceipt)?,
            },
            observation.copy_packets,
        ))
    }

    fn generated_copy_disjoint_custody_v1(
        &self,
        compute: &ComputeV1,
        copy_id: u64,
    ) -> Result<(), KfdGeneratedCopyCoexistenceFailureV1> {
        use KfdGeneratedCopyCoexistenceFailureV1 as Failure;
        let copy = &self.active_sdma[&copy_id];
        if compute.stream == copy.stream
            || [copy.source, copy.destination].contains(&compute.allocation)
            || self.allocations.get(&compute.allocation).is_some()
            || self.allocations.generated(copy.source).is_some()
            || self.allocations.generated(copy.destination).is_some()
        {
            return Err(Failure::AliasedOwners);
        }
        if self.generated_submissions.len() != 1
            || self.generated_shells.len() != 1
            || self
                .submissions
                .values()
                .any(|entry| entry.status == BackendPollV1::Pending)
            || self.allocation_custody.len() != 2
            || self.allocation_custody.iter().any(|(allocation, custody)| {
                ![copy.source, copy.destination].contains(allocation)
                    || custody.owners.len() != 1
                    || custody.owner_counts != [0, 1]
            })
        {
            return Err(Failure::UnaccountedCustody);
        }
        for (allocation, record) in self.allocations.ordinary_iter() {
            if matches!(
                record.sdma_storage,
                KfdRuntimeSdmaStorageV1::InFlight(_) | KfdRuntimeSdmaStorageV1::ComputeInFlight(_)
            ) && ![copy.source, copy.destination].contains(allocation)
            {
                return Err(Failure::UnaccountedCustody);
            }
        }
        let key = self.generated_submissions[&compute.publication.submission];
        let generated = self.generated_shells[&key]
            .native
            .as_ref()
            .ok_or(Failure::GeneratedLease)?;
        let lane = generated.native_lane.ok_or(Failure::GeneratedLease)?;
        let NativeReceiptV1::Singleton(ReceiptV1::Published(batch)) = &generated
            .submission
            .as_ref()
            .ok_or(Failure::GeneratedSource)?
            .receipt
        else {
            return Err(Failure::NativeComputeReceipt);
        };
        let queue = self.queue.as_ref().ok_or(Failure::NativeComputeReceipt)?;
        let ActiveSdmaPhaseV1::DirectionalPublished(owner) = &copy.phase else {
            return Err(Failure::UnsupportedCopy);
        };
        let disjoint = match owner.as_ref() {
            DirectionalSdmaSubmissionOwnerV1::NativeSingle { submission, .. } => {
                queue.observe_generated_single_copy_disjoint_custody_v1(lane, batch, submission)
            }
            DirectionalSdmaSubmissionOwnerV1::NativeWindow { submission } => {
                queue.observe_generated_window_copy_disjoint_custody_v1(lane, batch, submission)
            }
            #[cfg(test)]
            _ => false,
        };
        if !disjoint {
            return Err(Failure::AliasedOwners);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
