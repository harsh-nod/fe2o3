use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::Path;
use std::process::Command;

use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_rustc_invocation::{
    CompileEnvironmentV2, DigestError, InvocationDigestV2, InvocationDigestV3,
    RustcInvocationDescriptorV2, RustcInvocationDescriptorV3, RustcUnitV2, ValidationError,
};

/// An in-memory description of prepared rustc inputs.
///
/// This value is inert coordination data. It is not an execution receipt,
/// artifact evidence, an authenticator, or load/launch authority.
pub(crate) struct InertRustcInvocationCaptureV2 {
    descriptor: RustcInvocationDescriptorV2,
    digest: InvocationDigestV2,
}

impl InertRustcInvocationCaptureV2 {
    /// Captures the exact final argv and complete intended child environment.
    ///
    /// The caller must supply digests from the pinned executable objects and
    /// the complete environment that will replace the child's inherited
    /// environment. This capture does not persist the descriptor or plaintext
    /// environment to durable storage. The descriptor binds only the canonical
    /// working-directory pathname. S09 process consistency separately binds the
    /// pinned directory object; this inert capture does not claim an identity
    /// join between that object and the pathname.
    pub(crate) fn capture(
        command: &Command,
        configured_argv0: &OsStr,
        canonical_working_directory: &Path,
        complete_environment: &[(OsString, OsString)],
        rustc_executable_sha256: [u8; 32],
        codegen_backend_sha256: [u8; 32],
    ) -> Result<Self, InertRustcInvocationCaptureErrorV2> {
        let mut argv = Vec::with_capacity(command.get_args().len() + 1);
        argv.push(utf8(configured_argv0, "rustc argv", Some(0))?);
        for (index, argument) in command.get_args().enumerate() {
            argv.push(utf8(argument, "rustc argv", Some(index + 1))?);
        }
        let working_directory = utf8(
            canonical_working_directory.as_os_str(),
            "rustc working directory",
            None,
        )?;
        let rustc = RustcUnitV2::new(working_directory, argv)?;
        let compile_environment =
            CompileEnvironmentV2::from_child_environment(complete_environment.iter().cloned())?;
        let descriptor = RustcInvocationDescriptorV2::new(
            rustc_executable_sha256,
            codegen_backend_sha256,
            rustc,
            compile_environment,
        )?;
        let digest = InvocationDigestV2::calculate(&descriptor)?;
        Ok(Self { descriptor, digest })
    }

    pub(crate) const fn descriptor(&self) -> &RustcInvocationDescriptorV2 {
        &self.descriptor
    }

    pub(crate) const fn digest(&self) -> InvocationDigestV2 {
        self.digest
    }

    /// Upgrades the exact process description with a broker-authenticated compiler closure.
    pub(crate) fn upgrade(
        self,
        compiler_closure: CompilerClosureV2,
    ) -> Result<InertRustcInvocationCaptureV3, InertRustcInvocationCaptureErrorV2> {
        let descriptor = RustcInvocationDescriptorV3::from_v2_and_compiler_closure(
            self.descriptor,
            compiler_closure,
        )?;
        let digest = InvocationDigestV3::calculate(&descriptor)?;
        Ok(InertRustcInvocationCaptureV3 { descriptor, digest })
    }
}

/// A protected invocation description containing one exact process and compiler closure.
///
/// Like V2, this is coordination data rather than execution, publication, or launch authority.
pub(crate) struct InertRustcInvocationCaptureV3 {
    descriptor: RustcInvocationDescriptorV3,
    digest: InvocationDigestV3,
}

impl InertRustcInvocationCaptureV3 {
    pub(crate) const fn descriptor(&self) -> &RustcInvocationDescriptorV3 {
        &self.descriptor
    }

    pub(crate) const fn digest(&self) -> InvocationDigestV3 {
        self.digest
    }
}

/// The schema selected for one fully prepared rustc child.
pub(crate) enum InertPreparedRustcInvocationCapture {
    V2(InertRustcInvocationCaptureV2),
    V3(Box<InertRustcInvocationCaptureV3>),
}

impl InertPreparedRustcInvocationCapture {
    pub(crate) fn from_v2_and_protected_closure(
        capture: InertRustcInvocationCaptureV2,
        compiler_closure: Option<CompilerClosureV2>,
    ) -> Result<Self, InertRustcInvocationCaptureErrorV2> {
        match compiler_closure {
            Some(compiler_closure) => Ok(Self::V3(Box::new(capture.upgrade(compiler_closure)?))),
            None => Ok(Self::V2(capture)),
        }
    }

