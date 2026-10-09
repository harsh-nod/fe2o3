//! One held, original-request join. The retained coordinates are not authority.
use super::*;
use crate::native_v3::OriginalCompilerEnrollment as Enrollment;

const ENTRY: usize = 8;
const COMPARE_WORK: usize = 2 * 32 + size_of::<u64>();
pub(super) const WORK: usize = ENTRY + 3 * RECORD_WORK + COMPARE_WORK;
pub(super) const SCRATCH: usize =
    size_of::<Enrollment>() + 4 * size_of::<&Record>() + RECORD_SCRATCH;

impl RootCompilerRequest<'_> {
    pub(super) fn join_original_enrollment(
        &mut self,
        previous: State,
        b: &mut Budget<'_>,
    ) -> Result<()> {
        b.check_prior_denials_v1()?;
        b.with_prepaid_scope(self.reserved, ENTRY, ENTRY, SCRATCH, |b| {
            self.check_account(b)?;
            require_transition(
                previous,
                self.state,
                self.prepared.is_some(),
                self.enrollment.is_some(),
            )?;
            self.check_complete(b)?;
            let challenge = original_transcript(&self.receiver, b)?;
            let enrollment = self
                .attempt()?
                .with_original_enrollment(b, |value, b| {
                    matches_intake(value, challenge, b)?;
                    // The request's prepaid inline field owns the copy. No returned
                    // borrow, detached approval or replacement account is introduced.
                    Ok(*value)
                })
                .map_err(helper_error)?;
            b.check_prior_denials_v1()?;
            self.check_account(b)?;
            require_deadline(self.receiver.deadline)?;
            self.enrollment = Some(enrollment);
            Ok(())
        })
    }
}

fn require_deadline(deadline: Option<Instant>) -> Result<()> {
    if deadline.is_none_or(|deadline| Instant::now() >= deadline) {
        return Err(rejected("intake deadline exceeded during enrollment join"));
    }
    Ok(())
}

fn require_transition(
    previous: State,
    current: State,
    prepared: bool,
    enrolled: bool,
) -> Result<()> {
    // step consumed the original phase before any fallible work. The actual
    // Attempt additionally requires its issued owner and original controller.
    if previous != State::ConfirmedExec || current != State::Failed || prepared || enrolled {
        return Err(rejected(
            "enrollment join requires original held issuer transition",
        ));
    }
    Ok(())
}

fn original_transcript<'a>(receiver: &'a Receiver, b: &mut Budget<'_>) -> Result<&'a Record> {
    let (Some(hello), Some(challenge), Some(last), Some(ack)) = (
        &receiver.hello,
        &receiver.challenge,
        &receiver.last,
        &receiver.ack,
    ) else {
        return Err(rejected("enrollment join lost original intake transcript"));
    };
    if !challenge.matches_predecessor(hello, b).map_err(record)?
        || !last.matches_predecessor(challenge, b).map_err(record)?
        || !ack.matches_predecessor(last, b).map_err(record)?
    {
        return Err(rejected(
            "enrollment join differs from original intake transcript",
        ));
    }
    Ok(challenge)
}

fn matches_intake(
    value: &Enrollment,
    challenge: &Record,
    b: &mut Budget<'_>,
) -> std::result::Result<(), ProofHelperLaunchError> {
    b.check_prior_denials_v1()?;
    b.charge_work(COMPARE_WORK)?;
    if challenge.policy_identity() != &value.native_policy_sha256
        || challenge.invocation_identity() != &value.intake_invocation_identity
        || challenge.invocation_bytes() != value.invocation_bytes
    {
        return Err(ProofHelperLaunchError::Invalid(
            "original enrollment differs from intake",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "native_root_request_enrollment_tests.rs"]
mod tests;
