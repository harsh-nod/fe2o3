//! Single admission boundary for protected rustc invocation capabilities.

use std::env;
use std::ffi::OsStr;
use std::fmt;
use std::fs::{self, File, Metadata};
use std::io::Read as _;
use std::os::fd::RawFd;
use std::os::unix::fs::MetadataExt as _;
use std::path::Path;

use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_compiler_closure_capability::{
    RUSTC_INVOCATION_CHILD_FD_V1, RustcInvocationCapabilityV1,
};
use fe2o3_rustc_invocation::{CompileEnvironmentV2, RustcInvocationDescriptorV3};
use sha2::{Digest as _, Sha256};

use crate::pipeline_selection::{CompilationRouteV1, RustcInvocationPolicyV1};

const EXACT_PROTECTED_TARGET_V1: &str = "gfx942:xnack-";
const EXPECTED_COMPILER_CLOSURE_SHA256_ENV_V1: &str = "FE2O3_EXPECTED_COMPILER_CLOSURE_SHA256_V1";
const CODEGEN_BACKEND_BUILD_OBSERVATION_ENV_V2: &str = "FE2O3_CODEGEN_BACKEND_BUILD_OBSERVATION_V2";
const QUALIFICATION_CODEGEN_BACKEND_SHA256_ENV_V1: &str =
    "FE2O3_QUALIFICATION_CODEGEN_BACKEND_SHA256_V1";
const NON_PRODUCTION_UNPROTECTED_AUTHORITY_VALIDATION_ENV_V1: &str =
    "FE2O3_NON_PRODUCTION_UNPROTECTED_AUTHORITY_VALIDATION_V1";
const RUNNING_RUSTC_PATH: &str = "/proc/self/exe";

/// One retained V3 descriptor that exactly matched this rustc process.
pub(crate) struct AdmittedProtectedRustcInvocationV1 {
    capability: RustcInvocationCapabilityV1,
}

impl AdmittedProtectedRustcInvocationV1 {
    /// Revalidates the retained image and returns its complete canonical closure.
    pub(crate) fn compiler_closure(&self) -> Result<CompilerClosureV2, String> {
        self.capability.revalidate()?;
        Ok(*self.capability.descriptor().compiler_closure())
    }

    /// Consumes admission immediately before protected publication, repeats
    /// every live-process observation, and retains the sealed V3 capability.
    pub(crate) fn finish_for_publication(
        self,
    ) -> Result<FinishedProtectedRustcInvocationV3, ProtectedRustcInvocationErrorV1> {
        self.capability
            .revalidate()
            .map_err(ProtectedRustcInvocationErrorV1::RetainedCapabilityChanged)?;
        let observation = RustcProcessObservationV1::capture(self.capability.descriptor())?;
        self.finish_after_publication_observation(observation)
    }

    #[cfg(test)]
    fn finish_for_publication_with_observation(
        self,
        observation: RustcProcessObservationV1,
    ) -> Result<FinishedProtectedRustcInvocationV3, ProtectedRustcInvocationErrorV1> {
        self.capability
            .revalidate()
            .map_err(ProtectedRustcInvocationErrorV1::RetainedCapabilityChanged)?;
        self.finish_after_publication_observation(observation)
    }

    fn finish_after_publication_observation(
        self,
        observation: RustcProcessObservationV1,
    ) -> Result<FinishedProtectedRustcInvocationV3, ProtectedRustcInvocationErrorV1> {
        let admitted = validate_capability(self.capability, observation)?;
        Ok(FinishedProtectedRustcInvocationV3 {
            capability: admitted.capability,
        })
    }
}

/// Move-only custody of the exact sealed invocation after final live-process
/// remeasurement. It is private compiler authority, not a serializable receipt.
pub(crate) struct FinishedProtectedRustcInvocationV3 {
    capability: RustcInvocationCapabilityV1,
}

impl FinishedProtectedRustcInvocationV3 {
    /// Borrows the exact canonical V3 descriptor retained by the sealed image.
    pub(crate) fn descriptor(&self) -> &RustcInvocationDescriptorV3 {
        self.capability.descriptor()
    }

