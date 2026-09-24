//! Native prepared custody over the shared static-launch descriptor contract.
use crate::{
    AcceptedCompilerExecutionHandoffV2 as Accepted, ProtectedIssuerSupervisorV2 as Supervisor,
    launch_checks as checks,
};
use checks::*;
use fe2o3_broker_authority_service::current_process_start_time_ticks_v2;
use fe2o3_compiler_closure_capability::CompilerExecutionServiceLaunchCapabilityV2 as Capability;
use fe2o3_compiler_execution_protocol::CompilerExecutionServiceLaunchManifestV2 as Manifest;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_static_preexec_manifest::{
    PREEXEC_MANIFEST_BYTES_V1 as BYTES, StaticPreexecDescriptorV1 as Descriptor,
    StaticPreexecManifestV1 as StaticManifest, StaticPreexecObjectIdentityV1 as Object,
};
use std::{
    fmt,
    fs::File,
    mem::{align_of, size_of},
    os::fd::OwnedFd,
};

#[path = "launch_v2_error.rs"]
mod error;
#[path = "launch_v2_io.rs"]
mod image;
use ProtectedIssuerLaunchPreparationErrorV2 as Error;
pub use error::ProtectedIssuerLaunchPreparationErrorV2;
type Result<T> = std::result::Result<T, Error>;
const ENTRY: usize = 8;
use ProtectedIssuerLaunchStorageV2 as Storage;

/// Unreserved prepared-owner growth over the consumed accepted handoff.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtectedIssuerLaunchStorageV2(usize);
impl Storage {
    /// Reserve this growth while preserving the consumed handoff reservation.
    pub const fn additional_storage(self) -> usize {
        self.0
    }
}

/// Move-only native handoff, sealed program inputs and fixed static-launch table.
///
/// This is preparation only, not process creation, confinement, readiness,
/// service execution, proof or GPU authority. All nested operations share one
/// caller ledger. No admitted V1 owner or fallback is used. The V1-named static
/// manifest is an inert bounded wire record, not executable authority.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::PreparedProtectedIssuerLaunchV2;
/// fn clone<T: Clone>() {}
/// clone::<PreparedProtectedIssuerLaunchV2>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::PreparedProtectedIssuerLaunchV2;
/// fn descriptor<T: std::os::fd::AsFd>() {}
/// descriptor::<PreparedProtectedIssuerLaunchV2>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::{PreparedProtectedIssuerLaunchV1, PreparedProtectedIssuerLaunchV2};
/// fn upgrade(old: PreparedProtectedIssuerLaunchV1) -> PreparedProtectedIssuerLaunchV2 { old.into() }
/// ```
pub struct PreparedProtectedIssuerLaunchV2 {
    accepted: Accepted,
    launch_capability: Capability,
    launcher: File,
    issuer: File,
    static_manifest_file: File,
    sources: [File; SOURCE_COUNT_V1],
    stdout_reader: OwnedFd,
    stderr_reader: OwnedFd,
    readiness_reader: OwnedFd,
    static_manifest: StaticManifest,
    manifest_object: Object,
    retained: usize,
}
type Prepared = PreparedProtectedIssuerLaunchV2;

impl Prepared {
    /// Logical charge for each retained pipe endpoint, including receipt padding.
    pub const PIPE_STORAGE: usize = size_of::<(OwnedFd, Storage)>();
    /// Logical charge for the manifest File and its fixed sealed image.
    pub const MANIFEST_FILE_STORAGE: usize = size_of::<(File, Storage)>() + BYTES;
    /// Fixed metadata growth; descriptor/capability owners are charged separately.
    pub const OWNER_GROWTH: usize =
        size_of::<(StaticManifest, Object, usize, Storage)>() + align_of::<Self>();
    /// Outer logical allowance for fixed descriptor, pipe and manifest operations.
    /// Native authority/transport/parent/record operations charge separately.
    pub const WORK: usize = ENTRY + 512 * 1024 + 64 * BYTES;
    /// Logical staging and control scratch, not RSS, stack, kernel-memory or latency bounds.
    pub const SCRATCH: usize = 4 * size_of::<(Self, Storage)>() + 8 * BYTES + 8192;

