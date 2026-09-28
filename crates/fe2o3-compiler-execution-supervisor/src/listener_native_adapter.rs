//! Native service facts and errors; authority remains nominal in each instantiation.
macro_rules! native_service {
    ($service:ident, $storage:ident, $report:ident, $error:ident) => {
        /// Move-only native supervisor and fixed-path listener custody.
        ///
        /// This dispatches to the genuine native session API. It does not select
        /// a runtime, provision privileges, start workers or activate publication.
        /// Keep consumed reservations prepaid and reserve returned growth before
        /// retention. Release full retained storage only after final owner Drop.
        ///
        /// ```compile_fail
        /// use fe2o3_compiler_execution_supervisor::ProtectedIssuerServiceV3;
        /// fn clone<T: Clone>() {}
        /// clone::<ProtectedIssuerServiceV3>();
        /// ```
        /// ```compile_fail
        /// use fe2o3_compiler_execution_supervisor::ProtectedIssuerServiceV3;
        /// fn descriptor<T: std::os::fd::AsFd>() {}
        /// descriptor::<ProtectedIssuerServiceV3>();
        /// ```
        /// ```compile_fail
        /// use fe2o3_compiler_execution_supervisor::{ProtectedIssuerServiceV2 as V2,
        ///     ProtectedIssuerServiceV3 as V3};
        /// fn upgrade(service: V2) -> V3 { service.into() }
        /// ```
        /// ```compile_fail
        /// use fe2o3_compiler_execution_supervisor::{ProtectedIssuerServiceV3 as S,
        ///     ProtectedIssuerSupervisorV2 as Supervisor, ProtectedIssuerSessionLimitsV3 as L};
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as B;
        /// fn mix(s: Supervisor, fd: std::os::fd::OwnedFd, l: L, b: &mut B<'_>) {
        ///     let _ = S::bind(s, fd, l, b);
        /// }
        /// ```
        /// ```compile_fail
        /// use fe2o3_compiler_execution_supervisor::{ProtectedIssuerServiceV3 as S,
        ///     ProtectedIssuerWaitV2 as W, ProtectedIssuerCleanupServiceV2 as C};
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as B;
        /// fn mix(s: &S, w: W, c: &mut C, b: &mut B<'_>) { let _ = s.serve_one(w, c, b); }
        /// ```
        /// ```
        /// use fe2o3_compiler_execution_supervisor::{ProtectedIssuerServiceV3 as S,
        ///     ProtectedIssuerWaitV3 as W, ProtectedIssuerCleanupServiceV2 as C,
        ///     ProtectedIssuerSessionReportV3 as Report, ProtectedIssuerServiceErrorV3 as E};
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as B;
        /// fn dispatch(s: &S, w: W, c: &mut C, b: &mut B<'_>) -> Result<Report, E> {
        ///     s.serve_one(w, c, b)
        /// }
        /// ```
        pub struct $service {
            supervisor: Supervisor,
            socket: Socket,
            limits: SessionLimits,
            retained: usize,
        }
        /// Unreserved growth over the consumed supervisor and socket input charges.
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub struct $storage(usize);
        impl $storage {
            /// Additional logical retention, excluding the already funded inputs.
            pub const fn additional_storage(self) -> usize {
                self.0
            }
        }
        /// Inert observations after exact natural reaping and service revalidation.
        /// This contains no descriptor, readiness owner, receipt or launch authority.
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub struct $report {
            pid: u32,
            termination: crate::ProtectedIssuerTerminationV1,
            readiness: fe2o3_compiler_execution_protocol::CompilerExecutionServiceReadyIdentityV1,
        }
        impl $report {
            /// Exact PID whose terminal status was consumed by the session.
            pub const fn pid(self) -> u32 {
                self.pid
            }
            /// Observed natural terminal status, not a deferred cleanup claim.
            pub const fn termination(self) -> crate::ProtectedIssuerTerminationV1 {
                self.termination
            }
            /// Identity of the readiness record published during that session.
            pub const fn readiness_identity(
                self,
            ) -> fe2o3_compiler_execution_protocol::CompilerExecutionServiceReadyIdentityV1 {
                self.readiness
            }
        }
        /// Native listener admission, continuity or exact session failure.
        #[derive(Debug)]
        #[non_exhaustive]
        pub enum $error {
            /// Shared descriptor/pathname predicate refused the socket.
            InvalidListener(&'static str),
            /// Accept attempts or deadline are outside the finite contract.
            InvalidAcceptLimits,
            /// Dispatch count or cleanup scan size is outside its finite contract.
            InvalidDispatchLimits,
            /// The absolute accept deadline expired.
            AcceptTimeout,
            /// Every prepaid accept turn was used without a connection.
            AcceptAttempts,
            /// The caller's original work/storage account refused the operation.
            Resource(Resource),
            /// The actual native supervisor failed continuity checks.
            Supervisor(SupervisorError),
            /// The complete native session failed at its reported stage.
            Session(SessionError),
            /// A bounded operating-system operation failed.
            Io {
                /// Exact operation label.
                operation: &'static str,
                /// Original operating-system error.
                source: std::io::Error,
            },
        }
        impl From<Resource> for $error {
            fn from(e: Resource) -> Self {
                Self::Resource(e)
            }
        }
        impl From<SupervisorError> for $error {
            fn from(e: SupervisorError) -> Self {
                Self::Supervisor(e)
            }
        }
        impl From<SocketError> for $error {
            fn from(e: SocketError) -> Self {
                match e {
                    SocketError::InvalidListener(reason) => Self::InvalidListener(reason),
                    SocketError::Io { operation, source } => Self::Io { operation, source },
                }
            }
        }
        impl From<AcceptError> for $error {
            fn from(e: AcceptError) -> Self {
                match e {
                    AcceptError::InvalidLimits => Self::InvalidAcceptLimits,
                    AcceptError::Timeout => Self::AcceptTimeout,
                    AcceptError::Attempts => Self::AcceptAttempts,
                    AcceptError::Socket(e) => e.into(),
                }
            }
        }
        impl std::fmt::Display for $error {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    Self::InvalidListener(reason) => {
                        write!(f, "invalid native issuer listener: {reason}")
                    }
                    Self::InvalidAcceptLimits => f.write_str("invalid native issuer accept limits"),
                    Self::InvalidDispatchLimits => {
                        f.write_str("invalid native issuer dispatch limits")
                    }
                    Self::AcceptTimeout => f.write_str("native issuer accept timed out"),
                    Self::AcceptAttempts => f.write_str("native issuer accept attempts exhausted"),
                    Self::Resource(e) => e.fmt(f),
                    Self::Supervisor(e) => write!(f, "native issuer supervisor changed: {e}"),
                    Self::Session(e) => e.fmt(f),
                    Self::Io { operation, source } => write!(f, "{operation}: {source}"),
                }
            }
        }
        impl std::error::Error for $error {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                match self {
                    Self::Resource(e) => Some(e),
                    Self::Supervisor(e) => Some(e),
                    Self::Session(e) => Some(e),
                    Self::Io { source, .. } => Some(source),
                    _ => None,
                }
            }
        }
    };
}
pub(crate) use native_service;
