//! Shared native errors; caller aliases bind the actual policy family.
macro_rules! native_error {
    ($error:ident) => {
        use std::error::Error as StdError;
        /// Fixed failure at a native consuming lifecycle boundary; no legacy fallback.
        #[derive(Debug)]
        #[non_exhaustive]
        pub enum $error {
            /// The original caller ledger refused work, scratch, or retained storage.
            Resource(Resource),
            /// Native supervisor custody changed.
            Supervisor(SupervisorError),
            /// Native prepared descriptors or their exact bindings changed.
            Preparation(PreparationError),
            /// The fixed process or namespace observation failed.
            Profile(fe2o3_protected_service_profile::ProtectedServiceProfileErrorV2),
            /// Funded persistent cleanup custody could not be reserved.
            Cleanup(crate::ProtectedIssuerCleanupErrorV2),
            /// The native sealed launch frame failed revalidation.
            Capability(fe2o3_compiler_closure_capability::CompilerExecutionCapabilityErrorV2),
            /// Native readiness framing or matching failed.
            Readiness(ReadyError),
            /// Attempts or timeout were outside the fixed native wait limits.
            InvalidWait,
            /// The finite attempt envelope was exhausted.
            Attempts(Boundary),
            /// The observation deadline expired.
            Timeout(Boundary),
            /// The exact child exited before the requested boundary.
            ChildExited(Boundary),
            /// The direct pre-exec child reported a failed stage.
            ChildStage(u8),
            /// A private protocol or child-custody invariant was not satisfied.
            State(&'static str),
            /// Cleanup was transferred to the persistently funded pool, not completed.
            CleanupPending,
            /// A single-attempt operating-system operation failed.
            Io {
                /// Fixed operation name.
                operation: &'static str,
                /// Kernel errno; no allocated diagnostic is retained.
                errno: rustix::io::Errno,
            },
        }

        impl From<Resource> for Error {
            fn from(error: Resource) -> Self {
                Self::Resource(error)
            }
        }
        impl From<SupervisorError> for Error {
            fn from(error: SupervisorError) -> Self {
                Self::Supervisor(error)
            }
        }
        impl From<PreparationError> for Error {
            fn from(error: PreparationError) -> Self {
                Self::Preparation(error)
            }
        }
        impl From<fe2o3_protected_service_profile::ProtectedServiceProfileErrorV2> for Error {
            fn from(
                error: fe2o3_protected_service_profile::ProtectedServiceProfileErrorV2,
            ) -> Self {
                Self::Profile(error)
            }
        }
        impl From<crate::ProtectedIssuerCleanupErrorV2> for Error {
            fn from(error: crate::ProtectedIssuerCleanupErrorV2) -> Self {
                Self::Cleanup(error)
            }
        }
        impl From<fe2o3_compiler_closure_capability::CompilerExecutionCapabilityErrorV2> for Error {
            fn from(
                error: fe2o3_compiler_closure_capability::CompilerExecutionCapabilityErrorV2,
            ) -> Self {
                Self::Capability(error)
            }
        }
        impl From<ReadyError> for Error {
            fn from(error: ReadyError) -> Self {
                Self::Readiness(error)
            }
        }
        impl From<super::super::ChildProcessError> for Error {
            fn from(error: super::super::ChildProcessError) -> Self {
                match error {
                    super::super::ChildProcessError::State(reason) => Self::State(reason),
                    super::super::ChildProcessError::Io { operation, errno } => {
                        Self::Io { operation, errno }
                    }
                }
            }
        }
        impl From<crate::process_staging::StagedLaunchErrorV1> for Error {
            fn from(error: crate::process_staging::StagedLaunchErrorV1) -> Self {
                match error {
                    crate::process_staging::StagedLaunchErrorV1::InvalidProcessState(reason) => {
                        Self::State(reason)
                    }
                    crate::process_staging::StagedLaunchErrorV1::Io { operation, source } => {
                        Self::Io {
                            operation,
                            errno: source,
                        }
                    }
                }
            }
        }

        impl fmt::Display for Error {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self {
                    Self::Resource(e) => e.fmt(f),
                    Self::Supervisor(e) => e.fmt(f),
                    Self::Preparation(e) => e.fmt(f),
                    Self::Profile(e) => e.fmt(f),
                    Self::Cleanup(e) => e.fmt(f),
                    Self::Capability(e) => e.fmt(f),
                    Self::Readiness(e) => e.fmt(f),
                    Self::InvalidWait => f.write_str("native issuer wait exceeds fixed limits"),
                    Self::Attempts(stage) => {
                        write!(f, "native issuer attempts exhausted at {stage:?}")
                    }
                    Self::Timeout(stage) => {
                        write!(f, "native issuer deadline expired at {stage:?}")
                    }
                    Self::ChildExited(stage) => write!(f, "native issuer exited before {stage:?}"),
                    Self::ChildStage(stage) => {
                        write!(f, "native issuer pre-exec stage {stage} failed")
                    }
                    Self::State(reason) => f.write_str(reason),
                    Self::CleanupPending => {
                        f.write_str("native issuer cleanup retained by funded pool")
                    }
                    Self::Io { operation, errno } => write!(f, "{operation}: {errno}"),
                }
            }
        }
        impl StdError for Error {
            fn source(&self) -> Option<&(dyn StdError + 'static)> {
                match self {
                    Self::Resource(e) => Some(e),
                    Self::Supervisor(e) => Some(e),
                    Self::Preparation(e) => Some(e),
                    Self::Profile(e) => Some(e),
                    Self::Cleanup(e) => Some(e),
                    Self::Capability(e) => Some(e),
                    Self::Readiness(e) => Some(e),
                    Self::Io { errno, .. } => Some(errno),
                    _ => None,
                }
            }
        }
    };
}
pub(crate) use native_error;
