//! Policy-bound lifecycle coordination for one protected rustc execution session.

use std::error::Error;
use std::fmt;
use std::os::fd::OwnedFd;
use std::process::Command;
use std::time::{Duration, Instant};

use fe2o3_artifact_transaction::InertCompilerExecutionSubjectV1;
use fe2o3_compiler_closure_capability::{
    COMPILER_EXECUTION_POLICY_CHILD_FD_V1, CompilerExecutionPolicyCapabilityV1,
};
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionIssuerPolicyV1, CompilerExecutionReceiptCarriageV1,
    CompilerExecutionServiceLaunchManifestV1, CompilerExecutionServiceReadyV1,
};

use crate::child_channel::require_reserved_descriptor_unused;
use crate::supervisor_handoff::transfer_to_supervisor_inner;
use crate::{
    COMPILER_EXECUTION_RECEIPT_RETURN_CHILD_FD_V1, COMPILER_EXECUTION_SERVICE_CHILD_FD_V1,
    CompilerExecutionChildChannelErrorV1, CompilerExecutionClientProcessIdentityV1,
    CompilerExecutionHandoffErrorV1, CompilerExecutionReceiptReceiverV1,
    CompilerExecutionReceiptReturnErrorV1, CompilerExecutionServiceLaunchV1,
    CompilerExecutionSupervisorCredentialsV1, PendingCompilerExecutionChildChannelV1,
    PendingCompilerExecutionReceiptReturnV1, PendingCompilerExecutionSupervisorV1,
};

const _: () = assert!(
    COMPILER_EXECUTION_SERVICE_CHILD_FD_V1 != COMPILER_EXECUTION_RECEIPT_RETURN_CHILD_FD_V1
);
const _: () =
    assert!(COMPILER_EXECUTION_SERVICE_CHILD_FD_V1 != COMPILER_EXECUTION_POLICY_CHILD_FD_V1);
const _: () =
    assert!(COMPILER_EXECUTION_RECEIPT_RETURN_CHILD_FD_V1 != COMPILER_EXECUTION_POLICY_CHILD_FD_V1);

/// Prepared policy, service, and receipt-return installation for one unspawned rustc command.
///
/// This value is move-only and carries no issuer, signing, compiler, publication, loading, or
/// execution authority. If preparation returns an error, the command must be discarded because a
/// preceding child callback may already have been registered.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_client::PendingCompilerExecutionChildSessionV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<PendingCompilerExecutionChildSessionV1>();
/// ```
pub struct PendingCompilerExecutionChildSessionV1 {
    policy: CompilerExecutionPolicyCapabilityV1,
    service: PendingCompilerExecutionChildChannelV1,
    receipt: PendingCompilerExecutionReceiptReturnV1,
}

impl fmt::Debug for PendingCompilerExecutionChildSessionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PendingCompilerExecutionChildSessionV1")
            .field("authority", &"none")
            .field("policy", &self.policy.policy().identity())
            .finish_non_exhaustive()
    }
}

impl PendingCompilerExecutionChildSessionV1 {
    /// Installs an already provisioned sealed policy and prepares both child-created channels.
    ///
    /// The caller, not an ambient path or environment variable, supplies the policy capability.
    /// All three fixed destinations are checked before the command is modified.
    pub fn prepare(
        command: &mut Command,
        policy: CompilerExecutionPolicyCapabilityV1,
    ) -> Result<Self, CompilerExecutionChildSessionErrorV1> {
        policy
            .revalidate()
            .map_err(CompilerExecutionChildSessionErrorV1::PolicyCapability)?;
        for descriptor in [
            COMPILER_EXECUTION_SERVICE_CHILD_FD_V1,
            COMPILER_EXECUTION_RECEIPT_RETURN_CHILD_FD_V1,
        ] {
            require_reserved_descriptor_unused(descriptor)
                .map_err(CompilerExecutionChildSessionErrorV1::ChildChannel)?;
        }
        require_reserved_descriptor_unused(COMPILER_EXECUTION_POLICY_CHILD_FD_V1).map_err(
            |error| {
                CompilerExecutionChildSessionErrorV1::PolicyCapability(format!(
                    "reserved policy descriptor {COMPILER_EXECUTION_POLICY_CHILD_FD_V1} is unavailable: {error}"
                ))
            },
        )?;

        let service = PendingCompilerExecutionChildChannelV1::prepare(command)
            .map_err(CompilerExecutionChildSessionErrorV1::ChildChannel)?;
        let receipt = PendingCompilerExecutionReceiptReturnV1::prepare(command)
            .map_err(CompilerExecutionChildSessionErrorV1::ReceiptReturn)?;
        policy
            .inherit_for_child(command)
            .map_err(CompilerExecutionChildSessionErrorV1::PolicyCapability)?;
        policy
            .revalidate()
            .map_err(CompilerExecutionChildSessionErrorV1::PolicyCapability)?;
        Ok(Self {
            policy,
            service,
            receipt,
        })
    }