    /// Revalidates the retained immutable capability without projecting it to
    /// a copyable publication credential.
    pub(crate) fn revalidate(&self) -> Result<(), String> {
        self.capability.revalidate()
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum ProtectedRustcInvocationErrorV1 {
    Capability(String),
    UnexpectedProtectedSignals {
        descriptor_present: bool,
        compiler_closure_marker_present: bool,
        backend_marker_present: bool,
        qualification_backend_marker_present: bool,
    },
    QualificationObservationsMissing,
    QualificationCodegenBackendObservationMismatch,
    Observation(String),
    ArgumentsMismatch,
    WorkingDirectoryMismatch,
    CompileEnvironmentMismatch,
    TargetMismatch {
        found: String,
    },
    BackendPathMismatch {
        found: String,
    },
    RustcClosurePinMismatch,
    CodegenBackendClosurePinMismatch,
    RunningRustcMismatch,
    RunningCodegenBackendMismatch,
    InvalidClosedObservation {
        name: &'static str,
    },
    CompilerClosureObservationMismatch,
    CodegenBackendObservationMismatch,
    RetainedCapabilityChanged(String),
}

impl fmt::Display for ProtectedRustcInvocationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Capability(detail) => write!(
                formatter,
                "cannot admit canonical fd {RUSTC_INVOCATION_CHILD_FD_V1} as a sealed V3 capability: {detail}"
            ),
            Self::UnexpectedProtectedSignals {
                descriptor_present,
                compiler_closure_marker_present,
                backend_marker_present,
                qualification_backend_marker_present,
            } => write!(
                formatter,
                "rustc invocation signals are forbidden on this pipeline route (descriptor: {descriptor_present}, compiler-closure marker: {compiler_closure_marker_present}, protected-backend marker: {backend_marker_present}, qualification-backend marker: {qualification_backend_marker_present})"
            ),
            Self::QualificationObservationsMissing => formatter.write_str(
                "qualification-observed rustc requires paired compiler-closure and qualification-backend observations",
            ),
            Self::QualificationCodegenBackendObservationMismatch => formatter.write_str(
                "qualification backend observation differs from the retained loaded backend image",
            ),
            Self::Observation(detail) => write!(
                formatter,
                "cannot observe the current rustc process: {detail}"
            ),
            Self::ArgumentsMismatch => formatter.write_str(
                "sealed V3 argv, including argv0, differs from the current UTF-8 rustc argv",
            ),
            Self::WorkingDirectoryMismatch => formatter.write_str(
                "sealed V3 working directory differs from the canonical current directory",
            ),
            Self::CompileEnvironmentMismatch => formatter.write_str(
                "sealed V3 compile environment differs from the complete current environment",
            ),
            Self::TargetMismatch { found } => write!(
                formatter,
                "sealed V3 target must be exactly {EXACT_PROTECTED_TARGET_V1}, found {found}"
            ),
            Self::BackendPathMismatch { found } => write!(
                formatter,
                "sealed V3 backend path must be the retained backend capability {}, found {found}",
                fe2o3_artifact_transaction::BROKERED_CODEGEN_BACKEND_PATH_V1,
            ),
            Self::RustcClosurePinMismatch => formatter.write_str(
                "sealed V3 rustc pin differs from the rustc role in its full compiler closure",
            ),
            Self::CodegenBackendClosurePinMismatch => formatter.write_str(
                "sealed V3 backend pin differs from the backend role in its full compiler closure",
            ),
            Self::RunningRustcMismatch => formatter
                .write_str("sealed V3 rustc pin differs from the running /proc/self/exe image"),
            Self::RunningCodegenBackendMismatch => formatter.write_str(
                "sealed V3 backend pin differs from the retained backend image loaded by rustc",
            ),
            Self::InvalidClosedObservation { name } => write!(
                formatter,
                "closed observation {name} is not one canonical lowercase nonzero SHA-256"
            ),
            Self::CompilerClosureObservationMismatch => write!(
                formatter,
                "closed observation {EXPECTED_COMPILER_CLOSURE_SHA256_ENV_V1} differs from the sealed V3 full compiler closure"
            ),
            Self::CodegenBackendObservationMismatch => write!(
                formatter,
                "closed observation {CODEGEN_BACKEND_BUILD_OBSERVATION_ENV_V2} differs from the sealed V3 backend pin"
            ),
            Self::RetainedCapabilityChanged(detail) => {
                write!(
                    formatter,
                    "retained sealed V3 capability changed during admission: {detail}"
                )
            }
        }
    }
}

