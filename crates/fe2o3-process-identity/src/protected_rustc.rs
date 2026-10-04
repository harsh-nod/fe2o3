//! Shared semantic validation for local and externally supervised rustc observations.

use std::error::Error;
use std::fmt;

use fe2o3_rustc_invocation::{CompileEnvironmentV2, RustcInvocationDescriptorV3};

/// Closed environment binding for the complete compiler closure.
pub const EXPECTED_COMPILER_CLOSURE_SHA256_ENV_V1: &str =
    "FE2O3_EXPECTED_COMPILER_CLOSURE_SHA256_V1";
/// Closed environment binding for the exact codegen backend image.
pub const CODEGEN_BACKEND_BUILD_OBSERVATION_ENV_V2: &str =
    "FE2O3_CODEGEN_BACKEND_BUILD_OBSERVATION_V2";

/// Mismatch between a canonical V3 invocation and one process observation.
#[derive(Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ProtectedRustcProcessValidationErrorV1 {
    ArgumentsMismatch,
    WorkingDirectoryMismatch,
    CompileEnvironmentMismatch,
    TargetMismatch { found: String },
    BackendPathMismatch { found: String },
    ArtifactDirectoryPathMismatch { found: String },
    RustcClosurePinMismatch,
    CodegenBackendClosurePinMismatch,
    RunningRustcMismatch,
    RunningCodegenBackendMismatch,
    CompilerClosureObservationMismatch,
    CodegenBackendObservationMismatch,
    InvalidClosedObservation { name: &'static str },
}

impl fmt::Display for ProtectedRustcProcessValidationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ArgumentsMismatch => {
                formatter.write_str("observed rustc arguments do not match the V3 descriptor")
            }
            Self::WorkingDirectoryMismatch => formatter
                .write_str("observed rustc working directory does not match the V3 descriptor"),
            Self::CompileEnvironmentMismatch => {
                formatter.write_str("observed rustc environment does not match the V3 descriptor")
            }
            Self::TargetMismatch { found } => {
                write!(
                    formatter,
                    "V3 descriptor has unsupported AMD target {found}"
                )
            }
            Self::BackendPathMismatch { found } => write!(
                formatter,
                "V3 descriptor has backend path {found}, expected the protected path"
            ),
            Self::ArtifactDirectoryPathMismatch { found } => write!(
                formatter,
                "V3 descriptor has artifact-directory path {found}, expected the protected path"
            ),
            Self::RustcClosurePinMismatch => formatter
                .write_str("V3 rustc executable digest does not match its compiler closure"),
            Self::CodegenBackendClosurePinMismatch => {
                formatter.write_str("V3 backend digest does not match its compiler closure")
            }
            Self::RunningRustcMismatch => formatter
                .write_str("observed running rustc does not match its compiler-closure pin"),
            Self::RunningCodegenBackendMismatch => formatter.write_str(
                "observed running codegen backend does not match its compiler-closure pin",
            ),
            Self::CompilerClosureObservationMismatch => formatter.write_str(
                "closed compiler-closure environment observation does not match the V3 closure",
            ),
            Self::CodegenBackendObservationMismatch => formatter
                .write_str("closed backend environment observation does not match the V3 closure"),
            Self::InvalidClosedObservation { name } => {
                write!(
                    formatter,
                    "closed SHA-256 observation {name} is absent or invalid"
                )
            }
        }
    }
}

impl Error for ProtectedRustcProcessValidationErrorV1 {}