    /// Admits both parent endpoints against the same still-live spawned rustc PID.
    ///
    /// One absolute timeout covers both child transfers and both pidfd admissions.
    pub fn finish(
        self,
        child_pid: u32,
        timeout: Duration,
    ) -> Result<CompilerExecutionChildSessionV1, CompilerExecutionChildSessionErrorV1> {
        let deadline = deadline(timeout)?;
        self.policy
            .revalidate()
            .map_err(CompilerExecutionChildSessionErrorV1::PolicyCapability)?;
        let service = self
            .service
            .finish(child_pid, remaining(deadline)?)
            .map_err(CompilerExecutionChildSessionErrorV1::ChildChannel)?;
        let receipt = self
            .receipt
            .finish(child_pid, remaining(deadline)?)
            .map_err(CompilerExecutionChildSessionErrorV1::ReceiptReturn)?;
        if service.client() != receipt.client() {
            return Err(CompilerExecutionChildSessionErrorV1::ClientIdentityMismatch);
        }
        self.policy
            .revalidate()
            .map_err(CompilerExecutionChildSessionErrorV1::PolicyCapability)?;
        Ok(CompilerExecutionChildSessionV1 {
            policy: self.policy,
            service,
            receipt,
        })
    }
}

/// Exact policy and two admitted parent endpoints for one still-live rustc process.
///
/// The service endpoint can cross only the existing authenticated supervisor handoff. The receipt
/// endpoint remains privately retained until matching supervisor readiness has been admitted.
pub struct CompilerExecutionChildSessionV1 {
    policy: CompilerExecutionPolicyCapabilityV1,
    service: CompilerExecutionServiceLaunchV1,
    receipt: CompilerExecutionReceiptReceiverV1,
}

impl fmt::Debug for CompilerExecutionChildSessionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilerExecutionChildSessionV1")
            .field("authority", &"none")
            .field("client", &self.client())
            .field("policy", &self.policy.policy().identity())
            .finish_non_exhaustive()
    }
}

impl CompilerExecutionChildSessionV1 {
    /// Returns the exact rustc process identity shared by both admitted channels.
    pub const fn client(&self) -> CompilerExecutionClientProcessIdentityV1 {
        self.service.client()
    }

    /// Returns the caller-provisioned policy without exposing its sealed descriptor.
    pub const fn policy(&self) -> &CompilerExecutionIssuerPolicyV1 {
        self.policy.policy()
    }

    /// Transfers the service endpoint to one authenticated distinct-UID supervisor.
    pub fn transfer_to_supervisor(
        self,
        control: OwnedFd,
        expected_supervisor: CompilerExecutionSupervisorCredentialsV1,
        timeout: Duration,
    ) -> Result<PendingCompilerExecutionChildSupervisorV1, CompilerExecutionChildSessionErrorV1>
    {
        self.transfer_to_supervisor_inner::<true>(control, expected_supervisor, timeout)
    }

    fn transfer_to_supervisor_inner<const REQUIRE_DISTINCT_UID: bool>(
        self,
        control: OwnedFd,
        expected_supervisor: CompilerExecutionSupervisorCredentialsV1,
        timeout: Duration,
    ) -> Result<PendingCompilerExecutionChildSupervisorV1, CompilerExecutionChildSessionErrorV1>
    {
        self.policy
            .revalidate()
            .map_err(CompilerExecutionChildSessionErrorV1::PolicyCapability)?;
        let client = self.client();
        let pending = transfer_to_supervisor_inner::<REQUIRE_DISTINCT_UID>(
            self.service,
            control,
            expected_supervisor,
            self.policy.policy(),
            timeout,
        )
        .map_err(CompilerExecutionChildSessionErrorV1::SupervisorHandoff)?;
        Ok(PendingCompilerExecutionChildSupervisorV1 {
            policy: self.policy,
            pending,
            receipt: self.receipt,
            client,
        })
    }
}

