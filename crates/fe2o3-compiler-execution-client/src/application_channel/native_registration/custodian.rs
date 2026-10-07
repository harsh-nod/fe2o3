//! Native controller custody, distinct from ordinary Ready and legacy proof transport.
use super::*;
use fe2o3_compiler_closure_capability::ProductionNativeApplicationProofProfileV1 as ProofProfile;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionReceiptCarriageV3 as Carriage,
    VerifiedCompilerExecutionCurrentRecordV3 as Verified,
};
use fe2o3_runtime_protocol::{
    MAX_CONDITIONAL_WORKER_READINESS_BYTES_V5, NATIVE_APPLICATION_PROOF_MAX_PAYLOAD_BYTES_V1,
    NativeApplicationProofEvidenceV1 as Evidence, NativeApplicationProofInputsV1 as ProofInputs,
    NativeApplicationProofKindV1 as ProofKind, NativeApplicationProofMessageV1 as ProofMessage,
    NativeApplicationProofSessionV1 as Session,
};
use sha2::{Digest, Sha256};
use std::cell::Cell;

const MAX_EXECUTION: Duration = Duration::from_secs(300);
const MAX_PROBE: Duration = Duration::from_secs(30);
mod startup;

/// Single original native registration after the root's explicit CustodianReady.
/// Retains independently installed compiler/proof profiles and original controller
/// pidfd and original authenticated root through currentness startup. No ordinary Ready, caller
/// session, bare current record or legacy owner constructs this value.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_client::{RegisteredNativeApplicationCustodianV1 as Native,
///     RegisteredNativeApplicationProofEndpointV1 as Ordinary};
/// fn promote<'w>(ordinary: Ordinary<'w>) -> Native<'w> { ordinary.into() }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_client::RegisteredNativeApplicationCustodianV1 as Native;
/// fn clone(value: Native<'_>) { let _ = value.clone(); }
/// ```
pub struct RegisteredNativeApplicationCustodianV1<'work> {
    endpoint: RetainedApplicationProofEndpointV1,
    controller: NativePidfd<'work>,
    root: NativePidfd<'work>,
    root_sender: CompilerExecutionClientProcessIdentityV1,
    currentness_ready: Option<Box<fe2o3_runtime_protocol::NativeApplicationStartupRecordV1>>,
    sender: CompilerExecutionClientProcessIdentityV1,
    session: Session,
    binding: Binding,
    deployment: Deployment<'work>,
    proof_profile: ProofProfile<'work>,
    execution_deadline: Instant,
    poisoned: Cell<bool>,
}
impl fmt::Debug for RegisteredNativeApplicationCustodianV1<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RegisteredNativeApplicationCustodianV1")
            .field("session", &self.session.identity())
            .finish_non_exhaustive()
    }
}
impl RetainedApplicationProofEndpointV1 {
    /// Consumes exactly CustodianReady before startup ACK, with no Ready fallback.
    /// Prepay endpoint, inputs and both installed profiles. Any failed transition
    /// closes transferred descriptors and retains all operation reservations.
    pub fn register_native_custodian_pre_ack<'work>(
        self,
        inputs: Inputs,
        deployment: Deployment<'work>,
        proof_profile: ProofProfile<'work>,
        deadline: Instant,
        budget: &mut Budget<'work>,
    ) -> Result<(
        RegisteredNativeApplicationCustodianV1<'work>,
        NativeApplicationChannelStorageV1,
    )> {
        budget.charge_work(ATTEMPT_WORK)?;
        let profile_storage = proof_profile.retained_storage()?;
        let inherited = sum(
            RegisteredNativeApplicationProofEndpointV1::ENDPOINT_STORAGE,
            sum(
                inputs.retained_storage(),
                sum(deployment.retained_storage()?, profile_storage)?,
            )?,
        )?;
        if budget.storage() < inherited {
            return Err(Resource::Accounting.into());
        }
        proof_profile.revalidate(&deployment, budget)?;
        let execution_deadline = Instant::now() + MAX_EXECUTION;
        let accepted = self.begin_native_registration(inputs, deployment, deadline, budget)?;
        let (ready, mut raw) = receive(&accepted.endpoint, &[&accepted.root], deadline, budget)?;
        if ready.kind() != Kind::CustodianReady
            || raw.sender != accepted.sender
            || ready.transcript() != Some(accepted.transcript)
        {
            return Err(NativeApplicationChannelErrorV1::Binding(
                "native CustodianReady sender or phase",
            ));
        }
        let (session, charge) = ready.decode_proof_session(budget)?;
        budget.reserve_storage(charge.additional_storage())?;
        let (pid, uid, gid) = session.controller();
        check_session(
            &session,
            accepted.transcript,
            proof_profile.configuration(),
            accepted.sender.pid(),
            accepted.endpoint.application_pid,
            accepted.endpoint.snapshot.creator.pid(),
        )?;
        let (controller, charge) = NativePidfd::admit_received(
            raw.rights
                .pop()
                .ok_or(NativeApplicationChannelErrorV1::Binding(
                    "native controller pidfd",
                ))?,
            pid,
            budget,
        )
        .map_err(pidfd_error)?;
        budget.reserve_storage(charge.additional_storage())?;
        let sender =
            CompilerExecutionClientProcessIdentityV1::new(pid, uid, gid).map_err(|_| {
                NativeApplicationChannelErrorV1::Binding("native controller credentials")
            })?;
        accepted.root.revalidate(budget).map_err(pidfd_error)?;
        let value = RegisteredNativeApplicationCustodianV1 {
            endpoint: accepted.endpoint,
            controller,
            root: accepted.root,
            root_sender: accepted.sender,
            currentness_ready: None,
            sender,
            session,
            binding: accepted.binding,
            deployment: accepted.deployment,
            proof_profile,
            execution_deadline,
            poisoned: Cell::new(false),
        };
        value.revalidate_inner(budget)?;
        value.root.revalidate(budget).map_err(pidfd_error)?;
        transport::check_deadline(deadline)?;
        let additional = value
            .retained_storage()?
            .checked_sub(inherited)
            .ok_or(Resource::Arithmetic)?;
        drop((ready, raw));
        finish_native_operation(accepted.floor, budget)?;
        Ok((value, NativeApplicationChannelStorageV1(additional)))
    }
}

