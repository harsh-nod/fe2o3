use std::env;
use std::ffi::OsStr;

use crate::amdgpu_llvm;

/// Rustc-process evidence required independently of a route's product role.
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

/// A temporary, non-publishing oracle used to qualify production migrations.
///
/// These are deliberately not compilation routes. Production has no enum
/// variant or selector that can be confused with one of these oracles.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum QualificationOracleV1 {
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

impl QualificationOracleV1 {
    pub(crate) const ALL: [Self; 13] = [
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

    pub(crate) const fn selector_name(self) -> &'static str {
        match self {
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

    const fn rustc_invocation_policy(
        self,
        explicit_unprotected_qualification: bool,
    ) -> RustcInvocationPolicyV1 {
        match self {
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
}

/// One explicitly selected qualification-only backend route.
///
/// This token cannot be produced by the unset/default selection path. Passing
/// it into qualification collection keeps compatibility oracles out of the
/// production transaction by construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct QualificationPipelineV1 {
    oracle: QualificationOracleV1,
}

impl QualificationPipelineV1 {
    pub(crate) const fn oracle(self) -> QualificationOracleV1 {
        self.oracle
    }

    pub(crate) const fn requires_extended_collector_edges(self) -> bool {
        matches!(
            self.oracle,
            QualificationOracleV1::CollectedFlashAttentionV1
                | QualificationOracleV1::CollectedMoeTop2V1
        )
    }
}

/// Compiler execution has exactly one publishing route: `Production`.
///
/// The qualification case exists only while test oracles are migrated. It
/// carries a capability that cannot publish or complete a production build.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CompilationRouteV1 {
    Production,
    Qualification(QualificationPipelineV1),
}

impl CompilationRouteV1 {
    pub(crate) const fn is_production(self) -> bool {
        matches!(self, Self::Production)
    }

    pub(crate) const fn qualification(self) -> Option<QualificationPipelineV1> {
        match self {
            Self::Production => None,
            Self::Qualification(qualification) => Some(qualification),
        }
    }

    pub(crate) const fn qualification_oracle(self) -> Option<QualificationOracleV1> {
        match self.qualification() {
            Some(qualification) => Some(qualification.oracle()),
            None => None,
        }
    }

    pub(crate) const fn rustc_invocation_policy(
        self,
        explicit_unprotected_qualification: bool,
    ) -> RustcInvocationPolicyV1 {
        match self {
            Self::Production => RustcInvocationPolicyV1::ProtectedV3,
            Self::Qualification(qualification) => qualification
                .oracle()
                .rustc_invocation_policy(explicit_unprotected_qualification),
        }
    }

    pub(crate) fn device_route(
        self,
        production_transaction_complete: bool,
    ) -> Result<DevicePipelineRouteV1, String> {
        match (self, production_transaction_complete) {
            (Self::Production, true) => Ok(DevicePipelineRouteV1::ProductionComplete),
            (Self::Production, false) => Err(
                "production compilation did not complete its device transaction; qualification fallback is forbidden"
                    .to_owned(),
            ),
            (Self::Qualification(qualification), false) => {
                Ok(DevicePipelineRouteV1::QualificationOracle(qualification))
            }
            (Self::Qualification(qualification), true) => Err(format!(
                "qualification oracle `{}` cannot complete or publish as the production device transaction",
                qualification.oracle().selector_name(),
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DevicePipelineRouteV1 {
    ProductionComplete,
    QualificationOracle(QualificationPipelineV1),
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) enum PipelineSelection {
    #[default]
    DefaultProduction,
    ExplicitQualification(QualificationOracleV1),
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
        if let Some(oracle) = QualificationOracleV1::ALL
            .into_iter()
            .find(|oracle| value == OsStr::new(oracle.selector_name()))
        {
            return Self::ExplicitQualification(oracle);
        }
        let supported = QualificationOracleV1::ALL
            .into_iter()
            .map(|oracle| format!("`{}`", oracle.selector_name()))
            .collect::<Vec<_>>()
            .join(", ");
        Self::Invalid(format!(
            "{} must be unset for production compilation; temporary qualification selectors are {supported}; found {value:?}",
            crate::CODEGEN_PIPELINE_ENV,
        ))
    }

    pub(crate) fn resolve(&self) -> Result<CompilationRouteV1, amdgpu_llvm::EmitError> {
        match self {
            Self::DefaultProduction => Ok(CompilationRouteV1::Production),
            Self::ExplicitQualification(oracle) => {
                Ok(CompilationRouteV1::Qualification(QualificationPipelineV1 {
                    oracle: *oracle,
                }))
            }
            Self::Invalid(reason) => Err(amdgpu_llvm::EmitError::Preflight {
                reason: reason.clone(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::ffi::OsStr;

    use super::{
        CompilationRouteV1, DevicePipelineRouteV1, PipelineSelection, QualificationOracleV1,
        RustcInvocationPolicyV1,
    };

    #[test]
    fn production_has_no_selector_or_alternate_variant() {
        let route = PipelineSelection::from_value(None)
            .resolve()
            .expect("unset selection must resolve");
        assert_eq!(route, CompilationRouteV1::Production);
        assert!(route.is_production());
        assert_eq!(route.qualification(), None);
        assert_eq!(route.qualification_oracle(), None);

        let PipelineSelection::Invalid(reason) =
            PipelineSelection::from_value(Some(OsStr::new("production-v1")))
        else {
            panic!("production must not have an explicit selector")
        };
        assert!(reason.contains("must be unset for production compilation"));
    }

    #[test]
    fn qualification_selector_names_are_unique_and_round_trip() {
        let names = QualificationOracleV1::ALL
            .into_iter()
            .map(QualificationOracleV1::selector_name)
            .collect::<BTreeSet<_>>();
        assert_eq!(names.len(), QualificationOracleV1::ALL.len());
        for oracle in QualificationOracleV1::ALL {
            assert_eq!(
                PipelineSelection::from_value(Some(OsStr::new(oracle.selector_name()))),
                PipelineSelection::ExplicitQualification(oracle),
            );
        }
    }

    #[test]
    fn production_never_falls_back_to_qualification() {
        let route = PipelineSelection::default()
            .resolve()
            .expect("default must resolve");
        assert_eq!(
            route
                .device_route(true)
                .expect("completed production transaction"),
            DevicePipelineRouteV1::ProductionComplete,
        );
        assert!(
            route
                .device_route(false)
                .expect_err("production must never fall back")
                .contains("qualification fallback is forbidden")
        );
    }

    #[test]
    fn every_qualification_oracle_is_explicit_and_non_publishing() {
        for oracle in QualificationOracleV1::ALL {
            let route = PipelineSelection::from_value(Some(OsStr::new(oracle.selector_name())))
                .resolve()
                .expect("explicit qualification selector must resolve");
            assert!(!route.is_production());
            assert_eq!(route.qualification_oracle(), Some(oracle));
            let DevicePipelineRouteV1::QualificationOracle(qualification) = route
                .device_route(false)
                .expect("explicit qualification route")
            else {
                panic!("qualification selector became production")
            };
            assert_eq!(qualification.oracle(), oracle);
            assert_eq!(
                qualification.requires_extended_collector_edges(),
                matches!(
                    oracle,
                    QualificationOracleV1::CollectedFlashAttentionV1
                        | QualificationOracleV1::CollectedMoeTop2V1
                )
            );
            assert!(
                route
                    .device_route(true)
                    .expect_err("qualification cannot publish as production")
                    .contains("cannot complete or publish as the production")
            );
        }
    }

    #[test]
    fn invocation_authority_is_independent_of_route_role() {
        assert_eq!(
            CompilationRouteV1::Production.rustc_invocation_policy(true),
            RustcInvocationPolicyV1::ProtectedV3,
        );
        for oracle in QualificationOracleV1::ALL {
            let route = PipelineSelection::ExplicitQualification(oracle)
                .resolve()
                .expect("qualification route");
            let expected_without_override = match oracle {
                QualificationOracleV1::SimulationV1 => {
                    RustcInvocationPolicyV1::QualificationObserved
                }
                QualificationOracleV1::CollectedRowSoftmaxV1 => {
                    RustcInvocationPolicyV1::ProtectedV3
                }
                _ => RustcInvocationPolicyV1::Unmanaged,
            };
            assert_eq!(
                route.rustc_invocation_policy(false),
                expected_without_override,
            );
            assert_eq!(
                route.rustc_invocation_policy(true),
                RustcInvocationPolicyV1::QualificationObserved,
            );
        }
    }
}
