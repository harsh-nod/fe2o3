//! A separate application currentness connection; never a synthetic compiler trace.
use crate::compiler_execution_issuer::validate_retained_issuer_image_v3;
use crate::compiler_execution_root_channel::RootLaunchChannelV3 as Channel;
use crate::{
    ExpectedClientProcessIdentityV1 as Expected, LiveClientPidfdIdentityV2 as Client,
    PendingRootNativeApplicationV3 as Registration,
};
use fe2o3_compiler_execution_protocol::CompilerExecutionServiceReadyV3 as Ready;
use fe2o3_kernel_ir::{
    CanonicalKernelIrStorageAccountIdentityV1 as Account,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
};
use fe2o3_protected_service_profile::{
    ProtectedServiceCredentialProfileV1 as Credentials,
    ProtectedServiceNamespaceSetV2 as Namespaces, observations,
};
use fe2o3_protected_service_spawn::{
    launch_io, native_spawn::RootOwnedRetainedServiceChildV2 as Child,
};
use fe2o3_runtime_protocol::{
    NATIVE_APPLICATION_CURRENTNESS_ROOT_GATE_BYTES_V1 as BYTES,
    NativeApplicationCurrentnessRootRecordV1 as Record,
};
use std::{
    io,
    mem::size_of,
    time::{Duration, Instant},
};

#[path = "compiler_execution_application_currentness_custody.rs"]
mod custody;
pub use custody::ApplicationCurrentnessCustodyV3;

fn error(error: impl std::error::Error + Send + Sync + 'static) -> io::Error {
    io::Error::other(error)
}
fn refused(reason: &'static str) -> io::Error {
    io::Error::other(reason)
}

