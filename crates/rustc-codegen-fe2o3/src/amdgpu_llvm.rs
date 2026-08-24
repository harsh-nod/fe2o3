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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_OUTPUT: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn empty_preflight_reconciles_without_invoking_the_rocm_compiler() {
        let output = std::env::temp_dir().join(format!(
            "fe2o3-zero-kernel-reconciliation-{}-{}",
            std::process::id(),
            NEXT_OUTPUT.fetch_add(1, Ordering::Relaxed),
        ));
        let producer = ProducerIdentity::from_codegen(
            "zero_kernel_reconciliation",
            Some(Path::new("/tests/zero-kernel.rs")),
        )
        .unwrap();

        let artifacts = emit_collection_after_preflight(
            &producer,
            &output,
            &AmdGpuTarget::default(),
            None,
            || Ok(Vec::new()),
        )
        .unwrap();

        assert!(artifacts.is_empty());
        assert!(fs::read_dir(&output).unwrap().all(|entry| !matches!(
                    entry.unwrap().path().extension().and_then(|value| value.to_str()),
                    Some("ll" | "o" | "hsaco")
                )));
        fs::remove_dir_all(output).unwrap();
    }
}
