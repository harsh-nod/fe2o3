//! Nominal finite dispatch policy and inert observations, never service authority.
macro_rules! native_controller {
    ($limits:ident, $report:ident, $stop:ident) => {
        /// Finite sequential dispatch and cleanup policy for one native family.
        ///
        /// This bounds turns, not syscall latency or total service lifetime.
        /// Every call retains the original cumulative request and cleanup accounts.
        ///
        /// ```compile_fail
        /// use fe2o3_compiler_execution_supervisor::{ProtectedIssuerDispatchLimitsV2 as V2,
        ///     ProtectedIssuerDispatchLimitsV3 as V3};
        /// fn mix(limits: V2) -> V3 { limits.into() }
        /// ```
        /// ```compile_fail
        /// use fe2o3_compiler_execution_supervisor::{ProtectedIssuerServiceV3 as S,
        ///     ProtectedIssuerDispatchLimitsV2 as L, ProtectedIssuerCleanupServiceV2 as C};
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as B;
        /// fn mix(s: &S, l: L, c: &mut C, b: &mut B<'_>) { s.run_turns(l, c, b); }
        /// ```
        /// ```
        /// use fe2o3_compiler_execution_supervisor::{ProtectedIssuerServiceV3 as S,
        ///     ProtectedIssuerDispatchLimitsV3 as L, ProtectedIssuerCleanupServiceV2 as C,
        ///     ProtectedIssuerDispatchReportV3 as Report};
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as B;
        /// fn dispatch(s: &S, l: L, c: &mut C, b: &mut B<'_>) -> Report {
        ///     s.run_turns(l, c, b)
        /// }
        /// ```
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub struct $limits {
            turns: usize,
            accept: Wait,
            cleanup_visits: usize,
        }
        impl $limits {
            /// Maximum dispatch attempts in one bounded call.
            pub const MAX_TURNS: usize = 4096;

            /// Requires nonzero bounded dispatch turns and a valid cleanup scan size.
            pub fn new(turns: usize, accept: Wait, cleanup_visits: usize) -> Result<Self> {
                if turns == 0
                    || turns > Self::MAX_TURNS
                    || cleanup_visits == 0
                    || cleanup_visits > crate::MAX_PROTECTED_ISSUER_PROCESSES_V1
                {
                    return Err(Error::InvalidDispatchLimits);
                }
                Ok(Self {
                    turns,
                    accept,
                    cleanup_visits,
                })
            }

            /// Exact maximum number of dispatch attempts.
            pub const fn turns(self) -> usize {
                self.turns
            }
            /// Exact family-specific accept attempts and deadline.
            pub const fn accept(self) -> Wait {
                self.accept
            }
            /// Exact number of pool cells per cleanup pump.
            pub const fn cleanup_visits(self) -> usize {
                self.cleanup_visits
            }
        }

        /// Reason this bounded controller returned; no variant releases cleanup custody.
        #[derive(Debug)]
        #[non_exhaustive]
        pub enum $stop {
            /// All requested turns ran without a non-idle dispatch failure.
            TurnLimit,
            /// Exact first non-idle dispatch or controller-accounting failure.
            Dispatch(Error),
            /// Cleanup disallows admission or cannot fund the selected post-dispatch pump.
            AdmissionStopped,
            /// Cleanup observation or pumping failed; inspect the separate cleanup result.
            Cleanup,
        }

        /// Fixed inert aggregate, not proof that the cleanup pool is empty.
        ///
        /// A session rejection stops this call rather than hiding its error behind a
        /// later success. Accept timeouts/attempt exhaustion count as idle turns.
        /// The separate cleanup result preserves a simultaneous dispatch failure.
        /// There is no owned descriptor, budget, receipt or launch capability here.
        #[derive(Debug)]
        pub struct $report {
            /// Number of `serve_one` calls actually started, including a rejected call.
            pub turns: usize,
            /// Sessions that naturally exited and passed final service continuity.
            pub completed: usize,
            /// Accept waits that exhausted their deadline or attempts without a connection.
            pub idle: usize,
            /// Last completed session observation; earlier completions are counted only.
            pub last_completion: Option<Report>,
            /// Exact stopping condition, independent of cleanup's result.
            pub stop: $stop,
            /// Last actual cleanup observation or pump result, not a reaping receipt.
            pub cleanup: std::result::Result<
                crate::ProtectedIssuerCleanupReportV2,
                crate::ProtectedIssuerCleanupErrorV2,
            >,
        }
    };
}
pub(crate) use native_controller;
