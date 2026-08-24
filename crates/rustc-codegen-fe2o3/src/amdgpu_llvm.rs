use crate::{AmdGpuTarget, compile_llvm_ir_to_hsaco};
use fe2o3_artifact_transaction::{
    BuildAttempt, ProducerIdentity, emit_artifact_transaction_after_preflight,
    emit_artifact_transaction_after_preflight_for_attempt,
};
pub use fe2o3_artifact_transaction::{DeviceArtifact, EmitError};
use std::path::Path;

pub(crate) fn emit_collection_after_preflight(
    producer: &ProducerIdentity,
    output_dir: &Path,
    target: &AmdGpuTarget,
    attempt: Option<BuildAttempt>,
    preflight: impl FnOnce() -> Result<Vec<PreparedDeviceKernel>, EmitError>,
) -> Result<Vec<DeviceArtifact>, EmitError> {
    let compile = |llvm_ir_path: &Path, hsaco_path: &Path| {
        compile_llvm_ir_to_hsaco(llvm_ir_path, hsaco_path, target)
            .map_err(|error| EmitError::Compilation(Box::new(error)))
    };
    match attempt {
        Some(attempt) => emit_artifact_transaction_after_preflight_for_attempt(
            output_dir,
            producer,
            attempt,
            preflight,
            |kernel| &kernel.name,
            |kernel| Ok(kernel.llvm_ir.clone()),
            compile,
        ),
        None => emit_artifact_transaction_after_preflight(
            output_dir,
            producer,
            preflight,
            |kernel| &kernel.name,
            |kernel| Ok(kernel.llvm_ir.clone()),
            compile,
        ),
    }
}

#[derive(Debug)]
pub(crate) struct PreparedDeviceKernel {
    pub(crate) name: String,
    pub(crate) llvm_ir: String,
}
