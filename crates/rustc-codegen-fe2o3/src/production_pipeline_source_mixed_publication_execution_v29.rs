//! Exact admitted-runtime composition while original protected custody lives.
//! This transition cannot close capsule, finalizer, MIR or runtime-premise gates.

use super::*;
use fe2o3_verifier::{
    ExecutedTypedSourceTailV50 as Executed, FunctionalRefinementVerusRuntimeLeaseV1 as Runtime,
};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

use fe2o3_verifier::PreparedTypedSourceTailExecutionV50 as PreparedExecution;
include!("production_pipeline_source_mixed_publication_execution_family.rs");
mixed_publication_execution_family!(prepare_typed_execution_v50);