    pub(crate) fn amd_target(&self) -> &str {
        match self {
            Self::V2(capture) => capture.descriptor().amd_target(),
            Self::V3(capture) => capture.descriptor().amd_target(),
        }
    }

    pub(crate) const fn descriptor_version(&self) -> u16 {
        match self {
            Self::V2(_) => 2,
            Self::V3(_) => 3,
        }
    }

    pub(crate) fn digest_hex(&self) -> String {
        match self {
            Self::V2(capture) => capture.digest().to_hex(),
            Self::V3(capture) => capture.digest().to_hex(),
        }
    }

    pub(crate) const fn descriptor_v3(&self) -> Option<&RustcInvocationDescriptorV3> {
        match self {
            Self::V2(_) => None,
            Self::V3(capture) => Some(capture.descriptor()),
        }
    }
}

fn utf8(
    value: &OsStr,
    field: &'static str,
    index: Option<usize>,
) -> Result<String, InertRustcInvocationCaptureErrorV2> {
    value
        .to_str()
        .map(str::to_owned)
        .ok_or(InertRustcInvocationCaptureErrorV2::NonUtf8 { field, index })
}

#[derive(Debug)]
pub(crate) enum InertRustcInvocationCaptureErrorV2 {
    NonUtf8 {
        field: &'static str,
        index: Option<usize>,
    },
    Validation(ValidationError),
    Digest(DigestError),
}

impl fmt::Display for InertRustcInvocationCaptureErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonUtf8 {
                field,
                index: Some(index),
            } => write!(formatter, "{field}[{index}] is not valid UTF-8"),
            Self::NonUtf8 { field, index: None } => {
                write!(formatter, "{field} is not valid UTF-8")
            }
            Self::Validation(error) => error.fmt(formatter),
            Self::Digest(error) => error.fmt(formatter),
        }
    }
}

impl Error for InertRustcInvocationCaptureErrorV2 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Validation(error) => Some(error),
            Self::Digest(error) => Some(error),
            Self::NonUtf8 { .. } => None,
        }
    }
}

impl From<ValidationError> for InertRustcInvocationCaptureErrorV2 {
    fn from(value: ValidationError) -> Self {
        Self::Validation(value)
    }
}

