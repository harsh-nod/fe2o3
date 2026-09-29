//! Native observation reuses the frozen descriptor and process predicates, not
//! a legacy service/occurrence owner. Every syscall schedule and decode is prepaid.
use super::*;
use crate::{ProtectedServiceAdmissionErrorV2, ProtectedServiceAdmissionV2 as Service};
use fe2o3_compiler_closure_capability::CompilerExecutionCapabilityErrorV2;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_process_identity::{
    COMPILER_IMAGE_MEASUREMENT_STORAGE_V1, CompilerImageMeasurementErrorV1, CompilerImageRoleV1,
    measure_compiler_image_file_sha256_v1,
};
use fe2o3_protected_service_spawn::native_spawn::{
    ProtectedServiceSpawnErrorV2, RootTaskIdentityV2, RootTaskObservationV2,
};
use std::mem::size_of;

// Canonical V3 argv/environment encodings contain every observed byte plus
// length prefixes; no valid matching procfs record can exceed this bound.
const PROCESS_BYTES: usize = fe2o3_rustc_invocation::MAX_DESCRIPTOR_BYTES_V3;
const CWD_BYTES: usize = fe2o3_rustc_invocation::MAX_PATH_BYTES_V2;

#[derive(Debug)]
pub(crate) enum NativeObservationError {
    Resource(Resource),
    Service(ProtectedServiceAdmissionErrorV2),
    Spawn(ProtectedServiceSpawnErrorV2),
    Capability(CompilerExecutionCapabilityErrorV2),
    Inspection(CompilerExecutionSupervisionErrorV1),
}
impl std::fmt::Display for NativeObservationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(e) => write!(f, "{e}"),
            Self::Service(e) => write!(f, "{e}"),
            Self::Spawn(e) => write!(f, "{e}"),
            Self::Capability(e) => write!(f, "{e}"),
            Self::Inspection(e) => write!(f, "{e}"),
        }
    }
}
type Result<T> = std::result::Result<T, NativeObservationError>;
macro_rules! from_error {
    ($t:ty, $v:ident) => {
        impl From<$t> for NativeObservationError {
            fn from(e: $t) -> Self {
                Self::$v(e)
            }
        }
    };
}
from_error!(Resource, Resource);
from_error!(ProtectedServiceAdmissionErrorV2, Service);
from_error!(ProtectedServiceSpawnErrorV2, Spawn);
from_error!(CompilerExecutionCapabilityErrorV2, Capability);
from_error!(CompilerExecutionSupervisionErrorV1, Inspection);

// Only the two concrete, actually owned native custody paths can inspect inputs.
// A source borrows authority for this operation; it never manufactures an owner.
#[derive(Clone, Copy)]
pub(crate) enum NativeObservationSource<'a, 'trace, 'work> {
    Service(&'a Service),
    Root(&'a RootTaskObservationV2<'trace, 'work>),
}

impl NativeObservationSource<'_, '_, '_> {
    pub(crate) fn retained_storage(self) -> usize {
        match self {
            Self::Service(service) => service.retained_storage(),
            Self::Root(root) => root.retained_storage(),
        }
    }

    fn retain_root_identity(self, b: &mut Budget<'_>) -> Result<Option<RootTaskIdentityV2>> {
        match self {
            Self::Service(_) => Ok(None),
            Self::Root(root) => {
                let (identity, storage) = root.retain_identity(b)?;
                b.reserve_storage(storage.additional_storage())?;
                Ok(Some(identity))
            }
        }
    }

    fn validate_root_identity(
        self,
        expected: Option<&RootTaskIdentityV2>,
        b: &mut Budget<'_>,
    ) -> Result<()> {
        match (self, expected) {
            (Self::Service(_), None) => Ok(()),
            (Self::Root(_), Some(expected)) => {
                let actual = self.retain_root_identity(b)?;
                if actual
                    .as_ref()
                    .is_some_and(|actual| expected.matches(actual))
                {
                    Ok(())
                } else {
                    Err(CompilerExecutionSupervisionErrorV1::ProcessIdentityChanged.into())
                }
            }
            _ => Err(CompilerExecutionSupervisionErrorV1::InvalidObservation(
                "native observation changed its custody source",
            )
            .into()),
        }
    }

    fn client_identity(self, b: &mut Budget<'_>) -> Result<(u32, u64)> {
        match self {
            Self::Service(service) => {
                service.validate_continuity(b)?;
                Ok(service.client_process_identity())
            }
            Self::Root(root) => {
                root.validate_continuity(b)?;
                let pid = root.pid().as_raw_nonzero().get() as u32;
                let ticks = crate::linux::process_start_time_ticks_v2(pid, b)
                    .map_err(ProtectedServiceAdmissionErrorV2::from)?;
                root.validate_continuity(b)?;
                Ok((pid, ticks))
            }
        }
    }

    fn validate_client(self, expected: (u32, u64), b: &mut Budget<'_>) -> Result<()> {
        if self.client_identity(b)? != expected {
            return Err(CompilerExecutionSupervisionErrorV1::ProcessIdentityChanged.into());
        }
        Ok(())
    }

    fn remote(self, descriptor: i32, b: &mut Budget<'_>) -> Result<File> {
        match self {
            Self::Service(service) => remote(service, descriptor),
            Self::Root(root) => {
                let (file, storage) = root.duplicate_descriptor(descriptor, b)?;
                b.reserve_storage(storage.additional_storage())?;
                Ok(file)
            }
        }
    }
}