/// Fresh role-gate authentication of the actual root-owned issuer child and the
/// independently received native application. This is neither a compiler trace
/// nor a receipt/Ready decoder, and cannot grant proof or GPU execution authority.
/// Callers retain the original child/cleanup owner and revalidate at every use.
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::RootApplicationCurrentnessConnectionV3;
/// fn fake(fd:std::os::fd::OwnedFd) {let _=RootApplicationCurrentnessConnectionV3::from_fd(fd);}
/// ```
pub struct RootApplicationCurrentnessConnectionV3<'work> {
    channel: Channel<'work>,
    issuer: Client,
    namespaces: Namespaces,
    request: Record,
    registration_nonce: [u8; 32],
    deadline: Instant,
    ledger: Ledger,
    account: Option<Account>,
    thread: u32,
    retained: usize,
}
impl<'work> RootApplicationCurrentnessConnectionV3<'work> {
    pub const LOCAL_WORK: usize = 128 * 1024;
    pub const LOCAL_STORAGE: usize = 128 * 1024;
    /// The original launcher must have observed the exact private readiness frame
    /// and EOF and closed all parent/stage aliases of the issuer channel. The
    /// channel enforces its own alias closure; this method freshly measures the
    /// retained child, validates its native profile/parent/namespaces, and only
    /// then generates the unpredictable role challenge. Decoded Ready alone
    /// cannot complete this exchange. Failure consumes the channel but never
    /// discharges the caller's original funded child cleanup obligation.
    pub fn connect_after_readiness<T: Send + 'static>(
        registration: &Registration<'_, '_, 'work>,
        issuer: &Child<T>,
        ready: &Ready,
        channel: Channel<'work>,
        timeout: Duration,
        b: &mut Budget<'work>,
    ) -> io::Result<(Self, usize)> {
        b.charge_work(Self::LOCAL_WORK).map_err(error)?;
        let floor = b.storage();
        let input = registration
            .retained_storage()
            .checked_add(issuer.retained_storage())
            .and_then(|n| n.checked_add(ready.retained_storage()))
            .and_then(|n| n.checked_add(channel.retained_storage()))
            .ok_or_else(|| refused("currentness input accounting"))?;
        if floor < input {
            return Err(refused("currentness inputs not prepaid"));
        }
        b.reserve_storage(Self::LOCAL_STORAGE).map_err(error)?;
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or_else(|| refused("currentness deadline overflow"))?;
        if timeout.is_zero()
            || timeout > Duration::from_secs(120)
            || deadline > registration.startup_deadline()
        {
            return Err(refused("currentness deadline exceeds registration"));
        }
        registration.revalidate(b)?;
        channel.validate_root_endpoint(b).map_err(error)?;
        let policy = registration.deployment().policy();
        let manifest = registration.binding().compiler_handoff().launch_manifest();
        if !ready
            .matches_launch(issuer.pid().as_raw_pid() as u32, manifest, policy, b)
            .map_err(error)?
        {
            return Err(refused("currentness ready changed original native launch"));
        }
        let (fd, charge) = issuer.try_clone_pidfd(b).map_err(error)?;
        b.reserve_storage(charge.additional_storage())
            .map_err(error)?;
        let expected = Expected::new(
            issuer.pid().as_raw_pid() as u32,
            registration.deployment().supervisor().service_uid(),
            registration.deployment().supervisor().service_gid(),
        )
        .map_err(error)?;
        let (client, charge) = Client::admit(fd, expected, b).map_err(error)?;
        b.reserve_storage(charge.additional_storage())
            .map_err(error)?;
        let (namespaces, charge) = Namespaces::capture_self(b).map_err(error)?;
        b.reserve_storage(charge.additional_storage())
            .map_err(error)?;
        validate_child(registration, issuer, &client, &namespaces, b)?;
        let mut nonce = [0; 32];
        if rustix::rand::getrandom(&mut nonce, rustix::rand::GetRandomFlags::NONBLOCK)?
            != nonce.len()
            || nonce == [0; 32]
            || nonce == registration.transfer_nonce()
        {
            return Err(refused("currentness fresh nonce unavailable"));
        }
        let (request, charge) =
            Record::request(policy, manifest, registration.binding(), nonce, b).map_err(error)?;
        b.reserve_storage(charge.additional_storage())
            .map_err(error)?;
        let packet = request.gate_bytes(b).map_err(error)?;
        b.reserve_storage(BYTES).map_err(error)?;
        let sender = launch_io::MessageSender::new(
            issuer.pid().as_raw_pid(),
            expected.uid(),
            expected.gid(),
        );
        let mut sent = false;
        let mut completed = false;
        for _ in 0..launch_io::MAX_PHASE_ATTEMPTS {
            b.charge_work(Self::LOCAL_WORK).map_err(error)?;
            check_deadline(deadline)?;
            registration.revalidate(b)?;
            validate_child(registration, issuer, &client, &namespaces, b)?;
            if !sent {
                sent = channel.send_packet(&packet, b).map_err(error)?.is_some();
            } else if let Some(bytes) = channel.receive_packet(sender, b).map_err(error)? {
                b.reserve_storage(BYTES).map_err(error)?;
                let (reply, charge) = Record::decode_gate(&bytes, b).map_err(error)?;
                b.reserve_storage(charge.additional_storage())
                    .map_err(error)?;
                if !request.matches_reply(&reply, b).map_err(error)? {
                    return Err(refused("currentness root reply changed exact request"));
                }
                completed = true;
                break;
            }
            rustix::event::poll(
                &mut [],
                Some(&rustix::event::Timespec {
                    tv_sec: 0,
                    tv_nsec: 1_000_000,
                }),
            )?;
        }
        if !completed {
            return Err(refused("currentness root handshake attempt limit"));
        }
        registration.revalidate(b)?;
        validate_child(registration, issuer, &client, &namespaces, b)?;
        check_deadline(deadline)?;
        let retained = size_of::<Self>()
            .checked_add(channel.retained_storage())
            .and_then(|n| n.checked_add(client.retained_storage()))
            .and_then(|n| n.checked_add(namespaces.retained_storage()))
            .and_then(|n| n.checked_add(request.retained_storage()))
            .ok_or_else(|| refused("currentness retained accounting"))?;
        let value = Self {
            channel,
            issuer: client,
            namespaces,
            request,
            registration_nonce: registration.transfer_nonce(),
            deadline,
            ledger: b.work_ledger_identity_v1(),
            account: b.storage_account_identity_v1(),
            thread: rustix::thread::gettid().as_raw_pid() as u32,
            retained,
        };
        b.release_storage(
            b.storage()
                .checked_sub(floor)
                .ok_or_else(|| refused("currentness accounting"))?,
        )
        .map_err(error)?;
        // Channel input remains charged; return growth only, unlike a decoded
        // root record. The exact owner retains that original channel.
        Ok((
            value,
            retained
                .checked_sub(Channel::STORAGE)
                .ok_or_else(|| refused("currentness retained underflow"))?,
        ))
    }
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
    /// Publish only this actual root-connected issuer through the registration's
    /// original measured supervisor. Completion means that supervisor forwarded
    /// exact Ready and closed Cargo's control; it is not a proof/ACK transition.
    pub fn publish_to_supervisor<T: Send + 'static>(
        &self,
        registration: &mut Registration<'_, '_, 'work>,
        issuer: &Child<T>,
        ready: &Ready,
        b: &mut Budget<'work>,
    ) -> io::Result<()> {
        self.validate(registration, issuer, b)?;
        if !ready
            .matches_launch(
                issuer.pid().as_raw_pid() as u32,
                registration.binding().compiler_handoff().launch_manifest(),
                registration.deployment().policy(),
                b,
            )
            .map_err(error)?
        {
            return Err(refused("native publication Ready changed original launch"));
        }
        let (fd, charge) = issuer.try_clone_pidfd(b).map_err(error)?;
        b.reserve_storage(charge.additional_storage())
            .map_err(error)?;
        use std::os::fd::AsFd;
        registration.publish_currentness(ready, fd.as_fd(), self.request.identity(), b)?;
        drop(fd);
        self.validate(registration, issuer, b)
    }
    pub fn validate<T: Send + 'static>(
        &self,
        registration: &Registration<'_, '_, 'work>,
        issuer: &Child<T>,
        b: &mut Budget<'work>,
    ) -> io::Result<()> {
        b.charge_work(Self::LOCAL_WORK).map_err(error)?;
        let floor = self
            .retained
            .checked_add(registration.retained_storage())
            .and_then(|n| n.checked_add(issuer.retained_storage()))
            .ok_or_else(|| refused("currentness input accounting"))?;
        if self.ledger != b.work_ledger_identity_v1()
            || self.account != b.storage_account_identity_v1()
            || self.thread != rustix::thread::gettid().as_raw_pid() as u32
            || b.storage() < floor
        {
            return Err(refused("currentness account or thread changed"));
        }
        b.reserve_storage(Self::LOCAL_STORAGE).map_err(error)?;
        registration.revalidate(b)?;
        if self.registration_nonce != registration.transfer_nonce()
            || self.request.registration_identity() != *registration.binding().identity().as_bytes()
            || self.request.association_identity()
                != *registration.binding().association().identity().as_bytes()
            || self.request.carriage_identity()
                != registration.binding().association().carriage_identity()
            || !self
                .request
                .matches_launch(
                    registration.deployment().policy(),
                    registration.binding().compiler_handoff().launch_manifest(),
                    b,
                )
                .map_err(error)?
        {
            return Err(refused("currentness registration association changed"));
        }
        self.channel.validate_root_endpoint(b).map_err(error)?;
        validate_child(registration, issuer, &self.issuer, &self.namespaces, b)?;
        check_deadline(self.deadline)?;
        b.release_storage(Self::LOCAL_STORAGE).map_err(error)
    }
}
fn validate_child<T: Send + 'static>(
    registration: &Registration<'_, '_, '_>,
    issuer: &Child<T>,
    client: &Client,
    namespaces: &Namespaces,
    b: &mut Budget<'_>,
) -> io::Result<()> {
    validate_child_parts(
        registration.root_client(),
        issuer,
        client,
        namespaces,
        registration.deployment().policy(),
        b,
    )
}

