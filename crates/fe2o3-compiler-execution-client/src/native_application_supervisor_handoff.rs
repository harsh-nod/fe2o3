//! Native application transfer and two-phase authenticated startup completion.
use super::*;
use crate::{
    ApplicationProofTransferPeerV1 as ProofPeer, RetainedApplicationServiceLaunchV1 as Launch,
};
use fe2o3_runtime_protocol::{
    NativeApplicationRegistrationBindingV1 as Binding, NativeApplicationRootTransferV1 as Transfer,
    NativeApplicationStartupKindV1 as StartupKind, NativeApplicationStartupRecordV1 as Startup,
    WorkerV3ApplicationInputOccurrenceV1 as Occurrence,
};
use rustix::{
    fs::{FileType, OFlags},
    io::{FdFlags, fcntl_getfd},
};

/// Original native registration, issuer readiness and root-accepted application ACK.
/// No process descriptor or live currentness, proof, publication or GPU authority
/// can be extracted. The actual application keeps those separate native owners.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_client::NativeApplicationSupervisorReadinessV1 as R;
/// fn clone<T: Clone>() {} clone::<R>();
/// ```
pub struct NativeApplicationSupervisorReadinessV1 {
    binding: Binding,
    ready: Ready,
    startup: Startup,
}
impl NativeApplicationSupervisorReadinessV1 {
    pub const fn binding(&self) -> &Binding {
        &self.binding
    }
    pub const fn readiness(&self) -> &Ready {
        &self.ready
    }
    pub const fn startup(&self) -> &Startup {
        &self.startup
    }
    pub fn retained_storage(&self) -> usize {
        self.binding.retained_storage()
            + self.ready.retained_storage()
            + self.startup.retained_storage()
            + size_of::<(Self, Storage)>()
            - size_of::<Binding>()
            - size_of::<Ready>()
            - size_of::<Startup>()
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

impl Launch {
    /// Consumes the original service/application/proof/Cargo custody and sole
    /// startup ACK reader. The caller must not retain an ACK reader alias.
    ///
    /// Sends native binding904/four rights, then binding-bound reader40/one right;
    /// closes its reader and sends closure40/zero rights. Receives genuine native
    /// ServiceReady120, then root-accepted ACK208, and only then terminal EOF.
    /// Every transport phase makes one finite attempt on the same absolute
    /// deadline and original mutable budget. There is no old-family fallback.
    ///
    /// Prepay binding, profile and `NATIVE_APPLICATION_INPUT_STORAGE`. Success
    /// returns only additional storage above the consumed launch/proof/reader/
    /// binding inputs; keep that input reservation and add returned growth.
    /// Failure closes all owned descriptors and does not imply ACK acceptance.
    /// The profile's protected provenance remains the caller's obligation.
    #[allow(clippy::too_many_arguments)]
    pub fn handoff_native_application_until(
        self,
        proof: ProofPeer,
        acknowledgment_reader: OwnedFd,
        binding: Binding,
        profile: &Profile,
        deadline: Instant,
        budget: &mut Budget<'_>,
    ) -> Result<(NativeApplicationSupervisorReadinessV1, Storage)> {
        let inherited = sum(
            Self::NATIVE_APPLICATION_INPUT_STORAGE,
            binding.retained_storage(),
        )?;
        let floor = sum(inherited, profile.retained_storage())?;
        budget.with_prepaid_scope(floor, 8, 512 * 1024, 128 * 1024, |budget| {
            super::super::validate_boundary_deadline(deadline)?;
            let expected = credentials(profile)?;
            if self.client().uid() == expected.uid() {
                return Err(Failure::Mismatch("native application and supervisor UIDs"));
            }
            self.revalidate()?;
            let (manifest, charge) = Manifest::new(
                self.client(),
                profile.external_anchor_service(),
                profile.policy(),
                budget,
            )?;
            budget.reserve_storage(charge.additional_storage())?;
            let (handoff, charge) = Handoff::new(self.submitter(), manifest, budget)?;
            budget.reserve_storage(charge.additional_storage())?;
            if binding.compiler_handoff().canonical_bytes() != handoff.canonical_bytes() {
                return Err(Failure::Mismatch("native application original handoff"));
            }
            validate_reader(&acknowledgment_reader, &binding.occurrence().inputs()[2])?;
            let proof = proof.into_native_registration_descriptor(&binding)?;
            let control = io::connect_application(expected, deadline)?;
            rustix::net::sockopt::set_socket_passcred(&control, true)?;
            self.revalidate()?;
            let (service, application) = self.compiler.into_descriptors();
            io::send_application(
                &control,
                binding.canonical_bytes(),
                &[
                    service.as_fd(),
                    application.as_fd(),
                    proof.as_fd(),
                    self.parent_pidfd.as_fd(),
                ],
                deadline,
            )?;
            let reader_frame = Transfer::ack_reader_packet(*binding.identity().as_bytes(), budget)?;
            io::send_application(
                &control,
                &reader_frame,
                &[acknowledgment_reader.as_fd()],
                deadline,
            )?;
            drop(acknowledgment_reader);
            let closed =
                Transfer::ack_reader_closed_packet(*binding.identity().as_bytes(), budget)?;
            io::send_application(&control, &closed, &[], deadline)?;
            drop((service, application, proof, self.parent_pidfd));
            io::validate_application(&control, expected)?;
            let bytes = io::readiness(&control, deadline)?;
            budget.reserve_storage(bytes.len())?;
            let (ready, charge) = Ready::decode(&bytes, budget)?;
            budget.reserve_storage(charge.additional_storage())?;
            if !ready.matches_launch(
                ready.issuer_pid(),
                binding.compiler_handoff().launch_manifest(),
                profile.policy(),
                budget,
            )? {
                return Err(Failure::Mismatch("native application issuer readiness"));
            }
            let bytes = io::exact_record::<
                { fe2o3_runtime_protocol::NATIVE_APPLICATION_STARTUP_BYTES_V1 },
            >(&control, deadline)?;
            budget.reserve_storage(bytes.len())?;
            let (startup, charge) = Startup::decode(&bytes, budget)?;
            budget.reserve_storage(charge.additional_storage())?;
            if startup.kind() != StartupKind::Acknowledgment
                || startup.transcript().binding() != *binding.identity().as_bytes()
            {
                return Err(Failure::Mismatch(
                    "native root-accepted application startup",
                ));
            }
            io::eof(&control, deadline)?;
            io::validate_application(&control, expected)?;
            let owner = NativeApplicationSupervisorReadinessV1 {
                binding,
                ready,
                startup,
            };
            let growth = owner
                .retained_storage()
                .checked_sub(inherited)
                .ok_or(Resource::Accounting)?;
            Ok((owner, Storage(growth)))
        })
    }

    pub const NATIVE_APPLICATION_INPUT_STORAGE: usize =
        size_of::<Self>() + size_of::<ProofPeer>() + size_of::<OwnedFd>();
}

fn validate_reader(reader: &OwnedFd, expected: &Occurrence) -> Result<()> {
    let flags = rustix::fs::fcntl_getfl(reader)?;
    let stat = rustix::fs::fstat(reader)?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::Fifo
        || flags & OFlags::ACCMODE != OFlags::RDONLY
        || !flags.contains(OFlags::NONBLOCK)
        || flags.intersects(OFlags::PATH | OFlags::APPEND | OFlags::ASYNC | OFlags::DIRECT)
        || !fcntl_getfd(reader)?.contains(FdFlags::CLOEXEC)
    {
        return Err(Failure::Mismatch("native original ACK reader shape"));
    }
    let occurrence =
        Occurrence::from_linux_descriptor_v1(3, stat.st_dev, stat.st_ino, stat.st_mode)
            .map_err(|_| Failure::Mismatch("native original ACK reader occurrence"))?;
    if &occurrence != expected {
        return Err(Failure::Mismatch("native original ACK reader substitution"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustix::net::{self, AddressFamily, SocketFlags, SocketType};
    use std::time::Duration;

    #[test]
    fn native_completion_transport_rejects_early_eof_wrong_phase_and_rights() {
        for mode in 0..4 {
            let (receiver, sender) = net::socketpair(
                AddressFamily::UNIX,
                SocketType::SEQPACKET,
                SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
                None,
            )
            .unwrap();
            net::sockopt::set_socket_passcred(&receiver, true).unwrap();
            let deadline = Instant::now() + Duration::from_secs(2);
            let bytes = [7; fe2o3_runtime_protocol::NATIVE_APPLICATION_STARTUP_BYTES_V1];
            match mode {
                0 => io::send_application(&sender, &bytes, &[], deadline).unwrap(),
                1 => io::send_application(&sender, &[3; READY_BYTES], &[], deadline).unwrap(),
                2 => io::send_application(&sender, &bytes, &[sender.as_fd()], deadline).unwrap(),
                _ => {}
            }
            drop(sender);
            let result = io::exact_record::<
                { fe2o3_runtime_protocol::NATIVE_APPLICATION_STARTUP_BYTES_V1 },
            >(&receiver, deadline);
            assert_eq!(result.is_ok(), mode == 0);
            if mode == 0 {
                assert_eq!(result.unwrap(), bytes);
                io::eof(&receiver, deadline).unwrap();
            }
        }
    }

    #[test]
    fn native_reader_requires_original_pipe_read_role_and_flags() {
        use rustix::pipe::{PipeFlags, pipe_with};
        let (reader, writer) = pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK).unwrap();
        let stat = rustix::fs::fstat(&writer).unwrap();
        let occurrence =
            Occurrence::from_linux_descriptor_v1(3, stat.st_dev, stat.st_ino, stat.st_mode)
                .unwrap();
        validate_reader(&reader, &occurrence).unwrap();
        assert!(validate_reader(&writer, &occurrence).is_err());
        let (other, _) = pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK).unwrap();
        assert!(validate_reader(&other, &occurrence).is_err());
        rustix::fs::fcntl_setfl(&reader, OFlags::empty()).unwrap();
        assert!(validate_reader(&reader, &occurrence).is_err());
    }
}
