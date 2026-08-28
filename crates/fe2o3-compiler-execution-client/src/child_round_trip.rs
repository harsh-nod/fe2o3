//! Exact compiler child acquisition and receipt-return custody.

use std::error::Error;
use std::fmt;
use std::time::{Duration, Instant};

use fe2o3_artifact_transaction::InertCompilerExecutionSubjectV1;
use fe2o3_compiler_closure_capability::CompilerExecutionPolicyCapabilityV1;

use crate::{
    CompilerExecutionClientErrorV1, CompilerExecutionClientV1,
    CompilerExecutionReceiptReturnErrorV1, CompilerExecutionReceiptSenderV1,
};

/// Acquires and returns the exact receipt carriage for one already-published compiler occurrence.
///
/// The caller must supply the subject derived from the strict V3 handoff publication in the same
/// rustc process. This function derives no subject or policy from arguments, paths, or the
/// environment. It consumes inherited policy FD 202, service FD 195, and receipt-return FD 196
/// before beginning the issuer exchange; every present canonical slot is closed even if another
/// slot fails admission. One absolute monotonic deadline bounds admission, acquisition including
/// durable issuer side effects, and exact return.
pub fn acquire_and_return_inherited_compiler_execution_v1(
    subject: InertCompilerExecutionSubjectV1,
    timeout: Duration,
) -> Result<(), CompilerExecutionChildRoundTripErrorV1> {
    let deadline = deadline(timeout);
    // Invalid timeout input still consumes every present one-use canonical descriptor.
    let admission_deadline = deadline
        .as_ref()
        .copied()
        .unwrap_or_else(|_| Instant::now());
    let admissions = admit_inherited_round_trip(admission_deadline);
    let deadline = deadline?;
    let (policy, client, sender) = admissions?;

    require_deadline(deadline)?;
    policy
        .revalidate()
        .map_err(CompilerExecutionChildRoundTripErrorV1::PolicyCapability)?;
    require_deadline(deadline)?;
    let carriage = client
        .acquire(policy.policy(), subject.clone())
        .map_err(CompilerExecutionChildRoundTripErrorV1::Client)?;
    require_deadline(deadline)?;
    policy
        .revalidate()
        .map_err(CompilerExecutionChildRoundTripErrorV1::PolicyCapability)?;
    require_deadline(deadline)?;
    sender
        .send_exact_until(policy.policy(), &subject, carriage, deadline)
        .map_err(CompilerExecutionChildRoundTripErrorV1::ReceiptReturn)?;
    require_deadline(deadline)?;
    policy
        .revalidate()
        .map_err(CompilerExecutionChildRoundTripErrorV1::PolicyCapability)?;
    require_deadline(deadline)
}

fn admit_inherited_round_trip(
    deadline: Instant,
) -> Result<
    (
        CompilerExecutionPolicyCapabilityV1,
        CompilerExecutionClientV1,
        CompilerExecutionReceiptSenderV1,
    ),
    CompilerExecutionChildRoundTripErrorV1,
> {
    // Do not return early: each admission consumes its canonical slot on both success and failure.
    let policy = CompilerExecutionPolicyCapabilityV1::take_inherited_child()
        .map_err(CompilerExecutionChildRoundTripErrorV1::PolicyCapability);
    let client = CompilerExecutionClientV1::admit_inherited_child_until(deadline)
        .map_err(CompilerExecutionChildRoundTripErrorV1::Client);
    let sender = CompilerExecutionReceiptSenderV1::from_inherited_child_until(deadline)
        .map_err(CompilerExecutionChildRoundTripErrorV1::ReceiptReturn);
    Ok((policy?, client?, sender?))
}

fn deadline(timeout: Duration) -> Result<Instant, CompilerExecutionChildRoundTripErrorV1> {
    if timeout.is_zero() {
        return Err(CompilerExecutionChildRoundTripErrorV1::InvalidTimeout);
    }
    Instant::now()
        .checked_add(timeout)
        .ok_or(CompilerExecutionChildRoundTripErrorV1::DeadlineOverflow)
}

fn require_deadline(deadline: Instant) -> Result<(), CompilerExecutionChildRoundTripErrorV1> {
    if deadline.saturating_duration_since(Instant::now()).is_zero() {
        Err(CompilerExecutionChildRoundTripErrorV1::Timeout)
    } else {
        Ok(())
    }
}

