use super::*;

pub(in super::super) fn check(
    contexts: &AuthenticatedProductionKernelContextsV1,
    owner: &fe2o3_pliron::ProductionSemanticSsaOwnerV1,
) {
    let seal = contexts
        .transpose_source
        .as_ref()
        .expect("live importer attached source custody");
    let [root] = owner.source_semantic().roots() else {
        panic!("one real registered root")
    };
    assert!(seal.matches(owner, *root));
    // The callback calls the SAME predicate as construct_semantic_ssa; its
    // Context/issuance record comes from the live importer, never this test.
    AuthenticatedProductionKernelContextsV1::validate_transpose_source_ssa(Some(contexts), owner)
        .expect("complete live source and actual Move roster");
    let rejected =
        AuthenticatedProductionKernelContextsV1::validate_transpose_source_ssa(None, owner)
            .expect_err("decoded footer alone is not live source evidence");
    assert!(matches!(rejected,
    ProductionSemanticImportErrorV1::TransposeOwnedSource(error)
    if matches!(*error, PlanError::Source(Error::Source(
        "transpose source footer has no authenticated frontend owner",
    )))));

    for mutation in 0..3 {
        // Mutation-only copies of private fields; no positive fixture receipt.
        let mut changed = SourceSeal {
            semantic: seal.semantic,
            root: seal.root,
            rows: seal.rows,
        };
        match mutation {
            0 => changed.semantic[0] ^= 1,
            1 => changed.root = SemanticFunctionIdV1::from_index(u32::MAX),
            2 => changed.rows = changed.rows.checked_add(1).unwrap(),
            _ => unreachable!(),
        }
        assert!(!changed.matches(owner, *root));
        let mut work = 1_048_576;
        assert!(matches!(
            changed.rebind(owner, *root, &mut work),
            Err(PlanError::Source(Error::Source(
                "transpose final owner lost its live source seal"
            )))
        ));
    }
    let mut zero = 0;
    assert!(matches!(
        seal.rebind(owner, *root, &mut zero),
        Err(PlanError::Source(Error::Work))
    ));
    assert_eq!(zero, 0);
}
