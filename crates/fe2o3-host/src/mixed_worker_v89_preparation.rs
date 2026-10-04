//! Concrete V89 preparation; no execution authority is constructed.
use crate::generated_kfd_arguments::mixed_preparation_v53::prepare_predicated_v89 as prepare_generated;

const PREPARATION_LABEL: &str = "mixed V89 preparation";
include!("mixed_worker_preparation_family.rs");
impl MixedWorkerPreparationError {
    fn from_generated(error: crate::MixedWorkerV53PreparationError) -> Self {
        match error {
            crate::MixedWorkerV53PreparationError::Admission(e) => Self::Admission(e),
            crate::MixedWorkerV53PreparationError::Arguments(e) => Self::Arguments(e),
            crate::MixedWorkerV53PreparationError::Runtime(e) => Self::Runtime(e),
            crate::MixedWorkerV53PreparationError::Binding(e) => Self::Binding(e),
        }
    }
}
pub use MixedWorkerPreparationError as MixedWorkerV89PreparationError;
/// Retains predicated contracts and Rust borrows without execution authority.
/// ```compile_fail
/// use fe2o3_host::PreparedMixedWorkerV89Invocation;
/// fn clone<R, K>(v: PreparedMixedWorkerV89Invocation<'_, R, K>) { let _ = v.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_host::PreparedMixedWorkerV89Invocation;
/// fn execute<R, K>(v: PreparedMixedWorkerV89Invocation<'_, R, K>) { v.execute(); }
/// ```
/// ```compile_fail
/// use fe2o3_host::{PreparedMixedWorkerV89Invocation, PreparedMixedWorkerV53Invocation};
/// fn convert<'a, R, K>(v: PreparedMixedWorkerV89Invocation<'a, R, K>) -> PreparedMixedWorkerV53Invocation<'a, R, K> { v }
/// ```
pub use PreparedMixedWorkerInvocation as PreparedMixedWorkerV89Invocation;
