//! Private native staging. A capsule is matching data, never root registration authority.
use crate::{other, require, wire};
use fe2o3_broker_authority_service::{
    ExpectedClientProcessIdentityV1 as Expected, LiveClientPidfdIdentityV2 as Client,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrStorageAccountIdentityV1 as Account,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
    CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
};
use fe2o3_runtime_protocol::{
    NATIVE_APPLICATION_REGISTRATION_BYTES_V1, NativeApplicationProofSessionV1 as Session,
    NativeApplicationRegistrationBindingV1 as Binding,
    NativeApplicationSessionTranscriptV1 as Transcript,
};
use sha2::{Digest, Sha256};
use std::{
    io,
    marker::PhantomData,
    os::fd::{AsFd, OwnedFd},
};

pub(crate) mod control;
mod evidence;
pub(crate) mod resources;
mod transport;
#[allow(unsafe_code)]
pub(crate) mod worker;

const BINDING_BEGIN: usize = 152;
const BINDING_END: usize = BINDING_BEGIN + NATIVE_APPLICATION_REGISTRATION_BYTES_V1;
const IDENTITY: usize = BINDING_END + 48;
pub(crate) const CAPSULE_BYTES: usize = IDENTITY + 32;
const MAGIC: &[u8; 8] = b"F3NAPC1\0";
const DOMAIN: &[u8] = b"FE2O3/NATIVE-APPLICATION/CUSTODIAN-CAPSULE/V1\0";
const IO_WORK: usize = 64 * 1024;
const IO_STORAGE: usize = 32 * 1024;