    /// Returns inert fixed static-launch facts, without descriptors or process authority.
    pub const fn static_manifest(&self) -> &StaticManifest {
        &self.static_manifest
    }
    /// Returns the native canonical launch facts bound to the accepted handoff.
    pub const fn service_manifest(&self) -> &Manifest {
        self.launch_capability.manifest()
    }
    /// Full retained charge; retire only after drop or consuming transfer.
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }

    /// Rechecks every retained native owner, transferred object and parent identity.
    pub fn revalidate(&self, supervisor: &Supervisor, budget: &mut Budget<'_>) -> Result<()> {
        budget.charge_work(ENTRY)?;
        let floor = self
            .retained
            .checked_add(supervisor.retained_storage())
            .ok_or(Resource::Arithmetic)?;
        budget.with_prepaid_scope(floor, 0, Self::WORK - ENTRY, Self::SCRATCH, |b| {
            self.check(supervisor, b)
        })
    }

    fn check(&self, supervisor: &Supervisor, b: &mut Budget<'_>) -> Result<()> {
        supervisor.revalidate(b)?;
        self.accepted.revalidate(supervisor, b)?;
        self.launch_capability.revalidate(b)?;
        if self.service_manifest() != self.accepted.manifest() {
            return Err(Error::LaunchManifestMismatch);
        }
        let parent = parent_identity(b)?;
        if parent
            != (
                self.static_manifest.parent_pid(),
                self.static_manifest.parent_start_time(),
            )
        {
            return Err(Error::ParentChanged);
        }
        supervisor.revalidate_launch_inputs(&self.launcher, &self.issuer, &self.sources, b)?;
        self.accepted.revalidate_launch_peers(
            &self.sources[SERVICE_PEER_SOURCE_INDEX],
            &self.sources[CLIENT_PIDFD_SOURCE_INDEX],
            b,
        )?;
        self.launch_capability
            .validate_transfer(&self.sources[LAUNCH_MANIFEST_SOURCE_INDEX], b)?;
        checks::validate_pipe_pair(
            &self.sources[STDOUT_SOURCE_INDEX],
            &self.stdout_reader,
            "stdout",
        )?;
        checks::validate_pipe_pair(
            &self.sources[STDERR_SOURCE_INDEX],
            &self.stderr_reader,
            "stderr",
        )?;
        checks::validate_pipe_pair(
            &self.sources[READINESS_SOURCE_INDEX],
            &self.readiness_reader,
            "readiness",
        )?;
        checks::validate_pipe_end(
            &self.sources[STDIN_SOURCE_INDEX],
            rustix::fs::OFlags::RDONLY,
            "stdin",
        )?;
        checks::validate_pipe_end(
            &self.sources[STDOUT_SOURCE_INDEX],
            rustix::fs::OFlags::WRONLY,
            "stdout writer",
        )?;
        checks::validate_pipe_end(
            &self.sources[STDERR_SOURCE_INDEX],
            rustix::fs::OFlags::WRONLY,
            "stderr writer",
        )?;
        checks::validate_pipe_end(
            &self.sources[READINESS_SOURCE_INDEX],
            rustix::fs::OFlags::WRONLY,
            "readiness writer",
        )?;
        checks::validate_readiness_capacity(&self.sources[READINESS_SOURCE_INDEX])?;
        let objects = checks::source_identities(&self.sources)?;
        checks::validate_static_manifest_sources(
            self.static_manifest.executable(),
            self.static_manifest.descriptors(),
            &self.issuer,
            &objects,
        )?;
        image::validate(
            &self.static_manifest_file,
            &self.static_manifest,
            self.manifest_object,
        )?;
        checks::require_launcher_non_aliasing(
            &self.launcher,
            &self.issuer,
            &self.static_manifest_file,
            &self.sources,
        )?;
        Ok(())
    }
}

