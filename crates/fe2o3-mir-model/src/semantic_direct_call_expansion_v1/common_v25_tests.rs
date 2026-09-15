use super::*;

#[test]
fn common_v25_repeated_expansion_replays_original_frame_and_source_rosters() {
    let old = repeated();
    let before = SemanticCallExpansionV1::try_new(&old, SemanticCallExpansionLimitsV1::default()).unwrap();
    let current = old.with_transpose_owned_flows_v25(vec![], SemanticMirLimitsV1::default()).unwrap();
    let after = SemanticCallExpansionV1::try_new(&current, SemanticCallExpansionLimitsV1::default()).unwrap();
    after.verify_replay(&current).unwrap();
    assert!(matches!(before.verify_replay(&current), Err(SemanticCallExpansionErrorV1::SourceMismatch)));
    assert_ne!(before.identity(), after.identity());
    assert_eq!(before.roots().len(), after.roots().len());
    for (old, new) in before.roots().iter().zip(after.roots()) {
        assert!(new.has_expanded_calls());
        // Node identities are seeded by the complete owner hash. Compare all
        // execution content and origins, not old-owner identity bytes.
        assert_ne!(old.body().identity(), new.body().identity());
        assert_eq!(old.body().abi(), new.body().abi());
        assert_eq!(old.body().role(), new.body().role());
        assert_eq!(old.body().source(), new.body().source());
        assert_eq!(old.body().entry(), new.body().entry());
        assert_eq!(old.body().locals().len(), new.body().locals().len());
        for (a, b) in old.body().locals().iter().zip(new.body().locals()) {
            assert_eq!((a.ty(), a.role(), a.source()), (b.ty(), b.role(), b.source()));
        }
        assert_eq!(old.body().blocks().len(), new.body().blocks().len());
        for (a, b) in old.body().blocks().iter().zip(new.body().blocks()) {
            assert_eq!(a.source(), b.source());
            assert_eq!(a.statements(), b.statements());
            assert_eq!(a.terminator(), b.terminator());
        }
        assert_eq!(old.instances(), new.instances());
        assert_eq!(old.local_origins(), new.local_origins());
        assert_eq!(old.block_origins(), new.block_origins());
    }
    let decoded = AdmittedInertSemanticMirV1::decode_current_production_canonical(current.canonical_encoding(), SemanticMirLimitsV1::default()).unwrap();
    after.verify_replay(&decoded).unwrap();
}
