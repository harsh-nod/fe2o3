use super::*;
use crate::{application::Capsule, deployment::ProductionApplicationProofCustodianDeploymentV1};
use fe2o3_runtime_protocol::{
    WorkerV3ApplicationProofSessionV1 as Session,
    WorkerV3ApplicationRegistrationBindingV1 as Binding,
    WorkerV3ApplicationSessionTranscriptV1 as Transcript,
};

#[cfg(test)]
#[allow(unsafe_code)]
mod qualification;

/// Pending fixed application-controller resource admission, not authenticated registration.
/// Callers must supply the exact peer and original pidfds from registered root custody.
///
/// ```compile_fail
/// use fe2o3_proof_custodian::PendingRootApplicationProofControllerV1;
/// fn sendable<T: Send>() {}
/// sendable::<PendingRootApplicationProofControllerV1>();
/// ```
pub struct PendingRootApplicationProofControllerV1 {
    pending: PendingRootProofControllerLaunchV1,
    session: Session,
}
impl ProductionApplicationProofCustodianDeploymentV1 {
    /// Consumes the manager's staging aliases; only the fixed child inherits duplicates.
    /// The registering coordinator still owns its alias until sending Ready. It must
    /// drop that alias without shutdown before calling the staged owner's activation.
    pub fn begin_application(
        self,
        binding: Binding,
        transcript: Transcript,
        application_pidfd: OwnedFd,
        cargo_pidfd: OwnedFd,
        proof_peer: OwnedFd,
    ) -> io::Result<PendingRootApplicationProofControllerV1> {
        self.revalidate()?;
        let deadline = Instant::now() + EXECUTION_TIMEOUT;
        let peer = wire::ControlEndpoint::admit(proof_peer)?;
        let capsule =
            Capsule::capture(binding, transcript, &application_pidfd, &cargo_pidfd, &peer)?;
        let request = wire::seal(&capsule.encode())?;
        let credentials = self.0.config.credentials()?;
        let deployment = self.0.config.identity();
        let mut pending = self.0.begin_descriptor_launch(
            capsule.nonce,
            request.as_fd(),
            &[application_pidfd.as_fd(), cargo_pidfd.as_fd(), peer.as_fd()],
            deadline,
        )?;
        let pid = pending.controller.as_ref().unwrap().child_pid();
        let session = Session::new(
            transcript,
            deployment,
            capsule.nonce,
            (pid, credentials.uid(), credentials.gid()),
        )
        .map_err(other)?;
        pending.expected_ready = ExpectedReady::new(session.canonical_bytes());
        // All parent-side staging aliases are dropped before this pending owner is returned.
        Ok(PendingRootApplicationProofControllerV1 { pending, session })
    }
}
impl PendingRootApplicationProofControllerV1 {
    pub fn poll(&mut self) -> io::Result<bool> {
        self.pending.poll()
    }
    pub fn poll_cancel(&mut self) -> io::Result<bool> {
        self.pending.poll_cancel()
    }
    pub fn take_ready(&mut self) -> io::Result<Option<RootStagedApplicationProofControllerV1>> {
        Ok(self
            .pending
            .take_ready()?
            .map(|controller| RootStagedApplicationProofControllerV1 {
                controller,
                session: self.session.clone(),
                activation_sent: false,
                activated: false,
                proof_observed: false,
                probe_deadline: None,
                probe_sent: false,
            }))
    }
}