pub(crate) struct Capsule {
    pub(crate) parent: u32,
    pub(crate) nonce: [u8; 32],
    pub(crate) binding: Binding,
    pub(crate) transcript: Transcript,
    peer: [u8; 32],
    app_start: u64,
    cargo_start: u64,
}
impl Capsule {
    /// Only a root registration owner may decide to stage this result. This
    /// private observation does not create that registration or launch authority.
    pub(crate) fn capture(
        binding: Binding,
        transcript: Transcript,
        application: &Client,
        cargo: &Client,
        peer: &wire::ControlEndpoint,
        budget: &mut Budget<'_>,
    ) -> io::Result<(Self, usize)> {
        let floor = budget.storage();
        budget.charge_work(IO_WORK).map_err(other)?;
        let required = binding
            .retained_storage()
            .checked_add(application.retained_storage())
            .and_then(|n| n.checked_add(cargo.retained_storage()))
            .and_then(|n| n.checked_add(size_of::<wire::ControlEndpoint>()))
            .ok_or_else(|| io::Error::other("native capsule input overflow"))?;
        require(floor >= required, "native capsule inputs not prepaid")?;
        budget.reserve_storage(IO_STORAGE).map_err(other)?;
        fe2o3_protected_service_spawn::require_exact_root_identity_v1().map_err(other)?;
        require(
            matches(
                application.expected_client(),
                binding.compiler_handoff().launch_manifest().client(),
            ) && matches(
                cargo.expected_client(),
                binding.compiler_handoff().submitter(),
            ),
            "native capsule process claims differ",
        )?;
        application.validate_parent(cargo, budget).map_err(other)?;
        let mut nonce = [0; 32];
        require(
            rustix::rand::getrandom(&mut nonce, rustix::rand::GetRandomFlags::empty())?
                == nonce.len(),
            "short native capsule randomness",
        )?;
        let inherited = binding.retained_storage();
        let value = Self {
            parent: std::process::id(),
            nonce,
            binding,
            transcript,
            peer: peer.fingerprint(),
            app_start: application.start_time_ticks(),
            cargo_start: cargo.start_time_ticks(),
        };
        value.validate()?;
        value.check_peer(peer)?;
        application.validate_parent(cargo, budget).map_err(other)?;
        let additional = value
            .retained_storage()
            .checked_sub(inherited)
            .ok_or_else(|| io::Error::other("native capsule retained accounting"))?;
        budget
            .release_storage(budget.storage() - floor)
            .map_err(other)?;
        Ok((value, additional))
    }
    fn validate(&self) -> io::Result<()> {
        let app = self.binding.compiler_handoff().launch_manifest().client();
        let cargo = self.binding.compiler_handoff().submitter();
        require(
            self.parent > 0
                && self.parent <= i32::MAX as u32
                && self.parent != app.pid()
                && self.parent != cargo.pid()
                && self.nonce != [0; 32]
                && self.peer != [0; 32]
                && self.nonce != self.transcript.app_nonce()
                && self.nonce != self.transcript.root_nonce()
                && self.transcript.binding() == *self.binding.identity().as_bytes()
                && self.app_start > 0
                && self.cargo_start > 0
                && app.pid() != cargo.pid()
                && app.uid() > 0
                && app.gid() > 0
                && cargo.uid() == app.uid()
                && cargo.gid() == app.gid(),
            "invalid native application capsule",
        )
    }
    pub(crate) fn retained_storage(&self) -> usize {
        size_of::<Self>() - size_of::<Binding>()
            + self.binding.retained_storage()
            + size_of::<usize>()
    }
    pub(crate) fn encode(
        &self,
        budget: &mut Budget<'_>,
    ) -> io::Result<([u8; CAPSULE_BYTES], usize)> {
        budget.charge_work(IO_WORK).map_err(other)?;
        require(
            budget.storage() >= self.retained_storage(),
            "native capsule not prepaid",
        )?;
        budget.reserve_storage(IO_STORAGE).map_err(other)?;
        self.validate()?;
        let mut bytes = [0; CAPSULE_BYTES];
        bytes[..8].copy_from_slice(MAGIC);
        bytes[8..10].copy_from_slice(&1u16.to_le_bytes());
        bytes[12..16].copy_from_slice(&(CAPSULE_BYTES as u32).to_le_bytes());
        bytes[16..20].copy_from_slice(&self.parent.to_le_bytes());
        bytes[24..56].copy_from_slice(&self.nonce);
        bytes[56..BINDING_BEGIN].copy_from_slice(&self.transcript.canonical_bytes());
        bytes[BINDING_BEGIN..BINDING_END].copy_from_slice(self.binding.canonical_bytes());
        bytes[BINDING_END..BINDING_END + 32].copy_from_slice(&self.peer);
        bytes[BINDING_END + 32..BINDING_END + 40].copy_from_slice(&self.app_start.to_le_bytes());
        bytes[BINDING_END + 40..IDENTITY].copy_from_slice(&self.cargo_start.to_le_bytes());
        let identity = hash(&bytes[..IDENTITY]);
        bytes[IDENTITY..].copy_from_slice(&identity);
        budget.release_storage(IO_STORAGE).map_err(other)?;
        Ok((bytes, CAPSULE_BYTES))
    }
    pub(crate) fn decode(bytes: &[u8], budget: &mut Budget<'_>) -> io::Result<(Self, usize)> {
        let floor = budget.storage();
        budget.charge_work(IO_WORK).map_err(other)?;
        require(bytes.len() == CAPSULE_BYTES, "native capsule length")?;
        require(floor >= bytes.len(), "native capsule wire not prepaid")?;
        budget.reserve_storage(IO_STORAGE).map_err(other)?;
        require(
            &bytes[..8] == MAGIC
                && bytes[8..10] == 1u16.to_le_bytes()
                && bytes[10..12] == [0; 2]
                && bytes[12..16] == (CAPSULE_BYTES as u32).to_le_bytes()
                && bytes[20..24] == [0; 4]
                && bytes[IDENTITY..] == hash(&bytes[..IDENTITY]),
            "noncanonical native capsule",
        )?;
        let (transcript, storage) =
            Transcript::decode(&bytes[56..BINDING_BEGIN], budget).map_err(other)?;
        budget
            .reserve_storage(storage.additional_storage())
            .map_err(other)?;
        let (binding, storage) =
            Binding::decode(&bytes[BINDING_BEGIN..BINDING_END], budget).map_err(other)?;
        budget
            .reserve_storage(storage.additional_storage())
            .map_err(other)?;
        let value = Self {
            parent: u32::from_le_bytes(bytes[16..20].try_into().unwrap()),
            nonce: bytes[24..56].try_into().unwrap(),
            binding,
            transcript,
            peer: bytes[BINDING_END..BINDING_END + 32].try_into().unwrap(),
            app_start: u64::from_le_bytes(
                bytes[BINDING_END + 32..BINDING_END + 40]
                    .try_into()
                    .unwrap(),
            ),
            cargo_start: u64::from_le_bytes(bytes[BINDING_END + 40..IDENTITY].try_into().unwrap()),
        };
        value.validate()?;
        let retained = value.retained_storage();
        budget
            .release_storage(budget.storage() - floor)
            .map_err(other)?;
        Ok((value, retained))
    }
    fn check_peer(&self, peer: &wire::ControlEndpoint) -> io::Result<()> {
        peer.revalidate()?;
        let observed = rustix::net::sockopt::socket_peercred(peer)?;
        let creator = self.binding.compiler_handoff().submitter();
        require(
            peer.fingerprint() == self.peer
                && (
                    observed.pid.as_raw_pid() as u32,
                    observed.uid.as_raw(),
                    observed.gid.as_raw(),
                ) == (creator.pid(), creator.uid(), creator.gid()),
            "native proof peer differs from original Cargo endpoint",
        )
    }
}
fn hash(bytes: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(DOMAIN);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    hash.finalize().into()
}
fn expected(
    value: fe2o3_compiler_execution_protocol::CompilerExecutionClientProcessIdentityV1,
) -> io::Result<Expected> {
    Expected::new(value.pid(), value.uid(), value.gid()).map_err(other)
}
fn matches(
    actual: Expected,
    value: fe2o3_compiler_execution_protocol::CompilerExecutionClientProcessIdentityV1,
) -> bool {
    (actual.pid(), actual.uid(), actual.gid()) == (value.pid(), value.uid(), value.gid())
}

