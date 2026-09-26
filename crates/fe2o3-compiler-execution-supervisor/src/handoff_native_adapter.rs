//! Shared native admission/revalidation only; concrete protocol families stay local.
macro_rules! handoff {
    ($accepted:ident, $storage:ident, $error:ident, $frame_storage:ident) => {
        /// Unreserved additional retained storage; preserve consumed control-FD charges.
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub struct $storage(usize);
        impl Storage {
            /// Reserve this delta before retaining the result; release full storage after drop.
            pub const fn additional_storage(self) -> usize {
                self.0
            }
        }

        impl Accepted {
            /// Logical input charge for the consumed control descriptor, including receipt padding.
            pub const CONTROL_STORAGE: usize = size_of::<(OwnedFd, Storage)>();
            /// Fixed metadata growth over two socket owners, frame and live-client custody.
            pub const OWNER_GROWTH: usize =
                size_of::<(Snapshot, Snapshot, Snapshot, usize, Storage)>() + align_of::<Self>();
            /// Outer logical work, including at most one poll and one receive.
            /// Native supervisor/frame/client operations additionally charge the caller ledger.
            pub const WORK: usize = ENTRY + 128 * 1024;
            /// Outer logical scratch; nested checks charge additional scratch. Not RSS,
            /// generated-stack, wall-time or kernel socket-buffer bounds.
            pub const SCRATCH: usize = 4 * size_of::<(Self, Storage)>()
                + 4 * BYTES
                + 2 * transport::ANCILLARY_BYTES
                + 4096;

            /// Returns inert canonical facts, not descriptor or launch authority.
            pub const fn manifest(&self) -> &Manifest {
                self.handoff.launch_manifest()
            }
            /// Returns the authenticated connection-time submitter identity.
            pub const fn submitter(&self) -> Client {
                self.handoff.submitter()
            }
            /// Full retained charge; retire only after drop or transfer.
            pub const fn retained_storage(&self) -> usize {
                self.retained
            }

            /// Rechecks native supervisor, policy, roles, socket custody and live client.
            pub fn revalidate(
                &self,
                supervisor: &Supervisor,
                budget: &mut Budget<'_>,
            ) -> Result<()> {
                budget.charge_work(ENTRY)?;
                let floor = self
                    .retained
                    .checked_add(supervisor.retained_storage())
                    .ok_or(Resource::Arithmetic)?;
                budget.with_prepaid_scope(floor, 0, Self::WORK - ENTRY, Self::SCRATCH, |budget| {
                    self.check(supervisor, budget)
                })
            }

            fn check(&self, supervisor: &Supervisor, budget: &mut Budget<'_>) -> Result<()> {
                supervisor.revalidate(budget)?;
                context(&self.handoff, supervisor, budget)?;
                if checks::snapshot(&self.control)? != self.control_snapshot
                    || checks::snapshot(&self.service_peer)? != self.service_snapshot
                    || pidfd_snapshot(&self.client) != self.pidfd_snapshot
                {
                    return Err($error::DescriptorChanged);
                }
                checks::distinct(
                    self.control_snapshot,
                    self.service_snapshot,
                    self.pidfd_snapshot,
                )?;
                checks::control_shape(&self.control)?;
                if checks::control_peer(&self.control)? != self.submitter() {
                    return Err($error::SubmitterCredentialsMismatch);
                }
                checks::service_peer(&self.service_peer, self.manifest().client())?;
                self.client.validate_liveness(budget)?;
                Ok(())
            }
        }

        impl Supervisor {
            /// Consumes one control connection and admits its native frame and ordered
            /// service-peer/pidfd rights. Always requires distinct client/service UIDs.
            ///
            /// The complete operation has an absolute receive deadline and a finite
            /// syscall schedule. EINTR or a readiness race refuses rather than retrying.
            /// Consuming refusal closes the control and any received rights, even on
            /// budget failure. Every nested operation shares the caller's ledger;
            /// temporary storage is restored without refunding work or denial history.
            pub fn accept_handoff(
                &self,
                control: OwnedFd,
                timeout: Duration,
                budget: &mut Budget<'_>,
            ) -> Result<(Accepted, Storage)> {
                budget.charge_work(ENTRY)?;
                let floor = self
                    .retained_storage()
                    .checked_add(Accepted::CONTROL_STORAGE)
                    .ok_or(Resource::Arithmetic)?;
                budget.with_prepaid_scope(
                    floor,
                    0,
                    Accepted::WORK - ENTRY,
                    Accepted::SCRATCH,
                    |budget| {
                        if timeout.is_zero() {
                            return Err($error::InvalidTimeout);
                        }
                        let deadline = Instant::now()
                            .checked_add(timeout)
                            .ok_or($error::DeadlineOverflow)?;
                        self.revalidate(budget)?;
                        checks::control_shape(&control)?;
                        let submitter = checks::control_peer(&control)?;
                        if submitter.uid() == self.credentials().uid() {
                            return Err($error::ClientAndSupervisorUidMatch);
                        }
                        let control_snapshot = checks::snapshot(&control)?;
                        let (payload, [service_peer, pidfd]) =
                            transport::receive(&control, deadline)?;
                        budget
                            .reserve_storage(Accepted::CONTROL_STORAGE + LiveClient::FD_STORAGE)?;
                        let (handoff, delta) = Frame::decode(&payload, budget)?;
                        budget.reserve_storage(delta.additional_storage())?;
                        // Keep the legacy semantic refusal order: policy, anchor, submitter,
                        // client/anchor PID, descriptor roles, socket peer, live pidfd.
                        if !handoff
                            .launch_manifest()
                            .matches_policy(self.policy(), budget)?
                        {
                            return Err($error::PolicyMismatch);
                        }
                        if handoff.launch_manifest().external_anchor_service()
                            != self.external_anchor_service()
                        {
                            return Err($error::ExternalAnchorServiceMismatch);
                        }
                        if submitter != handoff.submitter() {
                            return Err($error::SubmitterCredentialsMismatch);
                        }
                        let expected = handoff.launch_manifest().client();
                        if expected.pid() == self.external_anchor_process().pid() {
                            return Err($error::ClientAndExternalAnchorProcessMatch);
                        }
                        let service_snapshot = checks::snapshot(&service_peer)?;
                        let pidfd_snapshot = checks::snapshot(&pidfd)?;
                        checks::distinct(control_snapshot, service_snapshot, pidfd_snapshot)?;
                        checks::service_peer(&service_peer, expected)?;
                        // The canonical client PID is nonzero; no allocating legacy error
                        // is constructed on this checked branch.
                        let expected = ExpectedClientProcessIdentityV1::new(
                            expected.pid(),
                            expected.uid(),
                            expected.gid(),
                        )
                        .expect("canonical handoff client PID is nonzero");
                        let (client, delta) = LiveClient::admit(pidfd, expected, budget)?;
                        budget.reserve_storage(delta.additional_storage())?;
                        self.revalidate(budget)?;
                        let retained = (2 * Accepted::CONTROL_STORAGE)
                            .checked_add(handoff.retained_storage())
                            .and_then(|n| n.checked_add(client.retained_storage()))
                            .and_then(|n| n.checked_add(Accepted::OWNER_GROWTH))
                            .ok_or(Resource::Arithmetic)?;
                        let accepted = Accepted {
                            control,
                            handoff,
                            service_peer,
                            client,
                            control_snapshot,
                            service_snapshot,
                            pidfd_snapshot,
                            retained,
                        };
                        budget.reserve_storage(Accepted::OWNER_GROWTH)?;
                        accepted.check(self, budget)?;
                        Ok((accepted, Storage(retained - Accepted::CONTROL_STORAGE)))
                    },
                )
            }
        }

        fn context(frame: &Frame, supervisor: &Supervisor, budget: &mut Budget<'_>) -> Result<()> {
            let manifest = frame.launch_manifest();
            if !manifest.matches_policy(supervisor.policy(), budget)? {
                return Err($error::PolicyMismatch);
            }
            if manifest.external_anchor_service() != supervisor.external_anchor_service() {
                return Err($error::ExternalAnchorServiceMismatch);
            }
            if manifest.client().pid() == supervisor.external_anchor_process().pid() {
                return Err($error::ClientAndExternalAnchorProcessMatch);
            }
            if manifest.client().uid() == supervisor.credentials().uid() {
                return Err($error::ClientAndSupervisorUidMatch);
            }
            Ok(())
        }
        fn pidfd_snapshot(client: &LiveClient) -> Snapshot {
            let (device, inode, mode) = client.descriptor_identity();
            Snapshot(device, inode, mode)
        }
        impl fmt::Debug for Accepted {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($accepted))
                    .field("authority", &"session-custody-only")
                    .field("handoff", &self.handoff.identity())
                    .finish_non_exhaustive()
            }
        }

        /// Fixed native admission failure; no refusal dispatches to V1.
        #[derive(Debug)]
        #[non_exhaustive]
        pub enum $error {
            /// The caller's logical work or storage ledger refused the operation.
            Resource(Resource),
            /// Native supervisor custody failed revalidation.
            Supervisor(SupervisorError),
            /// The complete timeout is zero.
            InvalidTimeout,
            /// The monotonic deadline cannot be represented.
            DeadlineOverflow,
            /// Invalid descriptor or control socket property.
            InvalidControl(&'static str),
            /// Client and protected supervisor use the same UID.
            ClientAndSupervisorUidMatch,
            /// Control peer credentials differ from the canonical submitter.
            SubmitterCredentialsMismatch,
            /// Payload length, ancillary shape or ordered rights count is invalid.
            MalformedTransfer,
            /// Native canonical handoff decoding failed.
            CanonicalHandoff(FrameError),
            /// Native contextual policy comparison failed.
            Manifest(ManifestError),
            /// The frame names another policy.
            PolicyMismatch,
            /// The frame names another anchor service.
            ExternalAnchorServiceMismatch,
            /// Client and anchor are the same process.
            ClientAndExternalAnchorProcessMatch,
            /// The service endpoint is not an unnamed connected CLOEXEC seqpacket socket.
            InvalidServicePeer,
            /// Service peer credentials do not name the exact client.
            ServicePeerCredentialsMismatch,
            /// Native pidfd admission or continuity failed.
            Pidfd(PidfdError),
            /// Two descriptors alias different custody roles.
            DescriptorAlias,
            /// A retained descriptor identity changed.
            DescriptorChanged,
            /// The receive deadline expired.
            Timeout,
            /// Control closed before a complete packet was received.
            ControlClosed,
            /// A finite-attempt syscall failed; EINTR is not retried.
            Io(rustix::io::Errno),
        }
        impl From<Resource> for $error {
            fn from(e: Resource) -> Self {
                Self::Resource(e)
            }
        }
        impl From<SupervisorError> for $error {
            fn from(e: SupervisorError) -> Self {
                Self::Supervisor(e)
            }
        }
        impl From<FrameError> for $error {
            fn from(e: FrameError) -> Self {
                Self::CanonicalHandoff(e)
            }
        }
        impl From<ManifestError> for $error {
            fn from(e: ManifestError) -> Self {
                Self::Manifest(e)
            }
        }
        impl From<PidfdError> for $error {
            fn from(e: PidfdError) -> Self {
                Self::Pidfd(e)
            }
        }
        impl From<rustix::io::Errno> for $error {
            fn from(e: rustix::io::Errno) -> Self {
                Self::Io(e)
            }
        }
        impl From<checks::Failure> for $error {
            fn from(e: checks::Failure) -> Self {
                match e {
                    checks::Failure::InvalidControl(s) => Self::InvalidControl(s),
                    checks::Failure::SubmitterCredentialsMismatch => {
                        Self::SubmitterCredentialsMismatch
                    }
                    checks::Failure::InvalidServicePeer => Self::InvalidServicePeer,
                    checks::Failure::ServicePeerCredentialsMismatch => {
                        Self::ServicePeerCredentialsMismatch
                    }
                    checks::Failure::DescriptorChanged => Self::DescriptorChanged,
                    checks::Failure::DescriptorAlias => Self::DescriptorAlias,
                    checks::Failure::Io(e) => Self::Io(e),
                }
            }
        }
        impl fmt::Display for $error {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self {
                    Self::Resource(e) => e.fmt(f),
                    Self::Supervisor(e) => write!(f, "protected supervisor changed: {e}"),
                    Self::InvalidTimeout => f.write_str("supervisor handoff timeout is zero"),
                    Self::DeadlineOverflow => f.write_str("supervisor handoff deadline overflowed"),
                    Self::InvalidControl(s) => write!(f, "invalid handoff control: {s}"),
                    Self::ClientAndSupervisorUidMatch => {
                        f.write_str("rustc client and protected supervisor use the same UID")
                    }
                    Self::SubmitterCredentialsMismatch => {
                        f.write_str("handoff submitter credentials do not match the rustc client")
                    }
                    Self::MalformedTransfer => f.write_str("rustc handoff packet is malformed"),
                    Self::CanonicalHandoff(e) => write!(f, "invalid canonical rustc handoff: {e}"),
                    Self::Manifest(e) => write!(f, "native launch manifest check failed: {e}"),
                    Self::PolicyMismatch => {
                        f.write_str("rustc launch manifest names another issuer policy")
                    }
                    Self::ExternalAnchorServiceMismatch => {
                        f.write_str("rustc launch manifest names another external-anchor service")
                    }
                    Self::ClientAndExternalAnchorProcessMatch => f.write_str(
                        "rustc client and external-anchor service name the same process",
                    ),
                    Self::InvalidServicePeer => {
                        f.write_str("rustc service endpoint has the wrong socket shape")
                    }
                    Self::ServicePeerCredentialsMismatch => {
                        f.write_str("rustc service endpoint names another process")
                    }
                    Self::Pidfd(e) => write!(f, "rustc pidfd admission failed: {e}"),
                    Self::DescriptorAlias => f.write_str("handoff descriptors alias roles"),
                    Self::DescriptorChanged => f.write_str("a retained handoff descriptor changed"),
                    Self::Timeout => f.write_str("supervisor handoff deadline expired"),
                    Self::ControlClosed => f.write_str("supervisor handoff control closed"),
                    Self::Io(e) => write!(f, "supervisor handoff operation failed: {e}"),
                }
            }
        }
        impl Error for $error {
            fn source(&self) -> Option<&(dyn Error + 'static)> {
                match self {
                    Self::Resource(e) => Some(e),
                    Self::Supervisor(e) => Some(e),
                    Self::CanonicalHandoff(e) => Some(e),
                    Self::Manifest(e) => Some(e),
                    Self::Pidfd(e) => Some(e),
                    Self::Io(e) => Some(e),
                    _ => None,
                }
            }
        }

        const _: () = {
            use fe2o3_broker_authority_service::LiveClientPidfdStorageV2 as ClientStorage;
            use fe2o3_compiler_execution_protocol::$frame_storage as FrameStorage;
            use fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1 as Ledger;
            assert!(
                size_of::<(Accepted, Storage)>()
                    <= 2 * Accepted::CONTROL_STORAGE
                        + size_of::<(Frame, FrameStorage)>()
                        + size_of::<(LiveClient, ClientStorage)>()
                        + Accepted::OWNER_GROWTH
            );
            assert!(
                8 * size_of::<$error>()
                    + 128 * size_of::<usize>()
                    + 8 * size_of::<rustix::fs::Stat>()
                    + 4 * size_of::<libc::sockaddr_un>()
                    + size_of::<Ledger>()
                    + 4 * size_of::<Result<()>>()
                    <= 4096
            );
        };
    };
}
pub(crate) use handoff;