impl From<DigestError> for InertRustcInvocationCaptureErrorV2 {
    fn from(value: DigestError) -> Self {
        Self::Digest(value)
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use fe2o3_build_authority::CompilerClosureV2;

    use super::{InertPreparedRustcInvocationCapture, InertRustcInvocationCaptureV2};

    const RUSTC_SHA256: [u8; 32] = [0x11; 32];
    const BACKEND_SHA256: [u8; 32] = [0x22; 32];

    fn command(arguments: &[&str]) -> Command {
        let mut command = Command::new("/proc/self/fd/9");
        command.args(arguments);
        command
    }

    fn environment(verification: Option<&str>) -> Vec<(OsString, OsString)> {
        let mut environment = vec![
            (OsString::from("PATH"), OsString::from("/usr/bin")),
            (
                OsString::from("FE2O3_HSACO_DIR"),
                OsString::from("/proc/self/fd/197"),
            ),
            (
                OsString::from("FE2O3_TARGET"),
                OsString::from("gfx942:xnack-"),
            ),
        ];
        if let Some(verification) = verification {
            environment.push((
                OsString::from("FE2O3_VERIFY_KERNEL_IR"),
                OsString::from(verification),
            ));
        }
        environment
    }

    fn capture(
        command: &Command,
        working_directory: &Path,
        environment: &[(OsString, OsString)],
        rustc_sha256: [u8; 32],
        backend_sha256: [u8; 32],
    ) -> InertRustcInvocationCaptureV2 {
        InertRustcInvocationCaptureV2::capture(
            command,
            "/toolchains/rustc".as_ref(),
            working_directory,
            environment,
            rustc_sha256,
            backend_sha256,
        )
        .unwrap()
    }

    fn baseline_command() -> Command {
        command(&[
            "--crate-name",
            "scalar_gemm_v1",
            "--crate-type",
            "cdylib",
            "-Zcodegen-backend=/proc/./self/fd/198",
        ])
    }

    fn compiler_closure(rustc: [u8; 32], backend: [u8; 32]) -> CompilerClosureV2 {
        CompilerClosureV2::new(
            [0x31; 32], [0x32; 32], [0x33; 32], rustc, [0x35; 32], backend,
        )
        .unwrap()
    }

    #[test]
    fn captures_exact_scalar_worker_inputs_without_granting_authority() {
        let capture = capture(
            &baseline_command(),
            Path::new("/workspace/scalar"),
            &environment(None),
            RUSTC_SHA256,
            BACKEND_SHA256,
        );
        let descriptor = capture.descriptor();

        assert_eq!(descriptor.rustc_executable_sha256(), &RUSTC_SHA256);
        assert_eq!(descriptor.codegen_backend_sha256(), &BACKEND_SHA256);
        assert_eq!(descriptor.rustc_executable_path(), "/toolchains/rustc");
        assert_eq!(
            descriptor.rustc().argv().collect::<Vec<_>>(),
            [
                "/toolchains/rustc",
                "--crate-name",
                "scalar_gemm_v1",
                "--crate-type",
                "cdylib",
                "-Zcodegen-backend=/proc/./self/fd/198",
            ]
        );
        assert_eq!(descriptor.rustc().working_directory(), "/workspace/scalar");
        assert_eq!(descriptor.codegen_backend_path(), "/proc/./self/fd/198");
        assert_eq!(descriptor.amd_target(), "gfx942:xnack-");
        assert_eq!(descriptor.artifact_output_directory(), "/proc/self/fd/197");
        assert!(!descriptor.verification_required());
        assert_ne!(capture.digest().into_bytes(), [0; 32]);
    }

    #[test]
    fn pinned_object_digest_mutations_change_the_capture() {
        let command = baseline_command();
        let environment = environment(None);
        let baseline = capture(
            &command,
            Path::new("/workspace/scalar"),
            &environment,
            RUSTC_SHA256,
            BACKEND_SHA256,
        )
        .digest();

        assert_ne!(
            capture(
                &command,
                Path::new("/workspace/scalar"),
                &environment,
                [0x33; 32],
                BACKEND_SHA256,
            )
            .digest(),
            baseline
        );
        assert_ne!(
            capture(
                &command,
                Path::new("/workspace/scalar"),
                &environment,
                RUSTC_SHA256,
                [0x44; 32],
            )
            .digest(),
            baseline
        );
    }

    #[test]
    fn protected_capture_upgrades_the_exact_v2_process_to_v3() {
        let capture = capture(
            &baseline_command(),
            Path::new("/workspace/scalar"),
            &environment(Some("1")),
            RUSTC_SHA256,
            BACKEND_SHA256,
        );
        let descriptor_v2 = capture.descriptor().clone();
        let closure = compiler_closure(RUSTC_SHA256, BACKEND_SHA256);
        let capture = capture.upgrade(closure).unwrap();

        assert_eq!(capture.descriptor().descriptor_v2(), &descriptor_v2);
        assert_eq!(capture.descriptor().compiler_closure(), &closure);
        assert_eq!(
            capture.descriptor().compiler_closure_identity_sha256(),
            closure.identity_sha256()
        );
        assert_ne!(capture.digest().into_bytes(), [0; 32]);
    }

    #[test]
    fn protected_capture_rejects_closure_process_pin_mismatches() {
        for closure in [
            compiler_closure([0x91; 32], BACKEND_SHA256),
            compiler_closure(RUSTC_SHA256, [0x92; 32]),
        ] {
            let capture = capture(
                &baseline_command(),
                Path::new("/workspace/scalar"),
                &environment(None),
                RUSTC_SHA256,
                BACKEND_SHA256,
            );
            assert!(capture.upgrade(closure).is_err());
        }
    }

    #[test]
    fn prepared_capture_selects_v2_or_v3_without_conflating_schemas() {
        let make_capture = || {
            capture(
                &baseline_command(),
                Path::new("/workspace/scalar"),
                &environment(None),
                RUSTC_SHA256,
                BACKEND_SHA256,
            )
        };
        let ordinary = InertPreparedRustcInvocationCapture::from_v2_and_protected_closure(
            make_capture(),
            None,
        )
        .unwrap();
        let protected = InertPreparedRustcInvocationCapture::from_v2_and_protected_closure(
            make_capture(),
            Some(compiler_closure(RUSTC_SHA256, BACKEND_SHA256)),
        )
        .unwrap();

        assert_eq!(ordinary.descriptor_version(), 2);
        assert!(ordinary.descriptor_v3().is_none());
        assert_eq!(protected.descriptor_version(), 3);
        assert!(protected.descriptor_v3().is_some());
        assert_eq!(ordinary.amd_target(), protected.amd_target());
        assert_ne!(ordinary.digest_hex(), protected.digest_hex());
    }

    #[test]
    fn argv_order_and_backend_path_mutations_change_the_capture() {
        let environment = environment(None);
        let baseline = capture(
            &baseline_command(),
            Path::new("/workspace/scalar"),
            &environment,
            RUSTC_SHA256,
            BACKEND_SHA256,
        )
        .digest();
        let reordered = command(&[
            "--crate-type",
            "cdylib",
            "--crate-name",
            "scalar_gemm_v1",
            "-Zcodegen-backend=/proc/./self/fd/198",
        ]);
        let different_backend_path = command(&[
            "--crate-name",
            "scalar_gemm_v1",
            "--crate-type",
            "cdylib",
            "-Zcodegen-backend=/opt/reviewed/backend.so",
        ]);

        assert_ne!(
            capture(
                &reordered,
                Path::new("/workspace/scalar"),
                &environment,
                RUSTC_SHA256,
                BACKEND_SHA256,
            )
            .digest(),
            baseline
        );
        assert_ne!(
            capture(
                &different_backend_path,
                Path::new("/workspace/scalar"),
                &environment,
                RUSTC_SHA256,
                BACKEND_SHA256,
            )
            .digest(),
            baseline
        );
    }

    #[test]
    fn cwd_target_output_verification_and_environment_mutations_are_bound() {
        let command = baseline_command();
        let baseline_environment = environment(Some("0"));
        let baseline = capture(
            &command,
            Path::new("/workspace/scalar"),
            &baseline_environment,
            RUSTC_SHA256,
            BACKEND_SHA256,
        )
        .digest();

        assert_ne!(
            capture(
                &command,
                Path::new("/workspace/other"),
                &baseline_environment,
                RUSTC_SHA256,
                BACKEND_SHA256,
            )
            .digest(),
            baseline
        );
        for (name, value) in [
            ("FE2O3_TARGET", "gfx942:sramecc+:xnack-"),
            ("FE2O3_HSACO_DIR", "/proc/self/fd/196"),
            ("FE2O3_VERIFY_KERNEL_IR", "1"),
            ("PATH", "/reviewed/bin"),
        ] {
            let mut mutation = baseline_environment.clone();
            mutation
                .iter_mut()
                .find(|(entry, _)| entry == name)
                .unwrap()
                .1 = OsString::from(value);
            assert_ne!(
                capture(
                    &command,
                    Path::new("/workspace/scalar"),
                    &mutation,
                    RUSTC_SHA256,
                    BACKEND_SHA256,
                )
                .digest(),
                baseline,
                "mutation of {name} was not bound"
            );
        }
    }

    #[test]
    fn rejects_non_utf8_and_noncanonical_process_inputs() {
        let environment = environment(None);
        let command = baseline_command();
        for path in ["relative", "/workspace/../other", "/workspace//other"] {
            assert!(
                InertRustcInvocationCaptureV2::capture(
                    &command,
                    "/toolchains/rustc".as_ref(),
                    Path::new(path),
                    &environment,
                    RUSTC_SHA256,
                    BACKEND_SHA256,
                )
                .is_err(),
                "accepted noncanonical working directory {path}"
            );
        }

        let mut non_utf8_argument = baseline_command();
        non_utf8_argument.arg(OsString::from_vec(vec![0xff]));
        assert!(
            InertRustcInvocationCaptureV2::capture(
                &non_utf8_argument,
                "/toolchains/rustc".as_ref(),
                Path::new("/workspace/scalar"),
                &environment,
                RUSTC_SHA256,
                BACKEND_SHA256,
            )
            .is_err()
        );

        let non_utf8_cwd = PathBuf::from(OsString::from_vec(vec![b'/', 0xff]));
        assert!(
            InertRustcInvocationCaptureV2::capture(
                &command,
                "/toolchains/rustc".as_ref(),
                &non_utf8_cwd,
                &environment,
                RUSTC_SHA256,
                BACKEND_SHA256,
            )
            .is_err()
        );

        let mut non_utf8_environment = environment;
        non_utf8_environment.push((OsString::from("INPUT"), OsString::from_vec(vec![0xff])));
        assert!(
            InertRustcInvocationCaptureV2::capture(
                &command,
                "/toolchains/rustc".as_ref(),
                Path::new("/workspace/scalar"),
                &non_utf8_environment,
                RUSTC_SHA256,
                BACKEND_SHA256,
            )
            .is_err()
        );
    }
}