/// No application endpoint accessor exists before authenticated root activation.
pub(crate) struct StagedNativeApplication<'work> {
    capsule: Capsule,
    application: Client,
    cargo: Client,
    peer: wire::ControlEndpoint,
    deployment: [u8; 32],
    controller: (u32, u32),
    ledger: Ledger,
    account: Option<Account>,
    lifetime: PhantomData<&'work Work>,
}
impl<'work> StagedNativeApplication<'work> {
    pub(crate) fn admit(
        capsule: Capsule,
        app: OwnedFd,
        cargo: OwnedFd,
        peer: OwnedFd,
        config: &crate::NativeApplicationProofCustodianDeploymentV1,
        budget: &mut Budget<'work>,
    ) -> io::Result<(Self, usize)> {
        let floor = budget.storage();
        budget.charge_work(IO_WORK).map_err(other)?;
        let input = capsule
            .retained_storage()
            .checked_add(2 * Client::FD_STORAGE + size_of::<OwnedFd>())
            .ok_or_else(|| io::Error::other("native staged input overflow"))?;
        require(
            floor
                >= input
                    .checked_add(crate::NativeApplicationProofCustodianDeploymentV1::RETAINED)
                    .ok_or_else(|| io::Error::other("native staged configuration accounting"))?,
            "native staged inputs not prepaid",
        )?;
        budget.reserve_storage(IO_STORAGE).map_err(other)?;
        capsule.validate()?;
        let app_expected = expected(
            capsule
                .binding
                .compiler_handoff()
                .launch_manifest()
                .client(),
        )?;
        let cargo_expected = expected(capsule.binding.compiler_handoff().submitter())?;
        let credentials = config.credentials()?;
        let controller_uid = credentials.uid();
        require(
            controller_uid > 0
                && controller_uid != u32::MAX
                && controller_uid != app_expected.uid()
                && controller_uid != cargo_expected.uid(),
            "native proof controller UID overlaps application",
        )?;
        require(
            config.compiler_policy_identity()
                == *capsule
                    .binding
                    .compiler_handoff()
                    .launch_manifest()
                    .policy_identity()
                    .as_bytes(),
            "native staged compiler policy differs from deployment",
        )?;
        let (application, charge) = Client::admit(app, app_expected, budget).map_err(other)?;
        budget
            .reserve_storage(charge.additional_storage())
            .map_err(other)?;
        let (cargo, charge) = Client::admit(cargo, cargo_expected, budget).map_err(other)?;
        budget
            .reserve_storage(charge.additional_storage())
            .map_err(other)?;
        require(
            application.start_time_ticks() == capsule.app_start
                && cargo.start_time_ticks() == capsule.cargo_start,
            "native staged process occurrence changed",
        )?;
        let value = Self {
            capsule,
            application,
            cargo,
            peer: wire::ControlEndpoint::admit(peer)?,
            deployment: config.identity(),
            controller: (credentials.uid(), credentials.gid()),
            ledger: budget.work_ledger_identity_v1(),
            account: budget.storage_account_identity_v1(),
            lifetime: PhantomData,
        };
        value.revalidate_inner(budget)?;
        let additional = value
            .retained_storage()
            .checked_sub(input)
            .ok_or_else(|| io::Error::other("native staged output accounting"))?;
        budget
            .release_storage(budget.storage() - floor)
            .map_err(other)?;
        Ok((value, additional))
    }
    pub(crate) fn retained_storage(&self) -> usize {
        size_of::<Self>() - size_of::<Capsule>() - 2 * size_of::<Client>()
            + self.capsule.retained_storage()
            + self.application.retained_storage()
            + self.cargo.retained_storage()
    }
    fn revalidate_inner(&self, budget: &mut Budget<'work>) -> io::Result<()> {
        require(
            self.ledger == budget.work_ledger_identity_v1()
                && self.account == budget.storage_account_identity_v1(),
            "native staged account replaced",
        )?;
        self.application
            .validate_parent(&self.cargo, budget)
            .map_err(other)?;
        self.capsule.check_peer(&self.peer)
    }
    pub(crate) fn revalidate(&self, budget: &mut Budget<'work>) -> io::Result<()> {
        budget.charge_work(IO_WORK).map_err(other)?;
        require(
            budget.storage() >= self.retained_storage(),
            "native staged owner not prepaid",
        )?;
        budget.reserve_storage(IO_STORAGE).map_err(other)?;
        self.revalidate_inner(budget)?;
        budget.release_storage(IO_STORAGE).map_err(other)
    }

