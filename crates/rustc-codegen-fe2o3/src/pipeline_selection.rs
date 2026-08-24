use std::env;
use std::ffi::OsStr;

use crate::amdgpu_llvm;

/// Architectural role of a selectable backend route.
///
/// Only `Production` may become the default device compiler. Qualification
/// oracles retain migration evidence but must not be called by the production
/// transaction or treated as fallback implementations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PipelinePurposeV1 {
    Production,
    QualificationOracle,
}

/// Rustc-process evidence required independently of a pipeline's product role.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RustcInvocationPolicyV1 {
    /// Publication-capable compilation requires the sealed V3 descriptor.
    ProtectedV3,
    /// Qualification-only compilation retains authenticated backend
    /// observations but grants no publication authority.
    QualificationObserved,
    /// Ambient rustc invocation with no fe2o3 authority or observations.
    Unmanaged,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CodegenPipeline {
    ProductionV1,
    SimulationV1,
    KernelIrV1,
    KernelIrWorkerV2,
    CollectedExecutableScalarControlFlowV2,
    CollectedFlashAttentionV1,
    CollectedGeneralGemmV1,
    CollectedMoeTop2V1,
    CollectedRowSoftmaxV1,
    CollectedScalarGemmV1,
    CollectedTiledGemmV1,
    CollectedWave64CollectivesV1,
    CollectedLdsReductionV1,
    CollectedScopedAtomicV1,
}

impl CodegenPipeline {
    pub(crate) const ALL: [Self; 14] = [
        Self::ProductionV1,
        Self::SimulationV1,
        Self::KernelIrV1,
        Self::KernelIrWorkerV2,
        Self::CollectedExecutableScalarControlFlowV2,
        Self::CollectedFlashAttentionV1,
        Self::CollectedGeneralGemmV1,
        Self::CollectedMoeTop2V1,
        Self::CollectedRowSoftmaxV1,
        Self::CollectedScalarGemmV1,
        Self::CollectedTiledGemmV1,
        Self::CollectedWave64CollectivesV1,
        Self::CollectedLdsReductionV1,
        Self::CollectedScopedAtomicV1,
    ];

    pub(crate) const fn purpose(self) -> PipelinePurposeV1 {
        match self {
            Self::ProductionV1 => PipelinePurposeV1::Production,
            Self::SimulationV1
            | Self::KernelIrV1
            | Self::KernelIrWorkerV2
            | Self::CollectedExecutableScalarControlFlowV2
            | Self::CollectedFlashAttentionV1
            | Self::CollectedGeneralGemmV1
            | Self::CollectedMoeTop2V1
            | Self::CollectedRowSoftmaxV1
            | Self::CollectedScalarGemmV1
            | Self::CollectedTiledGemmV1
            | Self::CollectedWave64CollectivesV1
            | Self::CollectedLdsReductionV1
            | Self::CollectedScopedAtomicV1 => PipelinePurposeV1::QualificationOracle,
        }
    }

    pub(crate) const fn rustc_invocation_policy(
        self,
        explicit_unprotected_qualification: bool,
    ) -> RustcInvocationPolicyV1 {
        match self {
            Self::ProductionV1 => RustcInvocationPolicyV1::ProtectedV3,
            Self::SimulationV1 => RustcInvocationPolicyV1::QualificationObserved,
            Self::CollectedRowSoftmaxV1 => {
                if explicit_unprotected_qualification {
                    RustcInvocationPolicyV1::QualificationObserved
                } else {
                    RustcInvocationPolicyV1::ProtectedV3
                }
            }
            Self::KernelIrV1
            | Self::KernelIrWorkerV2
            | Self::CollectedExecutableScalarControlFlowV2
            | Self::CollectedFlashAttentionV1
            | Self::CollectedGeneralGemmV1
            | Self::CollectedMoeTop2V1
            | Self::CollectedScalarGemmV1
            | Self::CollectedTiledGemmV1
            | Self::CollectedWave64CollectivesV1
            | Self::CollectedLdsReductionV1
            | Self::CollectedScopedAtomicV1 => {
                if explicit_unprotected_qualification {
                    RustcInvocationPolicyV1::QualificationObserved
                } else {
                    RustcInvocationPolicyV1::Unmanaged
                }
            }
        }
    }

