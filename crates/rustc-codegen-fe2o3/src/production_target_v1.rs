//! Retained source and device target inputs for the production transaction.

use std::fmt;

use dialect_mir::GFX942_TARGET_CPU;
use rustc_middle::ty::TyCtxt;

use crate::AmdGpuTarget;
use crate::semantic_layout_bridge::{
    SemanticLayoutBridgeError, SemanticLayoutTargetV1, rustc_semantic_layout_target_v1,
};

pub(crate) const PRODUCTION_TARGET_V1: &str = "gfx942:xnack-";

/// Move-only retention of the exact source-layout context and the separately
/// configured production device target. The compatibility backend currently
/// runs rustc analysis under the host target, so these facts must not be
/// conflated. The semantic importer must consume and bridge or reject them.
#[derive(Debug)]
pub(crate) struct RetainedProductionTargetV1 {
    rustc_layout: SemanticLayoutTargetV1,
}

impl RetainedProductionTargetV1 {
    pub(crate) fn capture(
        tcx: TyCtxt<'_>,
        configured_cpu: &AmdGpuTarget,
    ) -> Result<Self, ProductionTargetErrorV1> {
        if !configured_target_is_production_cpu_v1(configured_cpu) {
            return Err(ProductionTargetErrorV1::ConfiguredCpu {
                observed: configured_cpu.as_str().to_owned(),
            });
        }
        let rustc_layout = rustc_semantic_layout_target_v1(tcx)
            .map_err(ProductionTargetErrorV1::RustcObservation)?;
        Ok(Self { rustc_layout })
    }

    pub(crate) fn canonical_name(&self) -> &'static str {
        PRODUCTION_TARGET_V1
    }

    pub(crate) fn into_rustc_layout(self) -> SemanticLayoutTargetV1 {
        self.rustc_layout
    }
}

fn configured_target_is_production_cpu_v1(target: &AmdGpuTarget) -> bool {
    target.as_str() == GFX942_TARGET_CPU
}

#[derive(Debug)]
pub(crate) enum ProductionTargetErrorV1 {
    ConfiguredCpu { observed: String },
    RustcObservation(SemanticLayoutBridgeError),
}

impl fmt::Display for ProductionTargetErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConfiguredCpu { observed } => write!(
                formatter,
                "production-v1 requires configured target CPU {GFX942_TARGET_CPU:?}; found {observed:?}"
            ),
            Self::RustcObservation(error) => {
                write!(
                    formatter,
                    "production-v1 could not capture the rustc target: {error}"
                )
            }
        }
    }
}

impl std::error::Error for ProductionTargetErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::RustcObservation(error) => Some(error),
            Self::ConfiguredCpu { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::configured_target_is_production_cpu_v1;
    use crate::AmdGpuTarget;

    #[test]
    fn production_device_target_accepts_only_configured_gfx942_cpu() {
        assert!(configured_target_is_production_cpu_v1(&AmdGpuTarget::new(
            "gfx942"
        )));
        for rejected in ["gfx942:xnack-", "gfx942:xnack+", "gfx950", "GFX942"] {
            assert!(!configured_target_is_production_cpu_v1(&AmdGpuTarget::new(
                rejected
            )));
        }
    }
}