    /// Called only after a distinct native root-control receiver has authenticated
    /// its sender and frame. Inert session bytes alone never expose the endpoint.
    pub(crate) fn matches_session(&self, session: &Session) -> io::Result<()> {
        require(
            session.transcript() == self.capsule.transcript
                && session.nonce() == self.capsule.nonce
                && session.deployment() == self.deployment
                && (session.controller().1, session.controller().2) == self.controller,
            "native activation session differs",
        )
    }

    pub(crate) fn try_activate(
        self,
        root: &wire::ControlEndpoint,
        session: &Session,
        budget: &mut Budget<'work>,
    ) -> io::Result<Activation<'work>> {
        self.revalidate(budget)?;
        self.matches_session(session)?;
        require(
            session.controller()
                == (
                    std::process::id(),
                    rustix::process::getuid().as_raw(),
                    rustix::process::getgid().as_raw(),
                ),
            "native activation targets another controller",
        )?;
        root.revalidate()?;
        require(
            root.fingerprint() != self.peer.fingerprint(),
            "native root/application endpoints alias",
        )?;
        let creator = rustix::net::sockopt::socket_peercred(root)?;
        let expected = (self.capsule.parent as i32, 0, 0);
        require(
            (
                creator.pid.as_raw_pid(),
                creator.uid.as_raw(),
                creator.gid.as_raw(),
            ) == expected,
            "native controller root endpoint creator",
        )?;
        match control::try_receive(
            root,
            expected,
            self.capsule.nonce,
            session.identity(),
            budget,
        )? {
            None => Ok(Activation::Pending(self)),
            Some(control::Kind::Activate) => {
                root.revalidate()?;
                self.revalidate(budget)?;
                Ok(Activation::Active(ActiveNativeApplication(self)))
            }
            Some(_) => Err(io::Error::other("native controller expected Activate")),
        }
    }
}

pub(crate) enum Activation<'work> {
    Pending(StagedNativeApplication<'work>),
    Active(ActiveNativeApplication<'work>),
}
pub(crate) struct ActiveNativeApplication<'work>(StagedNativeApplication<'work>);
impl<'work> ActiveNativeApplication<'work> {
    pub(crate) fn peer(&self, budget: &mut Budget<'work>) -> io::Result<&wire::ControlEndpoint> {
        self.0.revalidate(budget)?;
        Ok(&self.0.peer)
    }
    pub(crate) fn capsule(&self) -> &Capsule {
        &self.0.capsule
    }
}

#[cfg(test)]
mod tests;