    pub(crate) const fn selector_name(self) -> &'static str {
        match self {
            Self::ProductionV1 => crate::production_pipeline_v1::PRODUCTION_PIPELINE_V1,
            Self::SimulationV1 => crate::production_pipeline_v1::SIMULATION_PIPELINE_V1,
            Self::KernelIrV1 => "kernel-ir-v1",
            Self::KernelIrWorkerV2 => "kernel-ir-worker-v2",
            Self::CollectedExecutableScalarControlFlowV2 => {
                crate::collected_executable_scalar_control_flow_v2::COLLECTED_SCALAR_CONTROL_FLOW_PIPELINE_V2
            }
            Self::CollectedFlashAttentionV1 => {
                crate::collected_flash_attention_v1::COLLECTED_FLASH_ATTENTION_PIPELINE_V1
            }
            Self::CollectedGeneralGemmV1 => {
                crate::general_gemm_pipeline_v1::GENERAL_GEMM_PIPELINE_V1
            }
            Self::CollectedMoeTop2V1 => {
                crate::collected_moe_top2_v1::COLLECTED_MOE_TOP2_PIPELINE_V1
            }
            Self::CollectedRowSoftmaxV1 => {
                crate::collected_row_softmax_v1::COLLECTED_ROW_SOFTMAX_PIPELINE_V1
            }
            Self::CollectedScalarGemmV1 => {
                crate::collected_scalar_gemm_v1::COLLECTED_SCALAR_GEMM_PIPELINE_V1
            }
            Self::CollectedTiledGemmV1 => {
                crate::collected_tiled_gemm_v1::COLLECTED_TILED_GEMM_PIPELINE_V1
            }
            Self::CollectedWave64CollectivesV1 => {
                crate::collected_wave64_collectives_v1::COLLECTED_WAVE64_COLLECTIVES_PIPELINE_V1
            }
            Self::CollectedLdsReductionV1 => {
                crate::collected_workgroup_sync_v1::COLLECTED_LDS_REDUCTION_PIPELINE_V1
            }
            Self::CollectedScopedAtomicV1 => {
                crate::collected_workgroup_sync_v1::COLLECTED_SCOPED_ATOMIC_PIPELINE_V1
            }
        }
    }
}

/// One explicitly selected qualification-only backend route.
///
/// This token cannot be produced by the unset/default selection path. Passing
/// it into qualification collection keeps those compatibility routes out of
/// the ordinary production transaction by construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct QualificationPipelineV1 {
    pipeline: CodegenPipeline,
}

impl QualificationPipelineV1 {
    pub(crate) const fn pipeline(self) -> CodegenPipeline {
        self.pipeline
    }