use NativeObservationSource as Source;

pub(crate) struct NativeObservation {
    client: (u32, u64),
    root_identity: Option<RootTaskIdentityV2>,
    proc_dir: RetainedDirectoryV1,
    invocation: RustcInvocationCapabilityV1,
    rustc: RetainedMeasuredFileV1,
    backend: RetainedMeasuredFileV1,
    artifact: RetainedDirectoryV1,
    identity: [u8; 32],
}

impl NativeObservation {
    const FRAME: usize = 32 * PROCESS_BYTES + 64 * 1024;
    const WORK: usize = 4096 * PROCESS_BYTES + 128 * 1024;

    pub(crate) fn observe_from(
        source: Source<'_, '_, '_>,
        b: &mut Budget<'_>,
    ) -> Result<(Self, usize)> {
        b.with_prepaid_scope(source.retained_storage(), 8, Self::WORK, Self::FRAME, |b| {
            let root_identity = source.retain_root_identity(b)?;
            let client = source.client_identity(b)?;
            let proc_dir = open_process_directory(client.0)?;
            let invocation = native_invocation(source, b)?;
            let rustc = measured(open_proc_component(&proc_dir, PROC_EXE, false)?, true, b)?;
            let backend = measured(source.remote(CODEGEN_BACKEND_FD, b)?, false, b)?;
            let artifact = RetainedDirectoryV1::admit(
                source.remote(ARTIFACT_DIRECTORY_FD, b)?,
                "artifact directory",
                false,
            )?;
            let mut observed = Self {
                client,
                root_identity,
                proc_dir,
                invocation,
                rustc,
                backend,
                artifact,
                identity: [0; 32],
            };
            observed.check_inputs(b)?;
            observed.identity = derive_observation_identity(
                client.0,
                client.1,
                observed.invocation.descriptor(),
                &observed.rustc,
                &observed.backend,
                &observed.artifact,
            )?;
            source.validate_client(client, b)?;
            let storage = observed.retained_storage()?;
            Ok((observed, storage))
        })
    }

    pub(crate) fn revalidate_from(
        &self,
        source: Source<'_, '_, '_>,
        b: &mut Budget<'_>,
    ) -> Result<()> {
        let floor = self
            .retained_storage()?
            .checked_add(source.retained_storage())
            .ok_or(Resource::Arithmetic)?;
        b.with_prepaid_scope(floor, 8, Self::WORK, Self::FRAME, |b| {
            // Numeric process facts cannot substitute another trace owner, even
            // on the same account. A root observation never falls back to Service.
            source.validate_root_identity(self.root_identity.as_ref(), b)?;
            source.validate_client(self.client, b)?;
            self.proc_dir.revalidate("client procfs", true)?;
            self.invocation.revalidate_native(b)?;
            self.artifact.revalidate("artifact directory", false)?;
            let invocation = native_invocation(source, b)?;
            if invocation.descriptor() != self.invocation.descriptor() {
                return Err(CompilerExecutionSupervisionErrorV1::InvocationChanged.into());
            }
            let rustc = measured(
                open_proc_component(&self.proc_dir, PROC_EXE, false)?,
                true,
                b,
            )?;
            let backend = measured(source.remote(CODEGEN_BACKEND_FD, b)?, false, b)?;
            let artifact = RetainedDirectoryV1::admit(
                source.remote(ARTIFACT_DIRECTORY_FD, b)?,
                "artifact directory",
                false,
            )?;
            if !self.rustc.same_observation(&rustc)
                || !self.backend.same_observation(&backend)
                || self.artifact.snapshot != artifact.snapshot
            {
                return Err(CompilerExecutionSupervisionErrorV1::IdentityChanged.into());
            }
            self.check_inputs(b)?;
            source.validate_client(self.client, b)?;
            Ok(())
        })
    }

