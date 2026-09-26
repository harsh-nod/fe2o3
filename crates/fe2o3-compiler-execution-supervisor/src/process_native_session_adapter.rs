//! Nominal session policies and stage errors over one native lifecycle schedule.
macro_rules! native_session {
    ($limits:ident, $error:ident) => {
        /// Independent native lifecycle bounds, without a legacy timeout conversion.
        ///
        /// Every wait retains its finite attempt count and deadline. In particular,
        /// the serving bound is a native wait, not the legacy 24-hour session policy.
        ///
        /// ```compile_fail
        /// use fe2o3_compiler_execution_supervisor::{ProtectedIssuerSessionLimitsV2 as V2,
        ///     ProtectedIssuerSessionLimitsV3 as V3};
        /// fn mix(limits: V2) -> V3 { limits.into() }
        /// ```
        /// ```compile_fail
        /// use fe2o3_compiler_execution_supervisor::{ProtectedIssuerSessionTimeoutsV1 as V1,
        ///     ProtectedIssuerSessionLimitsV3 as V3};
        /// fn upgrade(limits: V1) -> V3 { limits.into() }
        /// ```
        /// ```compile_fail
        /// use fe2o3_compiler_execution_supervisor::{ProtectedIssuerSupervisorV3 as S,
        ///     ProtectedIssuerSessionLimitsV2 as L, ProtectedIssuerCleanupServiceV2 as C};
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as B;
        /// fn mix(s: &S, fd: std::os::fd::OwnedFd, c: &mut C, l: L, b: &mut B<'_>) {
        ///     let _ = s.run_session(fd, c, l, b);
        /// }
        /// ```
        /// ```compile_fail
        /// use fe2o3_compiler_execution_supervisor::{ProtectedIssuerSupervisorV3 as S,
        ///     ProtectedIssuerSessionLimitsV3 as L, ProtectedIssuerCleanupServiceV2 as C};
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as B;
        /// fn release(s: &S, fd: std::os::fd::OwnedFd, c: &mut C, l: L, b: &mut B<'_>) {
        ///     let exited = s.run_session(fd, c, l, b).unwrap();
        ///     b.release_storage(1).unwrap();
        ///     drop(exited);
        /// }
        /// ```
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub struct $limits {
            handoff: std::time::Duration,
            launch: Wait,
            readiness: Wait,
            publication: Wait,
            exit: Wait,
        }
        impl $limits {
            /// Admits a nonzero handoff bound up to two minutes and exact native waits.
            /// This handoff ceiling is session policy; no wait is clamped or rebuilt.
            pub fn new(
                handoff: std::time::Duration,
                launch: Wait,
                readiness: Wait,
                publication: Wait,
                exit: Wait,
            ) -> std::result::Result<Self, HandoffError> {
                if handoff.is_zero() || handoff > Wait::MAX_TIMEOUT {
                    return Err(HandoffError::InvalidTimeout);
                }
                Ok(Self {
                    handoff,
                    launch,
                    readiness,
                    publication,
                    exit,
                })
            }

            /// Exact handoff receive deadline interval.
            pub const fn handoff(self) -> std::time::Duration {
                self.handoff
            }
            /// Exact gated-launch wait policy.
            pub const fn launch(self) -> Wait {
                self.launch
            }
            /// Exact private readiness wait policy.
            pub const fn readiness(self) -> Wait {
                self.readiness
            }
            /// Exact atomic publication wait policy.
            pub const fn publication(self) -> Wait {
                self.publication
            }
            /// Exact natural terminal-wait policy.
            pub const fn exit(self) -> Wait {
                self.exit
            }
        }

        /// Exact failed native session stage, including that stage's storage growth.
        ///
        /// ```compile_fail
        /// use fe2o3_compiler_execution_supervisor::{ProtectedIssuerSessionErrorV2 as V2,
        ///     ProtectedIssuerSessionErrorV3 as V3};
        /// fn mix(error: V2) -> V3 { error.into() }
        /// ```
        #[derive(Debug)]
        #[non_exhaustive]
        pub enum $error {
            /// Control adoption, handoff authentication or retained handoff growth failed.
            Handoff(HandoffError),
            /// Static input preparation or retained prepared growth failed.
            Preparation(PreparationError),
            /// Gated process creation or authenticated exec failed.
            Launch(Error),
            /// Exact private readiness and EOF admission failed.
            Readiness(Error),
            /// Atomic readiness publication failed.
            Publication(Error),
            /// Natural terminal wait or exact reaping failed.
            Exit(Error),
        }
        impl std::fmt::Display for $error {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    Self::Handoff(e) => write!(f, "native issuer handoff failed: {e}"),
                    Self::Preparation(e) => write!(f, "native issuer preparation failed: {e}"),
                    Self::Launch(e) => write!(f, "native issuer launch failed: {e}"),
                    Self::Readiness(e) => write!(f, "native issuer readiness failed: {e}"),
                    Self::Publication(e) => write!(f, "native issuer publication failed: {e}"),
                    Self::Exit(e) => write!(f, "native issuer exit failed: {e}"),
                }
            }
        }
        impl std::error::Error for $error {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                match self {
                    Self::Handoff(e) => Some(e),
                    Self::Preparation(e) => Some(e),
                    Self::Launch(e) | Self::Readiness(e) | Self::Publication(e) | Self::Exit(e) => {
                        Some(e)
                    }
                }
            }
        }
    };
}
pub(crate) use native_session;
