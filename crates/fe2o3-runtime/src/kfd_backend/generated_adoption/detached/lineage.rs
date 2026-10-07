//! Internal correspondence to the still-retained producer, not consumer authority.

use super::*;

pub(super) struct ProducerLineageV1 {
    plan: GeneratedShellPlanV1,
    submission: u64,
    generation: u64,
    roster: GeneratedHostRosterV1,
    rejected: bool,
}

impl ProducerLineageV1 {
    fn new(
        plan: &GeneratedShellPlanV1,
        submission: &issue::GeneratedSubmissionV1,
        generation: u64,
    ) -> Option<Self> {
        let recycled = submission.receipt.retirement() == Some(RetirementV1::Recycled);
        let rejected = submission.receipt.rejected_retirement() == Some(RetirementV1::Recycled);
        if generation == 0
            || submission.id == 0
            || !(recycled || rejected)
            || submission.receipt.profile() != plan.profile
            || submission.roster.source_identity.profile() != plan.profile
            || !readback::roster_matches_plan_v1(plan, &submission.roster)
            || plan.members[plan.count..].iter().any(Option::is_some)
            || submission.roster.buffers[plan.count..]
                .iter()
                .any(Option::is_some)
            || plan.members[..plan.count]
                .iter()
                .enumerate()
                .any(|(ordinal, member)| {
                    member.is_none_or(|member| member.description.ordinal != ordinal)
                })
        {
            return None;
        }
        Some(Self {
            plan: *plan,
            submission: submission.id,
            generation,
            // Fixed arrays plus clones of the original Arc identities: no new
            // source identity, DATA, backing allocation or version is created.
            roster: submission.roster.clone(),
            rejected,
        })
    }

    fn matches(
        &self,
        plan: &GeneratedShellPlanV1,
        submission: &issue::GeneratedSubmissionV1,
    ) -> bool {
        self.plan == *plan
            && self.submission == submission.id
            && self.roster.matches(&submission.roster)
            && submission.receipt.profile() == plan.profile
            && if self.rejected {
                submission.receipt.rejected_retirement() == Some(RetirementV1::Recycled)
            } else {
                submission.receipt.retirement() == Some(RetirementV1::Recycled)
            }
    }
}

impl<T> RetainedDetachedV1<T> {
    pub(super) fn capture_producer<E>(
        &mut self,
        plan: &GeneratedShellPlanV1,
        submission: &issue::GeneratedSubmissionV1,
        generation: u64,
        capture: impl FnOnce() -> Result<T, E>,
    ) -> Result<(), CaptureErrorV1<E>> {
        if self.phase != DetachedPhaseV1::Empty || self.owner.is_some() || self.producer.is_some() {
            return Err(CaptureErrorV1::Occupied);
        }
        let lineage =
            ProducerLineageV1::new(plan, submission, generation).ok_or(CaptureErrorV1::Lineage)?;
        // Root correspondence before the consuming lower call. Failure leaves
        // the original producer and the entered state retained, never retryable.
        self.producer = Some(lineage);
        self.capture(capture)
    }

    pub(super) fn matches_producer(
        &self,
        plan: &GeneratedShellPlanV1,
        submission: &issue::GeneratedSubmissionV1,
        observation: impl FnOnce(&T) -> (u64, usize),
    ) -> bool {
        self.producer.as_ref().is_some_and(|lineage| {
            lineage.matches(plan, submission)
                && self
                    .matches(|owner| observation(owner) == (lineage.generation, lineage.plan.count))
        })
    }

    pub(super) fn take_producer_checked(
        &mut self,
        plan: &GeneratedShellPlanV1,
        submission: &issue::GeneratedSubmissionV1,
        observation: impl FnOnce(&T) -> (u64, usize),
    ) -> Option<T> {
        if !self.matches_producer(plan, submission, observation) {
            return None;
        }
        let owner = self.take_checked(|_| true)?;
        self.producer = None;
        Some(owner)
    }
}