/// Root lifecycle custody for one staged/activated application controller.
///
/// After any possibly delivered Activate, this owner cannot cancel, release or be
/// silently dropped. Drop fail-stops its manager; deployment must kill the entire
/// manager cgroup on death. Keep active custody until a future native-settlement
/// owner consumes it. App EOF and pidfd death never establish GPU settlement.
/// This transport increment supplies no native-settlement release API.
///
/// ```compile_fail
/// use fe2o3_proof_custodian::RootStagedApplicationProofControllerV1;
/// fn cloneable<T: Clone>() {}
/// cloneable::<RootStagedApplicationProofControllerV1>();
/// ```
/// ```compile_fail
/// use fe2o3_proof_custodian::RootStagedApplicationProofControllerV1;
/// fn sendable<T: Send>() {}
/// sendable::<RootStagedApplicationProofControllerV1>();
/// ```
pub struct RootStagedApplicationProofControllerV1 {
    controller: RootManagedProofControllerV1,
    session: Session,
    activation_sent: bool,
    activated: bool,
    proof_observed: bool,
    probe_deadline: Option<Instant>,
    probe_sent: bool,
}
impl RootStagedApplicationProofControllerV1 {
    /// Inert root Ready payload; its identity alone is not an approved custodian.
    pub fn session(&self) -> &Session {
        &self.session
    }
    pub fn try_clone_pidfd(&self) -> io::Result<OwnedFd> {
        self.controller.try_clone_pidfd()
    }
    pub fn revalidate(&self) -> io::Result<()> {
        self.controller.revalidate()
    }
    /// The registering root must have sent Ready and dropped every receive alias.
    /// This is a root ownership invariant, not something a socket syscall can prove.
    pub fn poll_activate(&mut self) -> io::Result<bool> {
        let result = self.activate_inner();
        if result.is_err() {
            self.controller.poisoned = true;
        }
        result
    }
    fn activate_inner(&mut self) -> io::Result<bool> {
        self.controller.revalidate()?;
        if self.activated {
            return Ok(true);
        }
        check_deadline(self.controller.deadline)?;
        if !self.activation_sent {
            // A send error is ambiguous; retain fail-stop custody unless no frame was sent.
            self.activation_sent = true;
            if !wire::try_send(
                self.controller.control.as_fd(),
                self.controller.nonce,
                wire::ACTIVATE,
                &self.session.identity(),
            )? {
                self.activation_sent = false;
            }
            return Ok(false);
        }
        let Some((kind, body)) = self.controller.try_receive()? else {
            return Ok(false);
        };
        require(
            kind == wire::ACTIVATED && body == self.session.identity(),
            "application activation acknowledgment mismatch",
        )?;
        self.activated = true;
        Ok(true)
    }
    /// Available only before Activate could have been delivered.
    pub fn poll_cancel(&mut self) -> io::Result<bool> {
        require(
            !self.activation_sent,
            "activated application proof custody cannot be cancelled as settlement",
        )?;
        self.controller.poll_cancel()
    }
    /// Reports recomputed original proof matching bytes, even after app quarantine.
    /// This observation confers neither a remote proof lease nor GPU authority.
    pub fn poll_probe(&mut self) -> io::Result<Option<(Vec<u8>, bool)>> {
        let result = self.probe_inner();
        if result.is_err() {
            self.controller.poisoned = true;
        }
        result
    }
    fn probe_inner(&mut self) -> io::Result<Option<(Vec<u8>, bool)>> {
        require(self.activated, "application controller not activated")?;
        self.controller.revalidate()?;
        // The worker cannot answer root probes during synchronous analysis/proving.
        // Only already-observed retained custody uses the shorter control bound.
        let deadline = *self.probe_deadline.get_or_insert_with(|| {
            if self.proof_observed {
                Instant::now() + CONTROL_TIMEOUT
            } else {
                self.controller.deadline
            }
        });
        check_deadline(deadline)?;
        if !self.probe_sent {
            self.probe_sent = wire::try_send(
                self.controller.control.as_fd(),
                self.controller.nonce,
                wire::PROBE,
                &[],
            )?;
            return Ok(None);
        }
        let Some((kind, body)) = self.controller.try_receive()? else {
            return Ok(None);
        };
        if kind == wire::REJECTED {
            return Err(io::Error::other(
                String::from_utf8_lossy(&body).into_owned(),
            ));
        }
        require(
            matches!(kind, wire::RETAINED | wire::QUARANTINED) && !body.is_empty(),
            "application proof retention response mismatch",
        )?;
        self.probe_sent = false;
        self.probe_deadline = None;
        self.proof_observed = true;
        Ok(Some((body, kind == wire::QUARANTINED)))
    }
    #[cfg(test)]
    pub(super) fn contain_qualification(&mut self) -> io::Result<()> {
        self.controller.cancel()?;
        self.activation_sent = false;
        Ok(())
    }
}
impl Drop for RootStagedApplicationProofControllerV1 {
    fn drop(&mut self) {
        if self.activation_sent {
            std::process::abort();
        }
    }
}
