//! Single production-pipeline transaction shell.
//!
//! This module owns the one integration point for issue #175. It deliberately
//! contains no workload recognition. The sole semantic-MIR importer owns the
//! consuming target-authentication boundary and moves an admitted request into
//! a typed stage before the still-pending middle-end transition fails closed.

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

#[derive(Debug)]
pub(crate) enum ProductionPipelineErrorV1 {
    CustomLlvmConfiguration,
    EmptyCollectedDeviceClosure,
    SemanticImport(crate::collector::ProductionSemanticImportErrorV1),
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
            Self::SemanticImport(error) => write!(formatter, "production-v1 {error}"),
        }
    }
}

impl std::error::Error for ProductionPipelineErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SemanticImport(error) => Some(error),
            Self::CustomLlvmConfiguration | Self::EmptyCollectedDeviceClosure => None,
        }
    }
}

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

pub(super) struct AdmittedSemanticMirStageV1 {
    semantic_mir: fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1,
    rustc_identity_inventory_sha256: [u8; 32],
    rustc_preflight_plan_sha256: [u8; 32],
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

    fn import_semantic_mir(
        self,
    ) -> Result<ProductionCompilationV1<'tcx, AdmittedSemanticMirStageV1>, ProductionPipelineErrorV1>
    {
        let CollectedRustStageV1 {
            tcx,
            closure,
            producer,
            output_dir,
            build_attempt,
        } = self.stage;
        let (semantic_mir, rustc_identity_inventory_sha256, rustc_preflight_plan_sha256) =
            crate::collector::construct_production_semantic_mir_v1(tcx, closure)
                .map_err(ProductionPipelineErrorV1::SemanticImport)?;
        Ok(ProductionCompilationV1 {
            stage: AdmittedSemanticMirStageV1 {
                semantic_mir,
                rustc_identity_inventory_sha256,
                rustc_preflight_plan_sha256,
                producer,
                output_dir,
                build_attempt,
            },
            invariant_session: PhantomData,
        })
    }

    /// Consumes the only production transaction through the sole importer.
    pub(crate) fn require_semantic_mir_import(self) -> ProductionPipelineErrorV1 {
        match self.import_semantic_mir() {
            Ok(transaction) => transaction.require_middle_end(),
            Err(error) => error,
        }
    }
}

impl<'tcx> ProductionCompilationV1<'tcx, AdmittedSemanticMirStageV1> {
    fn require_middle_end(self) -> ProductionPipelineErrorV1 {
        let AdmittedSemanticMirStageV1 {
            semantic_mir,
            rustc_identity_inventory_sha256,
            rustc_preflight_plan_sha256,
            producer,
            output_dir,
            build_attempt,
        } = self.stage;
        let error = crate::collector::ProductionSemanticImportErrorV1::SemanticMiddleEndPending {
            functions: semantic_mir.functions().len(),
            callables: semantic_mir.callables().len(),
            rustc_identity_inventory_sha256,
            rustc_preflight_plan_sha256,
            semantic_sha256: *semantic_mir.semantic_sha256().as_bytes(),
        };
        drop((semantic_mir, producer, output_dir, build_attempt));
        ProductionPipelineErrorV1::SemanticImport(error)
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
        assert!(reject_custom_llvm_configuration(false).is_ok());
        assert!(matches!(
            reject_custom_llvm_configuration(true),
            Err(ProductionPipelineErrorV1::CustomLlvmConfiguration)
        ));
    }

    #[test]
    fn unavailable_import_diagnostic_is_deterministic_and_fail_closed() {
        let error = ProductionPipelineErrorV1::SemanticImport(
            crate::collector::ProductionSemanticImportErrorV1::SemanticMiddleEndPending {
                functions: 3,
                callables: 6,
                rustc_identity_inventory_sha256: [0xab; 32],
                rustc_preflight_plan_sha256: [0xcd; 32],
                semantic_sha256: [0xef; 32],
            },
        );
        assert_eq!(
            error.to_string(),
            format!(
                "production-v1 semantic importer authenticated rustc identity inventory {} and bounded preflight plan {}, then admitted one complete semantic MIR request with 3 function(s), 6 callable(s), and canonical identity {}; semantic middle-end construction remains pending; no fallback or artifact emission was entered",
                "ab".repeat(32),
                "cd".repeat(32),
                "ef".repeat(32),
            )
        );
    }

    #[test]
    fn production_module_contains_no_profile_selection_vocabulary() {
        let sources = [
            include_str!("production_pipeline_v1.rs"),
            include_str!("collector/production_importer_v1.rs"),
            include_str!("rustc_semantic_adapter_v1.rs"),
            include_str!("rustc_semantic_plan_v1.rs"),
            include_str!("production_semantic_fn_abi_v1.rs"),
            include_str!("production_semantic_types_v1.rs"),
            include_str!("production_semantic_terminal_v1.rs"),
        ];
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
                !sources[0].contains(forbidden),
                "production transaction contains forbidden selector term {forbidden:?}"
            );
        }

        for forbidden_importer_term in [
            concat!("General", "Gemm"),
            concat!("Flash", "Attention"),
            concat!("Row", "Softmax"),
            concat!("Moe", "Top2"),
            concat!("source", " substring"),
            concat!("MIR", " transcript"),
            concat!("legacy", "-v1"),
            concat!("kernel-ir", "-v1"),
        ] {
            assert!(
                sources
                    .iter()
                    .skip(1)
                    .all(|source| !source.contains(forbidden_importer_term)),
                "production importer contains forbidden selector term {forbidden_importer_term:?}"
            );
        }

        for forbidden_dependency in [
            concat!("mir_import", "_v2"),
            concat!("same_session", "_rustc_v1"),
            concat!("frontend_record", "_bridge"),
            concat!("semantic_type", "_adapter_v2"),
            concat!("source_", "debug"),
            concat!("semantic_", "features"),
            concat!("crate::", "collected_"),
            concat!("collected_", "general_gemm_v1"),
        ] {
            assert!(
                sources
                    .iter()
                    .skip(1)
                    .all(|source| !source.contains(forbidden_dependency)),
                "production importer depends on qualification module {forbidden_dependency:?}"
            );
        }
    }

    #[test]
    fn production_backend_authenticates_target_before_monomorphization() {
        let backend = include_str!("lib.rs");
        let codegen = backend
            .split_once("fn codegen_crate")
            .expect("codegen entry")
            .1;
        let authentication = codegen
            .find("authenticate_before_collection")
            .expect("pre-collection target authentication");
        let monomorphization = codegen
            .find("collect_and_partition_mono_items")
            .expect("rustc monomorphization");
        assert!(authentication < monomorphization);
    }
}