impl std::error::Error for ProtectedRustcInvocationErrorV1 {}

pub(crate) fn admit_for_codegen(
    route: CompilationRouteV1,
) -> Result<Option<AdmittedProtectedRustcInvocationV1>, ProtectedRustcInvocationErrorV1> {
    let explicit_unprotected_qualification = explicit_unprotected_qualification_enabled();
    let compiler_closure_marker_present =
        env::var_os(EXPECTED_COMPILER_CLOSURE_SHA256_ENV_V1).is_some();
    let backend_marker_present = env::var_os(CODEGEN_BACKEND_BUILD_OBSERVATION_ENV_V2).is_some();
    let qualification_backend_marker_present =
        env::var_os(QUALIFICATION_CODEGEN_BACKEND_SHA256_ENV_V1).is_some();
    let qualification_observations_authenticated = if route
        .rustc_invocation_policy(explicit_unprotected_qualification)
        == RustcInvocationPolicyV1::QualificationObserved
        && compiler_closure_marker_present
        && qualification_backend_marker_present
    {
        authenticate_qualification_observations()?
    } else {
        false
    };
    admit_for_codegen_at(
        route,
        explicit_unprotected_qualification,
        RUSTC_INVOCATION_CHILD_FD_V1,
        compiler_closure_marker_present,
        backend_marker_present,
        qualification_backend_marker_present,
        qualification_observations_authenticated,
    )
}

fn admit_for_codegen_at(
    route: CompilationRouteV1,
    explicit_unprotected_qualification: bool,
    child_fd: RawFd,
    compiler_closure_marker_present: bool,
    backend_marker_present: bool,
    qualification_backend_marker_present: bool,
    qualification_observations_authenticated: bool,
) -> Result<Option<AdmittedProtectedRustcInvocationV1>, ProtectedRustcInvocationErrorV1> {
    match route.rustc_invocation_policy(explicit_unprotected_qualification) {
        RustcInvocationPolicyV1::Unmanaged => {
            reject_unexpected_rustc_signals_at(
                child_fd,
                compiler_closure_marker_present,
                backend_marker_present,
                qualification_backend_marker_present,
            )?;
            return Ok(None);
        }
        RustcInvocationPolicyV1::QualificationObserved => {
            reject_unexpected_rustc_signals_at(child_fd, false, backend_marker_present, false)?;
            if !compiler_closure_marker_present || !qualification_backend_marker_present {
                return Err(ProtectedRustcInvocationErrorV1::QualificationObservationsMissing);
            }
            if !qualification_observations_authenticated {
                return Err(
                    ProtectedRustcInvocationErrorV1::QualificationCodegenBackendObservationMismatch,
                );
            }
            return Ok(None);
        }
        RustcInvocationPolicyV1::ProtectedV3 => {
            if qualification_backend_marker_present {
                return Err(
                    ProtectedRustcInvocationErrorV1::UnexpectedProtectedSignals {
                        descriptor_present: false,
                        compiler_closure_marker_present: false,
                        backend_marker_present: false,
                        qualification_backend_marker_present: true,
                    },
                );
            }
        }
    }

    let capability = retain_inherited_capability_at(child_fd)?;
    let observation = RustcProcessObservationV1::capture(capability.descriptor())?;
    validate_capability(capability, observation).map(Some)
}

