//! Genuine generated replay, not fabricated generated-effect authority.
use super::*;
use fe2o3_lower_mir_kernel::{
    ProductionMirPlironTranslationErrorV1, ProductionRankedAccessSourceV1,
    ProductionSemanticKirErrorV1, ProductionSemanticKirOwnerV1,
};

fn actual_generated_program(case: u8) -> ProductionRankedSemanticProgramV1 {
    match case {
        0 => neutral_ranked_program_v1(),
        1 => neutral_scan_ranked_program_v1(SemanticWorkgroupScanKindV1::Inclusive, 3),
        2 => neutral_scan_ranked_program_v1(SemanticWorkgroupScanKindV1::Exclusive, 3),
        _ => unreachable!("closed generated fixture roster"),
    }
}

#[test]
fn source255_genuine_generated_only_memory_passes_shared_attribution() {
    for case in 0..3 {
        let ProductionRankedSemanticProgramV1 {
            materialized,
            roots,
            phase: _phase,
        } = actual_generated_program(case);
        assert_eq!(roots.len(), 1);
        let root = roots.into_vec().into_iter().next().unwrap();
        assert!(root.access_sources.is_empty());
        assert!(!root.executable_effect_sources.is_empty());
        assert!(root.executable_effect_sources.iter().any(|source| {
            matches!(
                root.verification
                    .ordinary()
                    .expect("ordinary test root")
                    .kernel()
                    .blocks()[source.ranked_block() as usize]
                    .operations()[source.ranked_operation() as usize],
                ProductionRankedOperationV1::AllocationEffect { .. }
            )
        }));
        let receipt = materialized_ranked_fixture_receipt_v1(materialized, root);
        let _owner = ProductionSemanticKirOwnerV1::try_attach_materialized_ranked_checks(receipt)
            .expect(
                "the complete genuine source/SSA/N/generated recipe must pass shared attribution",
            );
    }
}

#[test]
fn source255_generated_overlap_candidate_is_rejected_as_unused_ordinary_attribution() {
    for case in 0..3 {
        let ProductionRankedSemanticProgramV1 {
            materialized,
            roots,
            phase: _phase,
        } = actual_generated_program(case);
        assert_eq!(roots.len(), 1);
        let mut root = roots.into_vec().into_iter().next().unwrap();
        assert!(root.access_sources.is_empty());
        let generated = *root
            .executable_effect_sources
            .iter()
            .find(|source| {
                matches!(
                    root.verification
                        .ordinary()
                        .expect("ordinary test root")
                        .kernel()
                        .blocks()[source.ranked_block() as usize]
                        .operations()[source.ranked_operation() as usize],
                    ProductionRankedOperationV1::AllocationEffect { .. }
                )
            })
            .expect("a genuine generated memory event is required");
        let location = (generated.ranked_block(), generated.ranked_operation());
        assert_ne!(generated.recipe_identity(), [0; 32]);
        // Only the untrusted ordinary-row claim changes. Actual source, N, recipe,
        // generated rows and their recipe identities are retained unchanged.
        root.access_sources
            .push(ProductionRankedAccessSourceV1::new(
                generated.semantic_block(),
                None,
                0,
                location.0,
                location.1,
            ));
        let receipt = materialized_ranked_fixture_receipt_v1(materialized, root);
        assert!(matches!(
            ProductionSemanticKirOwnerV1::try_attach_materialized_ranked_checks(receipt),
            Err(ProductionSemanticKirErrorV1::MirPlironTranslation(
                ProductionMirPlironTranslationErrorV1::ExtraRankedEffect {
                    ranked_block, ranked_operation,
                }
            )) if (ranked_block, ranked_operation) == location
        ));
        // Generated-first consumer dispatch leaves this ordinary row unused.
        // Its earlier rejection does NOT cover the later ordinary==generated
        // double-attribution branch with an actually consumed ordinary effect.
    }
}
