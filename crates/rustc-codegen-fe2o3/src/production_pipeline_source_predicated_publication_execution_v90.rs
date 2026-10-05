//! Execute only the retained predicated source/prefix/LICM/final request.
use super::*;
use fe2o3_verifier::{
    ExecutedPredicatedTypedSourceTailV90 as Executed,
    FunctionalRefinementVerusRuntimeLeaseV1 as Runtime,
    PreparedPredicatedTypedSourceTailExecutionV90 as PreparedExecution,
};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

include!("production_pipeline_source_mixed_publication_execution_family.rs");
mixed_publication_execution_family!(prepare_predicated_typed_execution_v90);