/// Applies the one canonical comparison used by both in-process rustc custody and the protected
/// external supervisor. The observations are inert inputs; success grants no authority.
#[allow(clippy::too_many_arguments)]
pub fn validate_protected_rustc_process_observation_v1(
    descriptor: &RustcInvocationDescriptorV3,
    observed_argv: &[String],
    observed_canonical_working_directory: &str,
    observed_compile_environment: &CompileEnvironmentV2,
    observed_running_rustc_sha256: [u8; 32],
    observed_running_codegen_backend_sha256: [u8; 32],
    required_backend_path: &str,
    required_artifact_directory_path: &str,
) -> Result<(), ProtectedRustcProcessValidationErrorV1> {
    let closure = descriptor.compiler_closure();
    if descriptor
        .rustc()
        .argv()
        .ne(observed_argv.iter().map(String::as_str))
    {
        return Err(ProtectedRustcProcessValidationErrorV1::ArgumentsMismatch);
    }
    if descriptor.rustc().working_directory() != observed_canonical_working_directory {
        return Err(ProtectedRustcProcessValidationErrorV1::WorkingDirectoryMismatch);
    }
    if descriptor.compile_environment() != observed_compile_environment {
        return Err(ProtectedRustcProcessValidationErrorV1::CompileEnvironmentMismatch);
    }
    if fe2o3_amd_target::ProductionAmdTargetProfileV1::from_device_target(descriptor.amd_target())
        .is_none()
    {
        return Err(ProtectedRustcProcessValidationErrorV1::TargetMismatch {
            found: descriptor.amd_target().to_owned(),
        });
    }
    if descriptor.codegen_backend_path() != required_backend_path {
        return Err(
            ProtectedRustcProcessValidationErrorV1::BackendPathMismatch {
                found: descriptor.codegen_backend_path().to_owned(),
            },
        );
    }
    if descriptor.artifact_output_directory() != required_artifact_directory_path {
        return Err(
            ProtectedRustcProcessValidationErrorV1::ArtifactDirectoryPathMismatch {
                found: descriptor.artifact_output_directory().to_owned(),
            },
        );
    }
    if descriptor.rustc_executable_sha256() != &closure.rustc_executable_sha256() {
        return Err(ProtectedRustcProcessValidationErrorV1::RustcClosurePinMismatch);
    }
    if descriptor.codegen_backend_sha256() != &closure.codegen_backend_sha256() {
        return Err(ProtectedRustcProcessValidationErrorV1::CodegenBackendClosurePinMismatch);
    }
    if observed_running_rustc_sha256 != closure.rustc_executable_sha256() {
        return Err(ProtectedRustcProcessValidationErrorV1::RunningRustcMismatch);
    }
    if observed_running_codegen_backend_sha256 != closure.codegen_backend_sha256() {
        return Err(ProtectedRustcProcessValidationErrorV1::RunningCodegenBackendMismatch);
    }
    if closed_sha256_observation(
        observed_compile_environment,
        EXPECTED_COMPILER_CLOSURE_SHA256_ENV_V1,
    )? != closure.identity_sha256()
    {
        return Err(ProtectedRustcProcessValidationErrorV1::CompilerClosureObservationMismatch);
    }
    if closed_sha256_observation(
        observed_compile_environment,
        CODEGEN_BACKEND_BUILD_OBSERVATION_ENV_V2,
    )? != closure.codegen_backend_sha256()
    {
        return Err(ProtectedRustcProcessValidationErrorV1::CodegenBackendObservationMismatch);
    }
    Ok(())
}

fn closed_sha256_observation(
    environment: &CompileEnvironmentV2,
    name: &'static str,
) -> Result<[u8; 32], ProtectedRustcProcessValidationErrorV1> {
    let encoded = environment
        .entries()
        .iter()
        .find(|entry| entry.key() == name)
        .map(|entry| entry.value())
        .ok_or(ProtectedRustcProcessValidationErrorV1::InvalidClosedObservation { name })?;
    decode_sha256_observation(name, encoded)
}

fn decode_sha256_observation(
    name: &'static str,
    encoded: &str,
) -> Result<[u8; 32], ProtectedRustcProcessValidationErrorV1> {
    if encoded.len() != 64
        || !encoded
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(ProtectedRustcProcessValidationErrorV1::InvalidClosedObservation { name });
    }
    let mut digest = [0_u8; 32];
    for (output, pair) in digest.iter_mut().zip(encoded.as_bytes().chunks_exact(2)) {
        *output = (lower_hex_value(pair[0]) << 4) | lower_hex_value(pair[1]);
    }
    if digest == [0; 32] {
        return Err(ProtectedRustcProcessValidationErrorV1::InvalidClosedObservation { name });
    }
    Ok(digest)
}

