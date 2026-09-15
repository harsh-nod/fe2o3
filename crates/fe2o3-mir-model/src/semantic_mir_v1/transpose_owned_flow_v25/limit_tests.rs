use super::*;

fn row() -> SemanticTransposeOwnedFlowV1 {
    let site = |f| SemanticOwnedSourceCallSiteV1 { function: SemanticFunctionIdV1(f), block: SemanticBlockIdV1(0) };
    SemanticTransposeOwnedFlowV1::from_encoded_parts(
        SemanticTransposeOwnedFlowSitesV1 {
            issue: site(0),
            capture: SemanticOwnedSourceStatementSiteV1 { function: SemanticFunctionIdV1(0), block: SemanticBlockIdV1(0), statement: 0 },
            capture_field: 0, matrix_call: site(0), closure_call: site(1), stage: site(2), publish: site(0), workgroup_local: SemanticLocalIdV1(0),
        }, vec![site(0)],
        [(SemanticFunctionIdV1(0), [1; 32]), (SemanticFunctionIdV1(1), [2; 32]), (SemanticFunctionIdV1(2), [3; 32])], [4; 32],
    ).unwrap()
}

#[test]
fn common_v25_source_row_limit_is_checked_before_any_body_lookup() {
    let limits = SemanticMirLimitsV1::default().with_limit(SemanticMirResourceV1::Blocks, 1).unwrap();
    let mut work = 100;
    assert_eq!(rows(&[], &[], &[], &[row(), row()], &mut work, limits), Err(SemanticMirErrorV1::LimitExceeded { resource: SemanticMirResourceV1::Blocks, actual: 2, max: 1 }));
    assert_eq!(work, 100);
}

#[test]
fn common_v25_source_borrow_limit_matches_decoder_and_charges_work() {
    let limits = SemanticMirLimitsV1::default().with_limit(SemanticMirResourceV1::CallArguments, 0).unwrap();
    let mut work = 100;
    assert_eq!(rows(&[], &[], &[], &[row()], &mut work, limits), Err(SemanticMirErrorV1::LimitExceeded { resource: SemanticMirResourceV1::CallArguments, actual: 1, max: 0 }));
    assert_eq!(work, 99);
    let mut work = 0;
    assert_eq!(rows(&[], &[], &[], &[row()], &mut work, SemanticMirLimitsV1::default()), Err(SemanticMirErrorV1::LimitExceeded { resource: SemanticMirResourceV1::ValidationWork, actual: 1, max: 0 }));
    assert_eq!(work, 0);
}