fn check_session(
    session: &Session,
    transcript: Transcript,
    profile: &fe2o3_compiler_execution_protocol::NativeApplicationProofCustodianConfigurationV1,
    root: u32,
    app: u32,
    cargo: u32,
) -> Result<()> {
    let (pid, uid, gid) = session.controller();
    if session.transcript() != transcript
        || session.deployment() != profile.identity()
        || (uid, gid) != profile.parts().credentials
        || [root, app, cargo].contains(&pid)
    {
        return Err(NativeApplicationChannelErrorV1::Binding(
            "independently pinned native controller session",
        ));
    }
    Ok(())
}

impl<'work> RegisteredNativeApplicationCustodianV1<'work> {
    pub const fn registration(&self) -> &Binding {
        &self.binding
    }
    pub const fn session(&self) -> &Session {
        &self.session
    }
    pub const fn proof_profile(&self) -> &ProofProfile<'work> {
        &self.proof_profile
    }
    pub fn retained_storage(&self) -> Result<usize> {
        let mut bytes = size_of::<(Self, NativeApplicationChannelStorageV1)>();
        for extra in [
            self.binding
                .retained_storage()
                .checked_sub(size_of::<Binding>()),
            self.deployment
                .retained_storage()?
                .checked_sub(size_of::<Deployment<'work>>()),
            self.proof_profile
                .retained_storage()?
                .checked_sub(size_of::<ProofProfile<'work>>()),
            self.session
                .retained_storage()
                .checked_sub(size_of::<Session>()),
        ] {
            bytes = sum(bytes, extra.ok_or(Resource::Arithmetic)?)?;
        }
        if let Some(ready) = &self.currentness_ready {
            bytes = sum(bytes, ready.retained_storage())?;
        }
        Ok(bytes)
    }
    pub fn revalidate(&self, budget: &mut Budget<'work>) -> Result<()> {
        if self.poisoned.get() {
            return Err(NativeApplicationChannelErrorV1::Binding(
                "native controller channel terminal",
            ));
        }
        let result = (|| {
            budget.charge_work(ATTEMPT_WORK)?;
            if budget.storage() < self.retained_storage()? {
                return Err(Resource::Accounting.into());
            }
            self.revalidate_inner(budget)
        })();
        if result.is_err() {
            self.poisoned.set(true);
        }
        result
    }
    fn revalidate_inner(&self, budget: &mut Budget<'work>) -> Result<()> {
        self.proof_profile.revalidate(&self.deployment, budget)?;
        self.endpoint.revalidate()?;
        require_connected(&self.endpoint)?;
        self.controller.revalidate(budget).map_err(pidfd_error)?;
        self.root.revalidate(budget).map_err(pidfd_error)?;
        if self.session.controller() != (self.sender.pid(), self.sender.uid(), self.sender.gid())
            || self.controller.pid() != self.sender.pid()
            || self.session.deployment() != self.proof_profile.configuration().identity()
            || self.session.transcript().binding() != *self.binding.identity().as_bytes()
        {
            return Err(NativeApplicationChannelErrorV1::Binding(
                "native controller custody binding",
            ));
        }
        check_binding(&self.endpoint, &self.binding, &self.deployment)
    }

    /// Performs an actual fresh V3 exchange while retaining this proof-capable registration.
    /// Prepay this owner, exact readiness, carriage and CompilerExecutionClientV3::PEER_STORAGE.
    ///
    /// # Safety
    /// Exclusively transfer inherited FD195, with no owner/borrow or concurrent
    /// close/replacement. It is consumed once on every exit, including unwind.
    pub unsafe fn verify_inherited_native_currentness(
        self,
        readiness: &[u8],
        carriage: &Carriage,
        deadline: Instant,
        budget: &mut Budget<'work>,
    ) -> Result<(
        NativeCustodianCurrentRecordV1<'work>,
        NativeApplicationChannelStorageV1,
    )> {
        // SAFETY: forwards the exclusive inherited-slot transfer exactly once.
        let (registration, verified) =
            unsafe { currentness::verify(self, readiness, carriage, deadline, budget) }?;
        let value = NativeCustodianCurrentRecordV1 {
            registration,
            verified,
        };
        let additional = value
            .retained_storage()?
            .checked_sub(value.registration.retained_storage()?)
            .ok_or(Resource::Arithmetic)?;
        Ok((value, NativeApplicationChannelStorageV1(additional)))
    }

    fn send_proof(
        &self,
        message: &ProofMessage,
        rights: &[BorrowedFd<'_>],
        deadline: Instant,
        budget: &mut Budget<'work>,
    ) -> Result<()> {
        let result = (|| {
            self.revalidate(budget)?;
            if message.session_identity() != self.session.identity()
                || message.required_rights() != rights.len()
            {
                return Err(NativeApplicationChannelErrorV1::Binding(
                    "native outgoing proof shape",
                ));
            }
            transport::send_with_attempt(
                &self.endpoint,
                &[],
                message.canonical_bytes(),
                rights,
                deadline,
                &mut || {
                    budget
                        .charge_work(ATTEMPT_WORK)
                        .map_err(ApplicationProofChannelErrorV1::Resource)?;
                    self.controller.revalidate(budget).map_err(pidfd_error)
                },
            )?;
            self.revalidate(budget)?;
            transport::check_deadline(deadline)?;
            Ok(())
        })();
        if result.is_err() {
            self.poisoned.set(true);
        }
        result
    }
    fn receive_proof(
        &self,
        kind: ProofKind,
        sequence: u64,
        deadline: Instant,
        budget: &mut Budget<'work>,
    ) -> Result<ProofMessage> {
        let result = (|| {
            self.revalidate(budget)?;
            let raw = transport::receive_with_attempt(
                &self.endpoint,
                &[],
                transport::ReceiveProfile::NativeProof,
                deadline,
                &mut || {
                    budget
                        .charge_work(ATTEMPT_WORK)
                        .map_err(ApplicationProofChannelErrorV1::Resource)?;
                    self.controller.revalidate(budget).map_err(pidfd_error)
                },
            )?;
            let (message, charge) = ProofMessage::decode(&raw.bytes, &self.session, budget)?;
            budget.reserve_storage(charge.retained_storage())?;
            if raw.sender != self.sender
                || !raw.rights.is_empty()
                || message.required_rights() != 0
                || message.kind() != kind
                || message.sequence() != sequence
            {
                return Err(NativeApplicationChannelErrorV1::Binding(
                    "native proof sender, phase or sequence",
                ));
            }
            self.revalidate(budget)?;
            transport::check_deadline(deadline)?;
            Ok(message)
        })();
        if result.is_err() {
            self.poisoned.set(true);
        }
        result
    }
}
impl<'work> currentness::Source<'work> for RegisteredNativeApplicationCustodianV1<'work> {
    fn require_currentness_ready(&self) -> Result<()> {
        self.currentness_ready
            .as_ref()
            .map(|_| ())
            .ok_or(NativeApplicationChannelErrorV1::Binding(
                "native root currentness Ready missing",
            ))
    }
    fn binding(&self) -> &Binding {
        &self.binding
    }
    fn deployment(&self) -> &Deployment<'work> {
        &self.deployment
    }
    fn retained_storage(&self) -> Result<usize> {
        self.retained_storage()
    }
    fn revalidate(&self, budget: &mut Budget<'work>) -> Result<()> {
        self.revalidate(budget)
    }
}