    pub(crate) const fn requires_extended_collector_edges(self) -> bool {
        matches!(
            self.pipeline,
            CodegenPipeline::CollectedFlashAttentionV1 | CodegenPipeline::CollectedMoeTop2V1
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DevicePipelineRouteV1 {
    ProductionComplete,
    QualificationOracle(QualificationPipelineV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ResolvedPipelineV1 {
    pipeline: CodegenPipeline,
    explicitly_selected: bool,
}

impl ResolvedPipelineV1 {
    pub(crate) const fn pipeline(self) -> CodegenPipeline {
        self.pipeline
    }

    pub(crate) fn device_route(
        self,
        production_transaction_complete: bool,
    ) -> Result<DevicePipelineRouteV1, String> {
        match (
            self.pipeline.purpose(),
            self.explicitly_selected,
            production_transaction_complete,
        ) {
            (PipelinePurposeV1::Production, _, true) => {
                Ok(DevicePipelineRouteV1::ProductionComplete)
            }
            (PipelinePurposeV1::Production, _, false) => Err(format!(
                "production pipeline `{}` did not complete its device transaction; qualification fallback is forbidden",
                self.pipeline.selector_name(),
            )),
            (PipelinePurposeV1::QualificationOracle, true, false) => Ok(
                DevicePipelineRouteV1::QualificationOracle(QualificationPipelineV1 {
                    pipeline: self.pipeline,
                }),
            ),
            (PipelinePurposeV1::QualificationOracle, false, false) => Err(format!(
                "qualification pipeline `{}` was not explicitly selected",
                self.pipeline.selector_name(),
            )),
            (PipelinePurposeV1::QualificationOracle, _, true) => Err(format!(
                "qualification pipeline `{}` cannot complete or publish as the production device transaction",
                self.pipeline.selector_name(),
            )),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PipelineSelection {
    DefaultProduction,
    Explicit(CodegenPipeline),
    Invalid(String),
}

impl PipelineSelection {
    pub(crate) fn from_env() -> Self {
        Self::from_value(env::var_os(crate::CODEGEN_PIPELINE_ENV).as_deref())
    }

    pub(crate) fn from_value(value: Option<&OsStr>) -> Self {
        let Some(value) = value else {
            return Self::DefaultProduction;
        };
        if let Some(pipeline) = CodegenPipeline::ALL
            .into_iter()
            .find(|pipeline| value == OsStr::new(pipeline.selector_name()))
        {
            return Self::Explicit(pipeline);
        }
        let supported = CodegenPipeline::ALL
            .into_iter()
            .map(|pipeline| format!("`{}`", pipeline.selector_name()))
            .collect::<Vec<_>>()
            .join(", ");
        Self::Invalid(format!(
            "{} must be unset (selecting `production-v1`) or exactly one of {supported}; found {value:?}",
            crate::CODEGEN_PIPELINE_ENV,
        ))
    }

    pub(crate) fn resolve(&self) -> Result<ResolvedPipelineV1, amdgpu_llvm::EmitError> {
        match self {
            Self::DefaultProduction => Ok(ResolvedPipelineV1 {
                pipeline: CodegenPipeline::ProductionV1,
                explicitly_selected: false,
            }),
            Self::Explicit(pipeline) => Ok(ResolvedPipelineV1 {
                pipeline: *pipeline,
                explicitly_selected: true,
            }),
            Self::Invalid(reason) => Err(amdgpu_llvm::EmitError::Preflight {
                reason: reason.clone(),
            }),
        }
    }
}

impl Default for PipelineSelection {
    fn default() -> Self {
        Self::DefaultProduction
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::ffi::OsStr;

    use super::{
        CodegenPipeline, DevicePipelineRouteV1, PipelinePurposeV1, PipelineSelection,
        RustcInvocationPolicyV1,
    };

    #[test]
    fn exactly_one_selectable_route_is_a_production_pipeline() {
        let production = CodegenPipeline::ALL
            .into_iter()
            .filter(|pipeline| pipeline.purpose() == PipelinePurposeV1::Production)
            .collect::<Vec<_>>();
        assert_eq!(production, [CodegenPipeline::ProductionV1]);
    }

    #[test]
    fn selector_names_are_unique_and_round_trip_through_one_table() {
        let names = CodegenPipeline::ALL
            .into_iter()
            .map(CodegenPipeline::selector_name)
            .collect::<BTreeSet<_>>();
        assert_eq!(names.len(), CodegenPipeline::ALL.len());
        for pipeline in CodegenPipeline::ALL {
            assert_eq!(
                PipelineSelection::from_value(Some(OsStr::new(pipeline.selector_name()))),
                PipelineSelection::Explicit(pipeline),
            );
        }
    }

    #[test]
    fn unset_and_default_selection_resolve_only_to_production() {
        for selection in [
            PipelineSelection::from_value(None),
            PipelineSelection::default(),
        ] {
            assert_eq!(selection, PipelineSelection::DefaultProduction);
            let resolved = selection.resolve().expect("default must resolve");
            assert_eq!(resolved.pipeline(), CodegenPipeline::ProductionV1);
            assert_eq!(
                resolved
                    .device_route(true)
                    .expect("completed production transaction"),
                DevicePipelineRouteV1::ProductionComplete,
            );
            assert!(
                resolved
                    .device_route(false)
                    .expect_err("production must never fall back")
                    .contains("qualification fallback is forbidden")
            );
        }
    }

    #[test]
    fn every_qualification_route_requires_explicit_selection_and_cannot_publish_as_production() {
        for pipeline in CodegenPipeline::ALL {
            if pipeline.purpose() != PipelinePurposeV1::QualificationOracle {
                continue;
            }
            let explicit =
                PipelineSelection::from_value(Some(OsStr::new(pipeline.selector_name())))
                    .resolve()
                    .expect("explicit qualification selector must resolve");
            let DevicePipelineRouteV1::QualificationOracle(qualification) = explicit
                .device_route(false)
                .expect("explicit qualification route")
            else {
                panic!("qualification selector became production")
            };
            assert_eq!(qualification.pipeline(), pipeline);
            assert_eq!(
                qualification.requires_extended_collector_edges(),
                matches!(
                    pipeline,
                    CodegenPipeline::CollectedFlashAttentionV1
                        | CodegenPipeline::CollectedMoeTop2V1
                )
            );
            assert!(
                explicit
                    .device_route(true)
                    .expect_err("qualification cannot publish as production")
                    .contains("cannot complete or publish as the production")
            );

            let implicit = super::ResolvedPipelineV1 {
                pipeline,
                explicitly_selected: false,
            };
            assert!(
                implicit
                    .device_route(false)
                    .expect_err("implicit qualification route must be impossible")
                    .contains("was not explicitly selected")
            );
        }
    }

    #[test]
    fn invocation_authority_is_independent_of_pipeline_purpose() {
        assert_eq!(
            CodegenPipeline::ProductionV1.rustc_invocation_policy(true),
            RustcInvocationPolicyV1::ProtectedV3,
        );
        assert_eq!(
            CodegenPipeline::CollectedRowSoftmaxV1.rustc_invocation_policy(false),
            RustcInvocationPolicyV1::ProtectedV3,
        );
        for explicit_unprotected_qualification in [false, true] {
            assert_eq!(
                CodegenPipeline::SimulationV1
                    .rustc_invocation_policy(explicit_unprotected_qualification),
                RustcInvocationPolicyV1::QualificationObserved,
            );
        }
        for pipeline in CodegenPipeline::ALL {
            if matches!(
                pipeline,
                CodegenPipeline::ProductionV1
                    | CodegenPipeline::SimulationV1
                    | CodegenPipeline::CollectedRowSoftmaxV1
            ) {
                continue;
            }
            assert_eq!(
                pipeline.rustc_invocation_policy(false),
                RustcInvocationPolicyV1::Unmanaged,
            );
            assert_eq!(
                pipeline.rustc_invocation_policy(true),
                RustcInvocationPolicyV1::QualificationObserved,
            );
        }
    }
}
