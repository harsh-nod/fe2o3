use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::{ProductionSemanticSsaLimitsV1, ProductionSemanticSsaOwnerV1};

mod loan_retention {
    use super::*;
    include!("loan_retention_tests.rs");
}

mod operand_path_boundaries {
    use super::*;
    include!("operand_path_boundary_tests.rs");
}

fn input(root: u32) -> ProductionKernelContextLoweringInputV1 {
    ProductionKernelContextLoweringInputV1::new(
        SemanticFunctionIdV1::from_index(root),
        [1; 32],
        [2; 32],
        [3; 32],
        [4; 32],
        [5; 32],
    )
}

#[test]
fn compatible_empty_root_cannot_issue_scoped_matrix_custody() {
    let source = super::super::resource_tests::noop_semantic_owner(&["not_a_matrix_issuer"]);
    let owner =
        ProductionSemanticSsaOwnerV1::try_new(source, ProductionSemanticSsaLimitsV1::default())
            .unwrap();
    assert!(
        ProductionScopedMatrixUseRelationV1::checked_source_uses(
            &owner,
            &input(0),
            None,
            1_000_000
        )
        .is_err()
    );
    assert!(
        ProductionScopedMatrixUseRelationV1::checked_source_uses(
            &owner,
            &input(1),
            None,
            1_000_000
        )
        .is_err()
    );
}

#[test]
fn empty_or_foreign_frontend_identity_never_becomes_source_custody() {
    let source = super::super::resource_tests::noop_semantic_owner(&["no_context"]);
    let owner =
        ProductionSemanticSsaOwnerV1::try_new(source, ProductionSemanticSsaLimitsV1::default())
            .unwrap();
    for index in 0..5 {
        let mut identities = [[1; 32], [2; 32], [3; 32], [4; 32], [5; 32]];
        identities[index] = [0; 32];
        let input = ProductionKernelContextLoweringInputV1::new(
            SemanticFunctionIdV1::from_index(0),
            identities[0],
            identities[1],
            identities[2],
            identities[3],
            identities[4],
        );
        assert!(
            ProductionScopedMatrixUseRelationV1::checked_source_uses(
                &owner, &input, None, 1_000_000
            )
            .is_err()
        );
    }
}