/// Fresh V3 currentness retaining the single actual native proof-capable registration.
/// No ordinary Ready registration, bare verified record or decoded session converts here.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_client::NativeCustodianCurrentRecordV1 as Native;
/// use fe2o3_compiler_execution_protocol::VerifiedCompilerExecutionCurrentRecordV3 as Bare;
/// fn promote<'w>(bare: Bare) -> Native<'w> { bare.into() }
/// ```
#[derive(Debug)]
pub struct NativeCustodianCurrentRecordV1<'work> {
    registration: RegisteredNativeApplicationCustodianV1<'work>,
    verified: Verified,
}
impl<'work> NativeCustodianCurrentRecordV1<'work> {
    pub const fn verified(&self) -> &Verified {
        &self.verified
    }
    pub const fn registration(&self) -> &RegisteredNativeApplicationCustodianV1<'work> {
        &self.registration
    }
    pub fn retained_storage(&self) -> Result<usize> {
        sum(
            self.registration.retained_storage()?,
            size_of::<Self>() - size_of::<RegisteredNativeApplicationCustodianV1<'work>>(),
        )
    }
    pub fn revalidate_provenance(&self, budget: &mut Budget<'work>) -> Result<()> {
        if budget.storage() < self.retained_storage()? {
            self.registration.poisoned.set(true);
            return Err(Resource::Accounting.into());
        }
        self.registration.revalidate(budget)
    }

    /// After startup ACK, requests proof once for exact registered readiness and payload.
    /// Inputs are borrowed and remain prepaid; two additional sealed file copies stay
    /// retained. The first matched Retained response is mandatory before success.
    /// This does not authenticate original compiler custody or grant runtime authority.
    pub fn request_proof(
        self,
        readiness: &[u8],
        payload: &[u8],
        deadline: Instant,
        budget: &mut Budget<'work>,
    ) -> Result<(
        RetainedNativeApplicationProofV1<'work>,
        NativeApplicationChannelStorageV1,
    )> {
        let floor = budget.storage();
        budget.charge_work(ATTEMPT_WORK)?;
        let original_storage = self.retained_storage()?;
        let file_bytes = sum(readiness.len(), payload.len())?;
        if floor < sum(original_storage, file_bytes)? {
            return Err(Resource::Accounting.into());
        }
        budget.reserve_storage(sum(IO_SCRATCH, file_bytes)?)?;
        let deadline = deadline.min(self.registration.execution_deadline);
        transport::check_deadline(deadline)?;
        self.revalidate_provenance(budget)?;
        if readiness.is_empty()
            || readiness.len() > MAX_CONDITIONAL_WORKER_READINESS_BYTES_V5
            || payload.is_empty()
            || payload.len() > NATIVE_APPLICATION_PROOF_MAX_PAYLOAD_BYTES_V1
        {
            return Err(NativeApplicationChannelErrorV1::Binding(
                "native proof input bounds",
            ));
        }
        budget.charge_work(file_bytes)?;
        if (Sha256::digest(readiness).into(), readiness.len() as u64)
            != (
                self.registration.binding.inputs().readiness_sha256(),
                self.registration.binding.inputs().readiness_byte_len(),
            )
        {
            return Err(NativeApplicationChannelErrorV1::Binding(
                "native proof readiness substitution",
            ));
        }
        let (inputs, charge) = ProofInputs::new(
            &self.registration.session,
            &self.registration.binding,
            (Sha256::digest(payload).into(), payload.len() as u64),
            budget,
        )?;
        budget.reserve_storage(charge.retained_storage())?;
        let files = [
            seal(readiness, deadline, budget)?,
            seal(payload, deadline, budget)?,
        ];
        budget.charge_work(2)?;
        let first = rustix::fs::fstat(&files[0]).map_err(ApplicationProofChannelErrorV1::Io)?;
        let second = rustix::fs::fstat(&files[1]).map_err(ApplicationProofChannelErrorV1::Io)?;
        if (first.st_dev, first.st_ino) == (second.st_dev, second.st_ino) {
            return Err(NativeApplicationChannelErrorV1::Binding(
                "native proof files aliased",
            ));
        }
        let (request, charge) = ProofMessage::request(&self.registration.session, &inputs, budget)?;
        budget.reserve_storage(charge.retained_storage())?;
        let active = self
            .registration
            .receive_proof(ProofKind::Active, 1, deadline, budget)?;
        self.registration.send_proof(
            &request,
            &[files[0].as_fd(), files[1].as_fd()],
            deadline,
            budget,
        )?;
        let proved = self
            .registration
            .receive_proof(ProofKind::Proved, 1, deadline, budget)?;
        let (evidence, charge) = proved.decode_evidence(budget)?;
        budget.reserve_storage(charge.retained_storage())?;
        evidence.check_inputs(&inputs, budget)?;
        let mut value = RetainedNativeApplicationProofV1 {
            currentness: self,
            inputs,
            evidence,
            _files: files,
            file_bytes,
            next_sequence: 2,
        };
        // All output fields coexist within the still-prepaid operation reservation.
        value.probe_inner(deadline, budget)?;
        let additional = value
            .retained_storage()?
            .checked_sub(original_storage)
            .ok_or(Resource::Arithmetic)?;
        drop((request, active, proved));
        finish_native_operation(floor, budget)?;
        Ok((value, NativeApplicationChannelStorageV1(additional)))
    }
}

