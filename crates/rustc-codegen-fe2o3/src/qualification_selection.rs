use std::env;
use std::ffi::OsStr;

use crate::amdgpu_llvm;

#[cfg(feature = "qualification-oracles-test-only")]
const SIMULATION_ORACLE_NAME_V1: &str = "simulation-v1";

/// Rustc-process evidence required by production or a qualification oracle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RustcInvocationPolicy {
    /// Publication-capable compilation requires the sealed V3 descriptor.
    ProtectedV3,
    /// Qualification-only compilation retains authenticated backend
    /// observations but grants no publication authority.
    #[cfg(feature = "qualification-oracles-test-only")]
    QualificationObserved,
    /// Ambient rustc invocation with no fe2o3 authority or observations.
    #[cfg(feature = "qualification-oracles-test-only")]
    Unmanaged,
}

/// A temporary, non-publishing oracle used to qualify production migrations.
///
/// These are deliberately not compilation routes. Production has no enum
/// variant or selector that can be confused with one of these oracles.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(feature = "qualification-oracles-test-only")]
pub(crate) enum QualificationOracle {
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

#[cfg(feature = "qualification-oracles-test-only")]
impl QualificationOracle {
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

    pub(crate) const fn oracle_name(self) -> &'static str {
        match self {
            Self::SimulationV1 => SIMULATION_ORACLE_NAME_V1,
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
    ) -> RustcInvocationPolicy {
        match self {
            Self::SimulationV1 => RustcInvocationPolicy::QualificationObserved,
            Self::CollectedRowSoftmaxV1 => {
                if explicit_unprotected_qualification {
                    RustcInvocationPolicy::QualificationObserved
                } else {
                    RustcInvocationPolicy::ProtectedV3
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
                    RustcInvocationPolicy::QualificationObserved
                } else {
                    RustcInvocationPolicy::Unmanaged
                }
            }
        }
    }
}

/// One explicitly selected, non-publishing qualification oracle.
///
/// This token cannot be produced by the unset/default selection path. Passing
/// it into qualification collection keeps compatibility oracles out of the
/// production transaction by construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(feature = "qualification-oracles-test-only")]
pub(crate) struct SelectedQualificationOracle {
    oracle: QualificationOracle,
}

#[cfg(feature = "qualification-oracles-test-only")]
impl SelectedQualificationOracle {
    pub(crate) const fn oracle(self) -> QualificationOracle {
        self.oracle
    }

    pub(crate) const fn requires_extended_collector_edges(self) -> bool {
        matches!(
            self.oracle,
            QualificationOracle::CollectedFlashAttentionV1
                | QualificationOracle::CollectedMoeTop2V1
        )
    }