    fn check_inputs(&self, b: &mut Budget<'_>) -> Result<()> {
        let argv_bytes = proc_bytes(&self.proc_dir, PROC_CMDLINE)?;
        let argv = parse_nul_strings(&argv_bytes, "rustc argv", MAX_ARGUMENTS_V3)?;
        let environment_bytes = proc_bytes(&self.proc_dir, PROC_ENVIRON)?;
        let environment = parse_environment(&environment_bytes)?;
        let mut cwd = [0; CWD_BYTES + 1];
        let length = rustix::fs::readlinkat_raw(&self.proc_dir.file, PROC_CWD, &mut cwd[..])
            .map_err(|e| inspect_io("read observed cwd", e))?;
        if length == 0 || length > CWD_BYTES {
            return Err(CompilerExecutionSupervisionErrorV1::InvalidObservation(
                "cwd exceeds bound",
            )
            .into());
        }
        let cwd = std::str::from_utf8(&cwd[..length]).map_err(|_| {
            CompilerExecutionSupervisionErrorV1::InvalidObservation("cwd is not UTF-8")
        })?;
        if !cwd.starts_with('/') || cwd.ends_with(" (deleted)") || cwd.as_bytes().contains(&0) {
            return Err(CompilerExecutionSupervisionErrorV1::InvalidObservation(
                "cwd is not live and absolute",
            )
            .into());
        }
        // The enclosing fixed schedule covers process parsing and comparison;
        // the capability itself has independently admitted all descriptor work.
        b.charge_work(8)?;
        validate_descriptor_observation(
            self.invocation.descriptor(),
            &argv,
            cwd,
            &environment,
            self.rustc.sha256,
            self.backend.sha256,
        )?;
        Ok(())
    }

    pub(crate) fn retained_storage(&self) -> Result<usize> {
        self.invocation
            .native_retained_storage()?
            .checked_add(size_of::<Self>() + 4096)
            .and_then(|n| {
                n.checked_add(
                    self.root_identity
                        .as_ref()
                        .map_or(0, RootTaskIdentityV2::retained_storage),
                )
            })
            .ok_or_else(|| Resource::Arithmetic.into())
    }
    pub(crate) fn descriptor(&self) -> &RustcInvocationDescriptorV3 {
        self.invocation.descriptor()
    }
    pub(crate) fn identity(&self) -> &[u8; 32] {
        &self.identity
    }
    pub(crate) fn output_dir(&self) -> PathBuf {
        PathBuf::from(format!("/proc/self/fd/{}", self.artifact.file.as_raw_fd()))
    }
}