fn reject_unexpected_rustc_signals_at(
    child_fd: RawFd,
    compiler_closure_marker_present: bool,
    backend_marker_present: bool,
    qualification_backend_marker_present: bool,
) -> Result<(), ProtectedRustcInvocationErrorV1> {
    // Do not close an unexpected descriptor here: an ordinary rustc process
    // may have reused this number for a Rust-owned file. Returning an error
    // prevents fallback without violating that owner's descriptor lifetime.
    // SAFETY: fcntl observes but does not consume the descriptor.
    let descriptor_present = if unsafe { libc::fcntl(child_fd, libc::F_GETFD) } >= 0 {
        true
    } else {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::EBADF) {
            false
        } else {
            return Err(ProtectedRustcInvocationErrorV1::Capability(format!(
                "cannot close reserved inherited descriptor {child_fd}: {error}"
            )));
        }
    };
    if descriptor_present
        || compiler_closure_marker_present
        || backend_marker_present
        || qualification_backend_marker_present
    {
        Err(
            ProtectedRustcInvocationErrorV1::UnexpectedProtectedSignals {
                descriptor_present,
                compiler_closure_marker_present,
                backend_marker_present,
                qualification_backend_marker_present,
            },
        )
    } else {
        Ok(())
    }
}

fn explicit_unprotected_qualification_enabled() -> bool {
    cfg!(debug_assertions)
        && env::var_os(NON_PRODUCTION_UNPROTECTED_AUTHORITY_VALIDATION_ENV_V1).as_deref()
            == Some(OsStr::new("1"))
}

fn authenticate_qualification_observations() -> Result<bool, ProtectedRustcInvocationErrorV1> {
    let _compiler_closure = sha256_environment(EXPECTED_COMPILER_CLOSURE_SHA256_ENV_V1)?;
    let backend_observation = sha256_environment(QUALIFICATION_CODEGEN_BACKEND_SHA256_ENV_V1)?;
    let running_backend = measure_bounded_regular_file(
        Path::new(fe2o3_artifact_transaction::BROKERED_CODEGEN_BACKEND_PATH_V1),
        "qualification backend",
    )
    .map_err(ProtectedRustcInvocationErrorV1::Observation)?;
    if backend_observation != running_backend {
        return Err(
            ProtectedRustcInvocationErrorV1::QualificationCodegenBackendObservationMismatch,
        );
    }
    Ok(true)
}

fn sha256_environment(name: &'static str) -> Result<[u8; 32], ProtectedRustcInvocationErrorV1> {
    let encoded = env::var(name)
        .map_err(|_| ProtectedRustcInvocationErrorV1::InvalidClosedObservation { name })?;
    decode_sha256_observation(name, &encoded)
}

fn retain_inherited_capability_at(
    child_fd: RawFd,
) -> Result<RustcInvocationCapabilityV1, ProtectedRustcInvocationErrorV1> {
    let admission = RustcInvocationCapabilityV1::from_inherited_at(child_fd);
    // A successful admission retains a private close-on-exec duplicate. Always close the
    // canonical inherited slot so rejected descriptor versions cannot remain for a fallback.
    // SAFETY: close accepts any integer descriptor and reports EBADF for an absent slot.
    let close_result = unsafe { libc::close(child_fd) };

    match admission {
        Ok(capability) if close_result == 0 => Ok(capability),
        Ok(_) => Err(ProtectedRustcInvocationErrorV1::Capability(format!(
            "cannot close consumed inherited descriptor {child_fd}: {}",
            std::io::Error::last_os_error()
        ))),
        Err(detail) => Err(ProtectedRustcInvocationErrorV1::Capability(detail)),
    }
}

struct RustcProcessObservationV1 {
    argv: Vec<String>,
    canonical_working_directory: String,
    compile_environment: CompileEnvironmentV2,
    running_rustc_sha256: [u8; 32],
    running_codegen_backend_sha256: [u8; 32],
}