/// Actual native controller, fresh currentness, exact sealed inputs and retained evidence.
/// A decoded Evidence alone cannot construct this owner. No descriptor export,
/// cloning, Release/settlement message, V1 promotion or load/launch permission exists.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_client::RetainedNativeApplicationProofV1 as Proof;
/// fn clone(value: Proof<'_>) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_client::RetainedNativeApplicationProofV1 as Proof;
/// fn descriptor<T: std::os::fd::AsFd>() {}
/// descriptor::<Proof<'_>>();
/// ```
#[derive(Debug)]
pub struct RetainedNativeApplicationProofV1<'work> {
    currentness: NativeCustodianCurrentRecordV1<'work>,
    inputs: ProofInputs,
    evidence: Evidence,
    _files: [OwnedFd; 2],
    file_bytes: usize,
    next_sequence: u64,
}
impl<'work> RetainedNativeApplicationProofV1<'work> {
    pub const fn currentness(&self) -> &NativeCustodianCurrentRecordV1<'work> {
        &self.currentness
    }
    pub const fn inputs(&self) -> &ProofInputs {
        &self.inputs
    }
    pub const fn evidence(&self) -> &Evidence {
        &self.evidence
    }
    pub fn retained_storage(&self) -> Result<usize> {
        sum(
            self.currentness.retained_storage()?,
            sum(
                size_of::<Self>() - size_of::<NativeCustodianCurrentRecordV1<'work>>(),
                sum(
                    self.file_bytes,
                    sum(
                        self.inputs.retained_storage() - size_of::<ProofInputs>(),
                        self.evidence.retained_storage() - size_of::<Evidence>(),
                    )?,
                )?,
            )?,
        )
    }
    pub fn revalidate_provenance(&self, budget: &mut Budget<'work>) -> Result<()> {
        let result = (|| {
            if budget.storage() < self.retained_storage()? {
                return Err(Resource::Accounting.into());
            }
            self.currentness.revalidate_provenance(budget)?;
            self.inputs
                .check_registration(&self.currentness.registration.binding, budget)?;
            self.evidence.check_inputs(&self.inputs, budget)?;
            Ok(())
        })();
        if result.is_err() {
            self.currentness.registration.poisoned.set(true);
        }
        result
    }
    pub fn probe(&mut self, deadline: Instant, budget: &mut Budget<'work>) -> Result<()> {
        let floor = budget.storage();
        let result = (|| {
            self.revalidate_provenance(budget)?;
            budget.reserve_storage(IO_SCRATCH)?;
            self.probe_inner(deadline, budget)?;
            finish_native_operation(floor, budget)
        })();
        if result.is_err() {
            self.currentness.registration.poisoned.set(true);
        }
        result
    }
    fn probe_inner(&mut self, deadline: Instant, budget: &mut Budget<'work>) -> Result<()> {
        let deadline = deadline.min(Instant::now() + MAX_PROBE);
        transport::check_deadline(deadline)?;
        let next = self
            .next_sequence
            .checked_add(1)
            .filter(|next| *next < u64::MAX)
            .ok_or(NativeApplicationChannelErrorV1::Binding(
                "native proof sequence exhausted",
            ))?;
        let channel = &self.currentness.registration;
        let (probe, charge) = ProofMessage::probe(&channel.session, self.next_sequence, budget)?;
        budget.reserve_storage(charge.retained_storage())?;
        channel.send_proof(&probe, &[], deadline, budget)?;
        let retained =
            channel.receive_proof(ProofKind::Retained, self.next_sequence, deadline, budget)?;
        if retained.body() != self.evidence.canonical_bytes() {
            return Err(NativeApplicationChannelErrorV1::Binding(
                "native retained evidence substitution",
            ));
        }
        self.next_sequence = next;
        self.currentness.revalidate_provenance(budget)
    }
    pub const fn grants_load_authority(&self) -> bool {
        false
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

fn seal(bytes: &[u8], deadline: Instant, budget: &mut Budget<'_>) -> Result<OwnedFd> {
    budget.charge_work(1)?;
    transport::check_deadline(deadline)?;
    let file = rustix::fs::memfd_create(
        c"fe2o3-native-application-proof-v1",
        rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
    )
    .map_err(ApplicationProofChannelErrorV1::Io)?;
    let mut remaining = bytes;
    while !remaining.is_empty() {
        let chunk = &remaining[..remaining.len().min(16 * 1024)];
        budget.charge_work(sum(chunk.len(), 1)?)?;
        transport::check_deadline(deadline)?;
        match rustix::io::write(&file, chunk) {
            Ok(0) => {
                return Err(NativeApplicationChannelErrorV1::Binding(
                    "native proof input short write",
                ));
            }
            Ok(count) => remaining = &remaining[count..],
            Err(rustix::io::Errno::INTR) => continue,
            Err(error) => return Err(ApplicationProofChannelErrorV1::Io(error).into()),
        }
    }
    budget.charge_work(6)?;
    rustix::fs::fchmod(&file, rustix::fs::Mode::RUSR)
        .map_err(ApplicationProofChannelErrorV1::Io)?;
    rustix::fs::fcntl_add_seals(
        &file,
        rustix::fs::SealFlags::SEAL
            | rustix::fs::SealFlags::SHRINK
            | rustix::fs::SealFlags::GROW
            | rustix::fs::SealFlags::WRITE,
    )
    .map_err(ApplicationProofChannelErrorV1::Io)?;
    let reader = rustix::fs::openat(
        rustix::fs::CWD,
        format!("/proc/self/fd/{}", file.as_raw_fd()),
        OFlags::RDONLY | OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map_err(ApplicationProofChannelErrorV1::Io)?;
    let source = rustix::fs::fstat(&file).map_err(ApplicationProofChannelErrorV1::Io)?;
    let target = rustix::fs::fstat(&reader).map_err(ApplicationProofChannelErrorV1::Io)?;
    if (source.st_dev, source.st_ino, source.st_size)
        != (target.st_dev, target.st_ino, target.st_size)
        || target.st_size != bytes.len() as i64
        || rustix::fs::fcntl_getfl(&reader).map_err(ApplicationProofChannelErrorV1::Io)?
            & OFlags::ACCMODE
            != OFlags::RDONLY
    {
        return Err(NativeApplicationChannelErrorV1::Binding(
            "native proof sealed file identity",
        ));
    }
    transport::check_deadline(deadline)?;
    Ok(reader)
}

#[cfg(test)]
mod tests;
