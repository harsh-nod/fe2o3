//! Uses the existing parsed recipe; no native configuration parser or selector.
use super::PreparedProductionBuildConfig;
use crate::{
    compiler_execution_boundary::native::{
        ParentCompilerExecutionReadinessCustodyV3,
        pipeline::{
            ConditionalRecoveryPolicy, ContinuationError, ParentPreparedConditionalArtifact,
        },
    },
    protected_compiler_handoff_v3::ParentRustcInvocationCustody,
};
use fe2o3_artifact_transaction::{BuildAttempt, ProducerIdentity};
use std::path::Path;

impl PreparedProductionBuildConfig {
    /// The enclosing attempt admits/prepays configuration and parent invocation
    /// before native readiness borrows the account. No provider/options clone.
    #[allow(clippy::too_many_arguments, clippy::result_large_err)]
    pub(crate) fn finalize_conditional_current<'a, 'b, 'w>(
        self,
        readiness: ParentCompilerExecutionReadinessCustodyV3<'b, 'w>,
        output_dir: &Path,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        invocation: &'a ParentRustcInvocationCustody,
        policy: ConditionalRecoveryPolicy<'_>,
    ) -> Result<ParentPreparedConditionalArtifact<'a, 'b, 'w>, ContinuationError> {
        readiness.finalize_current_publication(
            output_dir,
            producer,
            attempt,
            invocation,
            policy,
            &self.link.worker,
            self.link.providers,
            self.link.link_options,
            self.link.candidate_output,
            self.link.limits,
        )
    }
}
