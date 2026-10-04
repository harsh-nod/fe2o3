//! Concrete V53 preparation; no execution authority is constructed.
use crate::generated_kfd_arguments::mixed_preparation_v53::prepare as prepare_generated;
#[cfg(target_os = "linux")]
#[path = "mixed_worker_v53_execution.rs"]
pub(crate) mod execution;

const PREPARATION_LABEL: &str = "mixed V53 preparation";
include!("mixed_worker_preparation_family.rs");
impl MixedWorkerPreparationError {
    fn from_generated(error: crate::MixedWorkerV53PreparationError) -> Self {
        error
    }
}
pub use MixedWorkerPreparationError as MixedWorkerV53PreparationError;
/// Retains generated Rust output borrows without granting execution authority.
/// ```compile_fail
/// use fe2o3_host::PreparedMixedWorkerV53Invocation;
/// fn clone<R, K>(v: PreparedMixedWorkerV53Invocation<'_, R, K>) { let _ = v.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_host::PreparedMixedWorkerV53Invocation;
/// fn execute<R, K>(v: PreparedMixedWorkerV53Invocation<'_, R, K>) { v.execute(); }
/// ```
pub use PreparedMixedWorkerInvocation as PreparedMixedWorkerV53Invocation;
