//! Private original DATA copy custody. No generated consumer profile is admitted.
//!
//! The destination remains an independently reserved ordinary allocation. The
//! source is the original detached DATA and original SDMA promotion bridge,
//! never an AllocationRef manufactured from descriptive generated metadata.

use super::*;
use fe2o3_kfd::{Gfx942DetachedFixedDispatchV1, Gfx942DirectionalQueuePersistentAllocationV1};

pub(super) mod backend;
mod native;
mod root;

use detached::lineage::ProducerLineageV1;
use native::{NativeOperations, NativeOwners};
use root::PendingCopy;
pub(super) use root::{Progress, Refusal};

pub(super) struct NativeCopyV1 {
    custody: PendingCopy<NativeOwners>,
    lineage: Option<ProducerLineageV1>,
    destination_offset: u64,
    bytes: u32,
}

impl NativeCopyV1 {
    pub(super) fn empty(destination_offset: u64, bytes: u32) -> Option<Self> {
        if bytes == 0 || destination_offset.checked_add(u64::from(bytes)).is_none() {
            return None;
        }
        Some(Self {
            custody: PendingCopy::empty(),
            lineage: None,
            destination_offset,
            bytes,
        })
    }

    // Refusal returns the original native destination without allocating after
    // removing it from its prepaid owner slot.
    #[allow(clippy::result_large_err)]
    pub(super) fn install(
        &mut self,
        plan: &GeneratedShellPlanV1,
        submission: &issue::GeneratedSubmissionV1,
        retained: &mut detached::RetainedDetachedV1<Gfx942DetachedFixedDispatchV1>,
        backing: &mut sdma_backing::NativeCustody,
        destination: Gfx942DirectionalQueuePersistentAllocationV1,
    ) -> Result<(), Gfx942DirectionalQueuePersistentAllocationV1> {
        if !self.custody.is_empty()
            || self.lineage.is_some()
            || plan.profile != crate::generated_source::GeneratedProfileV1::Singleton
            || plan.count != 1
            || plan.members[0]
                .is_none_or(|member| member.description.byte_len != u64::from(self.bytes))
            || !backing.is_bound()
            || submission.receipt.retirement() != Some(RetirementV1::Recycled)
            || !retained.matches_producer(plan, submission, |owner| {
                (owner.dispatch_generation(), owner.data_lease_count())
            })
            || self.destination_offset + u64::from(self.bytes) > destination.byte_len()
        {
            return Err(destination);
        }
        let Some((detached, lineage)) =
            retained.take_producer_for_copy(plan, submission, |owner| {
                (owner.dispatch_generation(), owner.data_lease_count())
            })
        else {
            std::process::abort()
        };
        let Some(bridge) = backing.take_bridge_for_copy() else {
            std::process::abort()
        };
        self.lineage = Some(lineage);
        match self.custody.install((detached, bridge), destination) {
            Ok(()) => (),
            Err(_originals) => std::process::abort(),
        }
        Ok(())
    }

    pub(super) fn advance(
        &mut self,
        queue: &mut ComputeAqlQueueSessionV1,
    ) -> Result<Progress, Refusal> {
        self.custody.advance(&mut NativeOperations {
            queue,
            destination_offset: self.destination_offset,
            bytes: self.bytes,
        })
    }

    pub(super) fn is_empty(&self) -> bool {
        self.custody.is_empty() && self.lineage.is_none()
    }

    pub(super) fn take_released(
        &mut self,
        plan: &GeneratedShellPlanV1,
        submission: &issue::GeneratedSubmissionV1,
    ) -> Option<ReleasedCopyV1> {
        if !self
            .lineage
            .as_ref()
            .is_some_and(|lineage| lineage.matches(plan, submission))
        {
            return None;
        }
        let destination = self.custody.take_released()?;
        let Some(lineage) = self.lineage.take() else {
            std::process::abort()
        };
        Some(ReleasedCopyV1 {
            destination,
            lineage,
        })
    }
}

/// A private owner-carrying receipt for actual SDMA completion, frontier
/// retirement and physical source release. It does not commit Context versions.
pub(super) struct ReleasedCopyV1 {
    destination: Gfx942DirectionalQueuePersistentAllocationV1,
    lineage: ProducerLineageV1,
}

impl ReleasedCopyV1 {
    pub(super) fn matches(
        &self,
        plan: &GeneratedShellPlanV1,
        submission: &issue::GeneratedSubmissionV1,
    ) -> bool {
        self.lineage.matches(plan, submission)
    }

    pub(super) fn into_destination(self) -> Gfx942DirectionalQueuePersistentAllocationV1 {
        self.destination
    }

    fn into_parts(
        self,
    ) -> (
        Gfx942DirectionalQueuePersistentAllocationV1,
        ReleasedSourceV1,
    ) {
        (
            self.destination,
            ReleasedSourceV1 {
                lineage: self.lineage,
            },
        )
    }
}

// Constructed only by consuming the original physically released copy receipt,
// after its destination has remained retained through frontier retirement.
struct ReleasedSourceV1 {
    lineage: ProducerLineageV1,
}

#[cfg(test)]
#[path = "data_copy/tests.rs"]
mod tests;