    pub(crate) const fn rustc_invocation_policy(
        self,
        explicit_unprotected_qualification: bool,
    ) -> RustcInvocationPolicy {
        self.oracle()
            .rustc_invocation_policy(explicit_unprotected_qualification)
    }
}

pub(crate) const fn rustc_invocation_policy(
    qualification: Option<SelectedQualificationOracle>,
    explicit_unprotected_qualification: bool,
) -> RustcInvocationPolicy {
    match qualification {
        None => RustcInvocationPolicy::ProtectedV3,
        Some(qualification) => {
            qualification.rustc_invocation_policy(explicit_unprotected_qualification)
        }
    }
}

pub(crate) fn validate_device_transaction(
    qualification: Option<SelectedQualificationOracle>,
    production_transaction_complete: bool,
) -> Result<(), String> {
    match (qualification, production_transaction_complete) {
        (None, true) => Ok(()),
        (None, false) => Err(
                "production compilation did not complete its device transaction; qualification fallback is forbidden"
                    .to_owned(),
        ),
        (Some(_), false) => Ok(()),
        (Some(qualification), true) => Err(format!(
            "qualification oracle `{}` cannot complete or publish as the production device transaction",
            qualification.oracle().oracle_name(),
        )),
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) enum QualificationSelection {
    #[default]
    NoOracle,
    #[cfg(feature = "qualification-oracles-test-only")]
    ExplicitQualification(QualificationOracle),
    Invalid(String),
}

impl QualificationSelection {
    pub(crate) fn from_env() -> Self {
        Self::from_values(
            env::var_os(crate::OBSOLETE_CODEGEN_PIPELINE_ENV).as_deref(),
            env::var_os(crate::QUALIFICATION_ORACLE_ENV).as_deref(),
        )
    }

    pub(crate) fn from_values(
        obsolete_pipeline: Option<&OsStr>,
        qualification_oracle: Option<&OsStr>,
    ) -> Self {
        if let Some(value) = obsolete_pipeline {
            return Self::Invalid(format!(
                "{} has been removed; production compilation has no selector and temporary test oracles use {}; found {value:?}",
                crate::OBSOLETE_CODEGEN_PIPELINE_ENV,
                crate::QUALIFICATION_ORACLE_ENV,
            ));
        }
        Self::from_value(qualification_oracle)
    }

    #[cfg(feature = "qualification-oracles-test-only")]
    pub(crate) fn from_value(value: Option<&OsStr>) -> Self {
        let Some(value) = value else {
            return Self::NoOracle;
        };
        if let Some(oracle) = QualificationOracle::ALL
            .into_iter()
            .find(|oracle| value == OsStr::new(oracle.oracle_name()))
        {
            return Self::ExplicitQualification(oracle);
        }
        let supported = QualificationOracle::ALL
            .into_iter()
            .map(|oracle| format!("`{}`", oracle.oracle_name()))
            .collect::<Vec<_>>()
            .join(", ");
        Self::Invalid(format!(
            "{} must be unset for production compilation; temporary qualification oracles are {supported}; found {value:?}",
            crate::QUALIFICATION_ORACLE_ENV,
        ))
    }

    #[cfg(not(feature = "qualification-oracles-test-only"))]
    pub(crate) fn from_value(value: Option<&OsStr>) -> Self {
        match value {
            None => Self::NoOracle,
            Some(value) => Self::Invalid(format!(
                "{} is unavailable in the production backend; temporary qualification oracles require backend feature `qualification-oracles-test-only`; found {value:?}",
                crate::QUALIFICATION_ORACLE_ENV,
            )),
        }
    }

    pub(crate) fn resolve(
        &self,
    ) -> Result<Option<SelectedQualificationOracle>, amdgpu_llvm::EmitError> {
        match self {
            Self::NoOracle => Ok(None),
            #[cfg(feature = "qualification-oracles-test-only")]
            Self::ExplicitQualification(oracle) => {
                Ok(Some(SelectedQualificationOracle { oracle: *oracle }))
            }
            Self::Invalid(reason) => Err(amdgpu_llvm::EmitError::Preflight {
                reason: reason.clone(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "qualification-oracles-test-only")]
    use std::collections::BTreeSet;
    use std::ffi::OsStr;

    #[cfg(feature = "qualification-oracles-test-only")]
    use super::QualificationOracle;
    use super::{
        QualificationSelection, RustcInvocationPolicy, rustc_invocation_policy,
        validate_device_transaction,
    };

    #[test]
    fn production_has_no_selector_or_alternate_variant() {
        let source = include_str!("qualification_selection.rs");
        assert!(!source.contains(concat!("enum Compilation", "Route")));
        let qualification = QualificationSelection::from_values(None, None)
            .resolve()
            .expect("unset selection must resolve");
        assert_eq!(qualification, None);

        let QualificationSelection::Invalid(reason) =
            QualificationSelection::from_value(Some(OsStr::new("production-v1")))
        else {
            panic!("production must not have an explicit selector")
        };
        assert!(reason.contains("FE2O3_QUALIFICATION_ORACLE_V1"));
    }

    #[test]
    fn obsolete_pipeline_environment_is_rejected_before_oracle_selection() {
        for qualification_oracle in [None, Some(OsStr::new("kernel-ir-v1"))] {
            let QualificationSelection::Invalid(reason) = QualificationSelection::from_values(
                Some(OsStr::new("production-v1")),
                qualification_oracle,
            ) else {
                panic!("obsolete pipeline environment was accepted")
            };
            assert!(reason.contains("FE2O3_CODEGEN_PIPELINE has been removed"));
            assert!(reason.contains("FE2O3_QUALIFICATION_ORACLE_V1"));
        }
    }

    #[test]
    #[cfg(feature = "qualification-oracles-test-only")]
    fn qualification_oracle_names_are_unique_and_round_trip() {
        let names = QualificationOracle::ALL
            .into_iter()
            .map(QualificationOracle::oracle_name)
            .collect::<BTreeSet<_>>();
        assert_eq!(names.len(), QualificationOracle::ALL.len());
        for oracle in QualificationOracle::ALL {
            assert_eq!(
                QualificationSelection::from_value(Some(OsStr::new(oracle.oracle_name()))),
                QualificationSelection::ExplicitQualification(oracle),
            );
        }
    }

    #[test]
    fn production_never_falls_back_to_qualification() {
        let qualification = QualificationSelection::default()
            .resolve()
            .expect("default must resolve");
        validate_device_transaction(qualification, true).expect("completed production transaction");
        assert!(
            validate_device_transaction(qualification, false)
                .expect_err("production must never fall back")
                .contains("qualification fallback is forbidden")
        );
    }

    #[test]
    #[cfg(feature = "qualification-oracles-test-only")]
    fn every_qualification_oracle_is_explicit_and_non_publishing() {
        for oracle in QualificationOracle::ALL {
            let qualification =
                QualificationSelection::from_value(Some(OsStr::new(oracle.oracle_name())))
                    .resolve()
                    .expect("explicit qualification oracle must resolve")
                    .expect("qualification token");
            validate_device_transaction(Some(qualification), false)
                .expect("explicit qualification oracle");
            assert_eq!(qualification.oracle(), oracle);
            assert_eq!(
                qualification.requires_extended_collector_edges(),
                matches!(
                    oracle,
                    QualificationOracle::CollectedFlashAttentionV1
                        | QualificationOracle::CollectedMoeTop2V1
                )
            );
            assert!(
                validate_device_transaction(Some(qualification), true)
                    .expect_err("qualification cannot publish as production")
                    .contains("cannot complete or publish as the production")
            );
        }
    }

    #[test]
    fn production_invocation_requires_protected_v3() {
        assert_eq!(
            rustc_invocation_policy(None, true),
            RustcInvocationPolicy::ProtectedV3,
        );
    }

    #[test]
    #[cfg(feature = "qualification-oracles-test-only")]
    fn invocation_authority_is_bound_to_the_selected_oracle() {
        for oracle in QualificationOracle::ALL {
            let qualification = QualificationSelection::ExplicitQualification(oracle)
                .resolve()
                .expect("qualification oracle")
                .expect("qualification token");
            let expected_without_override = match oracle {
                QualificationOracle::SimulationV1 => RustcInvocationPolicy::QualificationObserved,
                QualificationOracle::CollectedRowSoftmaxV1 => RustcInvocationPolicy::ProtectedV3,
                _ => RustcInvocationPolicy::Unmanaged,
            };
            assert_eq!(
                qualification.rustc_invocation_policy(false),
                expected_without_override,
            );
            assert_eq!(
                qualification.rustc_invocation_policy(true),
                RustcInvocationPolicy::QualificationObserved,
            );
        }
    }
}
