//! Native observation reuses the frozen descriptor and process predicates, not
//! a legacy service/occurrence owner. Every syscall schedule and decode is prepaid.
use super::*;
use crate::{ProtectedServiceAdmissionErrorV2, ProtectedServiceAdmissionV2 as Service};
use fe2o3_compiler_closure_capability::CompilerExecutionCapabilityErrorV2;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
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
    Capability(CompilerExecutionCapabilityErrorV2),
    Inspection(CompilerExecutionSupervisionErrorV1),
}
impl std::fmt::Display for NativeObservationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(e) => write!(f, "{e}"),
            Self::Service(e) => write!(f, "{e}"),
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
from_error!(CompilerExecutionCapabilityErrorV2, Capability);
from_error!(CompilerExecutionSupervisionErrorV1, Inspection);

pub(crate) struct NativeObservation {
    client: (u32, u64),
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

    pub(crate) fn observe(service: &Service, b: &mut Budget<'_>) -> Result<(Self, usize)> {
        b.with_prepaid_scope(
            service.retained_storage(),
            8,
            Self::WORK,
            Self::FRAME,
            |b| {
                service.validate_continuity(b)?;
                let client = service.client_process_identity();
                let proc_dir = open_process_directory(client.0)?;
                let invocation = native_invocation(service, b)?;
                let rustc = measured(open_proc_component(&proc_dir, PROC_EXE, false)?, true, b)?;
                let backend = measured(remote(service, CODEGEN_BACKEND_FD)?, false, b)?;
                let artifact = RetainedDirectoryV1::admit(
                    remote(service, ARTIFACT_DIRECTORY_FD)?,
                    "artifact directory",
                    false,
                )?;
                let mut observed = Self {
                    client,
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
                service.validate_continuity(b)?;
                let storage = observed.retained_storage()?;
                Ok((observed, storage))
            },
        )
    }

    pub(crate) fn revalidate(&self, service: &Service, b: &mut Budget<'_>) -> Result<()> {
        let floor = self
            .retained_storage()?
            .checked_add(service.retained_storage())
            .ok_or(Resource::Arithmetic)?;
        b.with_prepaid_scope(floor, 8, Self::WORK, Self::FRAME, |b| {
            service.validate_continuity(b)?;
            if service.client_process_identity() != self.client {
                return Err(CompilerExecutionSupervisionErrorV1::ProcessIdentityChanged.into());
            }
            self.proc_dir.revalidate("client procfs", true)?;
            self.invocation.revalidate_native(b)?;
            self.artifact.revalidate("artifact directory", false)?;
            let invocation = native_invocation(service, b)?;
            if invocation.descriptor() != self.invocation.descriptor() {
                return Err(CompilerExecutionSupervisionErrorV1::InvocationChanged.into());
            }
            let rustc = measured(
                open_proc_component(&self.proc_dir, PROC_EXE, false)?,
                true,
                b,
            )?;
            let backend = measured(remote(service, CODEGEN_BACKEND_FD)?, false, b)?;
            let artifact = RetainedDirectoryV1::admit(
                remote(service, ARTIFACT_DIRECTORY_FD)?,
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
            service.validate_continuity(b)?;
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
fn native_invocation(service: &Service, b: &mut Budget<'_>) -> Result<RustcInvocationCapabilityV1> {
    let file = remote(service, RUSTC_INVOCATION_CHILD_FD_V1)?;
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
    let length = usize::try_from(snapshot.length).map_err(|_| Resource::Arithmetic)?;
    let work = length
        .checked_mul(64)
        .and_then(|n| n.checked_add(64 * 1024))
        .ok_or(Resource::Arithmetic)?;
    let sha256 = b.with_prepaid_scope(size_of::<File>(), 8, work, 128 * 1024, |_| {
        let mut hash = Sha256::new();
        let mut buffer = [0; 64 * 1024];
        let mut offset = 0;
        while offset < length {
            let count = (length - offset).min(buffer.len());
            let n = rustix::io::pread(&file, &mut buffer[..count], offset as u64)
                .map_err(|e| inspect_io("hash observed image", e))?;
            if n != count {
                return Err(CompilerExecutionSupervisionErrorV1::InvalidObservation(
                    "short observed image read",
                )
                .into());
            }
            hash.update(&buffer[..count]);
            offset += count;
        }
        let mut extra = [0];
        if rustix::io::pread(&file, &mut extra[..], snapshot.length)
            .map_err(|e| inspect_io("check image EOF", e))?
            != 0
            || measure_file_snapshot(&file, "observed image", MAX_EXECUTABLE_BYTES_V3, executable)?
                != snapshot
        {
            return Err(CompilerExecutionSupervisionErrorV1::IdentityChanged.into());
        }
        Ok::<_, NativeObservationError>(hash.finalize().into())
    })?;
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