/// Pending exact supervisor readiness while rustc receipt-return custody remains retained.
pub struct PendingCompilerExecutionChildSupervisorV1 {
    policy: CompilerExecutionPolicyCapabilityV1,
    pending: PendingCompilerExecutionSupervisorV1,
    receipt: CompilerExecutionReceiptReceiverV1,
    client: CompilerExecutionClientProcessIdentityV1,
}

impl fmt::Debug for PendingCompilerExecutionChildSupervisorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PendingCompilerExecutionChildSupervisorV1")
            .field("authority", &"none")
            .field("client", &self.client)
            .field("policy", &self.policy.policy().identity())
            .finish_non_exhaustive()
    }
}

impl PendingCompilerExecutionChildSupervisorV1 {
    /// Returns the exact launch manifest while keeping the control endpoint private.
    pub const fn manifest(&self) -> &CompilerExecutionServiceLaunchManifestV1 {
        self.pending.manifest()
    }

    /// Consumes the control exchange and retains readiness beside the receipt receiver.
    pub fn await_readiness(
        self,
        timeout: Duration,
    ) -> Result<ReadyCompilerExecutionChildSessionV1, CompilerExecutionChildSessionErrorV1> {
        self.policy
            .revalidate()
            .map_err(CompilerExecutionChildSessionErrorV1::PolicyCapability)?;
        let readiness = self
            .pending
            .await_readiness(self.policy.policy(), timeout)
            .map_err(CompilerExecutionChildSessionErrorV1::SupervisorHandoff)?;
        self.policy
            .revalidate()
            .map_err(CompilerExecutionChildSessionErrorV1::PolicyCapability)?;
        Ok(ReadyCompilerExecutionChildSessionV1 {
            policy: self.policy,
            readiness,
            receipt: self.receipt,
            client: self.client,
        })
    }
}

/// Exact supervisor readiness retained with one rustc receipt-return endpoint.
///
/// Readiness alone grants no compiler or receipt authority. The only completion transition
/// consumes this value while receiving a real exact carriage from the admitted rustc endpoint.
pub struct ReadyCompilerExecutionChildSessionV1 {
    policy: CompilerExecutionPolicyCapabilityV1,
    readiness: CompilerExecutionServiceReadyV1,
    receipt: CompilerExecutionReceiptReceiverV1,
    client: CompilerExecutionClientProcessIdentityV1,
}

impl fmt::Debug for ReadyCompilerExecutionChildSessionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReadyCompilerExecutionChildSessionV1")
            .field("authority", &"readiness-only")
            .field("client", &self.client)
            .field("readiness", &self.readiness.identity())
            .finish_non_exhaustive()
    }
}

impl ReadyCompilerExecutionChildSessionV1 {
    /// Returns inert exact readiness without exposing receipt or process descriptors.
    pub const fn readiness(&self) -> &CompilerExecutionServiceReadyV1 {
        &self.readiness
    }

    /// Receives one real canonical carriage joined to the retained policy and exact subject.
    pub fn receive_exact(
        self,
        expected_subject: &InertCompilerExecutionSubjectV1,
        timeout: Duration,
    ) -> Result<CompletedCompilerExecutionChildSessionV1, CompilerExecutionChildSessionErrorV1>
    {
        self.policy
            .revalidate()
            .map_err(CompilerExecutionChildSessionErrorV1::PolicyCapability)?;
        let carriage = self
            .receipt
            .receive_exact(self.policy.policy(), expected_subject, timeout)
            .map_err(CompilerExecutionChildSessionErrorV1::ReceiptReturn)?;
        self.policy
            .revalidate()
            .map_err(CompilerExecutionChildSessionErrorV1::PolicyCapability)?;
        Ok(CompletedCompilerExecutionChildSessionV1 {
            policy: self.policy,
            readiness: self.readiness,
            carriage,
            client: self.client,
        })
    }
}