fn remote(service: &Service, descriptor: i32) -> Result<File> {
    // Linux's ptrace/LSM decision is authoritative. No procfd reopening fallback,
    // same-UID substitution, permission change or caller-supplied descriptor.
    rustix::process::pidfd_getfd(
        service.client_pidfd(),
        descriptor,
        rustix::process::PidfdGetfdFlags::empty(),
    )
    .map(File::from)
    .map_err(|e| inspect_io("observe client descriptor", e))
}
fn native_invocation(
    source: Source<'_, '_, '_>,
    b: &mut Budget<'_>,
) -> Result<RustcInvocationCapabilityV1> {
    let file = source.remote(RUSTC_INVOCATION_CHILD_FD_V1, b)?;
    let (invocation, charge) = RustcInvocationCapabilityV1::from_file_native(file, b)?;
    b.reserve_storage(charge.additional_storage())?;
    Ok(invocation)
}
fn proc_bytes(directory: &RetainedDirectoryV1, name: &'static str) -> Result<Vec<u8>> {
    let file = open_proc_component(directory, name, true)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(PROCESS_BYTES + 1)
        .map_err(|_| Resource::Allocation)?;
    bytes.resize(PROCESS_BYTES + 1, 0);
    let n = rustix::io::read(&file, bytes.as_mut_slice())
        .map_err(|e| inspect_io("read client procfs", e))?;
    let mut extra = [0];
    if n == 0
        || n > PROCESS_BYTES
        || rustix::io::read(&file, &mut extra[..]).map_err(|e| inspect_io("check procfs EOF", e))?
            != 0
    {
        return Err(CompilerExecutionSupervisionErrorV1::InvalidObservation(
            "incomplete or oversized client procfs record",
        )
        .into());
    }
    bytes.truncate(n);
    Ok(bytes)
}
fn measured(file: File, executable: bool, b: &mut Budget<'_>) -> Result<RetainedMeasuredFileV1> {
    require_close_on_exec(&file, "observed image")?;
    let snapshot =
        measure_file_snapshot(&file, "observed image", MAX_EXECUTABLE_BYTES_V3, executable)?;
    let role = if executable {
        CompilerImageRoleV1::Executable
    } else {
        CompilerImageRoleV1::CodegenBackend
    };
    let sha256 = b.with_prepaid_scope(
        size_of::<File>(),
        8,
        8 + 64 * 1024,
        COMPILER_IMAGE_MEASUREMENT_STORAGE_V1
            + size_of::<CompilerImageMeasurementErrorV1<Resource>>(),
        |b| {
            let digest =
                measure_compiler_image_file_sha256_v1(&file, role, |work| b.charge_work(work))
                    .map_err(|error| match error {
                        CompilerImageMeasurementErrorV1::Work(e) => {
                            NativeObservationError::Resource(e)
                        }
                        CompilerImageMeasurementErrorV1::Io(source) => {
                            CompilerExecutionSupervisionErrorV1::Io {
                                operation: "hash observed image",
                                source,
                            }
                            .into()
                        }
                        CompilerImageMeasurementErrorV1::Invalid(reason) => {
                            CompilerExecutionSupervisionErrorV1::InvalidObservation(reason).into()
                        }
                    })?;
            // Preserve the supervisor's stronger retained-object predicate, including
            // owner/link metadata, around the shared streaming measurement.
            if measure_file_snapshot(&file, "observed image", MAX_EXECUTABLE_BYTES_V3, executable)?
                != snapshot
            {
                return Err(CompilerExecutionSupervisionErrorV1::IdentityChanged.into());
            }
            Ok::<_, NativeObservationError>(digest)
        },
    )?;
    Ok(RetainedMeasuredFileV1 {
        file,
        snapshot,
        sha256,
        maximum: MAX_EXECUTABLE_BYTES_V3,
        require_executable: executable,
    })
}
fn inspect_io(operation: &'static str, e: rustix::io::Errno) -> NativeObservationError {
    CompilerExecutionSupervisionErrorV1::Io {
        operation,
        source: io::Error::from(e),
    }
    .into()
}

#[cfg(test)]
mod image_tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    use std::io::Write;

    fn image() -> File {
        let mut file = File::from(
            rustix::fs::memfd_create(c"fe2o3-native-image-test", rustix::fs::MemfdFlags::CLOEXEC)
                .unwrap(),
        );
        file.write_all(b"native compiler image").unwrap();
        file
    }

    #[test]
    fn shared_measurement_retains_the_supervisor_snapshot_on_the_original_budget() {
        let file = image();
        let snapshot =
            measure_file_snapshot(&file, "test", MAX_EXECUTABLE_BYTES_V3, false).unwrap();
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, 1024 * 1024);
        budget.reserve_storage(size_of::<File>()).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let retained = measured(file, false, &mut budget).unwrap();
        assert_eq!(retained.snapshot, snapshot);
        let digest: [u8; 32] = Sha256::digest(b"native compiler image").into();
        assert_eq!(retained.sha256, digest);
        assert_eq!(budget.storage(), size_of::<File>());
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(
            budget.work(),
            8 + 5 * 64 * 1024 + b"native compiler image".len()
        );
        assert_eq!(budget.failed_work(), None);
    }

    #[test]
    fn shared_measurement_payload_denial_preserves_resource_error_and_scratch_accounting() {
        let total = 8 + 5 * 64 * 1024 + b"native compiler image".len();
        let mut work = Work::new(total - 1);
        let mut budget = Budget::new(&mut work, 1024 * 1024);
        budget.reserve_storage(size_of::<File>()).unwrap();
        assert!(matches!(
            measured(image(), false, &mut budget),
            Err(NativeObservationError::Resource(Resource::Work(_)))
        ));
        assert_eq!(budget.storage(), size_of::<File>());
        assert_eq!(budget.work(), 8 + 2 * 64 * 1024);
        assert_eq!(budget.failed_work(), Some(total));
        assert!(budget.peak_storage() > size_of::<File>());
    }
}
