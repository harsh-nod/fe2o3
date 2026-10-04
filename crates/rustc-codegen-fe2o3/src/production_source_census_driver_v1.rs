//! Diagnostic mode identity for the existing extraction dispatcher.

use super::ProductionExtractionCallbacksV1;
use crate::collector::source_census_v1::ExtractionMode as CensusMode;

impl ProductionExtractionCallbacksV1 {
    pub(super) fn census_mode(&self, semantic_handoff: bool) -> CensusMode {
        if self.simulation_bundle_output.is_some() {
            CensusMode::SimulationBundle {
                version: match self.simulation_bundle_version {
                    version @ 2..=6 => version,
                    _ => 1,
                },
            }
        } else if let Some((_, expected_target)) = self.compiler_handoff_output.as_ref() {
            CensusMode::CompilerHandoff {
                version: if semantic_handoff { 3 } else { 1 },
                expected_target: *expected_target,
            }
        } else if self.amdgpu_llvm_output.is_some() {
            CensusMode::Llvm {
                expected_target: self.expected_llvm_target,
            }
        } else if self.ranked_memory {
            CensusMode::RankedMemory
        } else {
            CensusMode::SemanticMir
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn census_mode_records_existing_dispatch_and_exact_bundle_version() {
        use super::*;
        let mut callback = ProductionExtractionCallbacksV1::default();
        let value = |callback: &ProductionExtractionCallbacksV1, semantic| {
            serde_json::to_value(callback.census_mode(semantic)).unwrap()
        };
        assert_eq!(
            value(&callback, false),
            serde_json::json!({"kind":"semantic-mir"})
        );
        callback.ranked_memory = true;
        assert_eq!(
            value(&callback, false),
            serde_json::json!({"kind":"ranked-memory"})
        );
        callback.amdgpu_llvm_output = Some("output.ll".into());
        callback.expected_llvm_target = Some("gfx942:xnack-");
        assert_eq!(
            value(&callback, false),
            serde_json::json!({"kind":"llvm","expected_target":"gfx942:xnack-"})
        );
        callback.compiler_handoff_output = Some(("handoff".into(), None));
        for semantic in [false, true] {
            assert_eq!(
                value(&callback, semantic),
                serde_json::json!({"kind":"compiler-handoff","version":if semantic {3} else {1},"expected_target":null})
            );
        }
        callback.simulation_bundle_output = Some("bundle".into());
        for version in 0..=7 {
            callback.simulation_bundle_version = version;
            assert_eq!(
                value(&callback, true),
                serde_json::json!({"kind":"simulation-bundle","version":if (2..=6).contains(&version) {version} else {1}})
            );
        }
    }
}