/// Inert completed child session containing a real policy- and subject-matched carriage.
pub struct CompletedCompilerExecutionChildSessionV1 {
    policy: CompilerExecutionPolicyCapabilityV1,
    readiness: CompilerExecutionServiceReadyV1,
    carriage: CompilerExecutionReceiptCarriageV1,
    client: CompilerExecutionClientProcessIdentityV1,
}

impl fmt::Debug for CompletedCompilerExecutionChildSessionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompletedCompilerExecutionChildSessionV1")
            .field("authority", &"inert-receipt-carriage")
            .field("client", &self.client)
            .field("readiness", &self.readiness.identity())
            .field("carriage", &self.carriage.identity())
            .finish_non_exhaustive()
    }
}

impl CompletedCompilerExecutionChildSessionV1 {
    /// Returns the exact rustc identity bound to both child-created channels.
    pub const fn client(&self) -> CompilerExecutionClientProcessIdentityV1 {
        self.client
    }

    /// Returns the exact supervisor readiness retained through receipt completion.
    pub const fn readiness(&self) -> &CompilerExecutionServiceReadyV1 {
        &self.readiness
    }

    /// Returns the real exact carriage without exposing the retained sealed policy descriptor.
    pub const fn carriage(&self) -> &CompilerExecutionReceiptCarriageV1 {
        &self.carriage
    }

    /// Consumes completed custody and returns the inert exact carriage.
    pub fn into_carriage(self) -> CompilerExecutionReceiptCarriageV1 {
        debug_assert_eq!(self.carriage.policy(), self.policy.policy());
        self.carriage
    }
}

fn deadline(timeout: Duration) -> Result<Instant, CompilerExecutionChildSessionErrorV1> {
    if timeout.is_zero() {
        return Err(CompilerExecutionChildSessionErrorV1::InvalidTimeout);
    }
    Instant::now()
        .checked_add(timeout)
        .ok_or(CompilerExecutionChildSessionErrorV1::DeadlineOverflow)
}

fn remaining(deadline: Instant) -> Result<Duration, CompilerExecutionChildSessionErrorV1> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        Err(CompilerExecutionChildSessionErrorV1::Timeout)
    } else {
        Ok(remaining)
    }
}

/// Stable policy-bound child-session orchestration failure.
#[derive(Debug)]
#[non_exhaustive]
pub enum CompilerExecutionChildSessionErrorV1 {
    /// The caller supplied a zero timeout.
    InvalidTimeout,
    /// The absolute deadline could not be represented.
    DeadlineOverflow,
    /// The shared absolute deadline expired between child admissions.
    Timeout,
    /// The sealed caller-provisioned policy failed revalidation or installation.
    PolicyCapability(String),
    /// Service-channel preparation or admission failed.
    ChildChannel(CompilerExecutionChildChannelErrorV1),
    /// Receipt-return preparation, admission, or carriage validation failed.
    ReceiptReturn(CompilerExecutionReceiptReturnErrorV1),
    /// Authenticated supervisor transfer or readiness admission failed.
    SupervisorHandoff(CompilerExecutionHandoffErrorV1),
    /// The two independently admitted child-created channels name different clients.
    ClientIdentityMismatch,
}

impl fmt::Display for CompilerExecutionChildSessionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTimeout => formatter.write_str("child-session timeout must be nonzero"),
            Self::DeadlineOverflow => formatter.write_str("child-session deadline overflowed"),
            Self::Timeout => formatter.write_str("child-session deadline expired"),
            Self::PolicyCapability(error) => {
                write!(formatter, "child-session policy capability failed: {error}")
            }
            Self::ChildChannel(error) => write!(formatter, "child-session service failed: {error}"),
            Self::ReceiptReturn(error) => {
                write!(formatter, "child-session receipt return failed: {error}")
            }
            Self::SupervisorHandoff(error) => {
                write!(
                    formatter,
                    "child-session supervisor handoff failed: {error}"
                )
            }
            Self::ClientIdentityMismatch => {
                formatter.write_str("child-session channels name different rustc clients")
            }
        }
    }
}

impl Error for CompilerExecutionChildSessionErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ChildChannel(error) => Some(error),
            Self::ReceiptReturn(error) => Some(error),
            Self::SupervisorHandoff(error) => Some(error),
            Self::InvalidTimeout
            | Self::DeadlineOverflow
            | Self::Timeout
            | Self::PolicyCapability(_)
            | Self::ClientIdentityMismatch => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::IoSliceMut;
    use std::mem::MaybeUninit;
    use std::os::fd::AsRawFd;

    use ed25519_dalek::SigningKey;
    use fe2o3_artifact_transaction::{
        INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1, INERT_COMPILER_EXECUTION_SUBJECT_MAGIC_V1,
        INERT_COMPILER_EXECUTION_SUBJECT_VERSION_V1,
    };
    use fe2o3_compiler_execution_protocol::{
        CompilerExecutionAttestationChallengeV1, CompilerExecutionAttestationReceiptV1,
        CompilerExecutionAttestationRequestV1, CompilerExecutionIssuerMeasurementV1,
        CompilerExecutionReceiptPublicationAckV1, CompilerExecutionReceiptPublicationV1,
    };
    use rustix::net::{
        AddressFamily, RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, SocketFlags,
        SocketType, socketpair,
    };
    use sha2::{Digest, Sha256};

    use super::*;

    static FIXED_DESCRIPTOR_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    const CHILD_MODE_ENV: &str = "FE2O3_TEST_CHILD_SESSION_MODE";
    const SUBJECT_IDENTITY_DOMAIN: &[u8] = b"FE2O3/INERT-COMPILER-EXECUTION-SUBJECT/V1\0";
    const COMPILER_CLOSURE_IDENTITY_DOMAIN: &[u8] = b"fe2o3-compiler-closure-identity-v2\0";

    #[derive(Clone)]
    struct Fixture {
        policy: CompilerExecutionIssuerPolicyV1,
        subject: InertCompilerExecutionSubjectV1,
        carriage: CompilerExecutionReceiptCarriageV1,
    }

    impl Fixture {
        fn new(seed: u8) -> Self {
            let key = SigningKey::from_bytes(&[seed; 32]);
            let policy = CompilerExecutionIssuerPolicyV1::new(
                u64::from(seed),
                CompilerExecutionIssuerMeasurementV1::new([seed + 1; 32], 123).unwrap(),
                CompilerExecutionIssuerMeasurementV1::new([seed + 2; 32], 456).unwrap(),
                key.verifying_key().to_bytes(),
            )
            .unwrap();
            let subject = subject(seed + 3);
            let challenge = CompilerExecutionAttestationChallengeV1::new(
                &policy,
                &subject,
                [seed + 4; 32],
                1,
                [0; 32],
            )
            .unwrap();
            let request =
                CompilerExecutionAttestationRequestV1::new(challenge, subject.clone()).unwrap();
            let receipt =
                CompilerExecutionAttestationReceiptV1::issue(&policy, &request, &key).unwrap();
            let publication =
                CompilerExecutionReceiptPublicationV1::new([seed + 5; 32], [seed + 6; 32], receipt)
                    .unwrap();
            let acknowledgment =
                CompilerExecutionReceiptPublicationAckV1::new(&publication, [seed + 7; 32])
                    .unwrap();
            let carriage = CompilerExecutionReceiptCarriageV1::new(
                policy.clone(),
                request,
                publication,
                acknowledgment,
            )
            .unwrap();
            Self {
                policy,
                subject,
                carriage,
            }
        }
    }

    fn policy(seed: u8) -> CompilerExecutionIssuerPolicyV1 {
        CompilerExecutionIssuerPolicyV1::new(
            u64::from(seed),
            CompilerExecutionIssuerMeasurementV1::new([seed; 32], 11).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([seed + 1; 32], 13).unwrap(),
            SigningKey::from_bytes(&[seed + 2; 32])
                .verifying_key()
                .to_bytes(),
        )
        .unwrap()
    }

    #[test]
    fn zero_deadline_and_preexisting_reserved_descriptor_fail_before_spawn() {
        let _guard = FIXED_DESCRIPTOR_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut command = Command::new("/bin/sleep");
        command.arg("30");
        let pending = PendingCompilerExecutionChildSessionV1::prepare(
            &mut command,
            CompilerExecutionPolicyCapabilityV1::create(policy(7)).unwrap(),
        )
        .unwrap();
        assert!(matches!(
            pending.finish(1, Duration::ZERO),
            Err(CompilerExecutionChildSessionErrorV1::InvalidTimeout)
        ));

        for (descriptor, expected_policy_error) in [
            (COMPILER_EXECUTION_RECEIPT_RETURN_CHILD_FD_V1, false),
            (COMPILER_EXECUTION_POLICY_CHILD_FD_V1, true),
        ] {
            let source = std::fs::File::open("/dev/null").unwrap();
            // SAFETY: the test serially owns the fixed descriptor and closes it below.
            let installed = unsafe { libc::dup2(source.as_raw_fd(), descriptor) };
            assert_eq!(installed, descriptor);
            let mut command = Command::new("/bin/true");
            let error = PendingCompilerExecutionChildSessionV1::prepare(
                &mut command,
                CompilerExecutionPolicyCapabilityV1::create(policy(7)).unwrap(),
            )
            .unwrap_err();
            assert_eq!(
                matches!(
                    &error,
                    CompilerExecutionChildSessionErrorV1::PolicyCapability(_)
                ),
                expected_policy_error
            );
            if !expected_policy_error {
                assert!(matches!(
                    &error,
                    CompilerExecutionChildSessionErrorV1::ChildChannel(
                        CompilerExecutionChildChannelErrorV1::ReservedDescriptorInUse
                    )
                ));
            }
            // SAFETY: this closes only the descriptor installed by the test.
            assert_eq!(unsafe { libc::close(descriptor) }, 0);
        }
    }

    #[test]
    fn same_uid_production_supervisor_is_rejected_after_exact_child_admission() {
        let _guard = FIXED_DESCRIPTOR_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let current_uid = rustix::process::geteuid().as_raw();
        let current_gid = rustix::process::getegid().as_raw();
        if current_uid == 0 || current_gid == 0 {
            return;
        }
        let mut command = Command::new("/bin/sleep");
        command.arg("30");
        let pending = PendingCompilerExecutionChildSessionV1::prepare(
            &mut command,
            CompilerExecutionPolicyCapabilityV1::create(policy(7)).unwrap(),
        )
        .unwrap();
        let mut child = command.spawn().unwrap();
        let session = pending.finish(child.id(), Duration::from_secs(2)).unwrap();
        assert_eq!(session.client().pid(), child.id());
        let (cargo, _supervisor) = socketpair(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC,
            None,
        )
        .unwrap();
        let credentials =
            CompilerExecutionSupervisorCredentialsV1::new(current_uid, current_gid).unwrap();
        assert!(matches!(
            session.transfer_to_supervisor(cargo, credentials, Duration::from_secs(1)),
            Err(CompilerExecutionChildSessionErrorV1::SupervisorHandoff(
                CompilerExecutionHandoffErrorV1::ClientAndSupervisorUidMatch
            ))
        ));
        child.kill().unwrap();
        child.wait().unwrap();
    }

    #[test]
    fn substituted_readiness_fails_while_receipt_custody_is_retained() {
        let _guard = FIXED_DESCRIPTOR_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let current_uid = rustix::process::geteuid().as_raw();
        let current_gid = rustix::process::getegid().as_raw();
        if current_uid == 0 || current_gid == 0 {
            return;
        }
        let mut command = Command::new("/bin/sleep");
        command.arg("30");
        let pending = PendingCompilerExecutionChildSessionV1::prepare(
            &mut command,
            CompilerExecutionPolicyCapabilityV1::create(policy(7)).unwrap(),
        )
        .unwrap();
        let mut child = command.spawn().unwrap();
        let session = pending.finish(child.id(), Duration::from_secs(2)).unwrap();
        let (cargo, supervisor) = socketpair(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC,
            None,
        )
        .unwrap();
        let credentials =
            CompilerExecutionSupervisorCredentialsV1::new(current_uid, current_gid).unwrap();
        let pending = session
            .transfer_to_supervisor_inner::<false>(cargo, credentials, Duration::from_secs(1))
            .unwrap();
        receive_supervisor_handoff(&supervisor);
        let substituted = policy(8);
        let manifest = CompilerExecutionServiceLaunchManifestV1::new(
            pending.manifest().client(),
            &substituted,
        );
        let readiness = CompilerExecutionServiceReadyV1::new(777, &manifest, &substituted).unwrap();
        assert_eq!(
            rustix::net::send(
                &supervisor,
                readiness.canonical_bytes(),
                rustix::net::SendFlags::NOSIGNAL,
            )
            .unwrap(),
            readiness.canonical_bytes().len()
        );
        drop(supervisor);
        let result = pending.await_readiness(Duration::from_secs(1));
        assert!(
            matches!(
                result,
                Err(CompilerExecutionChildSessionErrorV1::SupervisorHandoff(
                    CompilerExecutionHandoffErrorV1::ReadinessMismatch
                ))
            ),
            "unexpected substituted-readiness result: {result:?}"
        );
        child.kill().unwrap();
        child.wait().unwrap();
    }

    #[test]
    fn prepared_policy_is_installed_at_exact_child_descriptor() {
        let _guard = FIXED_DESCRIPTOR_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let expected = policy(7);
        let mut command = Command::new("/bin/sh");
        command.arg("-c").arg(format!(
            "test $(wc -c </proc/self/fd/{COMPILER_EXECUTION_POLICY_CHILD_FD_V1}) -eq {} && sleep 30",
            fe2o3_compiler_execution_protocol::COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V1,
        ));
        let pending = PendingCompilerExecutionChildSessionV1::prepare(
            &mut command,
            CompilerExecutionPolicyCapabilityV1::create(expected.clone()).unwrap(),
        )
        .unwrap();
        let mut child = command.spawn().unwrap();
        let session = pending.finish(child.id(), Duration::from_secs(2)).unwrap();
        assert_eq!(session.policy(), &expected);
        assert_eq!(session.client().pid(), child.id());
        child.kill().unwrap();
        child.wait().unwrap();
    }

    #[test]
    fn exact_readiness_and_real_child_carriage_complete_one_session() {
        let _guard = FIXED_DESCRIPTOR_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let fixture = Fixture::new(0x20);
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .arg("--exact")
            .arg("child_session::tests::child_session_entry")
            .arg("--nocapture")
            .env(CHILD_MODE_ENV, "exact");
        let pending = PendingCompilerExecutionChildSessionV1::prepare(
            &mut command,
            CompilerExecutionPolicyCapabilityV1::create(fixture.policy.clone()).unwrap(),
        )
        .unwrap();
        let mut child = command.spawn().unwrap();
        let session = pending.finish(child.id(), Duration::from_secs(2)).unwrap();

        let (cargo, supervisor) = socketpair(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC,
            None,
        )
        .unwrap();
        let credentials = CompilerExecutionSupervisorCredentialsV1::new(
            rustix::process::geteuid().as_raw(),
            rustix::process::getegid().as_raw(),
        )
        .unwrap();
        let pending = session
            .transfer_to_supervisor_inner::<false>(cargo, credentials, Duration::from_secs(2))
            .unwrap();
        receive_supervisor_handoff(&supervisor);
        let readiness =
            CompilerExecutionServiceReadyV1::new(777, pending.manifest(), &fixture.policy).unwrap();
        assert_eq!(
            rustix::net::send(
                &supervisor,
                readiness.canonical_bytes(),
                rustix::net::SendFlags::NOSIGNAL,
            )
            .unwrap(),
            readiness.canonical_bytes().len()
        );
        drop(supervisor);
        let ready = pending.await_readiness(Duration::from_secs(2)).unwrap();
        assert_eq!(ready.readiness(), &readiness);
        let completed = ready
            .receive_exact(&fixture.subject, Duration::from_secs(2))
            .unwrap();
        assert_eq!(completed.client().pid(), child.id());
        assert_eq!(completed.readiness(), &readiness);
        assert_eq!(completed.carriage(), &fixture.carriage);
        assert_eq!(completed.into_carriage(), fixture.carriage);
        assert!(child.wait().unwrap().success());
    }

    #[test]
    fn child_session_entry() {
        let Ok(mode) = std::env::var(CHILD_MODE_ENV) else {
            return;
        };
        assert_eq!(mode, "exact");
        let fixture = Fixture::new(0x20);
        let policy = CompilerExecutionPolicyCapabilityV1::from_inherited_child().unwrap();
        assert_eq!(policy.policy(), &fixture.policy);
        std::thread::sleep(Duration::from_millis(100));
        crate::CompilerExecutionReceiptSenderV1::from_inherited_child()
            .unwrap()
            .send_exact(
                &fixture.policy,
                &fixture.subject,
                fixture.carriage,
                Duration::from_secs(2),
            )
            .unwrap();
        std::thread::sleep(Duration::from_millis(100));
    }

    fn receive_supervisor_handoff(supervisor: &OwnedFd) {
        let mut handoff = [0_u8;
            fe2o3_compiler_execution_protocol::COMPILER_EXECUTION_SUPERVISOR_HANDOFF_BYTES_V1];
        let mut vectors = [IoSliceMut::new(&mut handoff)];
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(2))];
        let mut ancillary = RecvAncillaryBuffer::new(&mut space);
        let received = rustix::net::recvmsg(
            supervisor,
            &mut vectors,
            &mut ancillary,
            RecvFlags::CMSG_CLOEXEC,
        )
        .unwrap();
        assert_eq!(received.bytes, handoff.len());
        let transferred: Vec<_> = ancillary
            .drain()
            .flat_map(|message| match message {
                RecvAncillaryMessage::ScmRights(descriptors) => descriptors.collect::<Vec<_>>(),
                _ => Vec::new(),
            })
            .collect();
        assert_eq!(transferred.len(), 2);
    }

    fn subject(seed: u8) -> InertCompilerExecutionSubjectV1 {
        let closure_pins = [
            [seed; 32],
            [seed + 1; 32],
            [seed + 2; 32],
            [seed + 3; 32],
            [seed + 4; 32],
            [seed + 5; 32],
        ];
        let mut closure_digest = Sha256::new();
        closure_digest.update(COMPILER_CLOSURE_IDENTITY_DOMAIN);
        closure_digest.update(1_u16.to_le_bytes());
        for pin in closure_pins {
            closure_digest.update(pin);
        }
        let closure_identity: [u8; 32] = closure_digest.finalize().into();
        let mut bytes = [0_u8; INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1];
        let mut offset = 0;
        put(
            &mut bytes,
            &mut offset,
            &INERT_COMPILER_EXECUTION_SUBJECT_MAGIC_V1,
        );
        put(
            &mut bytes,
            &mut offset,
            &INERT_COMPILER_EXECUTION_SUBJECT_VERSION_V1.to_le_bytes(),
        );
        put(&mut bytes, &mut offset, &0_u16.to_le_bytes());
        put(
            &mut bytes,
            &mut offset,
            &(INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1 as u64).to_le_bytes(),
        );
        put(&mut bytes, &mut offset, &0_u32.to_le_bytes());
        put(&mut bytes, &mut offset, &9_u64.to_le_bytes());
        put(&mut bytes, &mut offset, &[seed + 6; 16]);
        put(&mut bytes, &mut offset, &[seed + 7; 32]);
        bytes[offset] = 0;
        offset += 8;
        put(&mut bytes, &mut offset, &[seed + 8; 32]);
        put(&mut bytes, &mut offset, &[seed + 9; 32]);
        for pin in closure_pins {
            put(&mut bytes, &mut offset, &pin);
        }
        put(&mut bytes, &mut offset, &1_u16.to_le_bytes());
        put(&mut bytes, &mut offset, &closure_identity);
        for axis in 0_u8..7 {
            put(&mut bytes, &mut offset, &[seed + 10 + axis; 32]);
            put(
                &mut bytes,
                &mut offset,
                &(1_000_u64 + u64::from(axis)).to_le_bytes(),
            );
        }
        let identity = digest(SUBJECT_IDENTITY_DOMAIN, &bytes[..offset]);
        put(&mut bytes, &mut offset, &identity);
        assert_eq!(offset, bytes.len());
        InertCompilerExecutionSubjectV1::decode(&bytes).unwrap()
    }

    fn digest(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
        let mut digest = Sha256::new();
        digest.update(domain);
        digest.update((bytes.len() as u64).to_le_bytes());
        digest.update(bytes);
        digest.finalize().into()
    }

    fn put(output: &mut [u8], offset: &mut usize, value: &[u8]) {
        let end = *offset + value.len();
        output[*offset..end].copy_from_slice(value);
        *offset = end;
    }
}