impl RustcProcessObservationV1 {
    fn capture(
        descriptor: &RustcInvocationDescriptorV3,
    ) -> Result<Self, ProtectedRustcInvocationErrorV1> {
        let argv = env::args_os()
            .enumerate()
            .map(|(index, value)| {
                value.into_string().map_err(|_| {
                    ProtectedRustcInvocationErrorV1::Observation(format!(
                        "argv[{index}] is not UTF-8"
                    ))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let current_directory = env::current_dir().map_err(|error| {
            ProtectedRustcInvocationErrorV1::Observation(format!(
                "cannot read current directory: {error}"
            ))
        })?;
        let canonical_working_directory =
            fs::canonicalize(&current_directory).map_err(|error| {
                ProtectedRustcInvocationErrorV1::Observation(format!(
                    "cannot canonicalize current directory {}: {error}",
                    current_directory.display()
                ))
            })?;
        let canonical_working_directory = canonical_working_directory
            .into_os_string()
            .into_string()
            .map_err(|_| {
                ProtectedRustcInvocationErrorV1::Observation(
                    "canonical current directory is not UTF-8".to_owned(),
                )
            })?;
        let compile_environment = CompileEnvironmentV2::capture_current().map_err(|error| {
            ProtectedRustcInvocationErrorV1::Observation(format!(
                "cannot capture complete compile environment: {error}"
            ))
        })?;
        let running_rustc_sha256 =
            fe2o3_process_identity::measure_executable_sha256_v3(Path::new(RUNNING_RUSTC_PATH))
                .map_err(|error| {
                    ProtectedRustcInvocationErrorV1::Observation(format!(
                        "cannot measure running rustc: {error}"
                    ))
                })?;
        let running_codegen_backend_sha256 =
            measure_bounded_regular_file(Path::new(descriptor.codegen_backend_path()), "backend")
                .map_err(ProtectedRustcInvocationErrorV1::Observation)?;
        Ok(Self {
            argv,
            canonical_working_directory,
            compile_environment,
            running_rustc_sha256,
            running_codegen_backend_sha256,
        })
    }
}

fn validate_capability(
    capability: RustcInvocationCapabilityV1,
    observation: RustcProcessObservationV1,
) -> Result<AdmittedProtectedRustcInvocationV1, ProtectedRustcInvocationErrorV1> {
    let descriptor = capability.descriptor();
    let closure = descriptor.compiler_closure();

    if descriptor
        .rustc()
        .argv()
        .ne(observation.argv.iter().map(String::as_str))
    {
        return Err(ProtectedRustcInvocationErrorV1::ArgumentsMismatch);
    }
    if descriptor.rustc().working_directory() != observation.canonical_working_directory {
        return Err(ProtectedRustcInvocationErrorV1::WorkingDirectoryMismatch);
    }
    if descriptor.compile_environment() != &observation.compile_environment {
        return Err(ProtectedRustcInvocationErrorV1::CompileEnvironmentMismatch);
    }
    if descriptor.amd_target() != EXACT_PROTECTED_TARGET_V1 {
        return Err(ProtectedRustcInvocationErrorV1::TargetMismatch {
            found: descriptor.amd_target().to_owned(),
        });
    }
    if descriptor.codegen_backend_path()
        != fe2o3_artifact_transaction::BROKERED_CODEGEN_BACKEND_PATH_V1
    {
        return Err(ProtectedRustcInvocationErrorV1::BackendPathMismatch {
            found: descriptor.codegen_backend_path().to_owned(),
        });
    }
    if descriptor.rustc_executable_sha256() != &closure.rustc_executable_sha256() {
        return Err(ProtectedRustcInvocationErrorV1::RustcClosurePinMismatch);
    }
    if descriptor.codegen_backend_sha256() != &closure.codegen_backend_sha256() {
        return Err(ProtectedRustcInvocationErrorV1::CodegenBackendClosurePinMismatch);
    }
    if observation.running_rustc_sha256 != closure.rustc_executable_sha256() {
        return Err(ProtectedRustcInvocationErrorV1::RunningRustcMismatch);
    }
    if observation.running_codegen_backend_sha256 != closure.codegen_backend_sha256() {
        return Err(ProtectedRustcInvocationErrorV1::RunningCodegenBackendMismatch);
    }

    let compiler_closure_observation = closed_sha256_observation(
        &observation.compile_environment,
        EXPECTED_COMPILER_CLOSURE_SHA256_ENV_V1,
    )?;
    if compiler_closure_observation != closure.identity_sha256() {
        return Err(ProtectedRustcInvocationErrorV1::CompilerClosureObservationMismatch);
    }
    let backend_observation = closed_sha256_observation(
        &observation.compile_environment,
        CODEGEN_BACKEND_BUILD_OBSERVATION_ENV_V2,
    )?;
    if backend_observation != closure.codegen_backend_sha256() {
        return Err(ProtectedRustcInvocationErrorV1::CodegenBackendObservationMismatch);
    }

    capability
        .revalidate()
        .map_err(ProtectedRustcInvocationErrorV1::RetainedCapabilityChanged)?;
    Ok(AdmittedProtectedRustcInvocationV1 { capability })
}

fn closed_sha256_observation(
    environment: &CompileEnvironmentV2,
    name: &'static str,
) -> Result<[u8; 32], ProtectedRustcInvocationErrorV1> {
    let encoded = environment
        .entries()
        .iter()
        .find(|entry| entry.key() == name)
        .map(|entry| entry.value())
        .ok_or(ProtectedRustcInvocationErrorV1::InvalidClosedObservation { name })?;
    decode_sha256_observation(name, encoded)
}

fn decode_sha256_observation(
    name: &'static str,
    encoded: &str,
) -> Result<[u8; 32], ProtectedRustcInvocationErrorV1> {
    if encoded.len() != 64
        || !encoded
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(ProtectedRustcInvocationErrorV1::InvalidClosedObservation { name });
    }
    let mut digest = [0_u8; 32];
    for (output, pair) in digest.iter_mut().zip(encoded.as_bytes().chunks_exact(2)) {
        *output = (lower_hex_value(pair[0]) << 4) | lower_hex_value(pair[1]);
    }
    if digest == [0; 32] {
        return Err(ProtectedRustcInvocationErrorV1::InvalidClosedObservation { name });
    }
    Ok(digest)
}

fn lower_hex_value(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        _ => unreachable!("lowercase hex was checked above"),
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct FileSnapshotV1 {
    device: u64,
    inode: u64,
    mode: u32,
    size: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

impl FileSnapshotV1 {
    fn from_metadata(metadata: &Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            mode: metadata.mode(),
            size: metadata.len(),
            modified_seconds: metadata.mtime(),
            modified_nanoseconds: metadata.mtime_nsec(),
            changed_seconds: metadata.ctime(),
            changed_nanoseconds: metadata.ctime_nsec(),
        }
    }
}

fn measure_bounded_regular_file(path: &Path, label: &str) -> Result<[u8; 32], String> {
    let mut file = File::open(path)
        .map_err(|error| format!("cannot open {label} {}: {error}", path.display()))?;
    let initial = file
        .metadata()
        .map_err(|error| format!("cannot inspect {label} {}: {error}", path.display()))?;
    if !initial.is_file()
        || initial.len() == 0
        || initial.len() > fe2o3_process_identity::MAX_EXECUTABLE_BYTES_V3
    {
        return Err(format!(
            "{label} {} has invalid bounded regular-file size {}",
            path.display(),
            initial.len()
        ));
    }
    let snapshot = FileSnapshotV1::from_metadata(&initial);
    let mut digest = Sha256::new();
    let mut remaining = initial.len();
    let mut buffer = [0_u8; 64 * 1024];
    while remaining != 0 {
        let requested = usize::try_from(remaining.min(buffer.len() as u64))
            .expect("bounded backend chunk fits usize");
        let read = file
            .read(&mut buffer[..requested])
            .map_err(|error| format!("cannot hash {label} {}: {error}", path.display()))?;
        if read == 0 {
            return Err(format!(
                "{label} {} became shorter while hashing",
                path.display()
            ));
        }
        digest.update(&buffer[..read]);
        remaining -= read as u64;
    }
    if file
        .read(&mut buffer[..1])
        .map_err(|error| format!("cannot finish hashing {label} {}: {error}", path.display()))?
        != 0
    {
        return Err(format!("{label} {} grew while hashing", path.display()));
    }
    let final_metadata = file
        .metadata()
        .map_err(|error| format!("cannot re-inspect {label} {}: {error}", path.display()))?;
    if FileSnapshotV1::from_metadata(&final_metadata) != snapshot {
        return Err(format!("{label} {} changed while hashing", path.display()));
    }
    Ok(digest.finalize().into())
}

#[cfg(test)]
mod tests;
