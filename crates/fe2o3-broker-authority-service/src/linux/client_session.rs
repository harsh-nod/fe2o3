//! Client custody independent of the signing service's private state directory.
use super::*;

/// Original connected compiler peer and live pidfd, without signing-service admission.
///
/// This checks kernel connection credentials and process identity, not compiler provenance
/// or caller privilege. It grants no signing, publication, loading or launch authority.
/// Privileged observers must additionally authenticate their coordinator/session handoff.
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::RetainedCompilerClientSessionV1;
/// fn cloneable<T: Clone>() {}
/// cloneable::<RetainedCompilerClientSessionV1>();
/// ```
/// ```compile_fail
/// use fe2o3_broker_authority_service::RetainedCompilerClientSessionV1;
/// fn descriptor<T: std::os::fd::AsFd>() {}
/// descriptor::<RetainedCompilerClientSessionV1>();
/// ```
pub struct RetainedCompilerClientSessionV1 {
    peer: OwnedFd,
    live_client: LiveClientPidfdIdentityV1,
    peer_identity: ObjectIdentityV1,
}

impl fmt::Debug for RetainedCompilerClientSessionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RetainedCompilerClientSessionV1")
            .field("client", &self.live_client.expected_client)
            .field("authority", &"none")
            .finish_non_exhaustive()
    }
}

impl RetainedCompilerClientSessionV1 {
    /// Consumes the exact retained peer and admitted pidfd. No process is opened by numeric PID.
    pub fn admit(
        peer: OwnedFd,
        live_client: LiveClientPidfdIdentityV1,
    ) -> Result<Self, ProtectedServiceAdmissionErrorV1> {
        live_client.validate_liveness()?;
        require_close_on_exec(
            &peer,
            AdmissionErrorKindV1::PeerCloseOnExec,
            "compiler peer",
        )?;
        let peer_identity = validate_peer_shape(&peer)?;
        if PeerCredentialsV1::inspect(&peer)? != live_client.expected_client.credentials() {
            return Err(ProtectedServiceAdmissionErrorV1::new(
                AdmissionErrorKindV1::PeerCredentialsMismatch,
                "compiler peer credentials do not match the retained client pidfd",
            ));
        }
        let session = Self {
            peer,
            live_client,
            peer_identity,
        };
        session.revalidate()?;
        Ok(session)
    }

    pub fn revalidate(&self) -> Result<(), ProtectedServiceAdmissionErrorV1> {
        validate_client_peer_continuity(&self.peer, &self.live_client, self.peer_identity, None)
    }

    pub const fn client(&self) -> ExpectedClientProcessIdentityV1 {
        self.live_client.expected_client
    }

    pub(crate) const fn process_identity(&self) -> (u32, u64) {
        (self.client().pid, self.live_client.start_time_ticks)
    }

    pub(crate) fn pidfd(&self) -> std::os::fd::BorrowedFd<'_> {
        self.live_client.pidfd.as_fd()
    }

    #[cfg(target_arch = "x86_64")]
    pub(crate) fn retain_process_identity(
        &self,
    ) -> Result<LiveClientPidfdIdentityV1, ProtectedServiceAdmissionErrorV1> {
        self.revalidate()?;
        let retained = self.live_client.try_clone()?;
        self.revalidate()?;
        Ok(retained)
    }

    #[cfg(target_arch = "x86_64")]
    pub(crate) fn retain_session(&self) -> Result<Self, ProtectedServiceAdmissionErrorV1> {
        self.revalidate()?;
        let peer = rustix::io::fcntl_dupfd_cloexec(&self.peer, 0).map_err(|error| {
            ProtectedServiceAdmissionErrorV1::io(
                AdmissionErrorKindV1::InspectPeer,
                "cannot retain original observer client peer",
                io::Error::from(error),
            )
        })?;
        let retained = Self {
            peer,
            live_client: self.live_client.try_clone()?,
            peer_identity: self.peer_identity,
        };
        retained.revalidate()?;
        self.revalidate()?;
        Ok(retained)
    }
}

pub(super) fn validate_client_peer_continuity(
    peer: &OwnedFd,
    live_client: &LiveClientPidfdIdentityV1,
    expected_peer: ObjectIdentityV1,
    distinct_service_uid: Option<u32>,
) -> Result<(), ProtectedServiceAdmissionErrorV1> {
    require_close_on_exec(peer, AdmissionErrorKindV1::PeerCloseOnExec, "service peer")?;
    live_client.validate_liveness()?;
    let current_peer = validate_peer_shape(peer)?;
    if current_peer != expected_peer {
        return Err(ProtectedServiceAdmissionErrorV1::new(
            AdmissionErrorKindV1::PeerIdentityChanged,
            "retained service peer descriptor identity changed",
        ));
    }
    require_distinct_peer_and_pidfd(current_peer, live_client.descriptor_identity)?;
    if distinct_service_uid == Some(live_client.expected_client.uid) {
        return Err(ProtectedServiceAdmissionErrorV1::new(
            AdmissionErrorKindV1::SameUidClient,
            "expected client UID equals protected service effective UID",
        ));
    }
    if PeerCredentialsV1::inspect(peer)? != live_client.expected_client.credentials() {
        return Err(ProtectedServiceAdmissionErrorV1::new(
            AdmissionErrorKindV1::PeerCredentialsChanged,
            "retained service peer SO_PEERCRED no longer matches expected client identity",
        ));
    }
    if ObjectIdentityV1::inspect(peer, AdmissionErrorKindV1::InspectPeer, "peer")? != expected_peer
    {
        return Err(ProtectedServiceAdmissionErrorV1::new(
            AdmissionErrorKindV1::PeerIdentityChanged,
            "retained service peer identity changed while checking credentials",
        ));
    }
    live_client.validate_liveness()
}