fn lower_hex_value(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        _ => unreachable!("lowercase hexadecimal was checked before decoding"),
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use fe2o3_build_authority::CompilerClosureV2;
    use fe2o3_rustc_invocation::{
        CompileEnvironmentV2, RustcInvocationDescriptorV2, RustcInvocationDescriptorV3, RustcUnitV2,
    };

    use super::*;

    const RUSTC: [u8; 32] = [0x44; 32];
    const BACKEND: [u8; 32] = [0x66; 32];
    const BACKEND_PATH: &str = "/proc/./self/fd/198";
    const ARTIFACT_PATH: &str = "/proc/self/fd/197";

    fn hex(digest: [u8; 32]) -> String {
        digest.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    fn descriptor() -> RustcInvocationDescriptorV3 {
        let closure = CompilerClosureV2::new(
            [0x11; 32], [0x22; 32], [0x33; 32], RUSTC, [0x55; 32], BACKEND,
        )
        .unwrap();
        let environment = CompileEnvironmentV2::from_child_environment([
            (
                OsString::from("FE2O3_HSACO_DIR"),
                OsString::from(ARTIFACT_PATH),
            ),
            (
                OsString::from("FE2O3_TARGET"),
                OsString::from("gfx942:xnack-"),
            ),
            (
                OsString::from(EXPECTED_COMPILER_CLOSURE_SHA256_ENV_V1),
                OsString::from(hex(closure.identity_sha256())),
            ),
            (
                OsString::from(CODEGEN_BACKEND_BUILD_OBSERVATION_ENV_V2),
                OsString::from(hex(BACKEND)),
            ),
        ])
        .unwrap();
        let rustc = RustcUnitV2::new(
            "/workspace",
            vec![
                "/toolchain/bin/rustc".into(),
                "--crate-name".into(),
                "fixture".into(),
                format!("-Zcodegen-backend={BACKEND_PATH}"),
            ],
        )
        .unwrap();
        RustcInvocationDescriptorV3::new(
            RustcInvocationDescriptorV2::new(RUSTC, BACKEND, rustc, environment).unwrap(),
            closure,
        )
        .unwrap()
    }

    fn validate(
        descriptor: &RustcInvocationDescriptorV3,
    ) -> Result<(), ProtectedRustcProcessValidationErrorV1> {
        validate_protected_rustc_process_observation_v1(
            descriptor,
            &descriptor
                .rustc()
                .argv()
                .map(str::to_owned)
                .collect::<Vec<_>>(),
            descriptor.rustc().working_directory(),
            descriptor.compile_environment(),
            RUSTC,
            BACKEND,
            BACKEND_PATH,
            ARTIFACT_PATH,
        )
    }

    #[test]
    fn exact_observation_passes_and_each_direct_axis_rejects() {
        let descriptor = descriptor();
        validate(&descriptor).unwrap();
        let argv = descriptor
            .rustc()
            .argv()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        assert_eq!(
            validate_protected_rustc_process_observation_v1(
                &descriptor,
                &["changed".to_owned()],
                descriptor.rustc().working_directory(),
                descriptor.compile_environment(),
                RUSTC,
                BACKEND,
                BACKEND_PATH,
                ARTIFACT_PATH,
            ),
            Err(ProtectedRustcProcessValidationErrorV1::ArgumentsMismatch)
        );
        assert_eq!(
            validate_protected_rustc_process_observation_v1(
                &descriptor,
                &argv,
                "/changed",
                descriptor.compile_environment(),
                RUSTC,
                BACKEND,
                BACKEND_PATH,
                ARTIFACT_PATH,
            ),
            Err(ProtectedRustcProcessValidationErrorV1::WorkingDirectoryMismatch)
        );
        assert_eq!(
            validate_protected_rustc_process_observation_v1(
                &descriptor,
                &argv,
                descriptor.rustc().working_directory(),
                descriptor.compile_environment(),
                [0x45; 32],
                BACKEND,
                BACKEND_PATH,
                ARTIFACT_PATH,
            ),
            Err(ProtectedRustcProcessValidationErrorV1::RunningRustcMismatch)
        );
        assert_eq!(
            validate_protected_rustc_process_observation_v1(
                &descriptor,
                &argv,
                descriptor.rustc().working_directory(),
                descriptor.compile_environment(),
                RUSTC,
                [0x67; 32],
                BACKEND_PATH,
                ARTIFACT_PATH,
            ),
            Err(ProtectedRustcProcessValidationErrorV1::RunningCodegenBackendMismatch)
        );
        assert!(matches!(
            validate_protected_rustc_process_observation_v1(
                &descriptor,
                &argv,
                descriptor.rustc().working_directory(),
                descriptor.compile_environment(),
                RUSTC,
                BACKEND,
                "/changed",
                ARTIFACT_PATH,
            ),
            Err(ProtectedRustcProcessValidationErrorV1::BackendPathMismatch { .. })
        ));
        assert!(matches!(
            validate_protected_rustc_process_observation_v1(
                &descriptor,
                &argv,
                descriptor.rustc().working_directory(),
                descriptor.compile_environment(),
                RUSTC,
                BACKEND,
                BACKEND_PATH,
                "/changed",
            ),
            Err(ProtectedRustcProcessValidationErrorV1::ArtifactDirectoryPathMismatch { .. })
        ));
    }
}
