//! Native custody over the shared seqpacket exchange mechanics, not a V1 owner.
use super::*;
use crate::{
    ProtectedCompilerExecutionIssuerServiceErrorV2 as Error,
    ProtectedExternalAnchorServiceAdmissionV2 as Admission,
};
use fe2o3_compiler_execution_protocol::CompilerExecutionIssuerPolicyV2 as Policy;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use std::mem::size_of;
use std::os::fd::{AsFd, OwnedFd};
type Result<T> = std::result::Result<T, Error>;
type WireError = ProtectedCompilerExecutionExternalAnchorErrorV1;

pub(crate) struct NativeAnchor<'a> {
    admission: &'a Admission,
    peer: OwnedFd,
    pidfd: OwnedFd,
    key: PinnedAnchorKeyV1,
    retained: usize,
    poisoned: bool,
}
impl<'a> NativeAnchor<'a> {
    pub(crate) fn new(admission: &'a Admission, p: &Policy, b: &mut Budget<'_>) -> Result<Self> {
        b.with_prepaid_scope(
            admission.retained_storage() + p.retained_storage(),
            8,
            262144,
            32768,
            |b| {
                admission.validate_continuity(b)?;
                let (peer, pidfd, charge) = admission.try_clone_for_transfer(b)?;
                b.reserve_storage(charge.additional_storage())?;
                admission.validate_transfer(peer.as_fd(), pidfd.as_fd(), b)?;
                let key = PinnedAnchorKeyV1::from_bytes(*p.external_anchor_verifying_key())?;
                Ok(Self {
                    admission,
                    peer,
                    pidfd,
                    key,
                    retained: size_of::<Self>() + charge.additional_storage(),
                    poisoned: false,
                })
            },
        )
    }
    pub(crate) const fn retained_storage(&self) -> usize {
        self.retained
    }
    pub(crate) fn exchange(
        &mut self,
        c: &AnchorChallengeV1,
        deadline: Instant,
        attempts: &mut usize,
        b: &mut Budget<'_>,
    ) -> Result<AnchorTransitionReceiptV1> {
        if self.poisoned {
            return Err(WireError::Poisoned.into());
        }
        let result = b.with_prepaid_scope(
            self.retained + self.admission.retained_storage() + size_of::<AnchorChallengeV1>(),
            8,
            1048576,
            32768,
            |b| self.exchange_once(c, deadline, attempts, b),
        );
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
    fn exchange_once(
        &self,
        c: &AnchorChallengeV1,
        session_deadline: Instant,
        attempts: &mut usize,
        b: &mut Budget<'_>,
    ) -> Result<AnchorTransitionReceiptV1> {
        self.validate(b)?;
        if c.anchor_key_identity() != self.key.identity() {
            return Err(AnchorProtocolErrorV1::AnchorKeyIdentityMismatch.into());
        }
        let deadline = Instant::now()
            .checked_add(COMPILER_EXECUTION_EXTERNAL_ANCHOR_TIMEOUT_V1)
            .ok_or(WireError::DeadlineOverflow)?
            .min(session_deadline);
        if Instant::now() >= deadline {
            return Err(WireError::Timeout.into());
        }
        let mut observation =
            receive_observation_metered(self.peer.as_fd(), &mut || permit(b, attempts))?;
        if observation.is_none() {
            wait_for_service_metered(
                self.peer.as_fd(),
                self.pidfd.as_fd(),
                libc::POLLOUT,
                deadline,
                &mut || permit(b, attempts),
            )?;
            self.validate(b)?;
            send_challenge_metered(self.peer.as_fd(), c, &mut || permit(b, attempts))?;
            wait_for_service_metered(
                self.peer.as_fd(),
                self.pidfd.as_fd(),
                libc::POLLIN,
                deadline,
                &mut || permit(b, attempts),
            )?;
            self.validate(b)?;
            observation =
                receive_observation_metered(self.peer.as_fd(), &mut || permit(b, attempts))?;
        }
        let observation = observation.ok_or(WireError::EndpointNotReady)?;
        let receipt = AnchorTransitionReceiptV1::new(c.clone(), &observation, &self.key)?;
        if receive_observation_metered(self.peer.as_fd(), &mut || permit(b, attempts))?.is_some() {
            return Err(WireError::DuplicateResponse.into());
        }
        self.validate(b)?;
        Ok(receipt)
    }
    fn validate(&self, b: &mut Budget<'_>) -> Result<()> {
        Ok(self
            .admission
            .validate_transfer(self.peer.as_fd(), self.pidfd.as_fd(), b)?)
    }
}
fn permit(b: &mut Budget<'_>, attempts: &mut usize) -> std::result::Result<(), WireError> {
    b.charge_work(4096).map_err(WireError::Resource)?;
    if *attempts >= 256 {
        return Err(WireError::AttemptLimit);
    }
    *attempts += 1;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

    #[test]
    fn native_anchor_attempt_limit_preserves_cumulative_work() {
        let mut work = Work::new(usize::MAX);
        let mut b = Budget::new(&mut work, usize::MAX);
        let mut attempts = 255;
        permit(&mut b, &mut attempts).unwrap();
        assert!(matches!(
            permit(&mut b, &mut attempts),
            Err(WireError::AttemptLimit)
        ));
        assert_eq!(b.work(), 8192);
        assert_eq!(attempts, 256);
    }

    #[test]
    fn native_anchor_resource_refusal_does_not_consume_queued_response() {
        use rustix::net::{self, AddressFamily, SendFlags, SocketFlags, SocketType};
        let (peer, service) = net::socketpair(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC,
            None,
        )
        .unwrap();
        let bytes = [7; ANCHOR_OBSERVATION_WIRE_LEN_V1];
        assert_eq!(
            net::send(&service, &bytes, SendFlags::NOSIGNAL).unwrap(),
            bytes.len()
        );
        let mut work = Work::new(1);
        let mut b = Budget::new(&mut work, usize::MAX);
        let mut attempts = 0;
        assert!(matches!(
            receive_observation_metered(peer.as_fd(), &mut || permit(&mut b, &mut attempts)),
            Err(WireError::Resource(_))
        ));
        assert_eq!(attempts, 0);
        assert_eq!(
            receive_observation_nonblocking(peer.as_fd()).unwrap(),
            Some(bytes)
        );
    }
}
