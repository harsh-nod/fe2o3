//! Protected-service admission foundation for future broker authority.
//!
//! This crate is deliberately inert: [`BROKER_AUTHORITY_SERVICE_AUTHORITY_V1`] is `"none"`.
//! Admission retains a supervisor-supplied directory file description, a connected unnamed Unix
//! `SOCK_SEQPACKET` peer, and an opaque client pidfd identity. Every descriptor must have
//! `FD_CLOEXEC`. The linked directory must be owned by the service's effective UID with mode
//! `0700`. Linux `SO_PEERCRED` must exactly match the PID, UID, and GID carried by the pidfd-bound
//! expected identity, and that UID must differ from the service UID. This split assumes a protected
//! supervisor launches the service under a dedicated UID and passes all three descriptors.
//!
//! The held pidfd removes numeric-PID reuse ambiguity for the retained process identity and is
//! polled for point-in-time liveness. The admission also captures the exact Linux procfs
//! `starttime` tick field for that pidfd target and revalidates it before and after liveness checks,
//! so a Broker V4 claim must match both PID and start time. `waitid(P_PIDFD, WNOWAIT)` supplements
//! polling for waitable children and never reaps. Exact PID binding first requests the 64-byte
//! `PIDFD_GET_INFO` v0 ABI.
//! Only `ENOTTY` or `EINVAL` from that exact request dispatches to a fail-closed, 4096-byte
//! `/proc/self/fdinfo/<fd>` inspection. `EINVAL` covers Linux 6.12, whose pidfd ioctl rejects a
//! nonzero argument before checking an unknown command; the errno alone never admits a descriptor.
//! The fallback must independently find exactly one `Pid:` and one octal `flags:` field in a
//! kernel procfs record and rejects `PIDFD_THREAD == O_EXCL`. It also verifies that `/proc/self`
//! and `/proc/<getpid>` are the same process entry in the selected procfs mount; this is a
//! consistent numeric-self mapping, not proof that the mount represents the caller's active PID
//! namespace. A compatible procfs mount is therefore always a trusted precondition for start-time
//! binding, including when the pidfd ioctl succeeds.
//!
//! Liveness is inherently transient: the client can exit immediately after a successful check.
//! `SO_PEERCRED` remains a connection-time credential snapshot, and neither it nor a pidfd proves
//! exclusive ownership of the peer endpoint. Admission cannot attest how the supervisor acquired
//! any descriptor before transfer. The admission value itself exposes no replay registry,
//! reservation, commit, host-link, publication, load, or launch operation. It supplies neither
//! anti-rollback state nor exclusive endpoint ownership; the durable foundation described below
//! does not add either guarantee or grant publication authority.
//!
//! The admission object is neither `Clone` nor `Copy`:
//!
//! ```compile_fail
//! use fe2o3_broker_authority_service::ProtectedBrokerServiceAdmissionV1;
//!
//! fn require_clone<T: Clone>() {}
//! require_clone::<ProtectedBrokerServiceAdmissionV1>();
//! ```
//!
//! ```compile_fail
//! use fe2o3_broker_authority_service::ProtectedBrokerServiceAdmissionV1;
//!
//! fn require_copy<T: Copy>() {}
//! require_copy::<ProtectedBrokerServiceAdmissionV1>();
//! ```
//!
//! The live-client token is also move-only, has no raw descriptor API, and implements no Serde
//! serialization trait:
//!
//! ```compile_fail
//! use fe2o3_broker_authority_service::LiveClientPidfdIdentityV1;
//!
//! fn require_clone<T: Clone>() {}
//! require_clone::<LiveClientPidfdIdentityV1>();
//! ```
//!
//! ```compile_fail
//! use std::os::fd::AsFd;
//! use fe2o3_broker_authority_service::LiveClientPidfdIdentityV1;
//!
//! fn require_as_fd<T: AsFd>() {}
//! require_as_fd::<LiveClientPidfdIdentityV1>();
//! ```
//!
//! ```compile_fail
//! use std::os::fd::IntoRawFd;
//! use fe2o3_broker_authority_service::LiveClientPidfdIdentityV1;
//!
//! fn require_into_raw_fd<T: IntoRawFd>() {}
//! require_into_raw_fd::<LiveClientPidfdIdentityV1>();
//! ```
//!
//! ```compile_fail
//! use fe2o3_broker_authority_service::LiveClientPidfdIdentityV1;
//!
//! fn require_serialize<T: serde::Serialize>() {}
//! require_serialize::<LiveClientPidfdIdentityV1>();
//! ```
//!
//! The [`BrokerSessionMachineV1`] is a separate fixed-capacity, in-memory lifecycle model. Its
//! broker-owned route retains prepared and granted Broker V4 state, requires the V4 static-LLD
//! identity to match the exact W0 closure, invokes only an externally approved static linker, and
//! keeps the authenticated execution and admitted output inside the service through terminal
//! commit. It then owns one external-anchor verification token through a logical consume or abort
//! decision. Anchor preparation consumes the machine into an opaque,
//! move-only [`BrokerAnchorPreparedSessionV1`] before the service attempt nonce, transaction, or
//! challenge exists. Durable preparation stages exact W0, obtains Linux `getrandom` entropy, and
//! forms the nonce-bound challenge internally. [`BrokerDurableSessionTransactionV1`] provides the
//! first live challenge-byte accessor only after the canonical prepared record is durable. A valid
//! signed proposed-position observation can then precede the exact-mode final rename and published
//! record. Restart can re-emit the challenge only through a validated
//! [`BrokerRecoveredPreparedSessionV1`]. Recovery distinguishes prepared, anchor-committed,
//! published, aborted, and invalid records. It remains `AUTHORITY=none`: this is not anti-rollback
//! storage, cross-system atomicity, key provenance, multi-writer exclusion, trusted tool-evidence
//! approval, publication authority, or runtime authority.
//!
#[cfg(not(target_os = "linux"))]
compile_error!(
    "fe2o3-broker-authority-service requires Linux descriptor and SO_PEERCRED semantics"
);