/// Stable failure for exact inherited compiler-execution acquisition and return.
#[derive(Debug)]
#[non_exhaustive]
pub enum CompilerExecutionChildRoundTripErrorV1 {
    /// The complete round-trip timeout was zero.
    InvalidTimeout,
    /// The complete round-trip deadline could not be represented.
    DeadlineOverflow,
    /// The complete round-trip deadline expired between custody stages.
    Timeout,
    /// The inherited sealed policy failed admission, consumption, or revalidation.
    PolicyCapability(String),
    /// The inherited compiler service failed admission or the exact acquisition failed.
    Client(CompilerExecutionClientErrorV1),
    /// The inherited return endpoint failed admission or exact-carriage return failed.
    ReceiptReturn(CompilerExecutionReceiptReturnErrorV1),
}

impl fmt::Display for CompilerExecutionChildRoundTripErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTimeout => {
                formatter.write_str("compiler-execution round-trip timeout must be nonzero")
            }
            Self::DeadlineOverflow => {
                formatter.write_str("compiler-execution round-trip deadline overflowed")
            }
            Self::Timeout => {
                formatter.write_str("compiler-execution round-trip absolute deadline expired")
            }
            Self::PolicyCapability(error) => {
                write!(
                    formatter,
                    "compiler-execution round-trip policy failed: {error}"
                )
            }
            Self::Client(error) => {
                write!(
                    formatter,
                    "compiler-execution round-trip acquire failed: {error}"
                )
            }
            Self::ReceiptReturn(error) => {
                write!(
                    formatter,
                    "compiler-execution round-trip return failed: {error}"
                )
            }
        }
    }
}

impl Error for CompilerExecutionChildRoundTripErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Client(error) => Some(error),
            Self::ReceiptReturn(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};

    use ed25519_dalek::SigningKey;
    use fe2o3_compiler_closure_capability::COMPILER_EXECUTION_POLICY_CHILD_FD_V1;
    use fe2o3_compiler_execution_protocol::{
        CompilerExecutionIssuerMeasurementV1, CompilerExecutionIssuerPolicyV1,
    };

    use super::*;
    use crate::{
        COMPILER_EXECUTION_RECEIPT_RETURN_CHILD_FD_V1, COMPILER_EXECUTION_SERVICE_CHILD_FD_V1,
        FIXED_DESCRIPTOR_TEST_LOCK,
    };

    #[derive(Clone, Copy)]
    enum RejectedSlot {
        Policy,
        Service,
        Receipt,
    }

    #[derive(Clone, Copy)]
    enum Rejection {
        WrongObject,
        CloseOnExec,
    }

    #[test]
    fn every_present_canonical_descriptor_is_consumed_on_each_admission_rejection() {
        let _guard = FIXED_DESCRIPTOR_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for slot in [
            RejectedSlot::Policy,
            RejectedSlot::Service,
            RejectedSlot::Receipt,
        ] {
            for rejection in [Rejection::WrongObject, Rejection::CloseOnExec] {
                let installed = install_valid_canonical_descriptors();
                match rejection {
                    Rejection::WrongObject => {
                        let ordinary = File::open("/dev/null").unwrap();
                        install_at(ordinary.as_raw_fd(), slot.descriptor(), false);
                    }
                    Rejection::CloseOnExec => set_close_on_exec(slot.descriptor()),
                }

                let result = admit_inherited_round_trip(
                    Instant::now().checked_add(Duration::from_secs(1)).unwrap(),
                );
                assert!(slot.matches(&result));
                assert_canonical_descriptors_closed();
                drop(installed);
            }
        }
    }

    #[test]
    fn expired_admission_deadline_still_consumes_every_canonical_descriptor() {
        let _guard = FIXED_DESCRIPTOR_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let installed = install_valid_canonical_descriptors();
        assert!(matches!(
            admit_inherited_round_trip(Instant::now()),
            Err(CompilerExecutionChildRoundTripErrorV1::Client(
                CompilerExecutionClientErrorV1::Timeout
            ))
        ));
        assert_canonical_descriptors_closed();
        drop(installed);
    }

    impl RejectedSlot {
        const fn descriptor(self) -> RawFd {
            match self {
                Self::Policy => COMPILER_EXECUTION_POLICY_CHILD_FD_V1,
                Self::Service => COMPILER_EXECUTION_SERVICE_CHILD_FD_V1,
                Self::Receipt => COMPILER_EXECUTION_RECEIPT_RETURN_CHILD_FD_V1,
            }
        }

        fn matches(
            self,
            result: &Result<
                (
                    CompilerExecutionPolicyCapabilityV1,
                    CompilerExecutionClientV1,
                    CompilerExecutionReceiptSenderV1,
                ),
                CompilerExecutionChildRoundTripErrorV1,
            >,
        ) -> bool {
            match self {
                Self::Policy => matches!(
                    result,
                    Err(CompilerExecutionChildRoundTripErrorV1::PolicyCapability(_))
                ),
                Self::Service => matches!(
                    result,
                    Err(CompilerExecutionChildRoundTripErrorV1::Client(_))
                ),
                Self::Receipt => matches!(
                    result,
                    Err(CompilerExecutionChildRoundTripErrorV1::ReceiptReturn(_))
                ),
            }
        }
    }

    struct InstalledDescriptors {
        _policy: CompilerExecutionPolicyCapabilityV1,
        _service: (OwnedFd, OwnedFd),
        _receipt: (OwnedFd, OwnedFd),
    }

    impl Drop for InstalledDescriptors {
        fn drop(&mut self) {
            close_canonical_descriptors();
        }
    }

    fn install_valid_canonical_descriptors() -> InstalledDescriptors {
        close_canonical_descriptors();
        let policy = CompilerExecutionPolicyCapabilityV1::create(policy()).unwrap();
        let policy_image = policy.try_clone_for_transfer().unwrap();
        let service = socket_pair();
        let receipt = socket_pair();
        install_at(
            policy_image.as_raw_fd(),
            COMPILER_EXECUTION_POLICY_CHILD_FD_V1,
            false,
        );
        install_at(
            service.0.as_raw_fd(),
            COMPILER_EXECUTION_SERVICE_CHILD_FD_V1,
            false,
        );
        install_at(
            receipt.0.as_raw_fd(),
            COMPILER_EXECUTION_RECEIPT_RETURN_CHILD_FD_V1,
            false,
        );
        InstalledDescriptors {
            _policy: policy,
            _service: service,
            _receipt: receipt,
        }
    }

    fn install_at(source: RawFd, target: RawFd, close_on_exec: bool) {
        // Retain a temporary above the reserved range so replacing `target` cannot alias `source`.
        // SAFETY: F_DUPFD_CLOEXEC returns one independent descriptor or reports failure.
        let temporary = unsafe { libc::fcntl(source, libc::F_DUPFD_CLOEXEC, 256) };
        assert!(temporary >= 256);
        // SAFETY: the fixed-descriptor lock serializes these canonical slots for this test binary,
        // and `temporary` is outside that range.
        assert_eq!(
            unsafe {
                libc::dup3(
                    temporary,
                    target,
                    if close_on_exec { libc::O_CLOEXEC } else { 0 },
                )
            },
            target
        );
        // SAFETY: successful F_DUPFD_CLOEXEC returned this test-owned temporary descriptor.
        assert_eq!(unsafe { libc::close(temporary) }, 0);
    }

    fn set_close_on_exec(descriptor: RawFd) {
        // SAFETY: the descriptor fixture owns the present canonical slot.
        assert_eq!(
            unsafe { libc::fcntl(descriptor, libc::F_SETFD, libc::FD_CLOEXEC) },
            0
        );
    }

    fn assert_canonical_descriptors_closed() {
        for descriptor in canonical_descriptors() {
            // SAFETY: F_GETFD inspects only the fixed scalar descriptor.
            assert_eq!(unsafe { libc::fcntl(descriptor, libc::F_GETFD) }, -1);
            assert_eq!(
                std::io::Error::last_os_error().raw_os_error(),
                Some(libc::EBADF)
            );
        }
    }

    fn close_canonical_descriptors() {
        for descriptor in canonical_descriptors() {
            // SAFETY: the fixed-descriptor lock gives this fixture exclusive ownership.
            let _ = unsafe { libc::close(descriptor) };
        }
    }

    const fn canonical_descriptors() -> [RawFd; 3] {
        [
            COMPILER_EXECUTION_POLICY_CHILD_FD_V1,
            COMPILER_EXECUTION_SERVICE_CHILD_FD_V1,
            COMPILER_EXECUTION_RECEIPT_RETURN_CHILD_FD_V1,
        ]
    }

    fn socket_pair() -> (OwnedFd, OwnedFd) {
        let mut descriptors = [-1; 2];
        // SAFETY: successful socketpair initializes both output descriptor slots.
        assert_eq!(
            unsafe {
                libc::socketpair(
                    libc::AF_UNIX,
                    libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC,
                    0,
                    descriptors.as_mut_ptr(),
                )
            },
            0
        );
        // SAFETY: successful socketpair returned two independently owned descriptors.
        unsafe {
            (
                OwnedFd::from_raw_fd(descriptors[0]),
                OwnedFd::from_raw_fd(descriptors[1]),
            )
        }
    }

    fn policy() -> CompilerExecutionIssuerPolicyV1 {
        let key = SigningKey::from_bytes(&[0x51; 32]);
        CompilerExecutionIssuerPolicyV1::new(
            7,
            CompilerExecutionIssuerMeasurementV1::new([0x61; 32], 123).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([0x62; 32], 456).unwrap(),
            key.verifying_key().to_bytes(),
        )
        .unwrap()
    }
}
