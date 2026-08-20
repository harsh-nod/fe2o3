//! Single production-pipeline transaction shell.
//!
//! This module owns the one integration point for issue #175. It deliberately
//! contains no workload recognition. Until the generic semantic-MIR importer
//! lands, selecting this path consumes the collected transaction and fails
//! closed without entering another code-generation route.

use std::fmt;
use std::marker::PhantomData;
use std::path::PathBuf;

use rustc_middle::ty::TyCtxt;

use crate::artifact_transaction::{BuildAttempt, ProducerIdentity};
use crate::collector::AuthenticatedCollectedKernelClosureV1;

pub(crate) const PRODUCTION_PIPELINE_V1: &str = "production-v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProductionDispositionV1 {
    HostOnly,
    DeviceTransaction,
}

pub(crate) const fn disposition(device_candidate_count: usize) -> ProductionDispositionV1 {
    if device_candidate_count == 0 {
        ProductionDispositionV1::HostOnly
    } else {
        ProductionDispositionV1::DeviceTransaction
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum ProductionPipelineErrorV1 {
    CustomLlvmConfiguration,
    EmptyCollectedDeviceClosure,
    SemanticImporterUnavailable {
        collected_functions: usize,
        registered_kernel_roots: usize,
        target: String,
    },
}

impl fmt::Display for ProductionPipelineErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CustomLlvmConfiguration => formatter.write_str(
                "production-v1 rejects caller-selected LLVM arguments or passes before transaction construction",
            ),
            Self::EmptyCollectedDeviceClosure => formatter.write_str(
                "production-v1 requires a nonempty collector-sealed device closure",
            ),
            Self::SemanticImporterUnavailable {
                collected_functions,
                registered_kernel_roots,
                target,
            } => write!(
                formatter,
                "production-v1 retained {collected_functions} collected device function(s), {registered_kernel_roots} registered kernel root(s), and configured target {target:?}, but the generic semantic-MIR import transition is not implemented; the transaction was consumed without fallback or artifact emission",
            ),
        }
    }
}

impl std::error::Error for ProductionPipelineErrorV1 {}

pub(crate) fn reject_custom_llvm_configuration(
    has_custom_llvm_configuration: bool,
) -> Result<(), ProductionPipelineErrorV1> {
    if has_custom_llvm_configuration {
        Err(ProductionPipelineErrorV1::CustomLlvmConfiguration)
    } else {
        Ok(())
    }
}

pub(super) struct CollectedRustStageV1<'tcx> {
    tcx: TyCtxt<'tcx>,
    closure: AuthenticatedCollectedKernelClosureV1<'tcx>,
    producer: ProducerIdentity,
    output_dir: PathBuf,
    build_attempt: Option<BuildAttempt>,
}

/// Move-only owner of one production compilation stage.
///
/// Its fields and stage types stay private so no caller can synthesize or
/// bypass a transition. The transaction carries no artifact, publication,
/// load, launch, or runtime authority.
pub(crate) struct ProductionCompilationV1<'tcx, Stage> {
    stage: Stage,
    invariant_session: PhantomData<fn(TyCtxt<'tcx>) -> TyCtxt<'tcx>>,
}

impl<'tcx> ProductionCompilationV1<'tcx, CollectedRustStageV1<'tcx>> {
    /// Retains the collector-sealed closure without granting semantic authority.
    /// The next transition must authenticate every imported MIR fact.
    pub(crate) fn from_collected_device_closure(
        tcx: TyCtxt<'tcx>,
        closure: AuthenticatedCollectedKernelClosureV1<'tcx>,
        producer: ProducerIdentity,
        output_dir: PathBuf,
        build_attempt: Option<BuildAttempt>,
    ) -> Result<Self, ProductionPipelineErrorV1> {
        if closure.function_count() == 0 {
            return Err(ProductionPipelineErrorV1::EmptyCollectedDeviceClosure);
        }
        Ok(Self {
            stage: CollectedRustStageV1 {
                tcx,
                closure,
                producer,
                output_dir,
                build_attempt,
            },
            invariant_session: PhantomData,
        })
    }

    /// Consumes the only production transaction at the first unavailable stage.
    pub(crate) fn require_semantic_mir_import(self) -> ProductionPipelineErrorV1 {
        let CollectedRustStageV1 {
            tcx,
            closure,
            producer,
            output_dir,
            build_attempt,
        } = self.stage;
        let collected_functions = closure.function_count();
        let registered_kernel_roots = closure.kernel_root_count();
        let target = closure.target().to_owned();
        let collection = closure.into_collection();

        // Retain and consume every future transaction input at this boundary.
        // None may be recovered to enter a different compiler route.
        drop((tcx, collection, producer, output_dir, build_attempt));

        ProductionPipelineErrorV1::SemanticImporterUnavailable {
            collected_functions,
            registered_kernel_roots,
            target,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_only_and_device_dispositions_are_exact() {
        assert_eq!(disposition(0), ProductionDispositionV1::HostOnly);
        assert_eq!(disposition(1), ProductionDispositionV1::DeviceTransaction);
        assert_eq!(
            disposition(usize::MAX),
            ProductionDispositionV1::DeviceTransaction
        );
    }

    #[test]
    fn custom_llvm_configuration_is_terminal_before_construction() {
        assert_eq!(reject_custom_llvm_configuration(false), Ok(()));
        assert_eq!(
            reject_custom_llvm_configuration(true),
            Err(ProductionPipelineErrorV1::CustomLlvmConfiguration)
        );
    }

    #[test]
    fn unavailable_import_diagnostic_is_deterministic_and_fail_closed() {
        let error = ProductionPipelineErrorV1::SemanticImporterUnavailable {
            collected_functions: 3,
            registered_kernel_roots: 2,
            target: "gfx942:xnack-".to_owned(),
        };
        assert_eq!(
            error.to_string(),
            "production-v1 retained 3 collected device function(s), 2 registered kernel root(s), and configured target \"gfx942:xnack-\", but the generic semantic-MIR import transition is not implemented; the transaction was consumed without fallback or artifact emission"
        );
    }

    #[test]
    fn production_module_contains_no_profile_selection_vocabulary() {
        let source = include_str!("production_pipeline_v1.rs");
        for forbidden in [
            concat!("General", "Gemm"),
            concat!("Flash", "Attention"),
            concat!("Row", "Softmax"),
            concat!("Moe", "Top2"),
            concat!("export", "_name"),
            concat!("source", " substring"),
            concat!("MIR", " transcript"),
            concat!("legacy", "-v1"),
            concat!("kernel-ir", "-v1"),
            concat!("Collection", "Result"),
            concat!("target: AmdGpu", "Target"),
        ] {
            assert!(
                !source.contains(forbidden),
                "production transaction contains forbidden selector term {forbidden:?}"
            );
        }
    }
}