fn validate_child_parts<T: Send + 'static>(
    root: &Client,
    issuer: &Child<T>,
    client: &Client,
    namespaces: &Namespaces,
    policy: &fe2o3_compiler_execution_protocol::CompilerExecutionIssuerPolicyV3,
    b: &mut Budget<'_>,
) -> io::Result<()> {
    if issuer.pid().as_raw_pid() as u32 != client.expected_client().pid()
        || !issuer.is_live(b).map_err(error)?
    {
        return Err(refused("currentness original issuer exited or changed"));
    }
    client.validate_parent(root, b).map_err(error)?;
    namespaces.revalidate_self(b).map_err(error)?;
    namespaces
        .revalidate_process(issuer.pid(), b)
        .map_err(error)?;
    let credentials = Credentials::new(
        client.expected_client().uid(),
        client.expected_client().gid(),
    )
    .map_err(error)?;
    b.charge_work(observations::PROCESS_VALIDATE_WORK)
        .map_err(error)?;
    b.reserve_storage(observations::PROCESS_VALIDATE_SCRATCH)
        .map_err(error)?;
    observations::validate_process(credentials, issuer.pid()).map_err(error)?;
    b.release_storage(observations::PROCESS_VALIDATE_SCRATCH)
        .map_err(error)?;
    validate_retained_issuer_image_v3(issuer, policy, b).map_err(error)?;
    client.validate_liveness(b).map_err(error)?;
    Ok(())
}
fn check_deadline(deadline: Instant) -> io::Result<()> {
    if Instant::now() >= deadline {
        return Err(refused("currentness root handshake deadline"));
    }
    Ok(())
}