#[cfg(target_os = "linux")]
mod durable_session_consume;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
mod session;
#[cfg(test)]
mod test_process_execution {
    use std::io;
    use std::process::{Child, Command, ExitStatus, Output, Stdio};

    pub(crate) fn spawn(command: &mut Command) -> io::Result<Child> {
        fe2o3_artifact_transaction::with_artifact_process_spawn_v1(|| command.spawn())
    }

    pub(crate) fn status(command: &mut Command) -> io::Result<ExitStatus> {
        spawn(command)?.wait()
    }

    pub(crate) fn capture_output(command: &mut Command) -> io::Result<Output> {
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        spawn(command)?.wait_with_output()
    }
}

#[cfg(target_os = "linux")]
pub use durable_session_consume::{
    BROKER_DURABLE_SESSION_AUTHORITY_V1, BrokerDurableFaultPointV1, BrokerDurableOptionsV1,
    BrokerDurableOutcomeV1, BrokerDurableRecordStageV1, BrokerDurableRecoveryV1,
    BrokerDurableSessionErrorV1, BrokerDurableSessionTransactionV1,
    BrokerRecoveredPreparedSessionV1, DurableBrokerPublicationPlanV1,
    MAX_BROKER_DURABLE_OUTPUT_BYTES_V1, MAX_BROKER_DURABLE_RECORD_BYTES_V1,
    inspect_durable_broker_session_v1, prepare_durable_broker_session_v1,
    prepare_durable_broker_session_v1_with_options, recover_durable_broker_session_v1,
    recover_durable_broker_session_v1_with_options, recover_prepared_durable_broker_session_v1,
};
#[cfg(target_os = "linux")]
pub use linux::{
    AdmissionErrorKindV1, BrokerAuthorityServiceAdmissionErrorV1, ExpectedClientProcessIdentityV1,
    LiveClientPidfdIdentityV1, ProtectedBrokerServiceAdmissionV1,
};
#[cfg(target_os = "linux")]
pub use session::{
    BROKER_LINK_RESERVATION_DIGEST_DOMAIN_V1, BROKER_SESSION_CAPACITY_V1,
    BROKER_SESSION_MACHINE_AUTHORITY_V1, BROKER_V4_COMPLETED_TRANSCRIPT_DIGEST_DOMAIN_V1,
    BrokerAnchorModeV1, BrokerAnchorPreparedSessionV1, BrokerCompletedHostLinkV1,
    BrokerHostLinkPermitV1, BrokerHostLinkPollV1, BrokerHostOutputObservationV1,
    BrokerOwnedHostLinkExecutionV1, BrokerReservedHostLinkSessionV1, BrokerSessionErrorKindV1,
    BrokerSessionIdV1, BrokerSessionMachineErrorV1, BrokerSessionMachineV1, BrokerSessionNonceV1,
    BrokerSessionObservationV1, BrokerSessionReservationV1, BrokerSessionStageV1,
    CommittedBrokerPublicationV1, DurablePublicationPlanIdentityV1,
    completed_broker_transcript_digest_v1,
};

/// This foundation grants no independent tool approval, persistence, publication, or GPU authority.
pub const BROKER_AUTHORITY_SERVICE_AUTHORITY_V1: &str = "none";