impl Supervisor {
    /// Consumes a prepaid authenticated handoff into native prepared custody.
    /// On refusal all consumed and newly acquired descriptors close before the
    /// caller retires the handoff reservation. No process is created here.
    pub fn prepare_launch(
        &self,
        accepted: Accepted,
        budget: &mut Budget<'_>,
    ) -> Result<(Prepared, Storage)> {
        budget.charge_work(ENTRY)?;
        let consumed = accepted.retained_storage();
        let floor = self
            .retained_storage()
            .checked_add(consumed)
            .ok_or(Resource::Arithmetic)?;
        budget.with_prepaid_scope(floor, 0, Prepared::WORK - ENTRY, Prepared::SCRATCH, |b| {
            self.revalidate(b)?;
            accepted.revalidate(self, b)?;
            let inputs = self.clone_launch_inputs(b)?;
            let mut growth = 0;
            keep(inputs.retained, &mut growth, b)?;
            let (service_peer, client_pidfd, bytes) = accepted.clone_launch_peers(b)?;
            keep(bytes, &mut growth, b)?;
            let (manifest, delta) = Manifest::new(
                accepted.manifest().client(),
                accepted.manifest().external_anchor_service(),
                self.policy(),
                b,
            )?;
            keep(delta.additional_storage(), &mut growth, b)?;
            let (launch_capability, delta) = Capability::create(manifest, b)?;
            keep(delta.additional_storage(), &mut growth, b)?;
            let (launch_manifest, delta) = launch_capability.try_clone_for_transfer(b)?;
            keep(delta.additional_storage(), &mut growth, b)?;
            let (stdin_reader, stdin_writer) = checks::protected_pipe("stdin")?;
            drop(stdin_writer);
            keep(Prepared::PIPE_STORAGE, &mut growth, b)?;
            let (stdout_reader, stdout_writer) = checks::protected_pipe("stdout")?;
            keep(2 * Prepared::PIPE_STORAGE, &mut growth, b)?;
            let (stderr_reader, stderr_writer) = checks::protected_pipe("stderr")?;
            keep(2 * Prepared::PIPE_STORAGE, &mut growth, b)?;
            let (readiness_reader, readiness_writer) = checks::protected_pipe("readiness")?;
            keep(2 * Prepared::PIPE_STORAGE, &mut growth, b)?;
            let sources = [
                stdin_reader.into(),
                stdout_writer.into(),
                stderr_writer.into(),
                inputs.root,
                service_peer,
                client_pidfd,
                inputs.policy,
                inputs.key,
                launch_manifest,
                readiness_writer.into(),
                inputs.anchor_peer,
                inputs.anchor_pidfd,
            ];
            let objects = checks::source_identities(&sources)?;
            let mut descriptors = [Descriptor::for_index(0, 0, objects[0])?; SOURCE_COUNT_V1];
            for (i, object) in objects.into_iter().enumerate() {
                descriptors[i] = Descriptor::for_index(i, DESTINATION_FDS_V1[i], object)?;
            }
            let (pid, start_time) = parent_identity(b)?;
            let static_manifest = StaticManifest::from_descriptors(
                pid,
                start_time,
                checks::object_identity(&inputs.issuer, "compiler issuer")?,
                &descriptors,
            )?;
            let (static_manifest_file, manifest_object) = image::create(&static_manifest)?;
            keep(
                Prepared::MANIFEST_FILE_STORAGE + Prepared::OWNER_GROWTH,
                &mut growth,
                b,
            )?;
            let retained = consumed.checked_add(growth).ok_or(Resource::Arithmetic)?;
            let prepared = Prepared {
                accepted,
                launch_capability,
                launcher: inputs.launcher,
                issuer: inputs.issuer,
                static_manifest_file,
                sources,
                stdout_reader,
                stderr_reader,
                readiness_reader,
                static_manifest,
                manifest_object,
                retained,
            };
            prepared.check(self, b)?;
            Ok((prepared, Storage(growth)))
        })
    }
}

fn keep(bytes: usize, growth: &mut usize, b: &mut Budget<'_>) -> Result<()> {
    let next = growth.checked_add(bytes).ok_or(Resource::Arithmetic)?;
    b.reserve_storage(bytes)?;
    *growth = next;
    Ok(())
}
fn parent_identity(b: &mut Budget<'_>) -> Result<(i32, u64)> {
    let pid = i32::try_from(std::process::id()).map_err(|_| Error::InvalidParentIdentity)?;
    Ok((pid, current_process_start_time_ticks_v2(b)?))
}
impl fmt::Debug for Prepared {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreparedProtectedIssuerLaunchV2")
            .field("authority", &"prepared-custody-only")
            .field("parent_pid", &self.static_manifest.parent_pid())
            .field("launch", &self.service_manifest().identity())
            .finish_non_exhaustive()
    }
}

const _: () = {
    use fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1 as Ledger;
    const fn envelope<T>() -> usize {
        size_of::<Result<T>>().saturating_sub(size_of::<T>())
            + size_of::<std::thread::Result<Result<T>>>().saturating_sub(size_of::<T>())
    }
    assert!(SOURCE_COUNT_V1 <= fe2o3_static_preexec_manifest::PREEXEC_MAX_DESCRIPTORS);
    assert!(size_of::<StaticManifest>() <= 2 * BYTES);
    assert!(
        size_of::<(Prepared, Storage)>()
            <= size_of::<Accepted>()
                + size_of::<Capability>()
                + 15 * size_of::<File>()
                + 3 * size_of::<OwnedFd>()
                + Prepared::OWNER_GROWTH
    );
    assert!(
        8 * size_of::<Error>()
            + 128 * size_of::<usize>()
            + 8 * size_of::<rustix::fs::Stat>()
            + size_of::<Ledger>()
            + size_of::<std::result::Result<(), Resource>>()
            + 2 * size_of::<bool>()
            + envelope::<(Prepared, Storage)>()
            + envelope::<()>()
            <= 8192
    );
};

#[cfg(test)]
#[path = "launch_v2_tests.rs"]
pub(crate) mod tests;
