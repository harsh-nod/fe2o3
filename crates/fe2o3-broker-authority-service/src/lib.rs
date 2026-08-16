//! Protected-service admission foundation for future broker authority.
//!
//! This crate is deliberately inert: [`BROKER_AUTHORITY_SERVICE_AUTHORITY_V1`] is `"none"`.
//! Admission retains a supervisor-supplied directory file description and a connected, unnamed
//! Unix `SOCK_SEQPACKET` peer. Both descriptors must have `FD_CLOEXEC`. The linked directory must
//! be owned by the service's effective UID with mode `0700`. Linux `SO_PEERCRED` must exactly match
//! the supervisor-supplied expected client PID, UID, and GID, and that UID must differ from the
//! service UID. This split assumes a protected supervisor launches the service under a dedicated
//! UID and passes both descriptors and the expected connection-time client identity.
//!
//! `SO_PEERCRED` is a connection-time credential snapshot. It does not prove that the named PID is
//! still live, that the PID has not been reused, or that the client exclusively owns its endpoint.
//! Admission itself does not resolve a path, but it cannot attest how the supervisor acquired the
//! directory descriptor before transfer. No replay registry, reservation, commit, host-link,
//! publication, load, or launch operation is exposed. Anti-rollback state, live process identity,
//! exclusive endpoint ownership, and atomic admitted-output publication remain future work.
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
#[cfg(not(target_os = "linux"))]
compile_error!(
    "fe2o3-broker-authority-service requires Linux descriptor and SO_PEERCRED semantics"
);

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "linux")]
pub use linux::{
    AdmissionErrorKindV1, BrokerAuthorityServiceAdmissionErrorV1, ExpectedClientProcessIdentityV1,
    ProtectedBrokerServiceAdmissionV1,
};

/// This foundation grants no execution, persistence, publication, or launch authority.
pub const BROKER_AUTHORITY_SERVICE_AUTHORITY_V1: &str = "none";
